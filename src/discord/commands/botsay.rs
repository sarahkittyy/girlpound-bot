use poise::serenity_prelude as serenity;
use poise::{self, CreateReply};
use serenity::{CreateAttachment, CreateMessage};

use super::Context;
use crate::Error;

/// says something on behalf of the bot
#[poise::command(slash_command)]
pub async fn botsay(
    ctx: Context<'_>,
    #[description = "text content"] content: Option<String>,
    #[description = "attachments"] attachment: Option<serenity::Attachment>,
) -> Result<(), Error> {
    let cid = ctx.channel_id();
    let mut message = CreateMessage::new();

    // content
    if let Some(content) = content {
        message = message.content(content);
    }

    // attachments
    if let Some(att) = attachment {
        let att = CreateAttachment::url(&ctx, &att.url).await?;
        message = message.files(vec![att]);
    }

    cid.send_message(&ctx, message).await?;

    ctx.send(
        CreateReply::default()
            .content("Sent :white_check_mark:")
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
