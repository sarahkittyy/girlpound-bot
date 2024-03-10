use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use crate::steamid::SteamIDClient;
use crate::{logs, sourcebans, Error};
use crate::{parse_env, Server};
use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude::{self as serenity, Mentionable};
use serenity::CreateMessage;

use sqlx::{MySql, Pool, Row};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Sender;
use tokio::sync::OnceCell;
use tokio::{self, sync::RwLock};

mod commands;
mod media_cooldown;
mod new_user;
mod nsfw_callout;
mod on_component_interaction;
mod on_delete;
mod on_message;
mod player_count;

pub struct PoiseData {
    /// all tf2 servers known by the bot
    pub servers: HashMap<SocketAddr, Server>,
    /// guild the bot operates in
    pub guild_id: serenity::GuildId,
    /// identify the pug server. tkgp specific
    pub pug_server: SocketAddr,
    /// list of pug cfgs available for use
    pub pug_cfgs: Vec<String>,
    /// image spam prevention
    pub media_cooldown: Arc<RwLock<media_cooldown::MediaCooldown>>,
    media_cooldown_thread: OnceCell<Sender<Cooldown>>,
    /// Users that have already been called out for going into NSFW <1 hr after joining
    pub horny_callouts: Arc<RwLock<HashSet<u64>>>,

    /// NSFW role
    pub horny_role: serenity::RoleId,
    /// role to seed teh tf2 server
    pub seeder_role: serenity::RoleId,
    /// role to give users to allow them into the server
    pub member_role: serenity::RoleId,

    /// #general
    pub general_channel: serenity::ChannelId,
    /// channel where deleted messages are logged
    pub deleted_message_log_channel: serenity::ChannelId,
    /// mod channel id
    pub mod_channel: serenity::ChannelId,
    /// "trial mod" channel, for "helpful reminders"
    pub trial_mod_channel: serenity::ChannelId,
    /// age verification channel
    pub birthday_channel: serenity::ChannelId,

    /// /seeder cooldown
    pub seeder_cooldown: Arc<RwLock<HashMap<SocketAddr, DateTime<Utc>>>>,
    /// Bot database pool
    pub local_pool: Pool<MySql>,
    /// Sourcebans database pool
    pub sb_pool: Pool<MySql>,
    /// steamid.uk api client
    pub steamid_client: SteamIDClient,
}
impl PoiseData {
    /// fetch the server with the given socket address
    pub fn server(&self, server_addr: SocketAddr) -> Result<&Server, Error> {
        self.servers
            .get(&server_addr)
            .ok_or("Server not found".into())
    }

    /// Fetches the tkgp pug server
    pub fn pug_server(&self) -> Result<&Server, Error> {
        self.servers
            .get(&self.pug_server)
            .ok_or("Pug server not found".into())
    }

    /// checks if a seeder ping is allowed. if on cooldown, returns time until usable
    pub async fn can_seed(&self, server_addr: SocketAddr) -> Result<(), Duration> {
        // 4 hrs
        const SEED_COOLDOWN: Duration = Duration::milliseconds(4 * 60 * 60 * 1000);

        let mut map = self.seeder_cooldown.write().await;
        let last_used = map.entry(server_addr).or_insert(DateTime::<Utc>::MIN_UTC);
        let now = chrono::Utc::now();

        let allowed_at = *last_used + SEED_COOLDOWN;

        if allowed_at < now {
            // allowed
            Ok(())
        } else {
            Err(allowed_at - now)
        }
    }

    /// marks the server as just seeded, resetting the cooldown
    pub async fn reset_seed_cooldown(&self, server_addr: SocketAddr) {
        let mut map = self.seeder_cooldown.write().await;
        let last_used = map.entry(server_addr).or_insert(DateTime::<Utc>::MIN_UTC);

        *last_used = chrono::Utc::now();
    }
}
pub type Context<'a> = poise::Context<'a, PoiseData, Error>;
pub type ApplicationContext<'a> = poise::ApplicationContext<'a, PoiseData, Error>;

struct Cooldown {
    user: serenity::UserId,
    channel: serenity::ChannelId,
    delete_at: DateTime<Utc>,
}

