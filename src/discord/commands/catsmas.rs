use poise;
use poise::serenity_prelude as serenity;

use crate::discord::Context;
use crate::Error;

/// secret santa stuff
#[poise::command(slash_command, subcommands("who", "ready"), subcommand_required)]
pub async fn catsmas(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// toggle ready (do you have your gift?)
#[poise::command(slash_command)]
async fn ready(ctx: Context<'_>) -> Result<(), Error> {
    let serenity::UserId(uid) = ctx.author().id;

    sqlx::query!(
        r#"UPDATE `catsmas_users` SET ready=!ready WHERE user_id=?"#,
        uid.to_string()
    )
    .execute(&ctx.data().pool)
    .await?;
    let status = sqlx::query!(
        r#"SELECT ready FROM `catsmas_users` WHERE user_id=?"#,
        uid.to_string()
    )
    .fetch_one(&ctx.data().pool)
    .await?;

    ctx.send(|m| {
        m.content(format!(
            "u are set as {}ready",
            if status.ready != 0 { "" } else { "not " }
        ))
    })
    .await?;

    Ok(())
}

/// get your secret santa
#[poise::command(slash_command, ephemeral)]
async fn who(ctx: Context<'_>) -> Result<(), Error> {
    let serenity::UserId(uid) = ctx.author().id;
    let pairing = sqlx::query!(
        r#"
		SELECT * FROM `catsmas_users`
		WHERE `user_id` = ?
	"#,
        uid.to_string()
    )
    .fetch_optional(&ctx.data().pool)
    .await?;
    let Some(pairing) = pairing else {
        ctx.send(|m| m.content("ur not in catsmas... >_<")).await?;
        return Ok(());
    };
    let Some(partner) = pairing.partner else {
        ctx.send(|m| m.content("catsmas pairings not generated yet... >_<"))
            .await?;
        return Ok(());
    };
    ctx.send(|m| m.embed(|e| e.title("click here for ur secret santa...").url(partner)))
        .await?;
    Ok(())
}
