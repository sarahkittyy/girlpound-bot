use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use super::{Context, PoiseData};
use crate::catcoin::inv::{claim_old_pull, CatcoinPullMessageData};
use crate::logs::remove_backticks;
use crate::profile::command::{get_profile, link, profile};
use crate::profile::{get_user_profile, get_user_profiles, UserProfile};
use crate::seederboard::command::seederboard;
use crate::steamid::SteamIDProfile;
use crate::tf2class::TF2Class;
use crate::util::get_bit;
use crate::{Error, Server};

pub mod util;
use futures::StreamExt;
use util::*;

mod map;
pub use map::map;

mod wacky;
pub use wacky::*;

mod mods;
pub use mods::*;

mod pug;
pub use pug::*;

pub mod birthday_check;
pub use birthday_check::*;

mod botsay;
pub use botsay::*;

mod bibleverse;
pub use bibleverse::*;

mod emojitop;
pub use emojitop::*;

mod remindme;
pub use remindme::*;

mod teamcaptain;
pub use teamcaptain::*;

use crate::catcoin::command::catcoin;
use crate::psychostats;

use poise;
use poise::serenity_prelude::{
    self as serenity, ButtonStyle, ChannelType, ComponentInteractionCollector, CreateActionRow,
    CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, Mentionable,
};
use poise::CreateReply;
use serenity::{CreateAllowedMentions, CreateEmbed, CreateEmbedFooter, CreateMessage, GetMessages};

use rand::prelude::*;
use regex::Regex;

pub static ALL: &[fn() -> poise::Command<PoiseData, Error>] = &[
    bibleverse,
    classes,
    catcoin,
    || poise::Command {
        slash_action: remindme_slash().slash_action,
        ..remindme()
    },
    seederboard,
    bhop,
    profile,
    get_profile,
    link,
    fixpulls,
    teamcaptain,
    wacky,
    givepro,
    psychostats,
    bark,
    botsay,
    emojitop,
    birthday_modal,
    pug,
    donate,
    rcon,
    snipers,
    seeder,
    respawntimes,
    playercap,
    meow,
    map,
    status,
    lookup,
    reacted_users,
    feedback,
    tf2ban,
    tf2banid,
    tf2unban,
    tf2kick,
    tf2mute,
    tf2unmute,
    tf2gag,
    tf2ungag,
];

async fn generate_classes_embed(
    ctx: &Context<'_>,
    members: &Vec<serenity::Member>,
) -> Result<CreateEmbed, Error> {
    let profiles = get_user_profiles(
        &ctx.data().local_pool,
        members.iter().map(|m| m.user.id).collect(),
    )
    .await?;
    let member_profiles: HashMap<serenity::UserId, (serenity::Member, Option<UserProfile>)> =
        members
            .iter()
            .map(|m| {
                let profile = profiles
                    .iter()
                    .find(|profile| profile.uid.parse::<u64>().unwrap() == m.user.id.get());
                (m.user.id, (m.clone(), profile.cloned()))
            })
            .collect();

    let classes = vec![
        TF2Class::Scout,
        TF2Class::Soldier,
        TF2Class::Demo,
        TF2Class::Medic,
    ];

    let mut embed = CreateEmbed::new().title("Class availability");

    for class in &classes {
        let bitno = class.as_number();
        let class_players: Vec<(&serenity::Member, &UserProfile)> = member_profiles
            .values()
            .filter_map(|(member, profile)| {
                let Some(profile) = profile else {
                    return None;
                };
                if get_bit(profile.classes, bitno) {
                    Some((member, profile))
                } else {
                    None
                }
            })
            .collect();
        embed = embed.field(
            ctx.data().class_emojis.get(class).unwrap(),
            class_players
                .iter()
                .map(|(member, _)| member.mention().to_string())
                .collect::<Vec<String>>()
                .join("\n"),
            true,
        );
    }

    let baiters: Vec<&serenity::Member> = member_profiles
        .values()
        .filter_map(|(m, p)| if p.is_none() { Some(m) } else { None })
        .collect();

    let unmatched: Vec<String> = member_profiles
        .values()
        .filter_map(|(member, profile)| {
            let Some(profile) = profile else {
                return Some(member.mention().to_string());
            };
            // if all necessary class bits are false, this user is unmatched
            if !classes
                .iter()
                .all(|class| get_bit(profile.classes, class.as_number()) == false)
            {
                None
            } else {
                Some(member.mention().to_string())
            }
        })
        .collect();
    if unmatched.len() > 0 {
        embed = embed.field(
            "None of the above",
            format!("{}", unmatched.join("\n")),
            true,
        );
    }
    if baiters.len() > 0 {
        embed = embed.field(
            "No /profile",
            baiters
                .iter()
                .map(|m| m.display_name().to_string())
                .collect::<Vec<String>>()
                .join("\n"),
            true,
        );
    }

    Ok(embed)
}

