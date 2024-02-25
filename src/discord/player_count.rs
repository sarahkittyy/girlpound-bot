use poise::serenity_prelude as serenity;
use std::sync::Arc;
use tokio::time;

use crate::Server;

/// spawns a thread that uses RCON to count the players on the server and update the corresponding channel name
pub fn spawn_player_count_thread(server: Server, ctx: Arc<serenity::CacheAndHttp>) {
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
                            println!("Error getting player count: {:?}", e);
                            let _ = rcon.reconnect().await;
                            continue;
                        }
                    }
                };
                // edit channel name to reflect player count
                let r = player_count_channel
                    .edit(ctx.as_ref(), |c| {
                        c.name(format!(
                            "{} {}/{} online",
                            server.emoji,
                            status.players.len(),
                            status.max_players,
                        ))
                    })
                    .await;
                if let Err(e) = r {
                    println!("Could not update player count channel: {e}");
                } else {
                    println!(
                        "Updated {} player count to {}",
                        server.name,
                        status.players.len()
                    );
                }
            }
        });
    }
}
