use std::sync::Arc;

use poise::serenity_prelude::{
    self as serenity, Color, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, Timestamp,
};
use rand::random;
use sqlx::{FromRow, MySql, Pool};

use tokio::{self, time};

use crate::logs::safe_strip;

#[allow(unused)]
#[derive(FromRow)]
struct BanProtest {
    // sb_protests
    pid: i32,
    bid: i32,
    datesubmitted: i32,
    reason: String,
    email: String,
    archiv: Option<bool>,
    archivedby: Option<i32>,
    pip: String,
    // sb_bans
    authid: String,
    name: String,
    created: i32,
    ends: i32,
    banreason: String,
    // sb_admins
    admin: String,
}

impl BanProtest {
    fn to_discord_embed(&self) -> CreateEmbed {
        let idfinder = format!("https://www.steamidfinder.com/lookup/{}", self.authid);
        let protests = "https://sourcebans.fluffycat.gay/index.php?p=admin&c=bans";
        let expires = if self.created == self.ends {
            "Never".to_owned()
        } else {
            format!("<t:{}:f>", self.ends)
        };
        CreateEmbed::new() //
		.author(CreateEmbedAuthor::new(format!("{} ({})", self.name, self.email)).url(idfinder))
		.title("Ban Appeal")
		.url(protests)
		.description(format!("`{}`", safe_strip(&self.reason)))
		.color(Color::from_rgb(random(), random(), random()))
		.fields([
			("Banned", format!("<t:{}:f>", self.created), true),
			("Expires", expires, true),
			("Admin", self.admin.clone(), true),
			("Ban Reason", self.banreason.clone(), true)
			("SteamID", self.authid.clone(), true)
		])
		.footer(CreateEmbedFooter::new("Appeal submitted"))
		.timestamp(Timestamp::from_unix_timestamp(self.datesubmitted.into()).unwrap())
    }
}

/// listens for ban protests on the sourcebans database and posts them to the mod channel.
pub fn spawn_ban_protest_thread(
    sb_pool: Pool<MySql>,
    output_channel: serenity::ChannelId,
    last_protest_pid: i32,
    ctx: Arc<serenity::Http>,
) {
    println!("Last ban protest id: {}", last_protest_pid);
    // interval to listen for new protest submissions
    let mut interval = time::interval(time::Duration::from_secs(30));
    tokio::spawn(async move {
        let mut last_pid = last_protest_pid;
        loop {
            interval.tick().await;
            // fetch all new ban protests
            let res: Vec<BanProtest> = match sqlx::query_as(&format!(
                r#"
				SELECT sb_protests.*,
					sb_bans.authid,
					sb_bans.name,
					sb_bans.created,
					sb_bans.ends,
					sb_bans.reason as banreason,
					sb_admins.user as admin
				FROM sb_protests
				INNER JOIN sb_bans
				ON sb_protests.bid = sb_bans.bid
				INNER JOIN sb_admins
				ON sb_bans.aid = sb_admins.aid
				WHERE pid>{}
				ORDER BY pid DESC;
				"#,
                last_pid
            ))
            .fetch_all(&sb_pool)
            .await
            {
                Ok(rows) if rows.len() == 0 => {
                    continue;
                }
                Ok(rows) => rows,
                Err(e) => {
                    println!("Could not fetch protests. Error: {:?}", e);
                    continue;
                }
            };

            // create a protest message
            let mut msg = serenity::CreateMessage::new();
            for protest in &res {
                msg = msg.embed(protest.to_discord_embed());
            }
            // send it
            match output_channel.send_message(&ctx, msg).await {
                Ok(_) => {
                    // if ok, update last protest id
                    last_pid = res.iter().map(|a| a.pid).max().unwrap();
                }
                _ => (),
            };
        }
    });
}
