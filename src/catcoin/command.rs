use poise::{self, CreateReply};

use super::get_catcoin;
use crate::{discord::Context, Error};

/// TKGP catcoin related stuff :3
#[poise::command(slash_command, subcommands("balance"))]
pub async fn catcoin(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Check your catcoin balance
#[poise::command(slash_command)]
async fn balance(ctx: Context<'_>) -> Result<(), Error> {
    let catcoin: i64 = get_catcoin(ctx, ctx.author().id).await?;
    ctx.send(
        CreateReply::default()
            .content(format!("You have {} catcoin.", catcoin))
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
