use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use common::Error;
use poise::serenity_prelude::{ChannelId, CreateMessage, Http};
use serde::{Deserialize, Serialize};
use sqlx::{MySql, Pool};

const API_BASE: &'static str = "http://logs.tf/api/v1";

pub fn init(pool: &Pool<MySql>, uploader: u64, http: Arc<Http>, channel_id: ChannelId) {
    let pool = pool.clone();
    tokio::spawn(async move {
        let mut last_id: i64 = sqlx::query!("SELECT `id` FROM `logstf_lastposted` LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("could not fetch logstf last_posted")
            .id;

        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let mut logs = match fetch_last_logs(uploader).await {
                Ok(logs) => logs,
                Err(e) => {
                    eprintln!("Could not fetch logs: {e}");
                    continue;
                }
            };

            logs.sort_by(|a, b| a.id.cmp(&b.id));
            for log in &logs {
                if log.id > last_id {
                    last_id = log.id;
                    let _ = channel_id
                        .send_message(&http, CreateMessage::new().content(log.url()))
                        .await
                        .inspect_err(|e| eprintln!("Could not send logstf message: {e}"));
                }
            }

            let _ = sqlx::query!("UPDATE `logstf_lastposted` SET `id` = ?", last_id)
                .execute(&pool)
                .await
                .inspect_err(|e| eprintln!("could not update lastid in db: {e}"));
        }
    });
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Log {
    id: i64,
}

impl Log {
    fn url(&self) -> String {
        format!("https://logs.tf/{}", self.id)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct LogResponse {
    logs: Vec<Log>,
}

async fn fetch_last_logs(uploader: u64) -> Result<Vec<Log>, Error> {
    let response = reqwest::get(format!("{API_BASE}/log?uploader={uploader}&limit=4"))
        .await?
        .json::<LogResponse>()
        .await?;
    Ok(response.logs)
}
