use poise::serenity_prelude::{self as serenity, CreateAllowedMentions};
use poise::{self, CreateReply};
use serenity::{CreateAttachment, CreateMessage};

use super::Context;
use crate::logs::safe_strip;
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
        message = message.content(safe_strip(&content));
    }

    // attachments
    if let Some(att) = attachment {
        let att = CreateAttachment::url(&ctx, &att.url).await?;
        message = message.files(vec![att]);
    }

    message = message.allowed_mentions(CreateAllowedMentions::new().empty_roles().empty_users());

    cid.send_message(&ctx, message).await?;

    ctx.send(
        CreateReply::default()
            .content("Sent :white_check_mark:")
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
