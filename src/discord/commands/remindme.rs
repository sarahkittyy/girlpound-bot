use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use poise::{
    self,
    serenity_prelude::{self as serenity, CreateMessage, MessageId, ReactionType},
    CreateReply,
};
use sqlx::{self, MySql, Pool, QueryBuilder};
use tokio::sync::RwLock;

use crate::{discord::Context, Error};
use humantime;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Reminder {
    pub mid: String,
    pub uid: String,
    pub cid: String,
    pub remind_at: DateTime<Utc>,
}

pub struct ReminderManager {
    reminders: Vec<Reminder>,
}

impl ReminderManager {
    pub async fn new_with_init(pool: &Pool<MySql>) -> Result<Self, Error> {
        let reminders: Vec<Reminder> = sqlx::query_as!(Reminder, "SELECT * FROM `reminders`")
            .fetch_all(pool)
            .await?;
        Ok(Self { reminders })
    }

    /// insert a new reminder
    pub async fn insert(&mut self, pool: &Pool<MySql>, reminder: Reminder) -> Result<(), Error> {
        sqlx::query!(
            "INSERT INTO `reminders` (`mid`, `cid`, `uid`, `remind_at`) VALUES (?, ?, ?, ?)",
            reminder.mid,
            reminder.cid,
            reminder.uid,
            reminder.remind_at
        )
        .execute(pool)
        .await?;
        self.reminders.push(reminder);
        Ok(())
    }

    /// removes reminders that are ready and returns them
    pub async fn pop_ready(&mut self, pool: &Pool<MySql>) -> Result<Vec<Reminder>, Error> {
        let mut res = vec![];

        let now = Utc::now();

        self.reminders.retain(|reminder| {
            if reminder.remind_at <= now {
                res.push(reminder.clone());
                false
            } else {
                true
            }
        });

        let mut qb = QueryBuilder::<MySql>::new("DELETE FROM `reminders` WHERE `mid` IN");
        qb.push_tuples(res.iter().map(|r| r.mid.clone()), |mut b, mid| {
            b.push_bind(mid);
        });
        let q = qb.build();
        q.execute(pool).await?;

        Ok(res)
    }
}

pub fn spawn_reminder_thread(
    ctx: Arc<serenity::Http>,
    pool: Pool<MySql>,
    guild_id: u64,
    reminders: Arc<RwLock<ReminderManager>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    tokio::spawn(async move {
        loop {
            interval.tick().await;

            let reminders = {
                let mut rm = reminders.write().await;
                rm.pop_ready(&pool).await.unwrap_or(vec![])
            };

            for reminder in reminders {
                let channel = serenity::ChannelId::new(reminder.cid.parse().unwrap());
                let _ = channel
                    .send_message(
                        ctx.clone(),
                        CreateMessage::new().content(format!(
                        "<@{}> Meow!!! Your reminder is up. https://discord.com/channels/{}/{}/{}",
                        reminder.uid, guild_id, reminder.cid, reminder.mid)),
                    )
                    .await
                    .inspect_err(|e| eprintln!("Could not send reminder: {e}"));
            }
        }
    });
}

/// Remind me of something
#[poise::command(slash_command)]
pub async fn remindme_slash(
    ctx: Context<'_>,
    #[description = "When to remind you? (eg. 6d 3h)"] when: String,
) -> Result<(), Error> {
    let now = ctx.created_at();

    let uid = ctx.author().id;
    let cid = ctx.channel_id();
    let duration = humantime::parse_duration(&when)?;

    let remind_at = now.to_utc() + duration;

    let msg = ctx
        .send(CreateReply::default().content(format!(
            "I will ping you on: <t:{}:f>",
            remind_at.timestamp()
        )))
        .await?;

    let reminder = Reminder {
        mid: msg.message().await?.id.to_string(),
        cid: cid.to_string(),
        uid: uid.to_string(),
        remind_at,
    };

    let mut rm = ctx.data().reminders.write().await;
    rm.insert(&ctx.data().local_pool, reminder).await?;

    Ok(())
}

/// Remind me of something
#[poise::command(slash_command, prefix_command)]
pub async fn remindme(
    ctx: Context<'_>,
    #[description = "When to remind you? (eg. 6d 3h)"] when: String,
) -> Result<(), Error> {
    let now = ctx.created_at();

    let uid = ctx.author().id;
    let mid = MessageId::new(ctx.id());
    let cid = ctx.channel_id();
    let duration = humantime::parse_duration(&when)?;

    let remind_at = now.to_utc() + duration;

    let reminder = Reminder {
        mid: mid.to_string(),
        cid: cid.to_string(),
        uid: uid.to_string(),
        remind_at,
    };

    let mut rm = ctx.data().reminders.write().await;
    rm.insert(&ctx.data().local_pool, reminder).await?;

    ctx.http()
        .create_reaction(cid, mid, &ReactionType::Unicode("✅".to_owned()))
        .await?;

    Ok(())
}
