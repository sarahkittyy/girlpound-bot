use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::steamid::SteamIDClient;
use crate::{logs::LogReceiver, Error};
use crate::{parse_env, Server};
use chrono::{DateTime, Duration, TimeDelta, Utc};
use poise::serenity_prelude::{self as serenity, Mentionable};

use rand::random;
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
    /// all tf2 servers known by the bot
    pub guild_id: serenity::GuildId,
    /// guild the bot operates in
    pub media_cooldown: Arc<RwLock<media_cooldown::MediaCooldown>>,
    pub horny_role: serenity::RoleId,
    pub general_channel: serenity::ChannelId,
    pub horny_callouts: Arc<RwLock<HashSet<u64>>>,
    media_cooldown_thread: OnceCell<Sender<Cooldown>>,
    deleted_message_log_channel: serenity::ChannelId,
    pub seeder_role: serenity::RoleId,
    pub trial_mod_channel: serenity::ChannelId,
    pub msg_counts: Arc<RwLock<HashMap<u64, u64>>>,
    pub seeder_cooldown: Arc<RwLock<HashMap<SocketAddr, DateTime<Utc>>>>,
    pub pool: Pool<MySql>,
    pub client: SteamIDClient,
}
impl PoiseData {
    /// fetch the server with the given socket address
    pub fn server(&self, server_addr: SocketAddr) -> Result<&Server, Error> {
        self.servers
            .get(&server_addr)
            .ok_or("Server not found".into())
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
            if let Some(old) = old_if_available {
                if let Some(joined_at) = new.joined_at {
                    let since_join: Duration = joined_at.signed_duration_since(Utc::now()).abs();
                    if !old.roles.contains(&data.horny_role)
                        && new.roles.contains(&data.horny_role)
                        && since_join <= TimeDelta::hours(1)
                        && data.horny_callouts.write().await.insert(new.user.id.0)
                    {
                        let total_s = since_join.num_seconds();
                        let s = total_s % 60;
                        let m = (total_s / 60) % 60;
                        let h = (total_s / 60) / 60;
                        let resp = format!(
                            "{} has assigned themselves the NSFW role. Time since joining: `{:0>2}:{:0>2}:{:0>2}`",
                            new.mention(),
                            h, m, s
                        );
                        data.general_channel
                            .send_message(&ctx, |m| m.content(resp))
                            .await?;
                    }
                }
            }
        }
        Event::GuildMemberAddition { new_member } => {
            const INTROS: [&str; 8] = [
                "welcome to tiny kitty's girl pound",
                "haiiiii ^_^ hi!! hiiiiii <3 haiiiiii hii :3",
                "gweetings fwom tiny kitty's girl pound",
                "o-omg hii.. >///<",
                "welcome to da girl pound <3",
                "hello girl pounder",
                "hii lol >w<",
                "whale cum to the girl pound",
            ];

            if let Some(guild) = new_member.guild_id.to_guild_cached(ctx) {
                if let Some(sid) = guild.system_channel_id {
                    let r = (random::<f32>() * INTROS.len() as f32).floor() as usize;
                    let g = (random::<f32>() * guild.emojis.len() as f32).floor() as usize;
                    let emoji = guild.emojis.values().skip(g).next();
                    let _ = sid
                        .send_message(ctx, |m| {
                            m.content(&format!(
                                "{} {} {} | total meowmbers: {}",
                                emoji
                                    .map(|e| e.to_string())
                                    .unwrap_or(":white_check_mark:".to_string()),
                                new_member.mention(),
                                INTROS[r],
                                guild.member_count
                            ))
                        })
                        .await;
                }
            }
        }
        Event::Message { new_message } => {
            const HELPFUL_REMINDERS: [&str; 2] = [
                "keep up the good work :white_check_mark:",
                "Please be respectful to all players on the server :thumbs_up:",
            ];

            if let Some(_guild_id) = new_message.guild_id {
                // trial mod channel positivity quota
                if new_message.channel_id == data.trial_mod_channel {
                    let r: f32 = random();
                    if r < 0.1 {
                        let g = (random::<f32>() * HELPFUL_REMINDERS.len() as f32).floor() as usize;
                        new_message
                            .channel_id
                            .send_message(ctx, |m| m.content(HELPFUL_REMINDERS[g]))
                            .await?;
                    }
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
        Event::MessageDelete {
            channel_id,
            deleted_message_id,
            ..
        } => {
            let Some(message) = ctx.cache.message(channel_id, deleted_message_id) else {
                return Err("Message not found in cache")?;
            };
            let Some(channel) = channel_id.to_channel(ctx).await?.guild() else {
                return Err("Channel not found.")?;
            };
            let _ = data
                .deleted_message_log_channel
                .send_message(&ctx, |m| {
                    m.embed(|e| {
                        e.title("Deleted Message");
                        e.field("Author", message.author.tag(), true);
                        e.field("Channel", channel.name(), true);
                        e.field("Content", message.content, false);
                        e
                    });
                    m
                })
                .await;
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
        poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![
                    commands::bark(),
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

                    ctx.set_activity(serenity::Activity::playing("tf2.fluffycat.gay"))
                        .await;

                    Ok(PoiseData {
                        servers,
                        media_cooldown: Arc::new(RwLock::new(
                            media_cooldown::MediaCooldown::from_env(),
                        )),
                        guild_id: serenity::GuildId(guild_id),
                        seeder_role: serenity::RoleId(seeder_role_id),
                        msg_counts: Arc::new(RwLock::new(HashMap::new())),
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
