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
    ctx.defer_ephemeral().await?;
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
