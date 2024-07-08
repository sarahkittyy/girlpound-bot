use std::sync::OnceLock;

use chrono::{DateTime, Days, NaiveDateTime, Utc};
use image::codecs::png::PngEncoder;
use poise::serenity_prelude::{self as serenity, CreateAttachment, CreateMessage};
use sqlx::{pool, MySql, Pool, QueryBuilder};
use tokio::sync::{Mutex, RwLock};
use tokio_cron_scheduler::{Job, JobBuilder, JobScheduler};

use crate::{
    discord::{Context, PoiseData},
    Error,
};

use self::plot::draw_stock_trends;

mod plot;

/// A company row to invest in
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Company {
    pub id: i32,
    pub name: String,
    pub tag: String,
    pub total_shares: i32,
    pub price: i32,
}

/// A price history entry
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PriceHistory {
    pub id: i32,
    pub company_id: i32,
    pub price: i32,
    pub timestamp: NaiveDateTime,
}

// fake datetime for debugging
static NOW: OnceLock<RwLock<NaiveDateTime>> = OnceLock::new();
fn market_time() -> &'static RwLock<NaiveDateTime> {
    NOW.get().unwrap()
}

/// Initialize stock market related jobs
pub async fn init(sched: &JobScheduler, pool: &Pool<MySql>) -> Result<(), Error> {
    println!("Initializing Stock Market...");
    let cdate = sqlx::query!("SELECT `datetime` FROM `catcoin_sim_time` WHERE `id` = 1")
        .fetch_one(pool)
        .await?;
    NOW.get_or_init(|| RwLock::new(cdate.datetime));

    Ok(())
}

/// update the market one full day.
async fn step_market(ctx: &serenity::Context, data: &PoiseData) -> Result<(), Error> {
    update_price_history(data).await?;
    post_market_floor(ctx, data).await?;

    let mut time = market_time().write().await;
    println!("Updating market for {}", time);
    *time = time.checked_add_days(Days::new(1)).ok_or("Bad add")?;
    sqlx::query!(
        "UPDATE `catcoin_sim_time` SET `datetime` = ? WHERE `id` = 1",
        *time
    )
    .execute(&data.local_pool)
    .await?;
    println!("It is now {}", time);

    Ok(())
}

/// log the current stock price of all companies to the history
async fn update_price_history(data: &PoiseData) -> Result<(), Error> {
    // for all companies, get their price, and push to the history table
    let companies = get_all_companies(&data.local_pool).await?;
    let now = market_time().read().await;
    let mut qb = QueryBuilder::new(
        "INSERT INTO `catcoin_price_history` (`company_id`, `price`, `timestamp`)",
    );
    qb.push_values(companies.iter(), |mut b, company| {
        b.push_bind(company.id)
            .push_bind(company.price)
            .push_bind(*now);
    });
    qb.build().execute(&data.local_pool).await?;
    Ok(())
}

/// post the trading floor where users can buy and sell stocks.
async fn post_market_floor(ctx: &serenity::Context, data: &PoiseData) -> Result<(), Error> {
    let companies = get_all_companies(&data.local_pool).await?;
    let c = companies.first().unwrap();
    let ph: Vec<PriceHistory> =
        sqlx::query_as!(PriceHistory, "SELECT * FROM `catcoin_price_history`")
            .fetch_all(&data.local_pool)
            .await?;
    let buf = draw_stock_trends(c, &ph)?;
    let attachment = CreateAttachment::bytes(buf, &format!("{}.png", c.tag));
    data.stock_market_channel
        .send_message(
            ctx,
            CreateMessage::new().add_file(attachment).content("meow"),
        )
        .await?;
    Ok(())
}

pub async fn get_all_companies(pool: &Pool<MySql>) -> Result<Vec<Company>, Error> {
    let res: Vec<Company> = sqlx::query_as!(Company, "SELECT * FROM `catcoin_company`")
        .fetch_all(pool)
        .await?;
    Ok(res)
}

/// TODO: DELETE IN PROD
#[poise::command(slash_command, subcommands("write", "step"))]
pub async fn stocks(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
async fn write(ctx: Context<'_>) -> Result<(), Error> {
    post_market_floor(ctx.serenity_context(), ctx.data()).await?;
    ctx.reply("ok").await?;
    Ok(())
}

#[poise::command(slash_command)]
async fn step(ctx: Context<'_>) -> Result<(), Error> {
    step_market(ctx.serenity_context(), ctx.data()).await?;
    ctx.reply("ok").await?;
    Ok(())
}
