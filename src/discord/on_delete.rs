use poise::serenity_prelude as serenity;

use super::PoiseData;
use crate::Error;

/// save deleted messages in a secret channel :3
pub async fn save_deleted(
    ctx: &serenity::Context,
    data: &PoiseData,
    channel_id: &serenity::ChannelId,
    deleted_message_id: &serenity::MessageId,
) -> Result<(), Error> {
    let Some(message) = ctx.cache.message(channel_id, deleted_message_id) else {
        return Err("Message not found in cache")?;
    };
    let Some(channel) = channel_id.to_channel(ctx).await?.guild() else {
        return Err("Channel not found.")?;
    };
    let _ = data
        .deleted_message_log_channel
        .send_message(&ctx, |m| {
            m.embed(|e| {
                e.title("Deleted Message");
                e.field("Author", message.author.tag(), true);
                e.field("Channel", channel.name(), true);
                e.field("Content", message.content, false);
                e
            });
            m
        })
        .await;
    Ok(())
}