/// Scour the channel for pulls
#[poise::command(slash_command, global_cooldown = 5)]
pub async fn fixpulls(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(
        CreateReply::default()
            .ephemeral(true)
            .content("Beginning scan..."),
    )
    .await?;

    let mut iter = ctx.channel_id().messages_iter(ctx).boxed();
    let mut count: usize = 0;

    let mut existing: usize = 0;
    let mut new: usize = 0;
    while let Some(next) = iter.next().await {
        count += 1;
        if count % 100 == 0 {
            println!(
                "Scanning pulls in channel <#{}>: {} done. ({} new, {} existing)",
                ctx.channel_id(),
                count,
                new,
                existing
            );
        }
        match next {
            Ok(msg) if msg.author.id == ctx.framework().bot_id => {
                if let Ok(pull) = CatcoinPullMessageData::try_from(msg) {
                    let _ = claim_old_pull(&ctx.data().local_pool, &pull)
                        .await
                        .inspect_err(|e| println!("claim fail: {e:?}"))
                        .inspect(|inserted| {
                            if *inserted {
                                new += 1;
                            } else {
                                existing += 1;
                            }
                        });
                }
            }
            _ => (),
        }
    }

    println!(
        "Done scanning <#{}>: {} done. ({} new, {} existing)",
        ctx.channel_id(),
        count,
        new,
        existing
    );

    Ok(())
}

/// View everyone's preferred classes from a voice chat.
#[poise::command(slash_command, global_cooldown = 5)]
pub async fn classes(
    ctx: Context<'_>,
    #[description = "The voice channel with users"] channel: serenity::GuildChannel,
) -> Result<(), Error> {
    ctx.defer().await?;
    let uuid = ctx.id();

    if !matches!(channel.kind, ChannelType::Voice) {
        ctx.reply(format!("Channel {} is not a voice channel!", channel.name))
            .await?;
        return Ok(());
    }
    let members = channel.members(ctx)?;

    let mut embed = generate_classes_embed(&ctx, &members).await?;

    let mut reply = CreateReply::default().embed(embed);

    let reload_id = format!("{uuid}-reload");

    let components = vec![CreateActionRow::Buttons(vec![
        CreateButton::new("profile.edit.classes")
            .label("Edit Classes")
            .emoji('🔫'),
        CreateButton::new(reload_id.clone())
            .style(ButtonStyle::Secondary)
            .emoji('🔃'),
    ])];

    reply = reply.components(components);

    ctx.send(reply).await?;

    while let Some(mci) = ComponentInteractionCollector::new(ctx)
        .channel_id(ctx.channel_id())
        .timeout(Duration::from_secs(200))
        .filter(move |mci| mci.data.custom_id.starts_with(&uuid.to_string()))
        .await
    {
        if mci.data.custom_id != reload_id {
            continue;
        }
        embed = generate_classes_embed(&ctx, &members).await?;
        mci.create_response(
            &ctx,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new().embed(embed),
            ),
        )
        .await?;
    }

    Ok(())
}

