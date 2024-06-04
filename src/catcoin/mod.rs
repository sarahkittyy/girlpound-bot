pub mod command;
pub mod random_drops;

use crate::Error;
use poise::serenity_prelude as serenity;

use random_drops::Reward;
use sqlx::{self, MySql, Pool};

/// get a user's catcoin count.
pub async fn get_catcoin(pool: &Pool<MySql>, uid: serenity::UserId) -> Result<i64, Error> {
    let record = sqlx::query!(r#"SELECT * FROM `catcoin` WHERE uid=?"#, uid.get())
        .fetch_optional(pool)
        .await?;

    Ok(record.map(|r| r.catcoin).unwrap_or(0))
}

/// Grab all catcoin rewards
pub async fn get_drops(pool: &Pool<MySql>) -> Result<Vec<Reward>, Error> {
    let rewards: Vec<Reward> = sqlx::query_as!(Reward, "SELECT * FROM `catcoin_reward`")
        .fetch_all(pool)
        .await?;
    Ok(rewards)
}

pub async fn increment_and_get_pulls(pool: &Pool<MySql>, reward_id: i32) -> Result<i32, Error> {
    sqlx::query!(
        r#"
		INSERT INTO `catcoin_reward_count` (`rid`, `pulls`)
		VALUES (?, ?)
		ON DUPLICATE KEY UPDATE `pulls` = `pulls` + 1
	"#,
        reward_id,
        1
    )
    .execute(pool)
    .await?;
    let pulls = sqlx::query!(
        r#"SELECT `pulls` from `catcoin_reward_count` WHERE `rid` = ?"#,
        reward_id
    )
    .fetch_one(pool)
    .await?;
    Ok(pulls.pulls)
}

/// grant a user catcoin
pub async fn grant_catcoin(
    pool: &Pool<MySql>,
    uid: serenity::UserId,
    catcoin: i64,
) -> Result<(), Error> {
    sqlx::query!(r#"INSERT INTO `catcoin` (`uid`, `catcoin`) VALUES (?, ?) ON DUPLICATE KEY UPDATE `catcoin` = `catcoin` + ?"#, uid.get(), catcoin, catcoin)
        .execute(pool)
        .await?;

    Ok(())
}
