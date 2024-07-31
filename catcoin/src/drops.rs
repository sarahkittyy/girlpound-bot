use std::time::Duration;

use rand::prelude::*;

use poise::serenity_prelude::{
    self as serenity, ComponentInteractionCollector, Context, CreateActionRow, CreateButton,
    CreateEmbed, CreateMessage, EditMessage, Mentionable, Message, ReactionType,
};

use common::Error;
use sqlx::{MySql, Pool};

const TRIP_MESSAGES: &'static [&'static str] = &[
    "meow!! >w< %u tripped and lost **%c** %e",
    "total noob fail!!!! %u fell over and dropped **%c** %e",
    "a kitty girl ripped a hole in %u's pocket.",
    "*kicks %u in the tummy really hard, spilling **%c** %e*",
    "*punches %u in the gut, spilling **%c** %e*",
    "%u slips on a banana peel, dropping **%c** %e",
];

fn random_trip_msg(name: &str, amount: u64, catcoin_emoji: &str) -> String {
    TRIP_MESSAGES
        .choose(&mut thread_rng())
        .unwrap() //
        .replace("%u", name)
        .replace("%c", &amount.to_string())
        .replace("%e", catcoin_emoji)
}

use crate::emoji;

use super::{get_catcoin, grant_catcoin, spend_catcoin};
/// Users rarely trip and drop some of their catcoin
pub async fn on_message(ctx: &Context, pool: &Pool<MySql>, msg: &Message) -> Result<(), Error> {
    let uuid = msg.id.get();

    // check if drop should occur
    {
        let mut rng = thread_rng();

        // chance to trip
        if !rng.gen_ratio(1, 3000) {
            return Ok(());
        }
    };

    // make sure wallet isnt empty
    let has = get_catcoin(pool, msg.author.id).await?;
    if has.catcoin == 0 {
        return Ok(());
    }

    // get amount to drop
    let amount: u64 = {
        let mut rng = thread_rng();

        // between 1-10% of total catcoin, min 1 max 15
        let percent: f32 = rng.gen_range(1..=10) as f32 / 100.0;
        (has.catcoin as f32 * percent).clamp(1., 15.).abs().round() as u64
    };

    // spend from wallet
    let did_spend = spend_catcoin(pool, msg.author.id, amount).await?;
    if !did_spend {
        return Err("try_spend_catcoin on pre-checked value failed!".into());
    }

    // post in chat
    let embed = CreateEmbed::new()
        .color(serenity::Color::from_rgb(random(), random(), random()))
        .title(random_trip_msg(
            msg.author.global_name.as_ref().unwrap_or(&msg.author.name),
            amount,
            &emoji(),
        ));
    let button = CreateActionRow::Buttons(vec![CreateButton::new(format!("{uuid}-spilled-coin"))
        .label(format!("{amount}"))
        .emoji(
            emoji()
                .parse::<ReactionType>()
                .expect("Could not parse catcoin emoji as ReactionType"),
        )]);

    let mut rh = msg
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new().embed(embed).components(vec![button]),
        )
        .await?;

    // wait for first interaction
    while let Some(mci) = ComponentInteractionCollector::new(ctx)
        .channel_id(msg.channel_id)
        .timeout(Duration::from_secs(120))
        .filter(move |mci| mci.data.custom_id.starts_with(&uuid.to_string()))
        .await
    {
        mci.create_response(ctx, serenity::CreateInteractionResponse::Acknowledge)
            .await?;
        let response = if mci.user.id == msg.author.id {
            format!(
                "{} picked their own **{}** {} back up. ^-^",
                mci.user.mention(),
                amount,
                emoji()
            )
        } else {
            format!(
                "{} stole {}'s **{}** {} off the ground! >//<",
                mci.user.mention(),
                msg.author.mention(),
                amount,
                emoji()
            )
        };
        rh.edit(
            ctx,
            EditMessage::default()
                .content(response)
                .components(vec![])
                .embeds(vec![]),
        )
        .await?;
        grant_catcoin(pool, mci.user.id, amount).await?;
        return Ok(());
    }

    rh.delete(ctx).await?;

    Ok(())
}
