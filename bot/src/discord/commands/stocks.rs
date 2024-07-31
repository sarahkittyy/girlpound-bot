use crate::discord::Context;

use common::Error;
use stocks::{post_market_floor, step_market};

/// TODO: DELETE IN PROD
#[poise::command(slash_command, subcommands("write", "step"))]
pub async fn stocks(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
async fn write(ctx: Context<'_>) -> Result<(), Error> {
    ctx.reply("ok").await?;
    post_market_floor(
        ctx.serenity_context(),
        &ctx.data().local_pool,
        ctx.data().stock_market_channel,
    )
    .await?;
    Ok(())
}

#[poise::command(slash_command)]
async fn step(ctx: Context<'_>) -> Result<(), Error> {
    ctx.reply("ok").await?;
    for _ in 0..=30 {
        step_market(ctx.serenity_context(), &ctx.data().local_pool).await?;
    }
    post_market_floor(
        ctx.serenity_context(),
        &ctx.data().local_pool,
        ctx.data().stock_market_channel,
    )
    .await?;
    Ok(())
}
