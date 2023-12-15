use poise;
use poise::serenity_prelude as serenity;

use crate::discord::Context;
use crate::Error;

/// secret santa stuff
#[poise::command(slash_command, ephemeral)]
pub async fn catsmas(ctx: Context<'_>) -> Result<(), Error> {
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
