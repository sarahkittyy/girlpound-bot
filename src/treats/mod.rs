pub mod command;

use crate::{discord::Context, Error};
use poise::serenity_prelude as serenity;

use sqlx;

/// get a user's treat count.
pub async fn get_treats(ctx: Context<'_>, uid: serenity::UserId) -> Result<i64, Error> {
    let record = sqlx::query!(r#"SELECT * FROM `treats` WHERE uid=?"#, uid.get())
        .fetch_optional(&ctx.data().local_pool)
        .await?;

    Ok(record.map(|r| r.treats).unwrap_or(0))
}
