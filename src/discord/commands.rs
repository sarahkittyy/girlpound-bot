use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use super::Context;
use crate::logs::safe_strip;
use crate::Error;

use poise::serenity_prelude::{self as serenity};
use poise::{self, AutocompleteChoice};
use rand::prelude::*;

pub async fn rcon_and_reply(
    ctx: Context<'_>,
    server: SocketAddr,
    cmd: String,
) -> Result<(), Error> {
    let mut rcon = ctx.data().server(server)?.controller.write().await;
    let reply = rcon.run(&cmd).await;
    match reply {
        Ok(output) => {
            if output.len() == 0 {
                ctx.say(":white_check_mark:").await
            } else {
                ctx.say(format!("```\n{}\n```", output)).await
            }
        }
        Err(e) => ctx.say(format!("RCON error: {:?}", e)).await,
    }?;
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

/// Sends an RCON command to the server.
#[poise::command(slash_command)]
pub async fn rcon(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The command to send."] cmd: String,
) -> Result<(), Error> {
    rcon_and_reply(ctx, server, cmd).await
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

/// Displays all stats for modding easily
#[poise::command(slash_command)]
pub async fn info(ctx: Context<'_>) -> Result<(), Error> {
    let mut msg = String::new();

    let servers = &ctx.data().servers;
    for (_addr, server) in servers.iter() {
        let mut rcon = server.controller.write().await;
        let state = rcon.status().await?;
        msg += &format!(
            "{} (`{}`) {}/{} on `{}`\n",
            server.emoji,
            server.addr,
            state.players.len(),
            state.max_players,
            state.map
        );
        for player in &state.players {
            msg += &format!("`{} {}`\n", safe_strip(&player.name), player.id);
        }
    }
    ctx.send(|m| m.content(msg).ephemeral(true)).await?;
    Ok(())
}

/// Displays current server player count & map.
#[poise::command(slash_command)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
) -> Result<(), Error> {
    let server = ctx.data().server(server)?;
    let mut rcon = server.controller.write().await;
    let state = rcon.status().await?;

    use crate::logs::safe_strip;

    println!("state: {:?}", state);

    let list = state
        .players
        .iter()
        .map(|p| safe_strip(&p.name))
        .collect::<Vec<String>>()
        .join(", ");
    let longest_online = state.players.iter().max_by_key(|p| p.connected);
    ctx.say(format!(
        "`{}` Currently playing: `{}`\nThere are `{}/{}` players fwagging :3.\n{}\n{}",
        server.name,
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
            format!("`{}`", list)
        } else {
            "".to_owned()
        }
    ))
    .await?;
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
    ];
    let r = (random::<f32>() * meows.len() as f32).floor() as usize;

    poise::send_reply(ctx, |message| message.content(meows[r])).await?;
    Ok(())
}
