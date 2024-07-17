use std::sync::OnceLock;

use chrono::{Days, NaiveDateTime, Utc};
use poise::serenity_prelude::{self as serenity, CreateAttachment, CreateEmbed, CreateMessage};
use sqlx::{MySql, Pool, QueryBuilder};
use tokio::sync::RwLock;
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
    pub logo: String,
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

    // precursor post
    let content = format!(
        "# State of the market {}",
        market_time().read().await.format("%m-%d")
    );
    data.stock_market_channel
        .send_message(ctx, CreateMessage::new().content(content))
        .await?;

    // all companies' price histories for the last 30 days.
    let ph: Vec<PriceHistory> = sqlx::query_as!(
		PriceHistory,
		"SELECT * FROM `catcoin_price_history` WHERE `timestamp` > CURRENT_DATE() - INTERVAL 1 MONTH",
	)
    .fetch_all(&data.local_pool)
    .await?;

    // bulk post 4 stock graphs at a time
    let mut bulk_post: Vec<CreateAttachment> = vec![];
    const BULK_AMNT: i32 = 4;
    let mut bulk_counter: i32 = BULK_AMNT;
    for company in &companies {
        // get company price history
        let ph = ph
            .iter()
            .cloned()
            .filter(|ph| ph.company_id == company.id)
            .collect();
        // draw chart
        let buf = draw_stock_trends(company, ph).await?;
        // post
        let attachment = CreateAttachment::bytes(buf, &format!("{}.png", company.tag));
        bulk_post.push(attachment);
        bulk_counter -= 1;
        if bulk_counter == 0 {
            data.stock_market_channel
                .send_message(ctx, CreateMessage::new().add_files(bulk_post))
                .await?;
            bulk_post = vec![];
            bulk_counter = BULK_AMNT;
        }
    }
    // send residual posts
    if bulk_post.len() > 0 {
        data.stock_market_channel
            .send_message(ctx, CreateMessage::new().add_files(bulk_post))
            .await?;
    }
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
    ctx.reply("ok").await?;
    post_market_floor(ctx.serenity_context(), ctx.data()).await?;
    Ok(())
}

#[poise::command(slash_command)]
async fn step(ctx: Context<'_>) -> Result<(), Error> {
    ctx.reply("ok").await?;
    for _ in 0..=30 {
        step_market(ctx.serenity_context(), ctx.data()).await?;
    }
    post_market_floor(ctx.serenity_context(), ctx.data()).await?;
    Ok(())
}
