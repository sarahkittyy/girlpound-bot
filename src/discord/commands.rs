use super::Context;
use crate::Error;

use poise;

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
