use super::Context;
use crate::Error;

use poise;
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

/// Displays current server player count.
#[poise::command(slash_command)]
pub async fn online(ctx: Context<'_>) -> Result<(), Error> {
    let mut rcon = ctx.data().rcon_controller.write().await;
    let count = rcon.player_count().await?;
    ctx.say(format!("There are {} players online.", count))
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
        ":3",
    ];
    let r = (random::<f32>() * meows.len() as f32).floor() as usize;

    poise::send_reply(ctx, |message| message.ephemeral(true).content(meows[r])).await?;
    Ok(())
}
