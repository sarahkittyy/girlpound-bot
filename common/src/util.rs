use std::{env, str::FromStr, time::Instant};
use tokio::sync::mpsc;

use chrono::{DateTime, Duration, Utc};

pub fn hhmmss(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:0>2}:{:0>2}:{:0>2}", hours, minutes, seconds)
}

pub fn parse_env<T: FromStr>(name: &str) -> T {
    env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .expect(&format!("Could not find env variable {}", name))
}

pub async fn recv_timeout<T>(mut rx: mpsc::Receiver<T>, duration: std::time::Duration) -> Vec<T> {
    let mut res = vec![];
    let timeout = Instant::now() + duration;
    loop {
        // calculate the remaining time for the timeout
        let remaining = timeout.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break; // stop if we've hit the 3-second mark
        }

        // try receiving with a timeout
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(msg)) => {
                res.push(msg); // collect message into the vector
            }
            Ok(None) => {
                // the channel is closed, so exit the loop
                break;
            }
            Err(_) => {
                // timeout happened, exit the loop
                break;
            }
        }
    }
    res
}

pub fn get_bit(value: u16, bit: u8) -> bool {
    value & (1 << bit) > 0
}

pub fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}

pub struct LeakyBucket {
    pub max: f64,
    pub per_minute: f64,
    pub cost: f64,
    pub last: DateTime<Utc>,
    pub tokens: f64,
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
            tokens: max,
        }
    }

    /// tries to subtract an action from the bucket, returns Ok if successful or Err with the time until the bucket can afford the action
    pub fn try_afford_one(&mut self) -> Result<(), Duration> {
        let now = chrono::Utc::now();
        let diff = now - self.last;
        let diff_mins: f64 = diff.num_milliseconds() as f64 / (1000. * 60.);
        // last remaining tokens + gained since last run, capped to max
        let current =
            (self.tokens as f64 + self.per_minute as f64 * diff_mins).min(self.max as f64);
        // if we can afford it ....
        if current >= self.cost {
            self.tokens = current - self.cost;
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

pub fn remove_backticks(s: &str) -> String {
    s.replace("`", "")
}

pub fn strip_markdown(s: &str) -> String {
    s.replace("*", "\\*")
        .replace(">", "\\>")
        .replace("_", "\\_")
        .replace("-", "\\-")
        .replace("#", "\\#")
        .replace("~", "\\~")
        .replace("`", "\\`")
        .replace("[", "\\[")
        .replace("\\", "\\\\")
}
