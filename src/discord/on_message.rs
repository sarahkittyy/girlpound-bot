use chrono::Utc;
use poise::serenity_prelude as serenity;
use regex::Regex;
use serenity::{CreateMessage, Message};

use crate::Error;

use emojito;
use rand::prelude::*;
use tokio::sync::mpsc::Sender;

use super::{media_cooldown::CooldownMessage, PoiseData};

const KATELYN_UID: u64 = 712534342445826078;
pub async fn hi_cat(
    ctx: &serenity::Context,
    data: &PoiseData,
    new_message: &Message,
) -> Result<(), Error> {
    if new_message.author.id.get() == KATELYN_UID
        && new_message.content.to_lowercase().contains("hi cat")
    {
        let hicats = sqlx::query!("SELECT count FROM `hicat` WHERE uid=?", KATELYN_UID)
            .fetch_optional(&data.local_pool)
            .await?;
        let mut count = hicats.map(|c| c.count).unwrap_or(0);
        count += 1;
        sqlx::query!(
            r#"
			INSERT INTO `hicat` (`uid`, `count`)
			VALUES (?, ?)
			ON DUPLICATE KEY UPDATE `count` = ?
			"#,
            KATELYN_UID,
            count,
            count
        )
        .execute(&data.local_pool)
        .await?;

        new_message
            .reply(&ctx, format!("\"hi cat\" counter: {}", count))
            .await?;
    }
    Ok(())
}

/// track emojis in messages
pub async fn watch_emojis(
    _ctx: &serenity::Context,
    data: &PoiseData,
    new_message: &Message,
) -> Result<(), Error> {
    if new_message.author.bot {
        return Ok(());
    }
    // find all unicode emoji
    let uemojis: Vec<&'static emojito::Emoji> = emojito::find_emoji(&new_message.content);
    //			name 		eid 		is_discord		animated
    let mut rows: Vec<(String, String, bool, bool)> = vec![];
    for uemoji in &uemojis {
        let name = uemoji.name;
        let id = uemoji.codepoint;
        rows.push((name.to_owned(), id.to_owned(), false, false));
    }

    // find all discord emoji
    let emoji = Regex::new(r#"<(a?):([A-Za-z0-9_-]+):(\d+)>"#).unwrap();
    for emoji_caps in emoji.captures_iter(&new_message.content) {
        let anim = emoji_caps.get(1).unwrap();
        let name = emoji_caps.get(2).unwrap();
        let id = emoji_caps.get(3).unwrap();
        rows.push((
            name.as_str().to_owned(),
            id.as_str().to_owned(),
            true,
            anim.as_str() == "a",
        ));
    }

    rows.sort_unstable_by_key(|r| r.1.clone());
    rows.dedup_by_key(|r| r.1.clone());

    let mut der = data.emoji_rank.write().await;
    for (name, eid, is_discord, animated) in rows.into_iter() {
        der.add_usage(eid, name, is_discord, animated);
    }
    Ok(())
}

/// send silly reminders in the trial mod channel
pub async fn trial_mod_reminders(
    ctx: &serenity::Context,
    data: &PoiseData,
    new_message: &Message,
) -> Result<(), Error> {
    const HELPFUL_REMINDERS: [&str; 2] = [
        "keep up the good work :white_check_mark:",
        "Please be respectful to all players on the server :thumbs_up:",
    ];

    if new_message.channel_id == data.trial_mod_channel {
        let r: f32 = random();
        if r < 0.1 {
            let g = (random::<f32>() * HELPFUL_REMINDERS.len() as f32).floor() as usize;
            new_message
                .channel_id
                .send_message(ctx, CreateMessage::new().content(HELPFUL_REMINDERS[g]))
                .await?;
        }
    }

    Ok(())
}

/// check the rate limit & send cooldown messages
pub async fn handle_cooldowns(
    ctx: &serenity::Context,
    data: &PoiseData,
    cooldown_handler: &Sender<CooldownMessage>,
    new_message: &Message,
) -> Result<(), Error> {
    if let Some(_) = new_message.guild_id {
        // media channel spam limit
        let mut media_cooldown = data.media_cooldown.write().await;
        // if we have to wait before posting an image...
        if let Err(time_left) = media_cooldown.try_allow_one(new_message) {
            // delete the image
            new_message.delete(ctx).await?;
            // send da cooldown msg
            let _ = cooldown_handler
                .send(CooldownMessage {
                    channel: new_message.channel_id,
                    user: new_message.author.id,
                    delete_at: Utc::now() + time_left,
                })
                .await;
        }
    }
    Ok(())
}
