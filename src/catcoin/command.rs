use poise::{self, serenity_prelude::CreateEmbed, CreateReply};

use super::{get_catcoin, get_top, CatcoinWallet};
use crate::{discord::Context, Error};

/// TKGP catcoin related stuff :3
#[poise::command(slash_command, subcommands("balance", "top"))]
pub async fn catcoin(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Check your catcoin balance
#[poise::command(slash_command)]
async fn balance(ctx: Context<'_>) -> Result<(), Error> {
    let catcoin: i64 = get_catcoin(&ctx.data().local_pool, ctx.author().id)
        .await?
        .catcoin;
    ctx.send(CreateReply::default().content(format!(
        "{} You have {} catcoin.",
        ctx.data().catcoin_emoji,
        catcoin
    )))
    .await?;
    Ok(())
}

/// Check top catcoin wallets
#[poise::command(slash_command)]
async fn top(ctx: Context<'_>) -> Result<(), Error> {
    let top: Vec<CatcoinWallet> = get_top(&ctx.data().local_pool).await?;
    let list = top
        .into_iter()
        .enumerate()
        .map(|(i, wallet)| {
            format!(
                "**{}**. <@{}> - {} {}",
                i + 1,
                wallet.uid,
                wallet.catcoin,
                ctx.data().catcoin_emoji
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    let embed = CreateEmbed::new()
        .title("Top Catcoin Holders 🚀🌕")
        .description(list);
    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}
