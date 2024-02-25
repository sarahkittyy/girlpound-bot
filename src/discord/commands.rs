use std::env;
use std::net::SocketAddr;

use super::Context;
use crate::{Error, Server};

pub mod util;
use util::*;

mod map;
pub use map::map;

mod mods;
pub use mods::*;

use poise;
use poise::serenity_prelude as serenity;
use rand::prelude::*;
use regex::Regex;

/// Sets the server player limit
#[poise::command(slash_command)]
pub async fn playercap(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The player cap 24 <= p <= 32"] count: u8,
) -> Result<(), Error> {
    let re = Regex::new(r#""maxplayers" is "(\d+)""#).unwrap();

    let min = 24;
    let max = ctx
        .data()
        .server(server)?
        .controller
        .write()
        .await
        .run("maxplayers")
        .await?;
    let max = re
        .captures(&max)
        .and_then(|caps| caps[1].parse::<u8>().ok())
        .unwrap_or(25)
        - 1;

    let visible = count.max(min).min(max);
    let reserved = (max - visible - 1).min(0);
    let cmd = format!(
        "sm_reserved_slots {}; sv_visiblemaxplayers {};",
        reserved, visible
    );
    rcon_and_reply(ctx, Some(server), cmd).await
}

/// Sends an RCON command to the server.
#[poise::command(slash_command)]
pub async fn rcon(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "The command to send."] cmd: String,
    #[description = "Hide the reply?"] hide_reply: Option<bool>,
) -> Result<(), Error> {
    let reply = rcon_user_output(&output_servers(ctx, server)?, cmd).await;
    let hide_reply = hide_reply.unwrap_or(false);
    ctx.send(|m| m.ephemeral(hide_reply).content(reply)).await?;
    Ok(())
}

/// Set the sniper limit on the server
#[poise::command(slash_command)]
pub async fn snipers(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "The sniper limit (-1 for enable)"] limit: i8,
    #[description = "Hide the reply?"] hide_reply: Option<bool>,
) -> Result<(), Error> {
    let cmd = format!(
        "sm_classrestrict_blu_snipers {0}; sm_classrestrict_red_snipers {0}",
        limit
    );
    let reply = rcon_user_output(&output_servers(ctx, server)?, cmd).await;
    let hide_reply = hide_reply.unwrap_or(false);
    ctx.send(|m| m.ephemeral(hide_reply).content(reply)).await?;

    Ok(())
}

/// Sends anonymous feedback to the server owner.
#[poise::command(slash_command)]
pub async fn feedback(
    ctx: Context<'_>,
    #[description = "The feedback to share."] msg: String,
    #[description = "An optional attachment"] attachment: Option<serenity::Attachment>,
) -> Result<(), Error> {
    // get the owner id in the env file
    let Ok(owner_id) = env::var("FEEDBACK_USER") else {
        poise::send_reply(ctx, |m| {
            m.ephemeral(true)
                .content("Feedback is not configured properly! Message an admin.")
        })
        .await?;
        return Ok(());
    };

    // get the owner
    let recip = serenity::UserId(owner_id.parse()?);
    let dm_channel = recip.create_dm_channel(ctx).await?;
    dm_channel
        .send_message(ctx, |m| {
            m.embed(|e| {
                let mut r = e.title("anon feedback").description(msg);

                if let Some(attachment) = attachment {
                    r = r.image(attachment.url);
                }

                r
            })
        })
        .await?;

    poise::send_reply(ctx, |m| {
        m.ephemeral(true).content("Feedback anonymously sent!")
    })
    .await?;
    Ok(())
}

/// Set / Get the status of the respawn timers ( resets on map change )
#[poise::command(slash_command)]
pub async fn respawntimes(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "Set to instant respawn"] instant: Option<bool>,
) -> Result<(), Error> {
    let cmd: String = match instant {
        None => format!("mp_disable_respawn_times"),
        Some(instant) => format!(
            "mp_disable_respawn_times {}",
            if instant { "1" } else { "0" }
        ),
    };
    let reply = rcon_user_output(&output_servers(ctx, server)?, cmd).await;
    ctx.send(|m| m.content(reply)).await?;

    Ok(())
}

