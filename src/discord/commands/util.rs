use poise::AutocompleteChoice;

use crate::discord::Context;
use crate::Error;
use crate::Server;
use std::net::SocketAddr;

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
pub async fn users_autocomplete(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<AutocompleteChoice<String>> {
    let mut res = vec![];
    for (_addr, server) in &ctx.data().servers {
        if let Some(state) = server.controller.write().await.status().await.ok() {
            res.extend(
                state
                    .players
                    .iter()
                    .filter(|p| p.name.to_lowercase().contains(&partial.to_lowercase()))
                    .map(|p| AutocompleteChoice {
                        name: p.name.clone(),
                        value: p.name.clone(),
                    }),
            );
        }
    }
    res
}

/// Returns the list of online users
pub async fn steam_id_autocomplete(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<AutocompleteChoice<String>> {
    let mut res = vec![];
    for (_addr, server) in &ctx.data().servers {
        if let Some(state) = server.controller.write().await.status().await.ok() {
            res.extend(
                state
                    .players
                    .iter()
                    .filter(|p| p.name.to_lowercase().contains(&partial.to_lowercase()))
                    .map(|p| AutocompleteChoice {
                        name: format!("{} {}", &p.name, &p.id),
                        value: p.id.clone(),
                    }),
            );
        }
    }
    res
}

/// Returns a list of maps available on the pug server
pub async fn pug_maps_autocomplete(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<AutocompleteChoice<String>> {
    let Ok(pug_server) = ctx.data().pug_server() else {
        return vec![];
    };
    let Ok(maps) = pug_server.maps().await else {
        return vec![];
    };

    maps.into_iter()
        .filter(|map| map.to_lowercase().contains(&partial.to_lowercase()))
        .map(|map| AutocompleteChoice {
            name: map.clone(),
            value: map.clone(),
        })
        .collect()
}

pub async fn pug_cfgs_autocomplete(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<AutocompleteChoice<String>> {
    ctx.data()
        .pug_cfgs
        .iter()
        .filter(|&cfg| cfg.to_lowercase().contains(&partial.to_lowercase()))
        .map(|cfg| AutocompleteChoice {
            name: cfg.clone(),
            value: cfg.clone(),
        })
        .collect()
}

/// Returns the list of connected servers
pub async fn servers_autocomplete(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<AutocompleteChoice<SocketAddr>> {
    ctx.data()
        .servers
        .iter()
        .filter(|(_addr, s)| s.name.to_lowercase().contains(&partial.to_lowercase()))
        .map(|(addr, s)| AutocompleteChoice {
            name: s.name.clone(),
            value: addr.clone(),
        })
        .collect()
}