fn spawn_cooldown_manager(ctx: serenity::Context) -> Sender<Cooldown> {
    let (cooldown_sender, mut cooldown_receiver) = tokio::sync::mpsc::channel::<Cooldown>(64);

    tokio::spawn(async move {
        let mut queue: Vec<(Cooldown, serenity::Message)> = vec![];
        loop {
            match cooldown_receiver.try_recv() {
                Err(TryRecvError::Disconnected) => break,
                Err(_) => (),
                // when a cooldown request is received...
                Ok(
                    cooldown @ Cooldown {
                        user,
                        channel,
                        delete_at,
                    },
                ) if !queue
                    .iter()
                    .any(|(cd, _)| cd.user == user && cd.channel == channel) =>
                {
                    let msg_string = format!(
                        "<@{}> guh!! >_<... post again <t:{}:R>",
                        user.get(),
                        delete_at.timestamp()
                    );
                    if let Ok(msg) = channel
                        .send_message(&ctx, CreateMessage::new().content(msg_string))
                        .await
                    {
                        queue.push((cooldown, msg));
                    }
                }
                Ok(_) => (),
            }
            queue.retain(|(cooldown, msg)| {
                let http = ctx.http.clone();
                // if it should be deleted by now
                let delete = Utc::now() - cooldown.delete_at > Duration::zero();
                if delete {
                    let mid = msg.id;
                    let cid = msg.channel_id;
                    tokio::task::spawn(async move {
                        http.delete_message(cid, mid, Some("media cooldown")).await
                    });
                }
                !delete
            });
            tokio::task::yield_now().await;
        }
    });

    cooldown_sender
}

/// handle discord events
async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, PoiseData, Error>,
    data: &PoiseData,
) -> Result<(), Error> {
    use serenity::FullEvent as Event;

    let cooldown_handler = {
        let ctx = ctx.clone();
        data.media_cooldown_thread
            .get_or_init(|| async { spawn_cooldown_manager(ctx) })
            .await
    };
    match event {
        Event::GuildMemberUpdate {
            old_if_available,
            new,
            ..
        } => {
            nsfw_callout::try_callout_nsfw_role(ctx, data, old_if_available, new).await?;
        }
        Event::GuildMemberAddition { new_member } => {
            new_user::welcome_user(ctx, new_member).await?;
        }
        Event::Message { new_message } => {
            let _ = on_message::trial_mod_reminders(ctx, data, new_message)
                .await
                .inspect_err(|e| eprintln!("trial mod reminder fail: {e}"));
            let _ = on_message::handle_cooldowns(ctx, data, cooldown_handler, new_message)
                .await
                .inspect_err(|e| eprintln!("media cooldown error: {e}"));
            let _ = on_message::hi_cat(ctx, data, new_message)
                .await
                .inspect_err(|e| eprintln!("hi cat error: {e}"));
        }
        Event::MessageDelete {
            channel_id,
            deleted_message_id,
            ..
        } => {
            on_delete::save_deleted(ctx, data, channel_id, deleted_message_id).await?;
        }
        Event::InteractionCreate { interaction } => {
            if let Some(mci) = interaction.as_message_component() {
                on_component_interaction::dispatch(ctx, data, mci).await?;
            }
        }
        Event::GuildMemberRemoval { user, .. } => {
            data.deleted_message_log_channel
                .send_message(
                    &ctx,
                    CreateMessage::new().content(format!(
                        "{} ({}) left the server",
                        user.mention(),
                        user.name
                    )),
                )
                .await?;
        }
        _ => (),
    };
    Ok(())
}

