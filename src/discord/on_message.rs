use chrono::Utc;
use poise::serenity_prelude as serenity;
use serenity::Message;

use crate::Error;

use rand::prelude::*;
use tokio::sync::mpsc::Sender;

use super::{Cooldown, PoiseData};

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
                .send_message(ctx, |m| m.content(HELPFUL_REMINDERS[g]))
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_cooldowns(
    ctx: &serenity::Context,
    data: &PoiseData,
    cooldown_handler: &Sender<Cooldown>,
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
                .send(Cooldown {
                    channel: new_message.channel_id,
                    user: new_message.author.id,
                    delete_at: Utc::now() + time_left,
                })
                .await;
        }
    }
    Ok(())
}
