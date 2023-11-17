use poise::serenity_prelude as serenity;
use std::env;
use std::sync::Arc;
use tokio::{sync::RwLock, time};

use crate::tf2_rcon::RconController;

/// spawns a thread that uses RCON to count the players on the server and update the corresponding channel name
pub fn spawn_player_count_thread(
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
