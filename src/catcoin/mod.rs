pub mod command;
pub mod drops;
pub mod random_pulls;

use std::collections::HashMap;

use crate::{util::LeakyBucket, Error};
use poise::serenity_prelude::{self as serenity, UserId};

use random_pulls::Reward;
use sqlx::{self, MySql, Pool};

/// For preventing message spam
pub struct SpamFilter {
    buckets: HashMap<UserId, LeakyBucket>,
}

impl SpamFilter {
    pub fn new() -> Self {
        Self {
            buckets: HashMap::new(),
        }
    }

    /// Checks if the user's message should roll
    pub fn try_roll(&mut self, uid: UserId) -> bool {
        let bucket: &mut LeakyBucket = self
            .buckets
            .entry(uid)
            .or_insert_with(|| LeakyBucket::new(80.0, 20.0, 1.0));
        bucket.try_afford_one().is_ok()
    }
}

#[derive(Clone, sqlx::FromRow)]
pub struct CatcoinWallet {
    pub uid: String,
    pub catcoin: i64,
}

/// get a user's catcoin count.
pub async fn get_catcoin(
    pool: &Pool<MySql>,
    uid: serenity::UserId,
) -> Result<CatcoinWallet, Error> {
    let record = sqlx::query_as!(
        CatcoinWallet,
        r#"SELECT * FROM `catcoin` WHERE uid=?"#,
        uid.get()
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.unwrap_or(CatcoinWallet {
        uid: uid.to_string(),
        catcoin: 0,
    }))
}

/// get the top catcoin wallets
pub async fn get_top(pool: &Pool<MySql>) -> Result<Vec<CatcoinWallet>, Error> {
    let record: Vec<CatcoinWallet> = sqlx::query_as!(
        CatcoinWallet,
        r#"SELECT * FROM `catcoin` ORDER BY `catcoin` DESC LIMIT 15"#
    )
    .fetch_all(pool)
    .await?;

    Ok(record)
}

/// Grab all catcoin rewards
pub async fn get_drops(pool: &Pool<MySql>) -> Result<Vec<Reward>, Error> {
    let rewards: Vec<Reward> = sqlx::query_as!(Reward, "SELECT * FROM `catcoin_reward`")
        .fetch_all(pool)
        .await?;
    Ok(rewards)
}

/// Try taking `amount` catcoin from the user's wallet. return false if not enough funds.
pub async fn try_spend_catcoin(
    pool: &Pool<MySql>,
    from: serenity::UserId,
    amount: u64,
) -> Result<bool, Error> {
    let mut tx = pool.begin().await?;
    let rc = sqlx::query!(
        "UPDATE `catcoin` SET `catcoin` = `catcoin` - ? WHERE `uid` = ? AND `catcoin` >= ?",
        amount,
        from.get(),
        amount
    )
    .execute(&mut *tx)
    .await?;
    if rc.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }
    tx.commit().await?;
    Ok(true)
}

/// Give catcoin from one user to another, returns false if you don't have enough.
pub async fn transact(
    pool: &Pool<MySql>,
    from: serenity::UserId,
    to: serenity::UserId,
    amount: u64,
) -> Result<bool, Error> {
    let mut tx = pool.begin().await?;
    let rc = sqlx::query!(
        "UPDATE `catcoin` SET `catcoin` = `catcoin` - ? WHERE `uid` = ? AND `catcoin` >= ?",
        amount,
        from.get(),
        amount
    )
    .execute(&mut *tx)
    .await?;
    if rc.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }
    sqlx::query!(
        "INSERT INTO `catcoin` (`uid`, `catcoin`) VALUES (?, ?) ON DUPLICATE KEY UPDATE `catcoin` = `catcoin` + ?", to.get(), amount, amount)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(true)
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
    catcoin: u64,
) -> Result<(), Error> {
    sqlx::query!(r#"INSERT INTO `catcoin` (`uid`, `catcoin`) VALUES (?, ?) ON DUPLICATE KEY UPDATE `catcoin` = `catcoin` + ?"#, uid.get(), catcoin, catcoin)
        .execute(pool)
        .await?;

    Ok(())
}