/// Request that people join you in a server
#[poise::command(slash_command)]
pub async fn seeder(
    ctx: Context<'_>,
    #[description = "The server to seed"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "Optional message to attach"] message: Option<String>,
) -> Result<(), Error> {
    // check cooldown
    match ctx.data().can_seed(server).await {
        Ok(()) => (),
        Err(time_left) => {
            let now = chrono::Utc::now();
            ctx.send(|m| {
                m.content(format!(
                    "Server was seeded too recently. Try again <t:{}:R>",
                    (now + time_left).timestamp()
                ))
            })
            .await?;
            return Ok(());
        }
    };

    let server_addr = server;
    let server = ctx.data().server(server)?;
    if !server.allow_seed {
        ctx.send(|m| m.content("This server is not seedable."))
            .await?;
        return Ok(());
    }

    let mut rcon = server.controller.write().await;
    let status = rcon.status().await?;
    let player_count = status.players.len();

    if player_count < 2 {
        ctx.send(|m| m.content("Server must have >2 players to ping."))
            .await?;
        return Ok(());
    }
    if player_count >= 16 {
        ctx.send(|m| m.content("Server must have <16 players to ping."))
            .await?;
        return Ok(());
    }

    let seeder_role = ctx.data().seeder_role;

    // send seed
    ctx.send(|m| {
        m.content(format!(
            "{}<@&{}> come fwag on {} :3\nraowquested by: <@{}>\n{}",
            if let Some(msg) = message {
                msg + "\n"
            } else {
                "".to_owned()
            },
            seeder_role.0,
            server.emoji,
            ctx.author().id,
            status.as_discord_output(server, false),
        ))
        .allowed_mentions(|am| am.roles(vec![seeder_role.0]))
    })
    .await?;
    // reset cooldown
    ctx.data().reset_seed_cooldown(server_addr).await;

    Ok(())
}

/// SteamID.uk discord command.
#[poise::command(slash_command, global_cooldown = 10)]
pub async fn lookup(
    ctx: Context<'_>,
    #[description = "SteamID, Steam2, Steam3, or vanity URL. Separate multiple by commas."]
    #[autocomplete = "steam_id_autocomplete"]
    query: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let client = &ctx.data().client;
    let data = client.lookup(&query).await?;
    // fetch important info

    ctx.send(|m| {
        m.content(format!("Results for query: `{}`", query));
        for user in &data {
            m.embed(|e| user.populate_embed(e));
        }
        m.ephemeral(true)
    })
    .await?;

    Ok(())
}

/// Displays current server player count & map.
#[poise::command(slash_command)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: Option<SocketAddr>,
    #[description = "Display user IDs?"] show_uids: Option<bool>,
) -> Result<(), Error> {
    // get all the servers to include in the result
    let mut servers = if let Some(server) = server {
        vec![ctx.data().server(server)?]
    } else {
        ctx.data()
            .servers
            .values()
            .filter(|s| s.show_status)
            .collect::<Vec<&Server>>()
    };
    servers.sort_by_key(|s| &s.name);

    let show_uids = show_uids.unwrap_or(false);

    let mut output = String::new();
    for server in servers {
        let mut rcon = server.controller.write().await;
        let state = rcon.status().await?;

        output += &state.as_discord_output(server, show_uids);
    }
    // delete last status msg
    let msgs = ctx
        .channel_id()
        .messages(ctx.http(), |gm| gm.limit(45))
        .await?;
    let bid = ctx.cache().current_user_id();
    for msg in &msgs {
        if msg.author.id == bid && (msg.content.starts_with("🅰️") || msg.content.starts_with("🅱️"))
        {
            msg.delete(ctx.http()).await?;
            break;
        }
    }
    // send status msg
    ctx.send(|m| m.content(output).ephemeral(show_uids)).await?;
    Ok(())
}

/// Pick a random user with the given role
#[poise::command(slash_command)]
pub async fn reacted_users(
    ctx: Context<'_>,
    #[description = "The message's channel"] channel_id: String,
    #[description = "The message to fetch reactions from"] message_id: String,
) -> Result<(), Error> {
    let mut total = vec![];
    let mut after: Option<serenity::UserId> = None;
    let channel: u64 = channel_id.parse()?;
    let message: u64 = message_id.parse()?;
    let msg = ctx.http().get_message(channel, message).await?;
    let r_type = &msg.reactions.first().unwrap().reaction_type;
    loop {
        let mut users = match msg
            .reaction_users(&ctx, r_type.clone(), Some(50), after)
            .await
        {
            Ok(users) => users,
            Err(e) => {
                println!("Error fetching users: {:?}", e);
                break;
            }
        };
        let user_count = users.len();
        if user_count == 0 {
            break;
        }
        let last_user_id = users.last().unwrap().id;
        total.append(&mut users);
        if user_count < 50 {
            break;
        } else {
            after = Some(last_user_id)
        }
    }
    let names = total.iter().map(|u| u.tag()).collect::<Vec<String>>();
    let winner = names.choose(&mut rand::thread_rng()).unwrap();

    ctx.reply(format!(
        "emoji: {}\ncount: {}\nchosen: {}",
        r_type,
        names.len(),
        winner
    ))
    .await?;
    Ok(())
}

