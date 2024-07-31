use chrono::NaiveDateTime;
use sqlx::{MySql, Pool, QueryBuilder};

use common::Error;

use super::{company::Company, market_time};

/// A price history entry
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PriceHistory {
    pub id: i32,
    pub company_id: i32,
    pub price: i32,
    pub timestamp: NaiveDateTime,
}

impl PriceHistory {
    /// all companies' price histories for the last 30 days.
    pub async fn fetch_all_last_month(pool: &Pool<MySql>) -> Result<Vec<PriceHistory>, Error> {
        let ph: Vec<PriceHistory> = sqlx::query_as!(
			PriceHistory,
			"SELECT * FROM `catcoin_price_history` WHERE `timestamp` > CURRENT_DATE() - INTERVAL 1 MONTH",
		)
        .fetch_all(pool)
        .await?;
        Ok(ph)
    }

    /// Flush the current price of all given companies as today's price history
    pub async fn flush_companies(
        pool: &Pool<MySql>,
        companies: &Vec<Company>,
    ) -> Result<(), Error> {
        // for all companies, get their price, and push to the history table
        let now = market_time().read().await;
        let mut qb = QueryBuilder::new(
            "INSERT INTO `catcoin_price_history` (`company_id`, `price`, `timestamp`)",
        );
        {
            qb.push_values(companies.iter(), |mut b, company| {
                b.push_bind(company.id)
                    .push_bind(company.price)
                    .push_bind(*now);
            });
        }
        qb.build().execute(pool).await?;
        Ok(())
    }
}
