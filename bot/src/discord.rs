use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude::{self as serenity, Mentionable};
use poise::PrefixFrameworkOptions;
use serenity::CreateMessage;

use api::ApiState;
use common::{util::parse_env, Error};
use steam::SteamIDClient;
use tf2::{logs, wacky, Server};

use tokio_cron_scheduler::JobScheduler;

use sqlx::{MySql, Pool, Row};
use tokio::sync::mpsc::Sender;
use tokio::sync::OnceCell;
use tokio::{self, sync::RwLock};

mod commands;

use self::commands::ReminderManager;
use self::media_cooldown::CooldownMessage;
mod emojirank;
mod media_cooldown;
mod new_user;
mod nsfw_callout;
mod on_component_interaction;
mod on_delete;
mod on_message;
mod on_react;
mod player_count;

pub type Context<'a> = poise::Context<'a, PoiseData, Error>;
pub type ApplicationContext<'a> = poise::ApplicationContext<'a, PoiseData, Error>;

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
    media_cooldown_sender: OnceCell<Sender<CooldownMessage>>,
    /// Users that have already been called out for going into NSFW <1 hr after joining
    pub horny_callouts: Arc<RwLock<HashSet<u64>>>,
    /// Emoji ranking cache for tracking emoji usage
    pub emoji_rank: Arc<RwLock<emojirank::EmojiWatcher>>,

    /// Shared api state
    pub api_state: ApiState,

    /// For preventing catcoin farming
    pub catcoin_spam_filter: Arc<RwLock<catcoin::SpamFilter>>,

    /// NSFW role
    pub horny_role: serenity::RoleId,
    /// role to seed teh tf2 server
    pub seeder_role: serenity::RoleId,
    /// role to give users to allow them into the server
    pub member_role: serenity::RoleId,
    /// role for 6s games
    pub scrim_role: serenity::RoleId,

    /// #general
    pub general_channel: serenity::ChannelId,
    /// channel where deleted messages are logged
    pub deleted_message_log_channel: serenity::ChannelId,
    /// channel where users who left are logged
    pub leaver_log_channel: serenity::ChannelId,
    /// mod channel id
    pub _mod_channel: serenity::ChannelId,
    /// age verification channel
    pub _birthday_channel: serenity::ChannelId,
    /// for posting stock market info daily
    pub stock_market_channel: serenity::ChannelId,

    /// /seeder cooldown
    pub seeder_cooldown: Arc<RwLock<HashMap<SocketAddr, DateTime<Utc>>>>,
    /// \ !remindme
    pub reminders: Arc<RwLock<ReminderManager>>,
    /// Bot database pool
    pub local_pool: Pool<MySql>,
    /// Sourcebans database pool
    pub _sb_pool: Pool<MySql>,
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

    /// Fetches the tkgp wacky server
    pub fn wacky_server(&self) -> Result<&Server, Error> {
        self.servers
            .iter()
            .map(|s| s.1)
            .filter(|s| s.wacky_server)
            .next()
            .ok_or("Pug server not found".into())
    }

    /// checks if a seeder ping is allowed. if on cooldown, returns time until usable
    pub async fn can_seed(&self, server_addr: SocketAddr) -> Result<(), Duration> {
        // 4 hrs
        let seed_cooldown: Duration = Duration::try_milliseconds(4 * 60 * 60 * 1000).unwrap();

        let mut map = self.seeder_cooldown.write().await;
        let last_used = map.entry(server_addr).or_insert(DateTime::<Utc>::MIN_UTC);
        let now = chrono::Utc::now();

        let allowed_at = *last_used + seed_cooldown;

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
        data.media_cooldown_sender
            .get_or_init(|| async { media_cooldown::spawn_cooldown_manager(ctx) })
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
            // ignore bots
            if new_message.author.bot {
                return Ok(());
            }
            let _ = on_message::watch_emojis(ctx, data, new_message)
                .await
                .inspect_err(|e| eprintln!("watch emojis fail: {e}"));
            let _ = on_message::handle_cooldowns(ctx, data, cooldown_handler, new_message)
                .await
                .inspect_err(|e| eprintln!("media cooldown error: {e}"));
            let _ = on_message::hi_cat(ctx, data, new_message)
                .await
                .inspect_err(|e| eprintln!("hi cat error: {e}"));
            let _ = on_message::praise_the_lord(ctx, data, new_message)
                .await
                .inspect_err(|e| eprintln!("satan's bidding: {e}"));
            // rate limit catcoin rng
            if data
                .catcoin_spam_filter
                .write()
                .await
                .try_roll(new_message.author.id)
            {
                let _ = catcoin::random_pulls::on_message(ctx, &data.local_pool, new_message)
                    .await
                    .inspect_err(|e| eprintln!("Random pull fail: {e}"));
                let _ = catcoin::drops::on_message(ctx, &data.local_pool, new_message)
                    .await
                    .inspect_err(|e| eprintln!("Drop fail: {e}"));
            }
        }
        Event::MessageDelete {
            channel_id,
            deleted_message_id,
            ..
        } => {
            on_delete::save_deleted(ctx, data, channel_id, deleted_message_id).await?;
        }
        Event::ReactionAdd { add_reaction } => {
            let _ = on_react::add(ctx, data, add_reaction)
                .await
                .inspect_err(|e| eprintln!("add react fail: {e}"));
        }
        Event::ReactionRemove { removed_reaction } => {
            let _ = on_react::rm(ctx, data, removed_reaction)
                .await
                .inspect_err(|e| eprintln!("rm react fail: {e}"));
        }
        Event::InteractionCreate { interaction } => {
            if let Some(mci) = interaction.as_message_component() {
                let _ = on_component_interaction::dispatch(ctx, data, mci)
                    .await
                    .inspect_err(|e| eprintln!("Could not handle interaction create: {e}"));
            }
        }
        Event::GuildMemberRemoval { user, .. } => {
            data.leaver_log_channel
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
    servers: HashMap<SocketAddr, Server>,
    api_state: ApiState,
) -> Result<(), Error> {
    let bot_token: String = parse_env("BOT_TOKEN");
    let guild_id: u64 = parse_env("GUILD_ID");
    let deleted_messages_log_channel_id: u64 = parse_env("DELETED_MESSAGE_LOG_CHANNEL_ID");
    let leaver_log_channel_id: u64 = parse_env("LEAVER_LOG_CHANNEL_ID");
    let seeder_role_id: u64 = parse_env("SEEDER_ROLE");
    let horny_role_id: u64 = parse_env("HORNY_ROLE");
    let member_role_id: u64 = parse_env("MEMBER_ROLE");
    let scrim_role_id: u64 = parse_env("SCRIM_ROLE");
    let general_channel_id: u64 = parse_env("GENERAL_CHANNEL_ID");
    let mod_channel_id: u64 = parse_env("MOD_CHANNEL_ID");
    let birthday_channel_id: u64 = parse_env("BIRTHDAY_CHANNEL_ID");
    let stock_market_channel_id: u64 = parse_env("STOCK_MARKET_CHANNEL_ID");

    let db_url: String = parse_env("DATABASE_URL");
    let sb_db_url: String = parse_env("SB_DATABASE_URL");

    // migrate the db
    let local_pool = Pool::<MySql>::connect(&db_url).await?;
    sqlx::migrate!("../migrations").run(&local_pool).await?;
    println!("DB Migrated.");

    let sb_pool = Pool::<MySql>::connect(&sb_db_url).await?;
    println!("Connected to sourcebans pool.");

    catcoin::init(&local_pool).await?;

    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::GUILD_MEMBERS;

    let watcher = Arc::new(RwLock::new(emojirank::EmojiWatcher::new()));

    let pug_server = "pug.fluffycat.gay:27015"
        .to_socket_addrs()
        .expect("Pug address DNS resolution failed")
        .next()
        .expect("Could not resolve PUG server address.");

    let reminders = Arc::new(RwLock::new(
        ReminderManager::new_with_init(&local_pool).await?,
    ));

    let framework = {
        let watcher = watcher.clone();
        let servers = servers.clone();
        let local_pool = local_pool.clone();
        let sb_pool = sb_pool.clone();
        let pug_server = pug_server.clone();
        let reminders = reminders.clone();
        poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: commands::ALL.iter().map(|f| f()).collect(),
                event_handler: |a, b, c, d| Box::pin(event_handler(a, b, c, d)),
                prefix_options: PrefixFrameworkOptions {
                    prefix: Some("!".to_owned()),
                    ..Default::default()
                },
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

                    ctx.set_activity(Some(serenity::ActivityData::playing("with touys ^-^")));

                    Ok(PoiseData {
                        servers,
                        media_cooldown: Arc::new(RwLock::new(
                            media_cooldown::MediaCooldown::from_env(),
                        )),
                        guild_id: serenity::GuildId::new(guild_id),
                        reminders,
                        pug_cfgs: [
                            "rgl_off",
                            "rgl_7s_koth_bo5",
                            "rgl_6s_koth_scrim",
                            "rgl_6s_koth_bo5",
                            "rgl_6s_5cp_scrim",
                            "rgl_6s_5cp_match_pro",
                            "tfcl_off",
                            "tfcl_UD_ultiduo",
                        ]
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                        catcoin_spam_filter: Arc::new(RwLock::new(catcoin::SpamFilter::new())),
                        pug_server,
                        api_state,
                        emoji_rank: watcher.clone(),
                        seeder_role: serenity::RoleId::new(seeder_role_id),
                        horny_role: serenity::RoleId::new(horny_role_id),
                        member_role: serenity::RoleId::new(member_role_id),
                        scrim_role: serenity::RoleId::new(scrim_role_id),
                        horny_callouts: Arc::new(RwLock::new(HashSet::new())),
                        general_channel: serenity::ChannelId::new(general_channel_id),
                        deleted_message_log_channel: serenity::ChannelId::new(
                            deleted_messages_log_channel_id,
                        ),
                        leaver_log_channel: serenity::ChannelId::new(leaver_log_channel_id),
                        _mod_channel: serenity::ChannelId::new(mod_channel_id),
                        _birthday_channel: serenity::ChannelId::new(birthday_channel_id),
                        stock_market_channel: serenity::ChannelId::new(stock_market_channel_id),
                        media_cooldown_sender: OnceCell::new(),
                        seeder_cooldown: Arc::new(RwLock::new(HashMap::new())),
                        local_pool,
                        _sb_pool: sb_pool,
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
    )
    .await;

    {
        // exclude pug server from seed tracking
        let mut servers = servers.clone();
        servers.remove(&pug_server);
        seederboard::spawn_tracker(log_receiver.clone(), servers, local_pool.clone()).await;
    }

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
    emojirank::spawn_flush_thread(watcher.clone(), local_pool.clone());
    commands::spawn_reminder_thread(
        client.http.clone(),
        local_pool.clone(),
        guild_id,
        reminders.clone(),
    );

    let sched = JobScheduler::new().await?;

    let wacky_server_ip = "tf2.fluffycat.gay:27015"
        .to_socket_addrs()
        .expect("Wacky address DNS resolution failed")
        .next()
        .expect("Could not resolve WACKY server address.");
    let wacky_server: &Server = servers
        .get(&wacky_server_ip)
        .expect("Wacky server dose not exist.");

    sched.add(wacky::start_job(wacky_server.clone())).await?;
    sched.add(wacky::end_job(wacky_server.clone())).await?;
    stocks::init(&sched, &local_pool).await?;

    sched.start().await?;

    let fut = client.start();
    println!("Bot started!");
    fut.await.expect("Bot broke"); //

    Ok(())
}
