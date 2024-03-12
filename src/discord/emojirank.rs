use sqlx::{self, MySql, Pool};
use std::{collections::HashMap, sync::Arc};
use tokio::{sync::RwLock, time};

use crate::Error;

pub struct EmojiUsage {
    eid: String,
    name: String,
    use_count: i32,
    react_count: i32,
    is_discord: bool,
    animated: bool,
}

pub struct EmojiWatcher {
    cache: HashMap<String, EmojiUsage>,
}

pub fn launch_flush_thread(watcher: Arc<RwLock<EmojiWatcher>>, pool: Pool<MySql>) {
    let mut interval = time::interval(time::Duration::from_secs(15));
    tokio::spawn(async move {
        loop {
            interval.tick().await;

            let _ = watcher
                .write()
                .await
                .flush(&pool)
                .await
                .inspect_err(|e| eprintln!("Could not flush emoji watcher: {e}"));
        }
    });
}

impl EmojiWatcher {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn add_usage(&mut self, eid: String, name: String, is_discord: bool, animated: bool) {
        if let Some(v) = self.cache.get_mut(&eid) {
            v.use_count += 1;
        } else {
            self.cache.insert(
                eid.clone(),
                EmojiUsage {
                    eid,
                    name,
                    use_count: 1,
                    react_count: 0,
                    is_discord,
                    animated,
                },
            );
        }
    }

    pub fn add_react(&mut self, eid: String, name: String, is_discord: bool, animated: bool) {
        if let Some(v) = self.cache.get_mut(&eid) {
            v.react_count += 1;
        } else {
            self.cache.insert(
                eid.clone(),
                EmojiUsage {
                    eid,
                    name,
                    use_count: 0,
                    react_count: 1,
                    is_discord,
                    animated,
                },
            );
        }
    }

    pub fn rm_react(&mut self, eid: String, name: String, is_discord: bool, animated: bool) {
        if let Some(v) = self.cache.get_mut(&eid) {
            v.react_count -= 1;
        } else {
            self.cache.insert(
                eid.clone(),
                EmojiUsage {
                    eid,
                    name,
                    use_count: 0,
                    react_count: -1,
                    is_discord,
                    animated,
                },
            );
        }
    }

    pub async fn flush(&mut self, pool: &Pool<MySql>) -> Result<(), Error> {
        if self.cache.is_empty() {
            return Ok(());
        }
        let entries = self.cache.values();

        // push into db
        let mut qb = sqlx::QueryBuilder::new(
            r#"
		INSERT INTO `emojirank` (`eid`, `name`, `use_count`, `react_count`, `is_discord`, `animated`)"#,
        );
        qb.push_values(entries, |mut b, usage| {
            b //
                .push_bind(&usage.eid)
                .push_bind(&usage.name)
                .push_bind(usage.use_count)
                .push_bind(usage.react_count)
                .push_bind(usage.is_discord)
                .push_bind(usage.animated);
        });
        qb.push("ON DUPLICATE KEY UPDATE `use_count` = `use_count` + VALUES(`use_count`), `react_count` = `react_count` + VALUES(`react_count`)");
        let q = qb.build();
        q.execute(pool).await?;
        self.cache.clear();
        Ok(())
    }
}
