use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::steamid::SteamIDClient;
use crate::{logs::LogReceiver, Error};
use crate::{parse_env, Server};
use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude::{self as serenity};

use sqlx::{MySql, Pool};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Sender;
use tokio::sync::OnceCell;
use tokio::{self, sync::RwLock};

mod commands;
mod log_handler;
mod media_cooldown;
mod player_count;

pub struct PoiseData {
    pub servers: HashMap<SocketAddr, Server>,
    pub guild_id: serenity::GuildId,
    pub member_role: serenity::RoleId,
    pub media_cooldown: Arc<RwLock<media_cooldown::MediaCooldown>>,
    media_cooldown_thread: OnceCell<Sender<Cooldown>>,
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
        // Event::GuildMemberAddition { new_member: member } => {}
        Event::Message { new_message } => {
            if let Some(guild_id) = new_message.guild_id {
                // member role assignment (for users who are active)
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
                // media channel spam limit
                let mut media_cooldown = data.media_cooldown.write().await;
                // if we have to wait before posting an image...
                if let Err(time_left) = media_cooldown.try_allow_one(new_message) {
                    // delete the image
                    new_message.delete(ctx).await?;
                    // send da cooldown msg
                    let _ = cooldown_handler
                        .send(Cooldown {
                            channel: new_message.channel_id,
                            user: new_message.author.id,
                            delete_at: Utc::now() + time_left,
                        })
                        .await;
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
    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let girlpounder = {
        let servers = servers.clone();
        let pool = pool.clone();
        poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![
                    commands::rcon(),
                    commands::catsmas(),
                    commands::snipers(),
                    commands::playercap(),
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
                        media_cooldown: Arc::new(RwLock::new(
                            media_cooldown::MediaCooldown::from_env(),
                        )),
                        guild_id: serenity::GuildId(guild_id),
                        private_channel: serenity::ChannelId(private_channel_id),
                        private_welcome_channel: serenity::ChannelId(private_welcome_channel_id),
                        msg_counts: Arc::new(RwLock::new(HashMap::new())),
                        media_cooldown_thread: OnceCell::new(),
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
