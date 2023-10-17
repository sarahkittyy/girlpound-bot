use std::env;
use std::sync::Arc;

use crate::{
    logs::{LogReceiver, ParsedLogMessage},
    tf2_rcon::RconController,
    Error,
};
use poise::serenity_prelude as serenity;

use tokio::{self, sync::RwLock, time};

mod commands;

pub struct PoiseData {
    pub rcon_controller: Arc<RwLock<RconController>>,
}
pub type Context<'a> = poise::Context<'a, PoiseData, Error>;

/// spawns a thread that uses RCON to count the players on the server and update the corresponding channel name
fn spawn_player_count_thread(
    rcon_controller: Arc<RwLock<RconController>>,
    ctx: Arc<serenity::CacheAndHttp>,
) {
    let live_player_channel: Option<serenity::ChannelId> = env::var("LIVE_PLAYER_CHANNEL_ID")
        .ok()
        .and_then(|id| id.parse::<u64>().ok().map(serenity::ChannelId));

    println!("LIVE_PLAYER_CHANNEL: {:?}", live_player_channel);

    if let Some(live_player_channel) = live_player_channel {
        // check player count in this interval
        let mut interval = time::interval(time::Duration::from_secs(5 * 60));
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                let player_count = {
                    let mut rcon = rcon_controller.write().await;
                    match rcon.player_count().await {
                        Ok(count) => count,
                        Err(e) => {
                            // try to reconnect on error.
                            println!("Error getting player count: {:?}", e);
                            let _ = rcon.reconnect().await;
                            continue;
                        }
                    }
                };
                // edit channel name to reflect player count
                live_player_channel
                    .edit(ctx.as_ref(), |c| {
                        c.name(format!("📶 {}/24 online", player_count))
                    })
                    .await
                    .expect("Could not edit channel name");
                println!("Updated player count to {}", player_count);
            }
        });
    }
}

/// receives logs from the tf2 server & posts them in a channel
fn spawn_log_thread(mut log_receiver: LogReceiver, ctx: Arc<serenity::CacheAndHttp>) {
    let logs_channel: Option<serenity::ChannelId> = env::var("SRCDS_LOG_CHANNEL_ID")
        .ok()
        .and_then(|id| id.parse::<u64>().ok().map(serenity::ChannelId));

    println!("SRCDS_LOG_CHANNEL_ID: {logs_channel:?}");

    if let Some(logs_channel) = logs_channel {
        let mut interval = time::interval(time::Duration::from_secs(1));
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                let msgs = log_receiver.drain().await;
                let mut output = String::new();
                for msg in msgs {
                    let parsed = ParsedLogMessage::from_message(&msg);
                    if parsed.is_known() {
                        output += parsed.as_discord_message().as_str();
                        output += "\n";
                    }
                }
                if output.len() == 0 {
                    continue;
                }
                if let Err(e) = logs_channel
                    .send_message(ctx.as_ref(), |m| m.content(output))
                    .await
                {
                    println!("Could not send message to logs channel: {:?}", e);
                }
            }
        });
    }
}

/// initialize the discord bot
pub async fn start_bot(rcon_controller: RconController, log_receiver: LogReceiver) {
    let rcon_controller = Arc::new(RwLock::new(rcon_controller));

    let bot_token = env::var("BOT_TOKEN").expect("Could not find env variable BOT_TOKEN");
    let guild_id = env::var("GUILD_ID")
        .expect("Could not find env variable GUILD_ID")
        .parse::<u64>()
        .expect("GUILD_ID could not be parsed into u64");

    let intents = serenity::GatewayIntents::non_privileged();

    let girlpounder = {
        let rcon_controller = rcon_controller.clone();
        poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![
                    commands::rcon(),
                    commands::meow(),
                    commands::status(),
                    commands::reacted_users(),
                ],
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

                    ctx.set_activity(serenity::Activity::playing("tf2.fluffycat.gay:19990"))
                        .await;

                    Ok(PoiseData { rcon_controller })
                })
            })
            .build()
            .await
            .expect("Failed to build girlpounder bot.")
    };

    // launch alt threads
    let ctx = girlpounder.client().cache_and_http.clone();
    spawn_player_count_thread(rcon_controller.clone(), ctx.clone());
    spawn_log_thread(log_receiver, ctx.clone());

    let fut = girlpounder.start();
    println!("Bot started!");
    fut.await.expect("Bot broke");
}
