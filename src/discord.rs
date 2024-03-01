use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use crate::steamid::SteamIDClient;
use crate::{logs::LogReceiver, Error};
use crate::{parse_env, Server};
use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude as serenity;

use sqlx::{MySql, Pool};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Sender;
use tokio::sync::OnceCell;
use tokio::{self, sync::RwLock};

mod commands;
mod log_handler;
mod media_cooldown;
mod new_user;
mod nsfw_callout;
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
    /// NSFW role
    pub horny_role: serenity::RoleId,
    /// #general
    pub general_channel: serenity::ChannelId,
    /// Users that have already been called out for going into NSFW <1 hr after joining
    pub horny_callouts: Arc<RwLock<HashSet<u64>>>,
    /// channel where deleted messages are logged
    deleted_message_log_channel: serenity::ChannelId,
    /// role to seed teh tf2 server
    pub seeder_role: serenity::RoleId,
    /// "trial mod" channel, for "helpful reminders"
    pub trial_mod_channel: serenity::ChannelId,
    /// /seeder cooldown
    pub seeder_cooldown: Arc<RwLock<HashMap<SocketAddr, DateTime<Utc>>>>,
    /// Database pool
    pub pool: Pool<MySql>,
    /// steamid.uk api client
    pub client: SteamIDClient,
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
                        user.0,
                        delete_at.timestamp()
                    );
                    if let Ok(msg) = ctx
                        .http
                        .send_message(channel.0, &serenity::json::json!({ "content": msg_string }))
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
                    let mid = msg.id.0;
                    let cid = msg.channel_id.0;
                    tokio::task::spawn(async move { http.delete_message(cid, mid).await });
                }
                !delete
            });
            tokio::task::yield_now().await;
        }
    });

    cooldown_sender
}

/// handle discord events
pub async fn event_handler(
    ctx: &serenity::Context,
    event: &poise::Event<'_>,
    _framework: poise::FrameworkContext<'_, PoiseData, Error>,
    data: &PoiseData,
) -> Result<(), Error> {
    use poise::Event;

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
        } => {
            nsfw_callout::try_callout_nsfw_role(ctx, data, old_if_available, new).await?;
        }
        Event::GuildMemberAddition { new_member } => {
            new_user::welcome_user(ctx, new_member).await?;
        }
        Event::Message { new_message } => {
            on_message::trial_mod_reminders(ctx, data, new_message).await?;
            on_message::handle_cooldowns(ctx, data, cooldown_handler, new_message).await?;
        }
        Event::MessageDelete {
            channel_id,
            deleted_message_id,
            ..
        } => {
            on_delete::save_deleted(ctx, data, channel_id, deleted_message_id).await?;
        }
        _ => (),
    };
    Ok(())
}

/// initialize the discord bot
pub async fn start_bot(
    pool: Pool<MySql>,
    log_receiver: LogReceiver,
    servers: HashMap<SocketAddr, crate::Server>,
) {
    let bot_token: String = parse_env("BOT_TOKEN");
    let guild_id: u64 = parse_env("GUILD_ID");
    let deleted_messages_log_channel_id: u64 = parse_env("DELETED_MESSAGE_LOG_CHANNEL_ID");
    let seeder_role_id: u64 = parse_env("SEEDER_ROLE");
    let horny_role_id: u64 = parse_env("HORNY_ROLE");
    let general_channel_id: u64 = parse_env("GENERAL_CHANNEL_ID");
    let trial_mod_channel_id: u64 = parse_env("TRIAL_MOD_CHANNEL_ID");
    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::GUILD_MEMBERS;

    let girlpounder = {
        let servers = servers.clone();
        let pool = pool.clone();
        let pug_server = "pug.fluffycat.gay:27015"
            .to_socket_addrs()
            .expect("Pug address DNS resolution failed")
            .next()
            .expect("Could not resolve PUG server address.");
        poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![
                    commands::bark(),
                    commands::pug(),
                    commands::rcon(),
                    commands::snipers(),
                    commands::seeder(),
                    commands::respawntimes(),
                    commands::playercap(),
                    commands::meow(),
                    commands::map(),
                    commands::status(),
                    commands::lookup(),
                    commands::reacted_users(),
                    commands::feedback(),
                    commands::tf2ban(),
                    commands::tf2banid(),
                    commands::tf2unban(),
                    commands::tf2kick(),
                    commands::tf2mute(),
                    commands::tf2unmute(),
                    commands::tf2gag(),
                    commands::tf2ungag(),
                ],
                event_handler: |a, b, c, d| Box::pin(event_handler(a, b, c, d)),
                ..Default::default()
            })
            .token(bot_token)
            .intents(intents)
            .setup(move |ctx, _ready, framework| {
                Box::pin(async move {
                    ctx.cache.set_max_messages(500);
                    poise::builtins::register_in_guild(
                        ctx,
                        &framework.options().commands,
                        serenity::GuildId(guild_id),
                    )
                    .await?;

                    ctx.set_activity(serenity::Activity::playing("!add 5 ultracide.net"))
                        .await;

                    Ok(PoiseData {
                        servers,
                        media_cooldown: Arc::new(RwLock::new(
                            media_cooldown::MediaCooldown::from_env(),
                        )),
                        guild_id: serenity::GuildId(guild_id),
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
                        .map(|s| s.to_owned())
                        .collect(),
                        pug_server,
                        seeder_role: serenity::RoleId(seeder_role_id),
                        horny_role: serenity::RoleId(horny_role_id),
                        horny_callouts: Arc::new(RwLock::new(HashSet::new())),
                        general_channel: serenity::ChannelId(general_channel_id),
                        deleted_message_log_channel: serenity::ChannelId(
                            deleted_messages_log_channel_id,
                        ),
                        trial_mod_channel: serenity::ChannelId(trial_mod_channel_id),
                        media_cooldown_thread: OnceCell::new(),
                        seeder_cooldown: Arc::new(RwLock::new(HashMap::new())),
                        pool,
                        client: SteamIDClient::new(
                            parse_env("STEAMID_MYID"),
                            parse_env("STEAMID_API_KEY"),
                        ),
                    })
                })
            })
            .build()
            .await
            .expect("Failed to build girlpounder bot.")
    };
    // launch alt threads

    let ctx = girlpounder.client().cache_and_http.clone();
    for (_addr, server) in servers.iter() {
        player_count::spawn_player_count_thread(server.clone(), ctx.clone());
    }

    log_handler::spawn_log_thread(
        log_receiver.clone(),
        servers.clone(),
        pool.clone(),
        ctx.clone(),
    );

    let fut = girlpounder.start();
    println!("Bot started!");
    fut.await.expect("Bot broke"); //
}
