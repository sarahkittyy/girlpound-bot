use poise;
use poise::serenity_prelude as serenity;

use crate::discord::Context;
use crate::Error;

/// secret santa stuff
#[poise::command(
    slash_command,
    subcommands("join", "leave", "who"),
    subcommand_required
)]
pub async fn catsmas(_: Context<'_>) -> Result<(), Error> {
    Ok(()) // never runs
}

/// join the secret santa
#[poise::command(slash_command, ephemeral)]
async fn join(ctx: Context<'_>) -> Result<(), Error> {
    let serenity::UserId(uid) = ctx.author().id;
    sqlx::query!(
        r#"
		INSERT IGNORE INTO `catsmas_users`
		VALUES (?)
	"#,
        uid.to_string()
    )
    .execute(&ctx.data().pool)
    .await?;
    ctx.send(|m| {
        m.embed(|e| {
            e.color(serenity::Color::DARK_GREEN)
                .title("*hacker voice* ur in...")
                .footer(|f| f.text("/catsmas who for updates <3"))
        })
    })
    .await?;

    Ok(())
}

/// leave the secret santa
#[poise::command(slash_command, ephemeral)]
async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    let serenity::UserId(uid) = ctx.author().id;
    sqlx::query!(
        r#"
		DELETE FROM `catsmas_users`
		WHERE `user_id` = ? 
	"#,
        uid.to_string()
    )
    .execute(&ctx.data().pool)
    .await?;
    ctx.send(|m| {
        m.embed(|e| {
            e.color(serenity::Color::DARK_GREEN)
                .title("nyo more catsmas for u...")
                .footer(|f| f.text("/catsmas who for updates <3"))
        })
    })
    .await?;

    Ok(())
}

/// fetch info on who ur secret santa is
#[poise::command(slash_command)]
async fn who(ctx: Context<'_>) -> Result<(), Error> {
    let serenity::UserId(uid) = ctx.author().id;
    // fetch how many entries there are
    let ct = sqlx::query!(
        r#"
		SELECT COUNT(user_id) AS `ct` FROM `catsmas_users`
		"#
    )
    .fetch_one(&ctx.data().pool)
    .await?;
    // check self status
    let me = sqlx::query!(
        r#"
		SELECT *
		FROM `catsmas_users`
		WHERE `user_id` = ?
		"#,
        uid.to_string()
    )
    .fetch_all(&ctx.data().pool)
    .await?;
    ctx.send(|m| {
        m.embed(|e| {
            e.title(if !me.is_empty() {
                "ur enwolled!! prepare to eventually get a gift for someone <3"
            } else {
                "ur nyot in"
            })
            .description(format!("thewe r {} users ready 2 gift...", ct.ct))
            .footer(|f| f.text("/catsmas join  |  /catsmas leave"))
        })
    })
    .await?;
    Ok(())
}
