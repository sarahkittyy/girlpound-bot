use poise::{serenity_prelude::CreateEmbed, CreateReply};
use sqlx;

use crate::{discord::Context, Error};

/// Returns the most used emoji in the server.
#[poise::command(slash_command, subcommands("text", "react"), subcommand_required)]
pub async fn emojitop(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

fn codepoint_to_emoji(code: String) -> String {
    let codes = code.split(" ");
    let hex = codes
        .into_iter()
        .map(|s| u32::from_str_radix(s, 16).unwrap());
    hex.map(char::from_u32)
        .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect::<String>()
}

/// Top used reactions
#[poise::command(slash_command, user_cooldown = 10)]
async fn react(ctx: Context<'_>) -> Result<(), Error> {
    let rows = sqlx::query!("SELECT * FROM `emojirank` ORDER BY `react_count` DESC LIMIT 10")
        .fetch_all(&ctx.data().local_pool)
        .await?;

    let desc: String = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            format!(
                "{}. {} - `{}`",
                i + 1,
                if row.is_discord == 1 {
                    format!(
                        "<{}:{}:{}>",
                        if row.animated == 1 { "a" } else { "" },
                        row.name,
                        row.eid
                    )
                } else {
                    codepoint_to_emoji(row.eid)
                },
                row.react_count
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut embed = CreateEmbed::new();
    embed = embed.title("Top emojis :3");
    embed = embed.description(desc);

    ctx.send(CreateReply::default().embed(embed).ephemeral(true))
        .await?;

    Ok(())
}

/// Top used emojis in messages
#[poise::command(slash_command, user_cooldown = 10)]
async fn text(ctx: Context<'_>) -> Result<(), Error> {
    let rows = sqlx::query!("SELECT * FROM `emojirank` ORDER BY `use_count` DESC LIMIT 10")
        .fetch_all(&ctx.data().local_pool)
        .await?;

    let desc: String = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            format!(
                "{}. {} - `{}`",
                i + 1,
                if row.is_discord == 1 {
                    format!(
                        "<{}:{}:{}>",
                        if row.animated == 1 { "a" } else { "" },
                        row.name,
                        row.eid
                    )
                } else {
                    codepoint_to_emoji(row.eid)
                },
                row.use_count
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut embed = CreateEmbed::new();
    embed = embed.title("Top emojis :3");
    embed = embed.description(desc);

    ctx.send(CreateReply::default().embed(embed).ephemeral(true))
        .await?;

    Ok(())
}