/// Bark
#[poise::command(slash_command, channel_cooldown = 4)]
pub async fn bark(ctx: Context<'_>) -> Result<(), Error> {
    let uid: String = ctx.author().id.to_string();
    let nickname: String = match ctx.author_member().await {
        Some(member) => member.display_name().to_string(),
        _ => ctx.author().name.to_owned(),
    };

    // log the barker
    sqlx::query!(
        r#"
		INSERT INTO `barkers` (`uid`, `last_nickname`)
		VALUES (?, ?)
		ON DUPLICATE KEY UPDATE `last_nickname` = ?, `updated_at` = CURRENT_TIMESTAMP
	"#,
        uid,
        nickname,
        nickname
    )
    .execute(&ctx.data().pool)
    .await?;

    // fetch recent barkers
    let results = sqlx::query!(
        r#"
		SELECT `last_nickname` from `barkers`
		ORDER BY `updated_at` DESC
		LIMIT 15
	"#
    )
    .fetch_all(&ctx.data().pool)
    .await?;

    let user_list = results
        .iter()
        .map(|n| &n.last_nickname)
        .fold(String::new(), |acc, s| acc + s + "\n");

    let response =
        format!("Barking is strictly prohibited. Your ID has been logged.\nLast 15 infractions:```\n{user_list}```");

    ctx.send(|c| c.ephemeral(true).content(response)).await?;

    Ok(())
}

/// Meow (suppawters only)
#[poise::command(slash_command, channel_cooldown = 4)]
pub async fn meow(ctx: Context<'_>) -> Result<(), Error> {
    let meows = [
        "meow!! :revolving_hearts:",
        "nya >w<",
        "prraow",
        "mrp",
        "prraow!! nya raow... mew !!! :D",
        "hehe, nya !!",
        "prrrp",
        "meow",
        "meow. >:(",
        "meow >:3",
        "MRRRAOW!!!",
        "ᵐᵉᵒʷ",
        "mew >w<",
        "nya~! >//<",
        "prraow raow... nya mrrp purrrr..",
        "purrr....... <3",
        "mp <333333",
        "*opens mouth, but doesn't actually meow*",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "purr~",
        "*meows*",
        "*meows at u*",
        "mrrrrrrrrrrrr",
        "mrrrrrraow.................",
        "mew !!! mew :3 myaow raow :3 !!!",
        "is she, yknow, like, *curls paw*?",
        "ehe, uhm, mraow !! >w<",
        "guh wuh huh ?? nya...",
        "eep!! *purrs*",
        "rawr i'm feral !!!! grrr >_<",
        "rrrr",
        ":3",
        "prraow raow... raow mrrrp :3",
        r#"I'd just wike to intewject fow a moment. What you'we wefewwing to as linux, is in fact, gnu/linux, ow as i've wecentwy taken to cawwing it, gnu pwus linux. linux is not an opewating system unto itsewf, but wathew anothew fwee component of a fuwwy functioning gnu system made usefuw by the gnu cowewibs, sheww utiwities and vitaw system components compwising a fuww os as defined by posix. many computew usews wun a modified vewsion of the gnu system evewy day, without weawizing it. Thwough a pecuwiaw tuwn of events, the vewsion of gnu which is widewy used today is often cawwed "linux", and many of its usews awe not awawe that it is basicawwy the gnu system, devewoped by the gnu pwoject. thewe weawwy is a linux, and these peopwe awe using it, but it is just a pawt of the system they use. Linux is the kewnew: the pwogwam in the system that awwocates the machine's wesouwces to the othew pwogwams that you wun. the kewnew is an essentiaw pawt of an opewating system, but usewess by itsewf; it can onwy function in the context of a compwete opewating system. Linux is nowmawwy used in combination with the gnu opewating system: the whowe system is basicawwy gnu with linux added, ow gnu/linux. Aww the so-cawwed "linux" distwibutions awe weawwy distwibutions of gnu/linux."#,
        "dm me immeowdiately!! :revolving_hearts:",
		"u sound feline add me NOW!!!!!!!!!!!!!!",
		"you sound feline enough add me",
		"save me balls of yarn\nballs of yarn\nballs of yarn save me",
		"any kitty girls in chat???? :3",
		"FWICK!!! *pukes on the carpet*",
		"mooooooooooods my food bowl is empty >_<",
		"https://media.discordapp.net/attachments/923967765302378496/1092546578859950190/meow.gif",
		"https://media.discordapp.net/attachments/716323693877395537/883757436434018355/tumblr_beb1f92611396501e6370766e57257dc_05f5405f_250.gif",
		"https://media.discordapp.net/attachments/901299978817925131/1126298371410366494/JMvRJlHy.gif",
		"https://media.discordapp.net/attachments/779900906665017405/1061512722228981860/attachment-19.gif",
		"https://media.discordapp.net/attachments/984367821901402153/1043398180722720848/kat.gif",
		"meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
        "meow",
    ];
    let r = (random::<f32>() * meows.len() as f32).floor() as usize;

    poise::send_reply(ctx, |message| message.content(meows[r])).await?;
    Ok(())
}
