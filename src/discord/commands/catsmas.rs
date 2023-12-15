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

/// fetch info on who ur secret santa is
#[poise::command(slash_command)]
async fn who(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let uid = ctx.author().id;
    // fetch users
    let user_ids: Vec<serenity::UserId> = sqlx::query!("SELECT * FROM `catsmas_users`")
        .fetch_all(&ctx.data().pool)
        .await?
        .into_iter()
        .map(|u| serenity::UserId(u.user_id.parse().unwrap()))
        .collect();
    let has_self = user_ids.contains(&uid);

    let users = {
        let (i, mut o) = tokio::sync::mpsc::channel(100);
        for uid in user_ids {
            let ctx = ctx.serenity_context().clone();
            let i = i.clone();
            tokio::spawn(async move {
                if let Ok(user) = uid.to_user(&ctx).await {
                    let _ = i.send(user).await;
                }
            });
        }
        drop(i);
        let mut users = Vec::new();
        while let Some(user) = o.recv().await {
            users.push(user);
        }
        users
    };

    ctx.send(|m| {
        m.embed(|e| {
            e.title(if has_self {
                "ur enwolled!! prepare to eventually get a gift for someone <3"
            } else {
                "ur nyot in"
            })
            .description(format!(
                "{} entered:\n```{}```",
                users.len(),
                if users.is_empty() {
                    "nyo one".to_owned()
                } else {
                    users
                        .iter()
                        .map(|u| format!("{}#{}", u.name, u.discriminator))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            ))
            .footer(|f| f.text("/catsmas join  |  /catsmas leave"))
        })
    })
    .await?;
    Ok(())
}
