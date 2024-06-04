use poise::{
    self,
    serenity_prelude::{self as serenity, CreateAllowedMentions, CreateEmbed, Mentionable},
    CreateReply,
};

use super::{get_catcoin, get_top, transact, CatcoinWallet};
use crate::{discord::Context, Error};

/// TKGP catcoin related stuff :3
#[poise::command(slash_command, subcommands("balance", "top", "pay"))]
pub async fn catcoin(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Pay a user some catcoin
#[poise::command(slash_command, user_cooldown = 10)]
async fn pay(
    ctx: Context<'_>,
    #[description = "The catcoin recipient."] to: serenity::User,
    #[description = "The amount to send."] amount: u64,
) -> Result<(), Error> {
    if to.id == ctx.author().id {
        ctx.send(
            CreateReply::default()
                .content("Cannot send yourself catcoin!")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }
    let (reply, ephemeral) =
        match transact(&ctx.data().local_pool, ctx.author().id, to.id, amount).await {
            Ok(true) => (
                format!(
                    "Sent **{}** {} to {}.",
                    amount,
                    ctx.data().catcoin_emoji,
                    to.mention()
                ),
                false,
            ),
            Ok(false) => (
                format!(
                    "You do not have enough catcoin {}.",
                    ctx.data().catcoin_emoji
                ),
                true,
            ),
            Err(e) => (format!("Internal payment error: `{:?}`", e), true),
        };

    ctx.send(
        CreateReply::default()
            .content(reply)
            .ephemeral(ephemeral)
            .allowed_mentions(CreateAllowedMentions::new().all_users(true)),
    )
    .await?;

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