/// initialize the discord bot
pub async fn start_bot(
    log_receiver: logs::LogReceiver,
    servers: HashMap<SocketAddr, crate::Server>,
) -> Result<(), Error> {
    let bot_token: String = parse_env("BOT_TOKEN");
    let guild_id: u64 = parse_env("GUILD_ID");
    let deleted_messages_log_channel_id: u64 = parse_env("DELETED_MESSAGE_LOG_CHANNEL_ID");
    let seeder_role_id: u64 = parse_env("SEEDER_ROLE");
    let horny_role_id: u64 = parse_env("HORNY_ROLE");
    let member_role_id: u64 = parse_env("MEMBER_ROLE");
    let general_channel_id: u64 = parse_env("GENERAL_CHANNEL_ID");
    let mod_channel_id: u64 = parse_env("MOD_CHANNEL_ID");
    let trial_mod_channel_id: u64 = parse_env("TRIAL_MOD_CHANNEL_ID");
    let birthday_channel_id: u64 = parse_env("BIRTHDAY_CHANNEL_ID");

    let db_url: String = parse_env("DATABASE_URL");
    let sb_db_url: String = parse_env("SB_DATABASE_URL");

    // migrate the db
    let local_pool = Pool::<MySql>::connect(&db_url).await?;
    sqlx::migrate!().run(&local_pool).await?;
    println!("DB Migrated.");

    let sb_pool = Pool::<MySql>::connect(&sb_db_url).await?;
    println!("Connected to sourcebans pool.");

    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::GUILD_MEMBERS;

    let framework = {
        let servers = servers.clone();
        let local_pool = local_pool.clone();
        let sb_pool = sb_pool.clone();
        let pug_server = "pug.fluffycat.gay:27015"
            .to_socket_addrs()
            .expect("Pug address DNS resolution failed")
            .next()
            .expect("Could not resolve PUG server address.");
        poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: commands::ALL.iter().map(|f| f()).collect(),
                event_handler: |a, b, c, d| Box::pin(event_handler(a, b, c, d)),
                ..Default::default()
            })
            .setup(move |ctx, _ready, framework| {
                Box::pin(async move {
                    ctx.cache.set_max_messages(500);
                    poise::builtins::register_in_guild(
                        ctx,
                        &framework.options().commands,
                        serenity::GuildId::new(guild_id),
                    )
                    .await?;

                    ctx.set_activity(Some(serenity::ActivityData::playing(
                        "!add 5 ultracide.net",
                    )));

                    Ok(PoiseData {
                        servers,
                        media_cooldown: Arc::new(RwLock::new(
                            media_cooldown::MediaCooldown::from_env(),
                        )),
                        guild_id: serenity::GuildId::new(guild_id),
                        pug_cfgs: [
                            "rgl_off",
                            "rgl_7s_koth",
                            "rgl_7s_koth_bo5",
                            "rgl_6s_koth_scrim",
                            "rgl_6s_koth_bo5",
                            "rgl_6s_koth",
                            "rgl_6s_5cp_scrim",
                            "rgl_6s_5cp_match_pro",
                        ]
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                        pug_server,
                        seeder_role: serenity::RoleId::new(seeder_role_id),
                        horny_role: serenity::RoleId::new(horny_role_id),
                        member_role: serenity::RoleId::new(member_role_id),
                        horny_callouts: Arc::new(RwLock::new(HashSet::new())),
                        general_channel: serenity::ChannelId::new(general_channel_id),
                        deleted_message_log_channel: serenity::ChannelId::new(
                            deleted_messages_log_channel_id,
                        ),
                        mod_channel: serenity::ChannelId::new(mod_channel_id),
                        trial_mod_channel: serenity::ChannelId::new(trial_mod_channel_id),
                        birthday_channel: serenity::ChannelId::new(birthday_channel_id),
                        media_cooldown_thread: OnceCell::new(),
                        seeder_cooldown: Arc::new(RwLock::new(HashMap::new())),
                        local_pool: local_pool,
                        sb_pool: sb_pool,
                        steamid_client: SteamIDClient::new(
                            parse_env("STEAMID_MYID"),
                            parse_env("STEAMID_API_KEY"),
                            parse_env("STEAM_API_KEY"),
                        ),
                    })
                })
            })
            .build()
    };
    // launch alt threads

    let mut client = serenity::Client::builder(bot_token, intents)
        .framework(framework)
        .await
        .expect("Could not initialize client.");

    for (_addr, server) in servers.iter() {
        player_count::spawn_player_count_thread(server.clone(), client.http.clone());
    }

    logs::spawn_log_thread(
        log_receiver.clone(),
        servers.clone(),
        local_pool.clone(),
        client.http.clone(),
    );

    // fetch the current latest protest
    let latest_protest_pid: i32 =
        sqlx::query("SELECT `pid` FROM `sb_protests` ORDER BY `pid` DESC LIMIT 1")
            .fetch_one(&sb_pool)
            .await?
            .try_get("pid")?;
    sourcebans::spawn_ban_protest_thread(
        sb_pool.clone(),
        serenity::ChannelId::new(mod_channel_id),
        latest_protest_pid,
        client.http.clone(),
    );

    let fut = client.start();
    println!("Bot started!");
    fut.await.expect("Bot broke"); //

    Ok(())
}
