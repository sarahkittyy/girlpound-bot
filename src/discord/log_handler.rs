use crate::logs::{LogReceiver, ParsedLogMessage};
use crate::Error;
use poise::serenity_prelude as serenity;
use sqlx::{MySql, Pool};
use std::env;
use std::sync::Arc;
use tokio::time;

/// receives logs from the tf2 server & posts them in a channel
pub fn spawn_log_thread(
    mut log_receiver: LogReceiver,
    pool: Pool<MySql>,
    ctx: Arc<serenity::CacheAndHttp>,
) {
    let logs_channel: Option<serenity::ChannelId> = env::var("SRCDS_LOG_CHANNEL_ID")
        .ok()
        .and_then(|id| id.parse::<u64>().ok().map(serenity::ChannelId));

    println!("SRCDS_LOG_CHANNEL_ID: {logs_channel:?}");

    if let Some(logs_channel) = logs_channel {
        let mut interval = time::interval(time::Duration::from_secs(1));
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                let msgs = log_receiver.drain().await;
                let mut output = String::new();
                for msg in msgs {
                    let parsed = ParsedLogMessage::from_message(&msg);

                    if parsed.is_unknown() {
                        continue;
                    }

                    let dom_score: Option<i32> = match update_domination_score(&pool, &parsed).await
                    {
                        Ok(score) => Some(score),
                        Err(e) => {
                            println!("Could not update dom score: {:?}", e);
                            None
                        }
                    };

                    output += parsed.as_discord_message(dom_score).as_str();
                    output += "\n";
                }
                if output.len() == 0 {
                    continue;
                }
                if let Err(e) = logs_channel
                    .send_message(ctx.as_ref(), |m| m.content(output))
                    .await
                {
                    println!("Could not send message to logs channel: {:?}", e);
                }
            }
        });
    }
}

/// updates the domination score between users
async fn update_domination_score(pool: &Pool<MySql>, msg: &ParsedLogMessage) -> Result<i32, Error> {
    let ParsedLogMessage::Domination {
        from: dominator,
        to: victim,
    } = msg
    else {
        return Err("Not a domination message".into());
    };

    let mut sign = 1;
    let lt_steamid: &String = if dominator.steamid < victim.steamid {
        sign = -1;
        &dominator.steamid
    } else {
        &victim.steamid
    };
    let gt_steamid: &String = if dominator.steamid > victim.steamid {
        &dominator.steamid
    } else {
        sign = -1;
        &victim.steamid
    };

    // try to fetch the existing score
    let results = sqlx::query!(
        r#"
        SELECT * FROM `domination`
        WHERE `lt_steamid` = ? AND `gt_steamid` = ?
    "#,
        lt_steamid,
        gt_steamid
    )
    .fetch_all(pool)
    .await?;

    if results.len() > 2 {
        unreachable!("More than two rows in the database for a domination relationship")
    }

    let new_score = if results.len() == 0 {
        sign
    } else {
        results.first().unwrap().score + sign
    };

    sqlx::query!(
        r#"
		INSERT INTO `domination` (`lt_steamid`, `gt_steamid`, `score`)
		VALUES (?, ?, ?)
		ON DUPLICATE KEY UPDATE `score` = ? 
	"#,
        lt_steamid,
        gt_steamid,
        new_score,
        new_score
    )
    .execute(pool)
    .await?;

    Ok(new_score * sign)
}
