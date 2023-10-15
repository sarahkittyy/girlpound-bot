use super::Context;
use crate::Error;

use poise;
use poise::serenity_prelude::{self as serenity};
use rand::prelude::*;

/// Sends an RCON command to the server.
#[poise::command(slash_command)]
pub async fn rcon(
    ctx: Context<'_>,
    #[description = "The command to send."] cmd: String,
) -> Result<(), Error> {
    let mut rcon = ctx.data().rcon_controller.write().await;
    let reply = rcon.run(&cmd).await;
    match reply {
        Ok(output) => ctx.say(format!("```\n{}\n```", output)).await,
        Err(e) => ctx.say(format!("RCON error: {:?}", e)).await,
    }?;
    Ok(())
}

/// Displays current server player count & map.
#[poise::command(slash_command)]
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let mut rcon = ctx.data().rcon_controller.write().await;
    let state = rcon.status().await?;
    let list = state
        .players
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<&str>>()
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
    ctx.reply(format!("```\n{}\n```", str)).await?;
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
        "rrrr",
        ":3",
    ];
    let r = (random::<f32>() * meows.len() as f32).floor() as usize;

    poise::send_reply(ctx, |message| message.content(meows[r])).await?;
    Ok(())
}
