use std::net::SocketAddr;

use crate::{discord::Context, Error};
use poise;
use poise::CreateReply;

use regex::Regex;

use super::util::{
    output_servers, rcon_and_reply, rcon_user_output, servers_autocomplete, users_autocomplete,
};

/// Ban a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2ban(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "The username to ban."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "Time to ban them for, in minutes"] minutes: u32,
    #[description = "The reason for the ban"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("undesirable".to_owned());
    let cmd = format!("sm_ban \"{}\" {} {}", username, minutes, reason);
    rcon_and_reply(ctx, server, cmd).await
}

/// Ban a steam id from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2banid(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "The steam id to ban"] id: String,
    #[description = "Time to ban them for, in minutes"] minutes: u32,
    #[description = "The reason for the ban"] reason: Option<String>,
) -> Result<(), Error> {
    let sid_re = Regex::new(r#"(?:(STEAM_\d+:\d+:\d+)|\[?(.:1:\d+)]?)"#).unwrap();
    let Some(caps) = sid_re.captures(&id) else {
        ctx.send(CreateReply::default().content(
			format!("Invalid steam id format. Examples: STEAM_0:0:150459717, [U:1:125818436], U:1:125818436")
		).ephemeral(true)).await?;
        return Ok(());
    };
    let id = if let Some(id) = caps.get(1) {
        id.as_str().to_owned()
    } else if let Some(id3) = caps.get(2) {
        format!("[{}]", id3.as_str())
    } else {
        ctx.send(CreateReply::default().content(
			format!("Invalid steam id format. Examples: STEAM_0:0:150459717, [U:1:125818436], U:1:125818436")
		).ephemeral(true)).await?;
        return Ok(());
    };
    let reason = reason.unwrap_or("undesirable".to_owned());
    let cmd = format!("sm_addban {} {} {}", minutes, id, reason);
    let reply = rcon_user_output(&output_servers(ctx, server)?, cmd).await;
    ctx.send(CreateReply::default().content(reply)).await?;

    Ok(())
}

/// Unban a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2unban(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "The steamid / ip to unban."] steamid: String,
    #[description = "The reason for the unban"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("chill".to_owned());
    rcon_and_reply(ctx, server, format!("sm_unban {} {}", steamid, reason)).await
}

/// Kick a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2kick(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "The username to kick."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "The reason for the kick"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("1984".to_owned());
    rcon_and_reply(ctx, server, format!("sm_kick \"{}\" {}", username, reason)).await
}

/// Mute a user's vc on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2mute(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "The username to mute."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "Time to mute them for, in minutes"] minutes: Option<u32>,
    #[description = "The reason for the mute"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("1984".to_owned());
    let minutes = minutes.unwrap_or(0);
    rcon_and_reply(
        ctx,
        server,
        format!("sm_mute \"{}\" {} {}", username, minutes, reason),
    )
    .await
}

/// Unmute a user's vc on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2unmute(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "The username to unmute."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "The reason for the unmute"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("vibin".to_owned());
    rcon_and_reply(
        ctx,
        server,
        format!("sm_unmute \"{}\" {}", username, reason),
    )
    .await
}

/// Gag a user's text chat on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2gag(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "The username to gag."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "Time to gag them for, in minutes"] minutes: Option<u32>,
    #[description = "The reason for the gag"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("1984".to_owned());
    let minutes = minutes.unwrap_or(0);
    rcon_and_reply(
        ctx,
        server,
        format!("sm_gag \"{}\" {} {}", username, minutes, reason),
    )
    .await
}

/// Ungag a user's text chat on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2ungag(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "The username to gag."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "The reason for the ungag"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("".to_owned());
    rcon_and_reply(ctx, server, format!("sm_ungag \"{}\" {}", username, reason)).await
}
