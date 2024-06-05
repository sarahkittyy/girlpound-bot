use std::{collections::HashMap, fmt::Display};

use poise::serenity_prelude::{
    Context, CreateAttachment, CreateEmbed, CreateEmbedFooter, CreateMessage, Message, UserId,
};

use crate::{discord::PoiseData, util::LeakyBucket, Error};
use rand::prelude::*;
use rand_distr::Normal;

use super::{grant_catcoin, increment_and_get_pulls};

/// Reward rarities
#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
pub enum Rarity {
    Common,
    Rare,
    Fluffy,
    Peak,
}

impl From<String> for Rarity {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Common" => Rarity::Common,
            "Rare" => Rarity::Rare,
            "Fluffy" => Rarity::Fluffy,
            "Peak" => Rarity::Peak,
            _ => unreachable!(),
        }
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

    pub fn reward_dist(&self) -> Normal<f32> {
        match self {
            Rarity::Common => Normal::new(2.0, 0.5).unwrap(),
            Rarity::Rare => Normal::new(10.0, 2.0).unwrap(),
            Rarity::Fluffy => Normal::new(50.0, 5.0).unwrap(),
            Rarity::Peak => Normal::new(150.0, 10.0).unwrap(),
        }
    }
}

#[derive(Clone, sqlx::FromRow)]
pub struct Reward {
    pub id: i32,
    pub name: String,
    pub file: String,
    pub rarity: Rarity,
}

pub struct SpamFilter {
    buckets: HashMap<UserId, LeakyBucket>,
}

impl SpamFilter {
    pub fn new() -> Self {
        Self {
            buckets: HashMap::new(),
        }
    }

    /// Checks if the user's message should roll
    pub fn try_roll(&mut self, uid: UserId) -> bool {
        let bucket: &mut LeakyBucket = self
            .buckets
            .entry(uid)
            .or_insert_with(|| LeakyBucket::new(80.0, 20.0, 1.0));
        bucket.try_afford_one().is_ok()
    }
}

/// Rarely drops goodies
pub async fn on_message(ctx: &Context, data: &PoiseData, message: &Message) -> Result<(), Error> {
    if message.author.bot {
        return Ok(());
    };
    if !data
        .catcoin_spam_filter
        .write()
        .await
        .try_roll(message.author.id)
    {
        return Ok(());
    }

    let (rarity, catcoins, reward) = {
        let mut rng = thread_rng();

        // chance to pull
        if !rng.gen_ratio(1, 500) {
            return Ok(());
        }

        let rarity = Rarity::pick(&mut rng);
        let reward = data
            .catcoin_drops
            .iter()
            .filter(|reward| reward.rarity == rarity)
            .choose(&mut rng)
            .ok_or("No rewards available")?;

        let catcoins = rarity.reward_dist().sample(&mut rng).round() as i64;
        (rarity, catcoins, reward)
    };

    println!("Got pull: {} {}", rarity, &reward.name);

    let pulls = increment_and_get_pulls(&data.local_pool, reward.id).await?;
    grant_catcoin(&data.local_pool, message.author.id, catcoins).await?;

    let attachment = CreateAttachment::path(&reward.file).await?;
    let embed = CreateEmbed::new()
        .title(format!(
            ":bangbang: {} Pull: {} #{} :sparkles:",
            rarity, reward.name, pulls
        ))
        .description(format!("{} **+{}**", data.catcoin_emoji, catcoins))
        .footer(CreateEmbedFooter::new("/catcoin balance"))
        .attachment(&attachment.filename);

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
