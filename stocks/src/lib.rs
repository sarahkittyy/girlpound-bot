use std::sync::OnceLock;

use chrono::{Days, NaiveDateTime};
use poise::serenity_prelude::{
    self as serenity, ComponentInteraction, ComponentInteractionDataKind, CreateAttachment,
    CreateInteractionResponse, CreateInteractionResponseFollowup, CreateMessage, Mentionable,
};
use sqlx::{MySql, Pool};
use tokio::sync::RwLock;
use tokio_cron_scheduler::JobScheduler;
use transaction::Transaction;

use common::Error;

use self::plot::draw_stock_trends;

mod account;
mod company;
mod plot;
mod price_history;
mod transaction;
mod ui;

use company::Company;
use price_history::PriceHistory;

// fake datetime for debugging
static NOW: OnceLock<RwLock<NaiveDateTime>> = OnceLock::new();
fn market_time() -> &'static RwLock<NaiveDateTime> {
    NOW.get().unwrap()
}

/// Handle all component interactions.
pub async fn interaction_dispatch(
    ctx: &serenity::Context,
    pool: &Pool<MySql>,
    mci: &ComponentInteraction,
) -> Result<(), Error> {
    match mci.data.custom_id.as_str() {
        // When the user first clicks the buy button
        "stock-market.buy" => ui::choose_company(ctx, pool, mci).await?,
        // When the user picks the company they want to buy from
        "stock-market.buy-from" => {
            match mci.data.kind {
                ComponentInteractionDataKind::StringSelect { ref values } => {
                    // get the company
                    let Some(v) = values.first() else {
                        return Err("No company choice response.".into());
                    };
                    let id: i32 = v.parse().unwrap();
                    let mut company = Company::fetch_by_id(pool, id)
                        .await?
                        .ok_or(Into::<Error>::into("Company not found."))?;
                    // get the amount of that company stock to buy
                    let (mi, amount_to_buy) =
                        match ui::buy_stocks_modal(ctx, &mci, &mut company).await {
                            Ok(amnt) => amnt,
                            Err(e) => {
                                mci.create_followup(
                                    &ctx,
                                    CreateInteractionResponseFollowup::new()
                                        .content(format!("Invalid response: {e}"))
                                        .ephemeral(true),
                                )
                                .await?;
                                return Err("Bad modal response.".into());
                            }
                        };
                    println!(
                        "User requested {} stocks of {}",
                        amount_to_buy, company.name
                    );
                    mi.create_response(ctx, CreateInteractionResponse::Acknowledge)
                        .await?;
                    // buy the shares
                    match Transaction::buy_shares_from_company(
                        pool,
                        mci.user.id,
                        amount_to_buy,
                        &company,
                    )
                    .await
                    {
                        Err(e) => {
                            mi.create_followup(
                                ctx,
                                CreateInteractionResponseFollowup::new()
                                    .content(format!("Failed to complete transaction: {e}",))
                                    .ephemeral(true),
                            )
                            .await?;
                            return Err("Failed to complete transaction".into());
                        }
                        _ => (),
                    };
                    mci.channel_id
                        .send_message(
                            ctx,
                            CreateMessage::new().content(format!(
                                "{} bought **{}** shares of _{}_ for **{}** {} (Each: **{}** {})",
                                mci.user.mention(),
                                amount_to_buy,
                                company.name,
                                amount_to_buy * company.price,
                                catcoin::emoji(),
                                company.price,
                                catcoin::emoji()
                            )),
                        )
                        .await?;
                }
                _ => (),
            }
        }
        _ => (),
    };

    Ok(())
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
pub async fn step_market(ctx: &serenity::Context, pool: &Pool<MySql>) -> Result<(), Error> {
    let mut companies = Company::fetch_all(pool).await?;

    // update to new price
    for company in &mut companies {
        *company = company.step_price(pool).await?;
    }

    // step simulated time
    {
        let mut time = market_time().write().await;
        println!("Updating market for {}", time);
        *time = time.checked_add_days(Days::new(1)).ok_or("Bad add")?;
        sqlx::query!(
            "UPDATE `catcoin_sim_time` SET `datetime` = ? WHERE `id` = 1",
            *time
        )
        .execute(pool)
        .await?;
        println!("It is now {}", time);
    }

    // flush current price
    PriceHistory::flush_companies(pool, &companies).await?;

    Ok(())
}

/// post the trading floor where users can buy and sell stocks.
pub async fn post_market_floor(
    ctx: &serenity::Context,
    pool: &Pool<MySql>,
    channel: serenity::ChannelId,
) -> Result<(), Error> {
    let companies = Company::fetch_all(pool).await?;

    // precursor post
    let content = format!(
        "# State of the market {}",
        market_time().read().await.format("%m-%d")
    );
    channel
        .send_message(ctx, CreateMessage::new().content(content))
        .await?;

    let ph = PriceHistory::fetch_all_last_month(pool).await?;

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
            channel
                .send_message(ctx, CreateMessage::new().add_files(bulk_post))
                .await?;
            bulk_post = vec![];
            bulk_counter = BULK_AMNT;
        }
    }
    // send residual posts
    if bulk_post.len() > 0 {
        channel
            .send_message(ctx, CreateMessage::new().add_files(bulk_post))
            .await?;
    }
    // send market hub.
    channel.send_message(ctx, ui::market_hub().await).await?;
    Ok(())
}
