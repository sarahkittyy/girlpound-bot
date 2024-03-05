use poise::{self, CreateReply};

use super::get_treats;
use crate::{discord::Context, Error};

/// TKGP treats stuff :3
#[poise::command(slash_command, subcommands("balance"))]
pub async fn treats(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Check your treats balance
#[poise::command(slash_command)]
async fn balance(ctx: Context<'_>) -> Result<(), Error> {
    let treats: i64 = get_treats(ctx, ctx.author().id).await?;
    ctx.send(
        CreateReply::default()
            .content(format!("You have {} kitty treats.", treats))
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
