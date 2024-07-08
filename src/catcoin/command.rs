use rand::prelude::*;
use std::time::Duration;

use futures::TryFutureExt;
use poise::{
    self,
    serenity_prelude::{
        self as serenity, parse_message_url, ComponentInteractionCollector, CreateActionRow,
        CreateAllowedMentions, CreateButton, CreateEmbed, CreateEmbedFooter,
        CreateInteractionResponseMessage, CreateMessage, Mentionable, ReactionType,
    },
    CreateReply,
};

use super::{
    get_catcoin, get_top, grant_catcoin, inv::claim_old_pull, inv::CatcoinPullMessageData,
    inv::PaginatedInventory, transact, try_spend_catcoin, CatcoinWallet,
};
use crate::{discord::Context, Error};

/// TKGP catcoin related stuff :3
#[poise::command(slash_command, subcommands("balance", "top", "pay", "drop", "inv"))]
pub async fn catcoin(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Display your inventory, or someone else's.
#[poise::command(slash_command, user_cooldown = 10)]
pub async fn inv(
    ctx: Context<'_>,
    #[description = "The user who's inventory to fetch"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let uuid = ctx.id();

    let user: &serenity::User = user.as_ref().unwrap_or(ctx.author());

    let mut pages: Vec<PaginatedInventory> =
        vec![PaginatedInventory::get(&ctx.data().local_pool, user.id).await?];
    let mut page: usize = 0;

    let prev_id = format!("{uuid}-prev");
    let next_id = format!("{uuid}-next");
    let close_id = format!("{uuid}-close");
    let page_buttons = |pages: &Vec<PaginatedInventory>, page: usize| {
        let mut v = vec![];
        if page > 0 {
            v.push(CreateButton::new(prev_id.clone()).emoji(ReactionType::Unicode("⬅️".to_owned())));
        }
        if pages[page].has_next() {
            v.push(CreateButton::new(next_id.clone()).emoji(ReactionType::Unicode("➡️".to_owned())));
        }
        v.push(CreateButton::new(close_id.clone()).emoji(ReactionType::Unicode("❌".to_owned())));
        vec![CreateActionRow::Buttons(v)]
    };

    // initial response
    let rh = ctx
        .send(
            CreateReply::default()
                .embed(
                    pages[page]
                        .to_embed(&ctx)
                        .await?
                        .footer(CreateEmbedFooter::new(format!("Page {}", page + 1))),
                )
                .components(page_buttons(&pages, page)),
        )
        .await?;

    while let Some(mci) = ComponentInteractionCollector::new(ctx)
        .channel_id(ctx.channel_id())
        .timeout(Duration::from_secs(120))
        .filter(move |mci| mci.data.custom_id.starts_with(&uuid.to_string()))
        .await
    {
        if mci.data.custom_id == prev_id {
            page = (page - 1).max(0);
            rh.edit(
                ctx,
                CreateReply::default()
                    .embed(
                        pages[page]
                            .to_embed(&ctx)
                            .await?
                            .footer(CreateEmbedFooter::new(format!("Page {}", page + 1))),
                    )
                    .components(page_buttons(&pages, page)),
            )
            .await?;
            mci.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge)
                .await?;
        } else if mci.data.custom_id == next_id {
            // if page is not fetched yet,
            if (page + 1) >= pages.len() {
                // fetch it
                pages.push(pages[page].next(&ctx.data().local_pool).await?.unwrap());
            }
            page = page + 1;
            rh.edit(
                ctx,
                CreateReply::default()
                    .embed(
                        pages[page]
                            .to_embed(&ctx)
                            .await?
                            .footer(CreateEmbedFooter::new(format!("Page {}", page + 1))),
                    )
                    .components(page_buttons(&pages, page)),
            )
            .await?;
            mci.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge)
                .await?;
        } else if mci.data.custom_id == close_id {
            rh.delete(ctx).await?;
            mci.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge)
                .await?;
            break;
        }
    }

    Ok(())
}

/// Claim an old pull
#[poise::command(slash_command, user_cooldown = 3)]
async fn _claim(
    ctx: Context<'_>,
    #[description = "The link to the pull message"] message_link: String,
) -> Result<(), Error> {
    let Some((_gid, cid, mid)) = parse_message_url(&message_link) else {
        ctx.send(
            CreateReply::default()
                .content("Invalid message url")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    };
    let msg = cid.message(ctx, mid).await?;
    if msg.author.id != ctx.framework().bot_id {
        ctx.send(
            CreateReply::default()
                .content("Not a bot message!")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }
    // try and get a catcoin pull from the message
    let Ok(pull) = CatcoinPullMessageData::try_from(msg) else {
        ctx.send(
            CreateReply::default()
                .content("Not a catcoin pull!")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    };

    let inserted = claim_old_pull(&ctx.data().local_pool, &pull).await?;

    if inserted {
        ctx.send(
            CreateReply::default()
                .content(format!(
                    "Added `{} | {} #{}` to <@{}>'s inventory.",
                    pull.rarity,
                    pull.name,
                    pull.number,
                    pull.uid.get()
                ))
                .ephemeral(true),
        )
        .await?;
    } else {
        ctx.send(
            CreateReply::default()
                .content("That reward has already been claimed!")
                .ephemeral(true),
        )
        .await?;
    }

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
            ctx.author()
                .global_name
                .as_deref()
                .unwrap_or(ctx.author().name.as_ref()),
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
