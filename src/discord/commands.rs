use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use super::Context;
use crate::logs::safe_strip;
use crate::{Error, Server};

mod map;
pub use map::map;
mod girlgift;
pub use girlgift::girlgift;

use poise::serenity_prelude::{self as serenity};
use poise::{self, AutocompleteChoice};
use rand::prelude::*;

pub async fn rcon_user_output(server: &Server, cmd: String) -> Result<String, Error> {
    let mut rcon = server.controller.write().await;
    let reply = rcon.run(&cmd).await;
    match reply {
        Ok(output) => Ok(if output.len() == 0 {
            ":white_check_mark:".to_owned()
        } else {
            format!("```\n{}\n```", output)
        }),
        Err(e) => Err(e.into()),
    }
}

pub async fn rcon_and_reply(
    ctx: Context<'_>,
    server: SocketAddr,
    cmd: String,
) -> Result<(), Error> {
    let server = ctx.data().server(server)?;
    ctx.say(rcon_user_output(&server, cmd).await?).await?;
    Ok(())
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
    let reply = if let Some(addr) = server {
        rcon_user_output(ctx.data().server(addr)?, cmd).await?
    } else {
        let mut output = String::new();
        for (_addr, server) in &ctx.data().servers {
            let res = rcon_user_output(server, cmd.clone()).await;
            output += &format!(
                "{}\n{}",
                server.emoji,
                match res {
                    Ok(res) => res,
                    Err(e) => format!("Error: {}", e),
                }
            );
        }
        output
    };
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

/// Returns the list of online users
async fn users_autocomplete(ctx: Context<'_>, partial: &str) -> Vec<AutocompleteChoice<String>> {
    let mut res = vec![];
    for (_addr, server) in &ctx.data().servers {
        if let Some(state) = server.controller.write().await.status().await.ok() {
            res.extend(
                state
                    .players
                    .iter()
                    .filter(|p| p.name.to_lowercase().contains(&partial.to_lowercase()))
                    .map(|p| AutocompleteChoice {
                        name: p.name.clone(),
                        value: p.name.clone(),
                    }),
            );
        }
    }
    res
}

/// Returns the list of online users
async fn steam_id_autocomplete(ctx: Context<'_>, partial: &str) -> Vec<AutocompleteChoice<String>> {
    let mut res = vec![];
    for (_addr, server) in &ctx.data().servers {
        if let Some(state) = server.controller.write().await.status().await.ok() {
            res.extend(
                state
                    .players
                    .iter()
                    .filter(|p| p.name.to_lowercase().contains(&partial.to_lowercase()))
                    .map(|p| AutocompleteChoice {
                        name: format!("{} {}", &p.name, &p.id),
                        value: p.id.clone(),
                    }),
            );
        }
    }
    res
}

/// Returns the list of connected servers
async fn servers_autocomplete(
    ctx: Context<'_>,
    partial: &str,
) -> Vec<AutocompleteChoice<SocketAddr>> {
    ctx.data()
        .servers
        .iter()
        .filter(|(_addr, s)| s.name.to_lowercase().contains(&partial.to_lowercase()))
        .map(|(addr, s)| AutocompleteChoice {
            name: s.name.clone(),
            value: addr.clone(),
        })
        .collect()
}

/// Ban a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2ban(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The username to ban."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "Time to ban them for, in minutes"] minutes: u32,
    #[description = "The reason for the ban"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("undesirable".to_owned());
    rcon_and_reply(
        ctx,
        server,
        format!("sm_ban \"{}\" {} {}", username, minutes, reason),
    )
    .await
}

/// Ban a steam id from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2banid(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The steam id to ban"] id: String,
    #[description = "Time to ban them for, in minutes"] minutes: u32,
    #[description = "The reason for the ban"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("undesirable".to_owned());
    rcon_and_reply(
        ctx,
        server,
        format!("sm_addban {} {} {} ", minutes, id, reason),
    )
    .await
}

/// Unban a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2unban(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The steamid / ip to unban."] steamid: String,
    #[description = "The reason for the unban"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("chill".to_owned());
    rcon_and_reply(ctx, server, format!("sm_unban {} {}", steamid, reason)).await
}

/// Kick a user from the tf2 server
#[poise::command(slash_command)]
pub async fn tf2kick(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The username to kick."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "The reason for the kick"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("1984".to_owned());
    rcon_and_reply(ctx, server, format!("sm_kick \"{}\" {}", username, reason)).await
}

/// Mute a user's vc on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2mute(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The username to mute."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "Time to mute them for, in minutes"] minutes: Option<u32>,
    #[description = "The reason for the mute"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("1984".to_owned());
    let minutes = minutes.unwrap_or(0);
    rcon_and_reply(
        ctx,
        server,
        format!("sm_mute \"{}\" {} {}", username, minutes, reason),
    )
    .await
}

