use crate::logs::{as_discord_message, LogReceiver};
use crate::{Error, Server};
use poise::serenity_prelude::{self as serenity};
use serenity::CreateMessage;
use sqlx::{MySql, Pool};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time;

use srcds_log_parser::MessageType;

/// receives logs from the tf2 server & posts them in a channel
pub fn spawn_log_thread(
    mut log_receiver: LogReceiver,
    servers: HashMap<SocketAddr, Server>,
    pool: Pool<MySql>,
    ctx: Arc<serenity::Http>,
) {
    let mut interval = time::interval(time::Duration::from_secs(3));
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            // drain all received log messages
            let msgs = log_receiver.drain().await;
            let mut output = HashMap::<SocketAddr, String>::new();
            for (from, msg) in msgs {
                let parsed = MessageType::from_message(msg.message.as_str());

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

                let dm = as_discord_message(&parsed, dom_score);

                if let Some(dm) = dm {
                    let v = output.entry(from).or_insert_with(|| "".to_owned());
                    *v += dm.as_str();
                    *v += "\n";
                }
            }
            // for every output msg...
            for (addr, msg) in &output {
                // get the server its from
                let Some(server) = servers.get(addr) else {
                    println!("addr {:?} has no associated server", addr);
                    continue;
                };
                // get the log channel
                let Some(logs_channel) = server.log_channel else {
                    continue;
                };
                // do not send empty messages
                if msg.len() == 0 {
                    continue;
                }
                // post it
                if let Err(e) = logs_channel
                    .send_message(ctx.as_ref(), CreateMessage::new().content(msg))
                    .await
                {
                    println!("Could not send message to logs channel: {:?}", e);
                }
            }
        }
    });
}

/// updates the domination score between users
async fn update_domination_score(pool: &Pool<MySql>, msg: &MessageType) -> Result<i32, Error> {
    let MessageType::InterPlayerAction {
        from: dominator,
        against: victim,
        action,
    } = msg
    else {
        return Err("Not a domination message".into());
    };
    if action != "domination" {
        return Err("Not a domination message".into());
    }

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