/// Toggle the pro role on a user
#[poise::command(
    slash_command,
    context_menu_command = "Toggle pro/scrim role.",
    global_cooldown = 5
)]
pub async fn givepro(
    ctx: Context<'_>,
    #[description = "The user to toggle"] user: serenity::User,
) -> Result<(), Error> {
    let Some(guild) = ctx.guild_id() else {
        return Ok(());
    };
    let member = guild.member(&ctx, user.id).await?;
    let pro_role = ctx.data().scrim_role;
    let no_mention = CreateAllowedMentions::new().empty_roles().empty_users();

    if member.roles.contains(&pro_role) {
        member.remove_role(&ctx, pro_role).await?;
        ctx.send(
            CreateReply::default()
                .content(format!(
                    "Removed <@&{}> from user {}",
                    pro_role.get(),
                    member.display_name()
                ))
                .allowed_mentions(no_mention)
                .ephemeral(true),
        )
        .await?;
    } else {
        member.add_role(&ctx, pro_role).await?;
        ctx.send(
            CreateReply::default()
                .content(format!(
                    "Gave <@&{}> to user {}",
                    pro_role.get(),
                    member.display_name()
                ))
                .allowed_mentions(no_mention)
                .ephemeral(true),
        )
        .await?;
    }

    Ok(())
}

/// Lookup your LEGACY tkgp psychostats (deprecated 6/18/24 in favor of gameME -> /profile)
#[poise::command(slash_command, user_cooldown = 15)]
pub async fn psychostats(
    ctx: Context<'_>,
    #[description = "Steam profile url, eg. https://steamcommunity.com/id/sarahkitty/"]
    profile: Option<String>,
    #[description = "Hide the resulting output"] hide_reply: Option<bool>,
) -> Result<(), Error> {
    if hide_reply.unwrap_or(true) {
        ctx.defer_ephemeral().await?;
    } else {
        ctx.defer().await?;
    };
    let profile: SteamIDProfile = match profile {
        None => {
            let profile = get_user_profile(&ctx.data().local_pool, ctx.author().id).await?;
            let Some(steamid) = profile.steamid else {
                ctx.send(CreateReply::default().content("No profile URL specified and no steam profile /link'd to this discord account.")).await?;
                return Ok(());
            };
            let profiles = ctx.data().steamid_client.lookup(&steamid).await?;
            profiles.into_iter().next().ok_or("Profile not found")?
        }
        Some(profile) => {
            let profiles = ctx.data().steamid_client.lookup(&profile).await?;
            profiles.into_iter().next().ok_or("Profile not found")?
        }
    };
    let summary = ctx
        .data()
        .steamid_client
        .get_player_summaries(&profile.steamid64)
        .await?;
    let summary = summary.first().ok_or("Profile not found.")?;
    let (tkgp4s, tkgp5s) = psychostats::find_plr(&profile.steamid).await?;

    let url4 = tkgp4s
        .map(|s| {
            format!(
                "[**#{}** _(Top {:.1}%)_]({}player.php?id={})",
                s.rank,
                s.percentile,
                psychostats::BASEURL4,
                s.id
            )
        })
        .unwrap_or("Not found.".to_owned());
    let url5 = tkgp5s
        .map(|s| {
            format!(
                "[**#{}** _(Top {:.1}%)_]({}player.php?id={})",
                s.rank,
                s.percentile,
                psychostats::BASEURL5,
                s.id
            )
        })
        .unwrap_or("Not found.".to_owned());
    let embed = CreateEmbed::new()
        .title(format!("PStats lookup for {}", summary.personaname))
        .url(&summary.profileurl)
        .thumbnail(&summary.avatarmedium)
        .footer(CreateEmbedFooter::new("Not you? DM @sarahkittyy :3"))
        .description(format!("### TKGP #4: {}\n### TKGP #5: {}", url4, url5));

    ctx.send(
        CreateReply::default()
            .embed(embed)
            .allowed_mentions(CreateAllowedMentions::new().empty_roles().empty_users()),
    )
    .await?;
    Ok(())
}

/// Sends the donation link
#[poise::command(slash_command)]
pub async fn donate(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(CreateReply::default().content("https://fluffycat.gay/donate"))
        .await?;
    Ok(())
}

