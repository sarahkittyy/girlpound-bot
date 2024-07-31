use poise::serenity_prelude as serenity;
use serenity::{CreateEmbed, CreateMessage};

use super::PoiseData;
use common::Error;

/// save deleted messages in a secret channel :3
pub async fn save_deleted(
    ctx: &serenity::Context,
    data: &PoiseData,
    channel_id: &serenity::ChannelId,
    deleted_message_id: &serenity::MessageId,
) -> Result<(), Error> {
    let Some(message) = ctx
        .cache
        .message(channel_id, deleted_message_id)
        .map(|m| m.clone())
    else {
        return Err("Message not found in cache")?;
    };
    let Some(channel) = channel_id.to_channel(ctx).await?.guild() else {
        return Err("Channel not found.")?;
    };
    let _ = data
        .deleted_message_log_channel
        .send_message(
            &ctx,
            CreateMessage::new().embed(
                CreateEmbed::new()
                    .title("Deleted Message")
                    .field("Author", message.author.tag(), true)
                    .field("Channel", channel.name(), true)
                    .field("Content", message.content.clone(), false),
            ),
        )
        .await;
    Ok(())
}