/// Unmute a user's vc on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2unmute(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The username to unmute."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "The reason for the unmute"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("vibin".to_owned());
    rcon_and_reply(
        ctx,
        server,
        format!("sm_unmute \"{}\" {}", username, reason),
    )
    .await
}

/// Gag a user's text chat on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2gag(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The username to gag."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "Time to gag them for, in minutes"] minutes: Option<u32>,
    #[description = "The reason for the gag"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("1984".to_owned());
    let minutes = minutes.unwrap_or(0);
    rcon_and_reply(
        ctx,
        server,
        format!("sm_gag \"{}\" {} {}", username, minutes, reason),
    )
    .await
}

/// Ungag a user's text chat on the tf2 server
#[poise::command(slash_command)]
pub async fn tf2ungag(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The username to gag."]
    #[autocomplete = "users_autocomplete"]
    username: String,
    #[description = "The reason for the ungag"] reason: Option<String>,
) -> Result<(), Error> {
    let reason = reason.unwrap_or("".to_owned());
    rcon_and_reply(ctx, server, format!("sm_ungag \"{}\" {}", username, reason)).await
}

fn hhmmss(duration: &Duration) -> String {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
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
        ctx.data().servers.values().collect::<Vec<&Server>>()
    };
    servers.sort_by_key(|s| &s.name);

    let show_uids = show_uids.unwrap_or(false);

    let mut output = String::new();
    for server in servers {
        let mut rcon = server.controller.write().await;
        let state = rcon.status().await?;

        let list = state
            .players
            .iter()
            .map(|p| {
                format!(
                    "{}{}",
                    p.name,
                    &if show_uids {
                        " ".to_owned() + &p.id
                    } else {
                        "".to_owned()
                    }
                )
            })
            .collect::<Vec<String>>();
        let longest_online = state.players.iter().max_by_key(|p| p.connected);
        output += &format!(
            "{} Currently playing: `{}`\nThere are `{}/{}` players fwagging :3.\n{}\n{}",
            server.emoji,
            state.map,
            state.players.len(),
            state.max_players,
            if let Some(longest_online) = longest_online {
                format!(
                    "Oldest player: `{}` for `{}`",
                    safe_strip(&longest_online.name),
                    hhmmss(&longest_online.connected)
                )
            } else {
                "".to_owned()
            },
            if !list.is_empty() {
                format!("`{}`\n", list.join(if show_uids { "\n" } else { " | " }))
            } else {
                "".to_owned()
            }
        );
    }
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

/// Add the given user to the secret channels
#[poise::command(context_menu_command = "Add to priv", slash_command)]
pub async fn private_add(ctx: Context<'_>, user: serenity::User) -> Result<(), Error> {
    // add perm
    let perms = serenity::PermissionOverwrite {
        allow: serenity::Permissions::VIEW_CHANNEL,
        deny: serenity::Permissions::empty(),
        kind: serenity::PermissionOverwriteType::Member(user.id),
    };
    let serenity::Channel::Category(cat) =
        ctx.http().get_channel(ctx.data().private_channel.0).await?
    else {
        Err("Could not get private channel".to_owned())?
    };
    if cat
        .permission_overwrites
        .iter()
        .any(|p| p.kind == serenity::PermissionOverwriteType::Member(user.id))
    {
        ctx.send(|m| {
            m.content(format!("{} is already added.", user.tag()))
                .ephemeral(true)
        })
        .await?;
        return Ok(());
    }
    cat.create_permission(ctx, &perms).await?;

    // send confirm message
    ctx.send(|m| {
        m.content(format!("Added {} to private channels", user.tag()))
            .ephemeral(true)
    })
    .await?;

    // send welcome message
    let welcome = ctx.data().private_welcome_channel;
    let name = user
        .nick_in(ctx, ctx.data().guild_id)
        .await
        .unwrap_or(user.tag());
    welcome
        .send_message(ctx, |m| {
            m.embed(|e| {
                e.color(serenity::Color::MEIBE_PINK)
                    .title(format!("Added {}", name))
                    .description("haiiiii ^_^ hi!! hiiiiii <3 haiiiiii hii :3")
                    .footer(|f| f.text("check pinned for info :3"))
                    .thumbnail(user.avatar_url().unwrap_or(user.default_avatar_url()))
            })
            .content(format!("<@{}>", user.id))
        })
        .await?;
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
		"https://tenor.com/view/cat-kitten-spilling-milk-milk-spilling-gif-25553835",
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
