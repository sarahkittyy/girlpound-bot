use poise::serenity_prelude as serenity;
use serenity::AutocompleteChoice;
use std::{net::SocketAddr, time::Duration};
use tokio::sync::mpsc;

use crate::discord::Context;

use common::Error;
use tf2::Server;

pub async fn rcon_user_output(servers: &[&Server], cmd: String) -> String {
    let mut outputs: Vec<String> = vec![];
    for server in servers {
        let mut rcon = server.controller.write().await;
        let output = match rcon.run(&cmd).await {
            Ok(output) => {
                if output.is_empty() {
                    ":white_check_mark:".to_owned()
                } else {
                    format!(" `{}`", output.trim())
                }
            }
            Err(e) => e.to_string(),
        };
        outputs.push(format!("{}{}", server.emoji, output))
    }
    outputs.sort();
    outputs.join("\n")
}

pub fn output_servers(ctx: Context<'_>, addr: Option<SocketAddr>) -> Result<Vec<&Server>, Error> {
    Ok(if let Some(addr) = addr {
        vec![ctx.data().server(addr)?]
    } else {
        ctx.data().servers.values().collect()
    })
}

pub async fn rcon_and_reply(
    ctx: Context<'_>,
    server: Option<SocketAddr>,
    cmd: String,
) -> Result<(), Error> {
    ctx.say(rcon_user_output(&output_servers(ctx, server)?, cmd).await)
        .await?;
    Ok(())
}

/// Returns the list of online users
pub async fn users_autocomplete(ctx: Context<'_>, partial: &str) -> Vec<AutocompleteChoice> {
    let (tx, rx) = mpsc::channel(100);
    for (_addr, server) in &ctx.data().servers {
        let tx = tx.clone();
        let server = server.clone();
        let partial = partial.to_owned();
        tokio::spawn(async move {
            if let Ok(state) = server.controller.write().await.status().await {
                let _ = tx
                    .send(
                        state
                            .players
                            .iter()
                            .filter(|p| p.name.to_lowercase().contains(&partial.to_lowercase()))
                            .map(|p| AutocompleteChoice::new(p.name.clone(), p.name.clone()))
                            .collect::<Vec<AutocompleteChoice>>(),
                    )
                    .await;
            }
        });
    }
    drop(tx);
    common::util::recv_timeout(rx, Duration::from_millis(2500))
        .await
        .concat()
}

/// Returns the list of online users
pub async fn steam_id_autocomplete(ctx: Context<'_>, partial: &str) -> Vec<AutocompleteChoice> {
    let (tx, rx) = mpsc::channel(100);
    for (_addr, server) in &ctx.data().servers {
        let tx = tx.clone();
        let server = server.clone();
        let partial = partial.to_owned();
        tokio::spawn(async move {
            if let Ok(state) = server.controller.write().await.status().await {
                let _ = tx
                    .send(
                        state
                            .players
                            .iter()
                            .filter(|p| p.name.to_lowercase().contains(&partial.to_lowercase()))
                            .map(|p| {
                                AutocompleteChoice::new(
                                    format!("{} {}", &p.name, &p.id),
                                    p.id.clone(),
                                )
                            })
                            .collect::<Vec<AutocompleteChoice>>(),
                    )
                    .await;
            }
        });
    }
    drop(tx);
    common::util::recv_timeout(rx, Duration::from_millis(2500))
        .await
        .concat()
}

/// Returns a list of maps available on the pug server
pub async fn pug_maps_autocomplete(ctx: Context<'_>, partial: &str) -> Vec<AutocompleteChoice> {
    let Ok(pug_server) = ctx.data().pug_server() else {
        return vec![];
    };
    let Ok(maps) = pug_server.maps().await else {
        return vec![];
    };

    maps.into_iter()
        .filter(|map| map.to_lowercase().contains(&partial.to_lowercase()))
        .map(|map| AutocompleteChoice::new(map.clone(), map.clone()))
        .collect()
}

pub async fn pug_cfgs_autocomplete(ctx: Context<'_>, partial: &str) -> Vec<AutocompleteChoice> {
    ctx.data()
        .pug_cfgs
        .iter()
        .filter(|&cfg| cfg.to_lowercase().contains(&partial.to_lowercase()))
        .map(|cfg| AutocompleteChoice::new(cfg.clone(), cfg.clone()))
        .collect()
}

/// Returns the list of connected servers
pub async fn servers_autocomplete(ctx: Context<'_>, partial: &str) -> Vec<AutocompleteChoice> {
    ctx.data()
        .servers
        .iter()
        .filter(|(_addr, s)| s.name.to_lowercase().contains(&partial.to_lowercase()))
        .map(|(addr, s)| AutocompleteChoice::new(s.name.clone(), addr.to_string()))
        .collect()
}
