use poise::{serenity_prelude::CreateEmbed, CreateReply};
use sqlx::{self, MySql, Pool};

use crate::{discord::Context, Error};

/// Returns the most used emoji in the server.
#[poise::command(
    slash_command,
    subcommands("text", "react", "total"),
    subcommand_required
)]
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

#[derive(sqlx::FromRow)]
struct EmojiRankRow {
    is_discord: i8,
    animated: i8,
    name: String,
    eid: String,
    count: i64,
}

async fn get_ranks(order_by: &str, pool: &Pool<MySql>) -> Result<String, Error> {
    let rows: Vec<EmojiRankRow> = sqlx::query_as(&format!(
        "SELECT *, {} AS count FROM `emojirank` ORDER BY count DESC LIMIT 10",
        order_by
    ))
    .fetch_all(pool)
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
                row.count
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(desc)
}

/// Top used emojis in total
#[poise::command(slash_command, user_cooldown = 10)]
async fn total(ctx: Context<'_>) -> Result<(), Error> {
    let mut embed = CreateEmbed::new();
    embed = embed.title("Top overall emojis :3");
    embed = embed.description(get_ranks("use_count+react_count", &ctx.data().local_pool).await?);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Top used reactions
#[poise::command(slash_command, user_cooldown = 10)]
async fn react(ctx: Context<'_>) -> Result<(), Error> {
    let mut embed = CreateEmbed::new();
    embed = embed.title("Top reaction emojis :3");
    embed = embed.description(get_ranks("react_count", &ctx.data().local_pool).await?);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Top used emojis in messages
#[poise::command(slash_command, user_cooldown = 10)]
async fn text(ctx: Context<'_>) -> Result<(), Error> {
    let mut embed = CreateEmbed::new();
    embed = embed.title("Top message emojis :3");
    embed = embed.description(get_ranks("use_count", &ctx.data().local_pool).await?);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}
