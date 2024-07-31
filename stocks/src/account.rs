use poise::serenity_prelude as serenity;
use sqlx::{Executor, MySql};

use common::Error;

use super::company::Company;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Account {
    pub uid: String,
    pub company_id: i32,
    pub shares_owned: i32,
}

impl Account {
    /// Give a user some shares in their account
    pub async fn grant_shares(
        pool: impl Executor<'_, Database = MySql>,
        to: serenity::UserId,
        from: &Company,
        amount: i32,
    ) -> Result<(), Error> {
        sqlx::query!("INSERT INTO `catcoin_user_shares` (`uid`, `company_id`, `shares_owned`) VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE `shares_owned` = `shares_owned` + ?", to.to_string(), from.id, amount, amount)
			.execute(pool)
			.await?;
        Ok(())
    }
}
