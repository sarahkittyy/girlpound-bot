use rand::prelude::*;
use sqlx::{Executor, MySql, Pool};

use common::Error;

/// A company row to invest in
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Company {
    pub id: i32,
    pub name: String,
    pub tag: String,
    pub total_shares: i32,
    pub logo: String,
    pub price: i32,
}

impl Company {
    pub async fn fetch_all(pool: &Pool<MySql>) -> Result<Vec<Company>, Error> {
        let res: Vec<Company> = sqlx::query_as!(Company, "SELECT * FROM `catcoin_company`")
            .fetch_all(pool)
            .await?;
        Ok(res)
    }

    /// fetch a company by it's id
    pub async fn fetch_by_id(pool: &Pool<MySql>, id: i32) -> Result<Option<Company>, Error> {
        let res: Option<Company> = sqlx::query_as!(
            Company,
            "SELECT * FROM `catcoin_company` WHERE `id` = ?",
            id
        )
        .fetch_optional(pool)
        .await?;
        Ok(res)
    }

    /// Get a stock price with a random deviation
    pub fn randomly_stepped_price(&self) -> i32 {
        let trend = 0.01;
        let amnt =
            rand_distr::Normal::new(self.price as f32 + trend, self.total_shares as f32 * 0.04)
                .unwrap();
        (amnt.sample(&mut thread_rng()).round() as i32).max(0)
    }

    /// Remove some shares from the amount available
    pub async fn try_remove_shares(
        &self,
        pool: impl Executor<'_, Database = MySql>,
        amount: i32,
    ) -> Result<Company, Error> {
        if amount > self.total_shares {
            return Err("Not enough shares available".into());
        }
        sqlx::query!(
            "UPDATE `catcoin_company` SET `total_shares` = ? WHERE `id` = ?",
            self.total_shares - amount,
            self.id
        )
        .execute(pool)
        .await?;
        Ok(Company {
            total_shares: self.total_shares - amount,
            ..self.clone()
        })
    }

    /// Step the price of this company
    pub async fn step_price(&self, pool: &Pool<MySql>) -> Result<Company, Error> {
        let new_price = self.randomly_stepped_price();
        sqlx::query!(
            "UPDATE `catcoin_company` SET `price` = ? WHERE `id` = ?",
            new_price,
            self.id
        )
        .execute(pool)
        .await?;

        Ok(Company {
            price: new_price,
            ..self.clone()
        })
    }
}
