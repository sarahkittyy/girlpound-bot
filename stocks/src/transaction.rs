use chrono::NaiveDateTime;
use poise::serenity_prelude as serenity;

use sqlx::{Executor, MySql, Pool};

use common::Error;

use super::{account::Account, company::Company};

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Transaction {
    pub id: i32,
    pub buyer_id: String,
    pub seller_id: Option<String>,
    pub company_id: i32,
    pub shares_bought: i32,
    pub price_per_share: i32,
    pub created_at: NaiveDateTime,
}

impl Transaction {
    /// Attempts to buy shares from the company and deposit them in the user's account
    pub async fn buy_shares_from_company(
        pool: &Pool<MySql>,
        uid: serenity::UserId,
        amount: i32,
        company: &Company,
    ) -> Result<(), Error> {
        let catcoin_cost = amount * company.price;

        // check if company has enough shares
        if company.total_shares < amount {
            return Err("The company does not have enough shares available.".into());
        }

        // begin sql transaction
        let mut tx = pool.begin().await?;

        match (async {
            // try remove catcoin from user's account
            print!("Deducting catcoin... ");
            if !catcoin::spend_catcoin(&mut *tx, uid, catcoin_cost as u64).await? {
                return Err(Into::<Error>::into(
                    "You do not have enough catcoin to buy this!",
                ));
            }
            // remove shares from company
            print!("Deducting shares... ");
            company.try_remove_shares(&mut *tx, amount).await?;
            // create transaction
            print!("Creating Transaction... ");
            Transaction::commit_new(&mut *tx, uid, None, &company, amount, company.price).await?;
            // add to share account
            print!("Granting shares... ");
            Account::grant_shares(&mut *tx, uid, &company, amount).await?;

            log::info!("Done!");
            Ok(())
        })
        .await
        {
            Err(e) => {
                tx.rollback().await?;
                return Err(format!("Failed to complete transaction: {e}").into());
            }
            _ => (),
        };
        tx.commit().await?;

        Ok(())
    }

    async fn commit_new(
        pool: impl Executor<'_, Database = MySql>,
        buyer_id: serenity::UserId,
        seller_id: Option<serenity::UserId>,
        company: &Company,
        shares_bought: i32,
        price_per_share: i32,
    ) -> Result<(), Error> {
        sqlx::query!("INSERT INTO `catcoin_share_transactions` (`buyer_id`, `seller_id`, `company_id`, `shares_bought`, `price_per_share`) VALUES (?, ?, ?, ?, ?)", buyer_id.to_string(), seller_id.map(|s| s.to_string()), company.id, shares_bought, price_per_share).execute(pool).await?;

        Ok(())
    }
}
