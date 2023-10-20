use super::Context;
use crate::Error;

use poise;
use poise::serenity_prelude::{self as serenity};
use rand::prelude::*;

pub async fn rcon_and_reply(ctx: Context<'_>, cmd: String) -> Result<(), Error> {
    let mut rcon = ctx.data().rcon_controller.write().await;
    let reply = rcon.run(&cmd).await;
    match reply {
        Ok(output) => {
            if output.len() == 0 {
                ctx.say(":white_check_mark:").await
            } else {
                ctx.say(format!("```\n{}\n```", output)).await
            }
        }
        Err(e) => ctx.say(format!("RCON error: {:?}", e)).await,
    }?;
    Ok(())
}

/// Sends an RCON command to the server.
#[poise::command(slash_command)]
pub async fn rcon(
    ctx: Context<'_>,
    #[description = "The command to send."] cmd: String,
) -> Result<(), Error> {
    rcon_and_reply(ctx, cmd).await
}

/// Ban a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2ban(
    ctx: Context<'_>,
    #[description = "The username to ban."] username: String,
    #[description = "Time to ban them for, in minutes"] minutes: u32,
    #[description = "The reason for the ban"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("undesirable".to_owned());
    rcon_and_reply(ctx, format!("sm_ban {} {} {}", username, minutes, reason)).await
}

/// Unban a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2unban(
    ctx: Context<'_>,
    #[description = "The steamid / ip to ban."] steamid: String,
    #[description = "The reason for the unban"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("chill".to_owned());
    rcon_and_reply(ctx, format!("sm_unban {} {}", steamid, reason)).await
}

/// Kick a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2kick(
    ctx: Context<'_>,
    #[description = "The username to kick."] username: String,
    #[description = "The reason for the kick"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("".to_owned());
    rcon_and_reply(ctx, format!("sm_kick {} {}", username, reason)).await
}

/// Mute a user's vc on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2mute(
    ctx: Context<'_>,
    #[description = "The username to mute."] username: String,
    #[description = "Time to mute them for, in minutes"] minutes: Option<u32>,
    #[description = "The reason for the mute"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("".to_owned());
    let minutes = minutes.unwrap_or(0);
    rcon_and_reply(ctx, format!("sm_mute {} {} {}", username, minutes, reason)).await
}

/// Unmute a user's vc on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2unmute(
    ctx: Context<'_>,
    #[description = "The username to unmute."] username: String,
    #[description = "The reason for the unmute"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("".to_owned());
    rcon_and_reply(ctx, format!("sm_unmute {} {}", username, reason)).await
}

/// Gag a user's text chat on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2gag(
    ctx: Context<'_>,
    #[description = "The username to gag."] username: String,
    #[description = "Time to gag them for, in minutes"] minutes: Option<u32>,
    #[description = "The reason for the gag"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("".to_owned());
    let minutes = minutes.unwrap_or(0);
    rcon_and_reply(ctx, format!("sm_gag {} {} {}", username, minutes, reason)).await
}

/// Ungag a user's text chat on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2ungag(
    ctx: Context<'_>,
    #[description = "The username to ungag."] username: String,
    #[description = "The reason for the ungag"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("".to_owned());
    rcon_and_reply(ctx, format!("sm_gag {} {}", username, reason)).await
}

/// Displays current server player count & map.
#[poise::command(slash_command)]
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let mut rcon = ctx.data().rcon_controller.write().await;
    let state = rcon.status().await?;

    use crate::logs::safe_strip;

    let list = state
        .players
        .iter()
        .map(|p| safe_strip(&p.name))
        .collect::<Vec<String>>()
        .join(", ");
    ctx.say(format!(
        "Currently playing: `{}`\nThere are {}/24 players online.\n`{}`",
        state.map,
        state.players.len(),
        list
    ))
    .await?;
    Ok(())
}

/// Pick a random user with the given role
#[poise::command(slash_command)]
pub async fn reacted_users(
    ctx: Context<'_>,
    #[description = "The message to fetch reactions from"] message: serenity::Message,
) -> Result<(), Error> {
    let mut total = vec![];
    let mut after: Option<serenity::UserId> = None;
    let r_type = &message.reactions.first().unwrap().reaction_type;
    loop {
        let mut users = match message
            .reaction_users(&ctx, r_type.clone(), Some(50), after)
            .await
        {
            Ok(users) => users,
            Err(e) => {
                println!("Error fetching users: {:?}", e);
                break;
            }
        };
        let user_count = users.len();
        if user_count == 0 {
            break;
        }
        let last_user_id = users.last().unwrap().id;
        total.append(&mut users);
        if user_count < 50 {
            break;
        } else {
            after = Some(last_user_id)
        }
    }
    let str = total
        .iter()
        .map(|u| u.tag())
        .collect::<Vec<String>>()
        .join("\n");
    ctx.reply(format!("emoji: {}\n```\n{}\n```", r_type, str))
        .await?;
    Ok(())
}

/// Meow (server boosters only)
#[poise::command(slash_command, channel_cooldown = 4)]
pub async fn meow(ctx: Context<'_>) -> Result<(), Error> {
    let meows = [
        "meow!! :revolving_hearts:",
        "nya >w<",
        "prraow",
        "mrp",
        "prraow!! nya raow... mew !!! :D",
        "hehe, nya !!",
        "prrrp",
        "meow",
        "meow. >:(",
        "meow >:3",
        "MRRRAOW!!!",
        "ᵐᵉᵒʷ",
        "mew >w<",
        "nya~! >//<",
        "prraow raow... nya mrrp purrrr..",
        "purrr....... <3",
        "mp <333333",
        "*opens mouth, but doesn't actually meow*",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "purr~",
        "*meows*",
        "*meows at u*",
        "mrrrrrrrrrrrr",
        "mrrrrrraow.................",
        "mew !!! mew :3 myaow raow :3 !!!",
        "is she, yknow, like, *curls paw*?",
        "ehe, uhm, mraow !! >w<",
        "guh wuh huh ?? nya...",
        "eep!! *purrs*",
        "rawr i'm feral !!!! grrr >_<",
        "rrrr",
        ":3",
    ];
    let r = (random::<f32>() * meows.len() as f32).floor() as usize;

    poise::send_reply(ctx, |message| message.content(meows[r])).await?;
    Ok(())
}
