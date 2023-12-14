use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::steamid::SteamIDClient;
use crate::{logs::LogReceiver, Error};
use crate::{parse_env, Server};
use poise::serenity_prelude::{self as serenity};

use sqlx::{MySql, Pool};
use tokio::{self, sync::RwLock};

mod commands;
mod log_handler;
mod player_count;

pub struct PoiseData {
    pub servers: HashMap<SocketAddr, Server>,
    pub guild_id: serenity::GuildId,
    pub member_role: serenity::RoleId,
    pub private_channel: serenity::ChannelId,
    pub private_welcome_channel: serenity::ChannelId,
    pub msg_counts: Arc<RwLock<HashMap<u64, u64>>>,
    pub pool: Pool<MySql>,
    pub client: SteamIDClient,
}
impl PoiseData {
    pub fn server(&self, server_addr: SocketAddr) -> Result<&Server, Error> {
        self.servers
            .get(&server_addr)
            .ok_or("Server not found".into())
    }
}
pub type Context<'a> = poise::Context<'a, PoiseData, Error>;

/// read console stuff
/*fn spawn_stdin_thread(log_receiver: LogReceiver) {
    tokio::spawn(async move {
        let mut stdin = BufReader::new(io::stdin()).lines();
        while let Ok(Some(line)) = stdin.next_line().await {
            log_receiver.spoof_message(&line).await;
        }
    });
}*/

/// takes in the messages of every new user, counts them, and returns if they should be let in
async fn give_new_member_access(msg: &serenity::Message, data: &PoiseData) -> Result<bool, Error> {
    let mut msg_counts = data.msg_counts.write().await;
    let count = msg_counts.entry(msg.author.id.0).or_insert(0);
    *count += 1;
    if *count >= 5
        && msg.member.as_ref().is_some_and(|m| {
            // member joined at least 5 minutes ago
            m.joined_at
                .is_some_and(|date| date.timestamp() + 300 <= chrono::Utc::now().timestamp())
        })
    {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// handle discord events
pub async fn event_handler(
    ctx: &serenity::Context,
    event: &poise::Event<'_>,
    _framework: poise::FrameworkContext<'_, PoiseData, Error>,
    data: &PoiseData,
) -> Result<(), Error> {
    use poise::Event;
    match event {
        // Event::GuildMemberAddition { new_member: member } => {}
        Event::Message { new_message } => {
            if let Some(guild_id) = new_message.guild_id {
                if give_new_member_access(&new_message, data).await? {
                    ctx.http
                        .add_member_role(
                            guild_id.0,
                            new_message.author.id.0,
                            data.member_role.0,
                            Some("New member"),
                        )
                        .await?;
                }
            }
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
    let member_role: u64 = parse_env("MEMBER_ROLE");
    let private_channel_id: u64 = parse_env("PRIVATE_CHANNEL_ID");
    let private_welcome_channel_id: u64 = parse_env("PRIVATE_WELCOME_CHANNEL_ID");

    let intents = serenity::GatewayIntents::non_privileged();

    let girlpounder = {
        let servers = servers.clone();
        let pool = pool.clone();
        poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![
                    commands::rcon(),
                    commands::catsmas(),
                    commands::snipers(),
                    commands::private_add(),
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
                    poise::builtins::register_in_guild(
                        ctx,
                        &framework.options().commands,
                        serenity::GuildId(guild_id),
                    )
                    .await?;

                    ctx.set_activity(serenity::Activity::playing("tf2.fluffycat.gay"))
                        .await;

                    Ok(PoiseData {
                        servers,
                        member_role: serenity::RoleId(member_role),
                        guild_id: serenity::GuildId(guild_id),
                        private_channel: serenity::ChannelId(private_channel_id),
                        private_welcome_channel: serenity::ChannelId(private_welcome_channel_id),
                        msg_counts: Arc::new(RwLock::new(HashMap::new())),
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
