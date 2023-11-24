use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use crate::{logs::LogReceiver, tf2_rcon::RconController, Error};
use poise::serenity_prelude as serenity;

use sqlx::{MySql, Pool};
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::{self, sync::RwLock};

mod commands;
mod log_handler;
mod player_count;

pub struct PoiseData {
    pub rcon_controller: Arc<RwLock<RconController>>,
    pub guild_id: serenity::GuildId,
    pub member_role: serenity::RoleId,
    pub private_channel: serenity::ChannelId,
    pub private_welcome_channel: serenity::ChannelId,
    pub msg_counts: Arc<RwLock<HashMap<u64, u64>>>,
}
pub type Context<'a> = poise::Context<'a, PoiseData, Error>;

/// read console stuff
fn spawn_stdin_thread(log_receiver: LogReceiver) {
    tokio::spawn(async move {
        let mut stdin = BufReader::new(io::stdin()).lines();
        while let Ok(Some(line)) = stdin.next_line().await {
            log_receiver.spoof_message(&line).await;
        }
    });
}

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
    rcon_controller: RconController,
    log_receiver: LogReceiver,
    pool: Pool<MySql>,
) {
    let rcon_controller = Arc::new(RwLock::new(rcon_controller));

    let bot_token = env::var("BOT_TOKEN").expect("Could not find env variable BOT_TOKEN");
    let guild_id = env::var("GUILD_ID")
        .expect("Could not find env variable GUILD_ID")
        .parse::<u64>()
        .expect("GUILD_ID could not be parsed into u64");
    let member_role = env::var("MEMBER_ROLE")
        .expect("Could not find env variable MEMBER_ROLE")
        .parse::<u64>()
        .expect("MEMBER_ROLE could not be parsed into u64");
    let private_channel_id = env::var("PRIVATE_CHANNEL_ID")
        .expect("Could not find env variable PRIVATE_CHANNEL_ID")
        .parse::<u64>()
        .expect("PRIVATE_CHANNEL_ID could not be parsed into u64");
    let private_welcome_channel_id = env::var("PRIVATE_WELCOME_CHANNEL_ID")
        .expect("Could not find env variable PRIVATE_WELCOME_CHANNEL_ID")
        .parse::<u64>()
        .expect("PRIVATE_WELCOME_CHANNEL_ID could not be parsed into u64");

    let intents = serenity::GatewayIntents::non_privileged();

    let girlpounder = {
        let rcon_controller = rcon_controller.clone();
        poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![
                    commands::rcon(),
                    commands::private_add(),
                    commands::meow(),
                    commands::status(),
                    commands::reacted_users(),
                    commands::feedback(),
                    commands::tf2ban(),
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
                        rcon_controller,
                        member_role: serenity::RoleId(member_role),
                        guild_id: serenity::GuildId(guild_id),
                        private_channel: serenity::ChannelId(private_channel_id),
                        private_welcome_channel: serenity::ChannelId(private_welcome_channel_id),
                        msg_counts: Arc::new(RwLock::new(HashMap::new())),
                    })
                })
            })
            .build()
            .await
            .expect("Failed to build girlpounder bot.")
    };
    // stdin thread

    // launch alt threads
    let ctx = girlpounder.client().cache_and_http.clone();
    player_count::spawn_player_count_thread(rcon_controller.clone(), ctx.clone());
    log_handler::spawn_log_thread(log_receiver.clone(), pool.clone(), ctx.clone());
    spawn_stdin_thread(log_receiver.clone());

    let fut = girlpounder.start();
    println!("Bot started!");
    fut.await.expect("Bot broke"); //
}
