use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude::{self as serenity, CreateMessage};
use tokio::sync::mpsc::{error::TryRecvError, Sender};

use crate::parse_env;

struct LeakyBucket {
    pub max: f64,
    pub per_minute: f64,
    pub cost: f64,
    pub last: DateTime<Utc>,
    pub prev: f64,
}

impl Default for LeakyBucket {
    fn default() -> Self {
        Self::new(15., 3., 4.)
    }
}

impl LeakyBucket {
    pub fn new(max: f64, per_minute: f64, cost: f64) -> Self {
        Self {
            max,
            per_minute,
            cost,
            last: Utc::now(),
            prev: max,
        }
    }

    /// tries to subtract an action from the bucket, returns Ok if successful or Err with the time until the bucket can afford the action
    pub fn try_afford_one(&mut self) -> Result<(), Duration> {
        let now = chrono::Utc::now();
        let diff = now - self.last;
        let diff_mins: f64 = diff.num_milliseconds() as f64 / (1000. * 60.);
        // last remaining tokens + gained since last run, capped to max
        let current = (self.prev as f64 + self.per_minute as f64 * diff_mins).min(self.max as f64);
        // if we can afford it ....
        if current >= self.cost {
            self.prev = current - self.cost;
            self.last = now;
            Ok(())
        } else {
            // otherwise
            // calculate how many tokens we need
            let needed = self.cost - current;
            // convert to minutes
            let needed_mins = needed / self.per_minute;
            Err(
                Duration::try_milliseconds((needed_mins * 60. * 1000.).floor() as i64)
                    .ok_or(Duration::zero())?,
            )
        }
    }
}

pub struct MediaCooldown {
    pub channels: Vec<serenity::ChannelId>,
    cooldown: HashMap<serenity::ChannelId, HashMap<serenity::UserId, LeakyBucket>>,
}

impl MediaCooldown {
    /// constructs the media cooldown from the MEDIA_COOLDOWN comma separated list of channel ids
    pub fn from_env() -> Self {
        let channels: Vec<serenity::ChannelId> = parse_env::<String>("MEDIA_COOLDOWN")
            .split(',')
            .map(|s| s.parse::<u64>().unwrap())
            .map(serenity::ChannelId::new)
            .collect();
        println!(
            "found media cooldown channels: {}",
            channels
                .iter()
                .map(|s| s.get().to_string())
                .collect::<Vec<String>>()
                .join(",")
        );
        Self {
            channels,
            cooldown: HashMap::new(),
        }
    }

    pub fn try_remove_from_bucket(
        &mut self,
        cid: &serenity::ChannelId,
        uid: &serenity::UserId,
    ) -> Result<(), Duration> {
        let channel_cooldowns = self.cooldown.entry(*cid).or_default();
        channel_cooldowns
            .entry(*uid)
            .or_insert(LeakyBucket::default())
            .try_afford_one()
    }

    /// Checks the message's channel & author & cooldowns and returns if the msg should go through
    pub fn try_allow_one(&mut self, msg: &serenity::Message) -> Result<(), Duration> {
        // only care about msgs in the media channels
        let cid = msg.channel_id;
        if !self.channels.contains(&cid) {
            return Ok(());
        }
        // only care about msgs with attachments
        if msg.attachments.is_empty() && msg.embeds.is_empty() {
            return Ok(());
        }
        let uid = msg.author.id;
        self.try_remove_from_bucket(&cid, &uid)
    }
}

pub struct CooldownMessage {
    pub user: serenity::UserId,
    pub channel: serenity::ChannelId,
    pub delete_at: DateTime<Utc>,
}

pub fn spawn_cooldown_manager(ctx: serenity::Context) -> Sender<CooldownMessage> {
    let (cooldown_sender, mut cooldown_receiver) =
        tokio::sync::mpsc::channel::<CooldownMessage>(64);

    tokio::spawn(async move {
        let mut queue: Vec<(CooldownMessage, serenity::Message)> = vec![];
        loop {
            match cooldown_receiver.try_recv() {
                Err(TryRecvError::Disconnected) => break,
                Err(_) => (),
                // when a cooldown request is received...
                Ok(
                    cooldown @ CooldownMessage {
                        user,
                        channel,
                        delete_at,
                    },
                ) if !queue
                    .iter()
                    .any(|(cd, _)| cd.user == user && cd.channel == channel) =>
                {
                    let msg_string = format!(
                        "<@{}> guh!! >_<... post again <t:{}:R>",
                        user.get(),
                        delete_at.timestamp()
                    );
                    if let Ok(msg) = channel
                        .send_message(&ctx, CreateMessage::new().content(msg_string))
                        .await
                    {
                        queue.push((cooldown, msg));
                    }
                }
                Ok(_) => (),
            }
            queue.retain(|(cooldown, msg)| {
                let http = ctx.http.clone();
                // if it should be deleted by now
                let delete = Utc::now() - cooldown.delete_at > Duration::zero();
                if delete {
                    let mid = msg.id;
                    let cid = msg.channel_id;
                    tokio::task::spawn(async move {
                        http.delete_message(cid, mid, Some("media cooldown")).await
                    });
                }
                !delete
            });
            tokio::task::yield_now().await;
        }
    });

    cooldown_sender
}
