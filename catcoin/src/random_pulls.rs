use std::fmt::Display;

use emoji::emoji;
use poise::serenity_prelude::{
    Color, Context, CreateAttachment, CreateEmbed, CreateEmbedFooter, CreateMessage, Message,
};
use sqlx::{MySql, Pool};

use crate::{drops, inventory::add_to_inventory};
use common::Error;
use rand::prelude::*;
use rand_distr::Normal;

use super::{grant_catcoin, increment_and_get_pulls};

/// Reward rarities
#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Debug)]
pub enum Rarity {
    Common,
    Rare,
    Fluffy,
    Peak,
}

impl From<&str> for Rarity {
    fn from(value: &str) -> Self {
        match value {
            "Common" => Rarity::Common,
            "Rare" => Rarity::Rare,
            "Fluffy" => Rarity::Fluffy,
            "Peak" => Rarity::Peak,
            _ => unreachable!(),
        }
    }
}

impl From<String> for Rarity {
    fn from(value: String) -> Self {
        Rarity::from(value.as_str())
    }
}

impl Display for Rarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Rarity::Common => "Common",
                Rarity::Rare => "Rare",
                Rarity::Fluffy => "Fluffy",
                Rarity::Peak => "Peak",
            }
        )
    }
}

impl Rarity {
    pub fn pick(rng: &mut impl Rng) -> Self {
        let rarity = rng.gen_range(0..=100);
        match rarity {
            0..=68 => Rarity::Common,
            69..=94 => Rarity::Rare,
            95..=99 => Rarity::Fluffy,
            _ => Rarity::Peak,
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Rarity::Common => Color::from_rgb(205, 127, 50),
            Rarity::Rare => Color::from_rgb(192, 192, 192),
            Rarity::Fluffy => Color::from_rgb(255, 215, 0),
            Rarity::Peak => Color::from_rgb(233, 138, 153),
        }
    }

    pub fn reward_dist(&self) -> Normal<f32> {
        match self {
            Rarity::Common => Normal::new(2.0, 0.5).unwrap(),
            Rarity::Rare => Normal::new(10.0, 2.0).unwrap(),
            Rarity::Fluffy => Normal::new(80.0, 5.0).unwrap(),
            Rarity::Peak => Normal::new(300.0, 50.0).unwrap(),
        }
    }
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Reward {
    pub id: i32,
    pub name: String,
    pub file: String,
    pub rarity: Rarity,
}

/// Rarely drops goodies
pub async fn on_message(ctx: &Context, pool: &Pool<MySql>, message: &Message) -> Result<(), Error> {
    let (rarity, catcoins, reward) = {
        let mut rng = thread_rng();

        // chance to pull
        if !rng.gen_ratio(1, 500) {
            return Ok(());
        }

        let rarity = Rarity::pick(&mut rng);
        let reward = drops()
            .iter()
            .filter(|reward| reward.rarity == rarity)
            .choose(&mut rng)
            .ok_or("No rewards available")?;

        let catcoins = rarity.reward_dist().sample(&mut rng).round().abs() as u64;
        (rarity, catcoins, reward)
    };

    log::info!("Got pull: {} {}", rarity, &reward.name);

    let pulls = increment_and_get_pulls(pool, reward.id).await?;
    add_to_inventory(pool, message.author.id, reward.id, pulls, catcoins).await?;
    grant_catcoin(pool, message.author.id, catcoins).await?;

    let attachment = CreateAttachment::path(&reward.file).await?;
    let embed = CreateEmbed::new()
        .title(format!(
            ":bangbang: {} Pull: {} #{} :sparkles:",
            rarity, reward.name, pulls
        ))
        .description(format!("{} **+{}**", emoji("catcoin"), catcoins))
        .footer(CreateEmbedFooter::new("/catcoin balance"))
        .attachment(&attachment.filename)
        .color(reward.rarity.color());

    message
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new()
                .reference_message(message)
                .add_file(attachment)
                .embed(embed),
        )
        .await?;
    Ok(())
}
