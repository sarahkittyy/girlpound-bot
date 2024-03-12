use poise::serenity_prelude::{self as serenity, Reaction};

use super::PoiseData;
use crate::Error;

use emojito;

/// on adding a reaction
pub async fn add(
    _ctx: &serenity::Context,
    data: &PoiseData,
    reaction: &Reaction,
) -> Result<(), Error> {
    let (eid, name, is_discord, animated) = match &reaction.emoji {
        serenity::ReactionType::Custom { id, name, animated } => (
            id.get().to_string(),
            name.to_owned().unwrap_or("Not Found".to_owned()),
            true,
            *animated,
        ),
        serenity::ReactionType::Unicode(ue) => {
            let emojis: Vec<&'static emojito::Emoji> = emojito::find_emoji(ue);
            if let Some(emoji) = emojis.first() {
                (
                    emoji.codepoint.to_owned(),
                    emoji.name.to_owned(),
                    false,
                    false,
                )
            } else {
                return Ok(());
            }
        }
        _ => return Ok(()),
    };
    data.emoji_rank
        .write()
        .await
        .add_react(eid, name, is_discord, animated);
    Ok(())
}

/// on removing a reaction
pub async fn rm(
    _ctx: &serenity::Context,
    data: &PoiseData,
    reaction: &Reaction,
) -> Result<(), Error> {
    let (eid, name, is_discord, animated) = match &reaction.emoji {
        serenity::ReactionType::Custom { id, name, animated } => (
            id.get().to_string(),
            name.to_owned().unwrap_or("Not Found".to_owned()),
            true,
            *animated,
        ),
        serenity::ReactionType::Unicode(ue) => {
            let emojis: Vec<&'static emojito::Emoji> = emojito::find_emoji(ue);
            if let Some(emoji) = emojis.first() {
                (
                    emoji.codepoint.to_owned(),
                    emoji.name.to_owned(),
                    false,
                    false,
                )
            } else {
                return Ok(());
            }
        }
        _ => return Ok(()),
    };
    data.emoji_rank
        .write()
        .await
        .rm_react(eid, name, is_discord, animated);
    Ok(())
}