/// Sets the server player limit
#[poise::command(slash_command)]
pub async fn playercap(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The player cap 24 <= p <= 32"] count: i32,
) -> Result<(), Error> {
    let re = Regex::new(r#""maxplayers" is "(\d+)""#).unwrap();

    let server = ctx.data().server(server)?;

    let min: i32 = 24;
    let max = server.controller.write().await.run("maxplayers").await?;
    let max: i32 = re
        .captures(&max)
        .and_then(|caps| caps[1].parse::<i32>().ok())
        .unwrap_or(25)
        - 1;

    let visible = count.max(min).min(max);
    let reserved = (max - visible - 1).max(0);

    let rs = format!("sm_reserved_slots {reserved}");
    let vmp = format!("sv_visiblemaxplayers {visible}");

    // update server.cfg for persistence
    server
        .ftp
        .add_or_edit_line("tf/cfg/server.cfg", "sm_reserved_slots", &rs)
        .await?;
    server
        .ftp
        .add_or_edit_line("tf/cfg/server.cfg", "sv_visiblemaxplayers", &vmp)
        .await?;

    let cmd = format!("{rs};{vmp}");
    let reply = rcon_user_output(&[server], cmd).await;
    ctx.send(CreateReply::default().content(reply)).await?;
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
    let reply = rcon_user_output(&output_servers(ctx, server)?, cmd).await;
    let hide_reply = hide_reply.unwrap_or(false);
    ctx.send(CreateReply::default().ephemeral(hide_reply).content(reply))
        .await?;
    Ok(())
}

/// Set the sniper limit on the server
#[poise::command(slash_command)]
pub async fn snipers(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "The sniper limit (-1 for enable)"] limit: i8,
    #[description = "Hide the reply?"] hide_reply: Option<bool>,
) -> Result<(), Error> {
    let cmd = format!(
        "sm_classrestrict_blu_snipers {0}; sm_classrestrict_red_snipers {0}",
        limit
    );
    let reply = rcon_user_output(&output_servers(ctx, Some(server))?, cmd).await;
    let hide_reply = hide_reply.unwrap_or(false);
    ctx.send(CreateReply::default().ephemeral(hide_reply).content(reply))
        .await?;

    Ok(())
}

/// Toggle bhop on a server (with or without autohop)
#[poise::command(slash_command)]
pub async fn bhop(
    ctx: Context<'_>,
    #[description = "The server to query"]
    #[autocomplete = "servers_autocomplete"]
    server: SocketAddr,
    #[description = "Whether you can bhop or not"] enabled: bool,
    #[description = "Allow autohop? (Default: False) (hold space vs timed jumps)"] autohop: Option<
        bool,
    >,
) -> Result<(), Error> {
    let autohop = autohop.unwrap_or(false);
    let cmd = format!(
        "cm_enabled {}; cm_allow_autohop {}",
        if enabled { 1 } else { 0 },
        if autohop { 1 } else { 0 }
    );
    let server = ctx.data().server(server)?;
    let _ = rcon_user_output(&[server], cmd).await;
    let reply = if enabled {
        format!(
            "{} `Enabled BHOP {} autohop`",
            server.emoji,
            if autohop { "with" } else { "without" }
        )
    } else {
        format!("{} `Disabled BHOP`", server.emoji)
    };
    ctx.send(CreateReply::default().content(reply)).await?;

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
        poise::send_reply(
            ctx,
            CreateReply::default()
                .ephemeral(true)
                .content("Feedback is not configured properly! Message an admin."),
        )
        .await?;
        return Ok(());
    };

    // get the owner
    let recip = serenity::UserId::new(owner_id.parse()?);
    let dm_channel = recip.create_dm_channel(ctx).await?;
    dm_channel
        .send_message(
            ctx,
            CreateMessage::default().embed({
                let mut r = CreateEmbed::new().title("anon feedback").description(msg);

                if let Some(attachment) = attachment {
                    r = r.image(attachment.url);
                }

                r
            }),
        )
        .await?;

    poise::send_reply(
        ctx,
        CreateReply::default()
            .ephemeral(true)
            .content("Feedback anonymously sent!"),
    )
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
            "mp_disable_respawn_times {}; sm_lowpop_enabled {}",
            if instant { "1" } else { "0" },
            if instant { "0" } else { "1" },
        ),
    };
    let reply = rcon_user_output(&output_servers(ctx, server)?, cmd).await;
    ctx.send(CreateReply::default().content(reply)).await?;

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
            ctx.send(CreateReply::default().content(format!(
                "Server was seeded too recently. Try again <t:{}:R>",
                (now + time_left).timestamp()
            )))
            .await?;
            return Ok(());
        }
    };

    let server_addr = server;
    let server = ctx.data().server(server)?;
    if !server.allow_seed {
        ctx.send(CreateReply::default().content("This server is not seedable."))
            .await?;
        return Ok(());
    }

    let mut rcon = server.controller.write().await;
    let status = rcon.status().await?;
    let player_count = status.players.len();

    if player_count < 2 {
        ctx.send(CreateReply::default().content("Server must have >2 players to ping."))
            .await?;
        return Ok(());
    }
    if player_count >= 16 {
        ctx.send(CreateReply::default().content("Server must have <16 players to ping."))
            .await?;
        return Ok(());
    }

    let seeder_role = ctx.data().seeder_role;

    // send seed
    ctx.send(
        CreateReply::default()
            .content(format!(
                "{}<@&{}> come fwag on {} :3\nraowquested by: <@{}>\n{}",
                if let Some(msg) = message {
                    remove_backticks(&(msg + "\n"))
                } else {
                    "".to_owned()
                },
                seeder_role.get(),
                server.emoji,
                ctx.author().id,
                status.as_discord_output(server, false),
            ))
            .allowed_mentions(CreateAllowedMentions::new().roles(vec![seeder_role.get()])),
    )
    .await?;
    // reset cooldown
    ctx.data().reset_seed_cooldown(server_addr).await;

    Ok(())
}

