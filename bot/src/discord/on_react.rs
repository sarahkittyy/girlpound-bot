use poise::serenity_prelude::{self as serenity, CreateMessage, Message, Reaction};
use regex::Regex;
use tf2::{rcon_user_output, Server};

use super::PoiseData;
use common::Error;

use emojito;

/// on adding a reaction
pub async fn add(
    ctx: &serenity::Context,
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
    let _ = ban_react(ctx, data, reaction)
        .await
        .inspect_err(|e| log::error!("Failed to check for ban reaction: {e:?}"));
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

pub async fn ban_react(
    ctx: &serenity::Context,
    data: &PoiseData,
    reaction: &Reaction,
) -> Result<(), Error> {
    // check if reaction in relay
    let in_relay: bool = data
        .servers
        .iter()
        .any(|(_addr, server)| server.log_channel.is_some_and(|c| c == reaction.channel_id));
    let is_hammer: bool = reaction.emoji.unicode_eq("🔨");
    let msg: Message = reaction.message(ctx).await?;
    let msg_content: String = msg.content;
    let uid_regex = Regex::new(r"(\[U:\d+:\d+\])")?;
    let uid_caps = uid_regex.captures(&msg_content);
    let uid: Option<&str> = uid_caps.and_then(|c| c.get(0).map(|m| m.as_str()));
    let is_join_leave: bool = msg_content.starts_with('+') || msg_content.starts_with("\\-");
    let is_admin = reaction
        .user(ctx)
        .await?
        .has_role(ctx, data.guild_id, data.mod_role)
        .await?;

    if in_relay && is_hammer && is_join_leave && is_admin && uid.is_some() {
        let uid = uid.unwrap();
        let result = tf2::banid(
            &data.steamid_client,
            uid,
            &[data
                .servers
                .values()
                .next()
                .ok_or::<Error>("no servers".into())?],
            0,
            "1984",
        )
        .await;
        log::info!(
            "hammer ban by {} on {} result: {result}",
            reaction.user(ctx).await?,
            &msg_content
        );
        reaction
            .channel_id
            .send_message(ctx, CreateMessage::new().content(result))
            .await?;
    }

    return Ok(());
}
