use rand::prelude::*;
use std::time::Duration;

use futures::TryFutureExt;
use poise::{
    self,
    serenity_prelude::{
        self as serenity, ComponentInteractionCollector, CreateActionRow, CreateAllowedMentions,
        CreateButton, CreateEmbed, CreateInteractionResponseMessage, CreateMessage, Mentionable,
        ReactionType,
    },
    CreateReply,
};

use super::{get_catcoin, get_top, grant_catcoin, transact, try_spend_catcoin, CatcoinWallet};
use crate::{discord::Context, Error};

/// TKGP catcoin related stuff :3
#[poise::command(slash_command, subcommands("balance", "top", "pay", "drop"))]
pub async fn catcoin(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Drop catcoin in the chat for anyone fast enough to claim.
#[poise::command(slash_command, user_cooldown = 30)]
async fn drop(
    ctx: Context<'_>,
    #[description = "The amount of catcoin to drop."] amount: u64,
    #[description = "The message to include with the drop."] message: Option<String>,
) -> Result<(), Error> {
    // zero catcoin check
    if amount == 0 {
        ctx.send(
            CreateReply::default()
                .content(format!("Cannot drop **0** {}", ctx.data().catcoin_emoji))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let uuid = ctx.id();

    // first expend the catcoin
    match try_spend_catcoin(&ctx.data().local_pool, ctx.author().id, amount).await {
        Ok(false) => {
            ctx.send(CreateReply::default().ephemeral(true).content(format!(
                "You do not have **{}** {}",
                amount,
                ctx.data().catcoin_emoji
            )))
            .await?;
            return Ok(());
        }
        Err(e) => {
            ctx.send(
                CreateReply::default()
                    .ephemeral(true)
                    .content(format!("Internal error dropping catcoin: `{:?}`", e)),
            )
            .await?;
            return Ok(());
        }
        Ok(true) => (),
    };

    // user specified message, or a default
    let msg = message.unwrap_or_else(|| {
        format!(
            "{} dropped **{}** {} on the ground!",
            ctx.author().mention(),
            amount,
            ctx.data().catcoin_emoji
        )
    });

    // then drop it in chat
    let embed = CreateEmbed::new()
        .color(serenity::Color::from_rgb(random(), random(), random()))
        .title(msg);
    let button = CreateActionRow::Buttons(vec![CreateButton::new(format!("{uuid}-claim"))
        .label(format!("{amount}"))
        .emoji(
            ctx.data()
                .catcoin_emoji
                .parse::<ReactionType>()
                .expect("Could not parse catcoin emoji as ReactionType"),
        )]);

    let rh = ctx
        .send(CreateReply::default().embed(embed).components(vec![button]))
        .await?;

    // wait for first interaction
    while let Some(mci) = ComponentInteractionCollector::new(ctx)
        .channel_id(ctx.channel_id())
        .timeout(Duration::from_secs(120))
        .filter(move |mci| mci.data.custom_id.starts_with(&uuid.to_string()))
        .await
    {
        if ctx.author().id == mci.user.id {
            mci.create_response(
                &ctx,
                serenity::CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .ephemeral(true)
                        .content("Cannot claim your own catcoin drop!"),
                ),
            )
            .await?;
            continue;
        }
        rh.edit(
            ctx,
            CreateReply::default()
                .content(format!(
                    "{} picked up {}'s **{}** {}.",
                    mci.user.mention(),
                    ctx.author().mention(),
                    amount,
                    ctx.data().catcoin_emoji
                ))
                .components(vec![]),
        )
        .await?;
        grant_catcoin(&ctx.data().local_pool, mci.user.id, amount).await?;
        mci.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge)
            .await?;
        return Ok(());
    }

    rh.delete(ctx).await?;
    ctx.channel_id()
        .send_message(
            &ctx,
            CreateMessage::new().content(format!(
                "{} Your drop has expired. Refunding **{}** {}",
                ctx.author().mention(),
                amount,
                ctx.data().catcoin_emoji
            )),
        )
        .await?;
    grant_catcoin(&ctx.data().local_pool, ctx.author().id, amount).await?;

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
    if amount == 0 {
        ctx.send(
            CreateReply::default()
                .content(format!("Cannot send **0** {}", ctx.data().catcoin_emoji))
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
    ctx.defer().await?;
    let top: Vec<CatcoinWallet> = get_top(&ctx.data().local_pool).await?;
    let members = futures::future::join_all(top.into_iter().map(|wallet: CatcoinWallet| {
        let user = wallet
            .uid
            .parse::<serenity::UserId>()
            .ok()
            .expect("Invalid userID in catcoin db");
        ctx.data()
            .guild_id
            .member(&ctx, user)
            .map_ok(|member| (wallet, member))
    }))
    .await;

    let list = members
        .into_iter()
        .flatten()
        .take(10)
        .enumerate()
        .map(|(i, (wallet, member))| {
            format!(
                "**{}**. **{}** - {} {}",
                i + 1,
                member.display_name(),
                wallet.catcoin,
                ctx.data().catcoin_emoji
            )
        });
    let output = list.into_iter().collect::<Vec<String>>().join("\n");
    let embed = CreateEmbed::new()
        .title("Top Catcoin Holders 🚀🌕")
        .description(output);
    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}