/// SteamID.uk discord command.
#[poise::command(slash_command, global_cooldown = 10)]
pub async fn lookup(
    ctx: Context<'_>,
    #[description = "SteamID, Steam2, Steam3, or vanity URL"]
    #[autocomplete = "steam_id_autocomplete"]
    query: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let client = &ctx.data().steamid_client;
    let data = client.lookup(&query).await?;
    // fetch important info

    ctx.send({
        let mut m = CreateReply::default().content(format!("Results for query: `{}`", query));
        for user in &data {
            m = m.embed(user.to_embed());
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
        .messages(ctx.http(), GetMessages::new().limit(45))
        .await?;
    let bid = ctx.cache().current_user().id;
    for msg in &msgs {
        if msg.author.id == bid && (msg.content.starts_with("🅰️") || msg.content.starts_with("🅱️"))
        {
            msg.delete(ctx.http()).await?;
            break;
        }
    }
    // send status msg
    ctx.send(CreateReply::default().content(output).ephemeral(show_uids))
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
    let msg = ctx
        .http()
        .get_message(channel.into(), message.into())
        .await?;
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
    .execute(&ctx.data().local_pool)
    .await?;

    // fetch recent barkers
    let results = sqlx::query!(
        r#"
		SELECT `last_nickname` from `barkers`
		ORDER BY `updated_at` DESC
		LIMIT 15
	"#
    )
    .fetch_all(&ctx.data().local_pool)
    .await?;

    let user_list = results
        .iter()
        .map(|n| &n.last_nickname)
        .fold(String::new(), |acc, s| acc + s + "\n");

    let response =
        format!("Barking is strictly prohibited. Your ID has been logged.\nLast 15 infractions:```\n{user_list}```");

    ctx.send(CreateReply::default().ephemeral(true).content(response))
        .await?;

    Ok(())
}

/// Meow (suppawters only)
#[poise::command(slash_command, channel_cooldown = 7)]
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
		"omg >w< like literally,, nya !!! :3",
		"mrraow !! raow raow >w< prraow raow raow",
		"raow nya.. prraow >w<",
		"meow meow meow",
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
		"https://tenor.com/view/cat-meow-angry-pet-hiss-gif-16838272",
		"https://tenor.com/view/cat-power-cat-cat-pillow-repost-this-post-this-cat-gif-23865940",
		"https://tenor.com/view/cat-gif-7623921",
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

    poise::send_reply(ctx, CreateReply::default().content(meows[r])).await?;
    Ok(())
}
