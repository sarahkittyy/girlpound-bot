use poise::serenity_prelude as serenity;
use serenity::EditChannel;
use std::sync::Arc;
use tokio::time;

use tf2::Server;

/// spawns a thread that uses RCON to count the players on the server and update the corresponding channel name
pub fn spawn_player_count_thread(server: Server, ctx: Arc<serenity::Http>) {
    if let Some(player_count_channel) = server.player_count_channel {
        // check player count in this interval
        let mut interval = time::interval(time::Duration::from_secs(5 * 61));
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                let status = {
                    let mut rcon = server.controller.write().await;
                    match rcon.status().await {
                        Ok(v) => v,
                        Err(e) => {
                            // try to reconnect on error.
                            log::info!("Error getting player count: {:?}", e);
                            let _ = rcon.reconnect().await;
                            continue;
                        }
                    }
                };
                // edit channel name to reflect player count
                let r = player_count_channel
                    .edit(
                        &ctx,
                        EditChannel::new().name(format!(
                            "{} {}/{} online",
                            server.emoji,
                            status.players.len(),
                            status.max_players,
                        )),
                    )
                    .await;
                if let Err(e) = r {
                    log::info!("Could not update player count channel: {e}");
                } else {
                    log::info!(
                        "Updated {} player count to {}",
                        server.name,
                        status.players.len()
                    );
                }
            }
        });
    }
}
