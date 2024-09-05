use std::{collections::HashMap, sync::Arc, time::Duration};

use poise::serenity_prelude::{
    self as serenity, ChannelId, Color, CreateAllowedMentions, CreateEmbed, CreateMessage,
    Mentionable, Message, UserId,
};
use sqlx::{MySql, Pool, QueryBuilder};
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobBuilder};

use common::Error;

pub struct YapTracker {
    cache: HashMap<UserId, i64>,
}

pub struct YapAwards {
    pub top10: Vec<(UserId, i64)>,
    pub total: i64,
}

impl YapTracker {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn track_one(&mut self, from: UserId) {
        self.cache
            .insert(from.clone(), self.cache.get(&from).unwrap_or(&0) + 1);
    }

    pub async fn flush_to_db(&mut self, db: &Pool<MySql>) -> Result<(), Error> {
        if self.cache.is_empty() {
            return Ok(());
        }
        let mut qb = QueryBuilder::new("INSERT INTO `yapawards` (`uid`, `count`)");
        qb.push_values(self.cache.iter(), |mut b, (from, count)| {
            b.push_bind(from.to_string()).push_bind(count);
        });
        qb.push("ON DUPLICATE KEY UPDATE `count` = `count` + VALUES(`count`)");
        let q = qb.build();
        q.execute(db).await?;
        self.cache.clear();
        Ok(())
    }

    pub async fn get_awards_and_reset(db: &Pool<MySql>) -> Result<YapAwards, Error> {
        let top10 = sqlx::query!("SELECT * FROM `yapawards` ORDER BY COUNT DESC")
            .fetch_all(db)
            .await?;
        let total = sqlx::query!("SELECT CAST(SUM(count) as INT) as `total` FROM `yapawards`")
            .fetch_one(db)
            .await?
            .total
            .unwrap_or(0);
        sqlx::query!("UPDATE `yapawards` SET `count` = 0")
            .execute(db)
            .await?;
        Ok(YapAwards {
            top10: top10
                .into_iter()
                .map(|r| (UserId::new(r.uid.parse::<u64>().unwrap()), r.count))
                .collect(),
            total,
        })
    }
}

impl YapAwards {
    pub fn to_embed(&self) -> CreateEmbed {
        let top10total = self.top10.iter().fold(0, |acc, (_, count)| acc + count) as f64;
        CreateEmbed::new()
            .title("🏆 Today's Yap Awards")
            .color(Color::from_rgb(0xEF, 0xBF, 0x04))
            .description(
                self.top10
                    .iter()
                    .enumerate()
                    .map(|(i, (from, count))| {
                        format!(
                            "{}**{}**. {} messages (`{:.0}%`/`{:.0}%`) - {}",
                            // fourth  place
                            if i == 3 { "🥇 " } else { "" },
                            i + 1,
                            count,
                            (*count as f64 / top10total) * 100.0,
                            (*count as f64 / self.total as f64) * 100.0,
                            from.mention(),
                        )
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            )
    }
}

/// Tracks the amount of messages each user sends per day.
pub async fn on_message(tracker: &mut YapTracker, msg: &Message) -> Result<(), Error> {
    tracker.track_one(msg.author.id);

    Ok(())
}

pub fn init(tracker: Arc<RwLock<YapTracker>>, pool: &Pool<MySql>) {
    let db = pool.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;

            let _ = tracker
                .write()
                .await
                .flush_to_db(&db)
                .await
                .inspect_err(|e| eprintln!("could not flush yap to db: {e:?}"));
        }
    });
}

pub fn start_job(http: Arc<serenity::Http>, channel: ChannelId, db: Pool<MySql>) -> Job {
    JobBuilder::new()
        .with_timezone(chrono_tz::US::Eastern)
        .with_cron_job_type()
        .with_schedule("0 0 20 * * *")
        //.with_schedule("0 1 * * * *")
        .unwrap()
        .with_run_async(Box::new(move |_uuid, _l| {
            let db = db.clone();
            let http = http.clone();
            Box::pin(async move {
                println!("Logging yap awards.");
                let awards = match YapTracker::get_awards_and_reset(&db).await {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("could not get yap awards: {e:?}");
                        return;
                    }
                };

                let _ = channel
                    .send_message(
                        http,
                        CreateMessage::new()
                            .embed(awards.to_embed())
                            .allowed_mentions(
                                CreateAllowedMentions::new().empty_roles().empty_users(),
                            ),
                    )
                    .await
                    .inspect_err(|e| eprintln!("failed to send award msg: {e:?}"));
            })
        }))
        .build()
        .unwrap()
}
