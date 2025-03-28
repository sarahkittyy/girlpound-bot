use crate::{discord::Context, Error};
use poise;
use poise::CreateReply;
use tf2::{banid, rcon_user_output};

use super::util::{
    rcon_and_reply, servers_autocomplete, steam_id_autocomplete, users_autocomplete,
};

/// Ban a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2ban(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<String>,
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

/// Raw ban (when sourcemod is down)
#[poise::command(slash_command)]
pub async fn tf2banraw(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: String,
    #[description = "The user to ban"]
    #[autocomplete = "steam_id_autocomplete"]
    user: String,
    #[description = "Time to ban them for, in minutes"] minutes: u32,
) -> Result<(), Error> {
    let cmd = format!("banid \"{}\" {} kick", minutes, user);
    rcon_and_reply(ctx, Some(server), cmd).await
}

/// Raw kick (when sourcemod is down)
#[poise::command(slash_command)]
pub async fn tf2kickraw(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: String,
    #[description = "The user to kick"]
    #[autocomplete = "steam_id_autocomplete"]
    user: String,
    #[description = "Reason"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("1984".to_owned());
    let cmd = format!("kickid \"{}\" {}", user, reason);
    rcon_and_reply(ctx, Some(server), cmd).await
}

/// Ban a steam id from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2banid(
    ctx: Context<'_>,
    #[description = "The steam id to ban"] id: String,
    #[description = "Time to ban them for, in minutes"] minutes: u32,
    #[description = "The reason for the ban"] reason: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let result = banid(
        &ctx.data().steamid_client,
        &id,
        &ctx.data().servers.values().collect::<Vec<&tf2::Server>>(),
        minutes,
        &reason.unwrap_or("undesirable".to_owned()),
    )
    .await;
    ctx.send(CreateReply::default().content(result)).await?;

    Ok(())
}

/// Unban a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2unban(
    ctx: Context<'_>,
    #[description = "The steamid / ip to unban."] steamid: String,
    #[description = "The reason for the unban"] reason: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let Ok(profile) = ctx
        .data()
        .steamid_client
        .lookup(&steamid)
        .await
        .and_then(|profiles| profiles.first().cloned().ok_or("No profile found".into()))
    else {
        ctx.send(
            CreateReply::default()
                .content(format!("Could not resolve given SteamID to a profile.")),
        )
        .await?;
        return Ok(());
    };

    let reason = reason.unwrap_or("chill".to_owned());
    let _ = rcon_user_output(
        &[ctx.data().servers.values().next().unwrap()],
        format!("sm_unban {} {}", profile.steamid, reason),
    )
    .await;

    ctx.send(CreateReply::default().content(format!(
        "Unbanned https://steamcommunity.com/profiles/{}",
        &profile.steamid64
    )))
    .await?;

    Ok(())
}

/// Kick a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2kick(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<String>,
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
    server: Option<String>,
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
    server: Option<String>,
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
    server: Option<String>,
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
    server: Option<String>,
    #[description = "The username to gag."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "The reason for the ungag"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("".to_owned());
    rcon_and_reply(ctx, server, format!("sm_ungag \"{}\" {}", username, reason)).await
}
