use crate::logs::{LogReceiver, ParsedLogMessage};
use crate::{Error, Server};
use poise::serenity_prelude as serenity;
use sqlx::{MySql, Pool};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time;

/// receives logs from the tf2 server & posts them in a channel
pub fn spawn_log_thread(
    mut log_receiver: LogReceiver,
    servers: HashMap<SocketAddr, Server>,
    pool: Pool<MySql>,
    ctx: Arc<serenity::CacheAndHttp>,
) {
    let mut interval = time::interval(time::Duration::from_secs(3));
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            // drain all received log messages
            let msgs = log_receiver.drain().await;
            let mut output = HashMap::<SocketAddr, String>::new();
            for msg in msgs {
                let from = msg.from;
                let parsed = ParsedLogMessage::from_message(&msg);

                if parsed.is_unknown() {
                    continue;
                }

                let dom_score: Option<i32> = match update_domination_score(&pool, &parsed).await {
                    Ok(score) => Some(score),
                    Err(_) => {
                        // println!("Could not update dom score: {:?}", e);
                        None
                    }
                };

                let dm = parsed.as_discord_message(dom_score);

                if let Some(dm) = dm {
                    let v = output.entry(from).or_insert_with(|| "".to_owned());
                    *v += dm.as_str();
                    *v += "\n";
                }
            }
            // for every server...
            for (addr, server) in &servers {
                // ...that has a log channel...
                let Some(logs_channel) = server.log_channel else {
                    continue;
                };
                // ...and has logs to post...
                if let Some(msg) = output.get(&addr) {
                    if msg.len() == 0 {
                        continue;
                    }
                    // ...post them
                    if let Err(e) = logs_channel
                        .send_message(ctx.as_ref(), |m| m.content(msg))
                        .await
                    {
                        println!("Could not send message to logs channel: {:?}", e);
                    }
                }
            }
        }
    });
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
