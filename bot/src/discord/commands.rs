use std::collections::HashMap;
use std::env;
use std::time::Duration;

use super::{Context, PoiseData};

use ::catcoin::inventory::{claim_old_pull, CatcoinPullMessageData};
use ::profile::{get_user_profile, get_user_profiles, UserProfile};
use common::{
    util::{get_bit, remove_backticks},
    Error,
};
use steam::SteamIDProfile;
use tf2::{rcon_user_output, Server, TF2Class};

pub mod util;
use futures::StreamExt;
use tokio::{io::AsyncWriteExt, sync::mpsc};
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

mod catcoin;
pub use catcoin::*;

mod seederboard;
pub use seederboard::*;

mod profile;
pub use profile::*;

mod stocks;
pub use stocks::*;

use stats::psychostats;

use poise::serenity_prelude::{
    self as serenity, ButtonStyle, ChannelType, ComponentInteractionCollector, CreateActionRow,
    CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, Mentionable,
    Timestamp,
};
use poise::{self, CreateReply};
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
    stocks,
    bhop,
    profile,
    delete_server,
    get_profile,
    link,
    fixpulls,
    teamcaptain,
    wacky,
    purge,
    givepro,
    stats,
    bark,
    botsay,
    emojitop,
    birthday_modal,
    pug,
    donate,
    //dihh,
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
    spawn_duel,
    spawn_poker,
    tf2banid,
    tf2banraw,
    tf2unban,
    tf2kick,
    tf2kickraw,
    tf2mute,
    tf2unmute,
    tf2gag,
    tf2ungag,
    get_videos,
    pingpugs,
];

/// ping pugs
#[poise::command(slash_command, global_cooldown = 5, user_cooldown = 300)]
pub async fn pingpugs(
    ctx: Context<'_>,
    #[description = "Optional message to attach"] message: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    ctx.send(
        CreateReply::default()
            .content(format!(
                "{}<@&{}>\nraowquested by: {}",
                if let Some(msg) = message {
                    remove_backticks(&(msg + "\n"))
                } else {
                    "".to_owned()
                },
                ctx.data().scrim_role.get(),
                ctx.author().mention()
            ))
            .allowed_mentions(
                CreateAllowedMentions::new()
                    .all_users(true)
                    .roles(vec![ctx.data().scrim_role.get()]),
            ),
    )
    .await?;
    Ok(())
}

/// dihh
#[poise::command(prefix_command, discard_spare_arguments)]
pub async fn dihh(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(member) = ctx.author_member().await {
        // update count
        if let Err(e) = sqlx::query!(
            r#"
		INSERT INTO dihh (uid, count) VALUES (?, 1)
		ON DUPLICATE KEY UPDATE count = count + 1
		"#,
            member.user.id.to_string()
        )
        .execute(&ctx.data().local_pool)
        .await
        {
            log::error!("failed to dihh db upsert: {}", e);
            return Ok(());
        };

        // fetch count
        let Ok(result) = sqlx::query!(
            "SELECT count FROM dihh WHERE uid = ?",
            member.user.id.to_string()
        )
        .fetch_one(&ctx.data().local_pool)
        .await
        else {
            log::error!("failed to dihh db get");
            return Ok(());
        };

        let mut member = member.into_owned();
        if let Err(e) = member
            .disable_communication_until_datetime(
                ctx,
                Timestamp::from(
                    chrono::Utc::now()
                        + Duration::from_secs(2u64.pow(result.count as u32).min(2160000)),
                ),
            )
            .await
        {
            log::error!("failed to dihh: {}", e);
        }
    }
    Ok(())
}

/// Forcefully spawn a poker duel lobby

#[poise::command(slash_command)]
pub async fn spawn_poker(
    ctx: Context<'_>,
    #[description = "The amount of catcoins to wager"] wager: Option<u64>,
) -> Result<(), Error> {
    let wager = wager.unwrap_or(25);
    ctx.send(CreateReply::default().content("Sent.").ephemeral(true))
        .await?;
    ::cardgames::create_poker_game(
        ctx.serenity_context(),
        &ctx.data().local_pool,
        ctx.channel_id(),
        ctx.id(),
        wager,
    )
    .await?;
    Ok(())
}

/// Forcefully spawn a catcoin duel lobby
#[poise::command(slash_command)]
pub async fn spawn_duel(
    ctx: Context<'_>,
    #[description = "The amount of catcoins to wager"] wager: Option<u64>,
) -> Result<(), Error> {
    let wager = wager.unwrap_or(25);
    ctx.send(CreateReply::default().content("Sent.").ephemeral(true))
        .await?;
    ::catcoin::duels::spawn_duel(
        ctx.serenity_context(),
        &ctx.data().local_pool,
        ctx.channel_id(),
        wager,
    )
    .await?;
    Ok(())
}

/// download videos
#[poise::command(slash_command)]
pub async fn get_videos(ctx: Context<'_>) -> Result<(), Error> {
    ctx.reply("running...").await?;
    let mut messages = ctx.channel_id().messages_iter(&ctx).boxed();
    while let Some(msg_result) = messages.next().await {
        match msg_result {
            Ok(msg) => {
                let Some(attachment) = msg.attachments.first() else {
                    continue;
                };
                if !attachment
                    .content_type
                    .as_ref()
                    .is_some_and(|c| c.starts_with("video/"))
                {
                    continue;
                }
                let user = msg.author_nick(ctx).await.unwrap_or(msg.author.name);
                let Ok(resp) = reqwest::get(&attachment.url).await else {
                    eprintln!("dl error for {user}, continuing");
                    continue;
                };
                let Ok(mut file) =
                    tokio::fs::File::create(format!("{}_{}", user, attachment.filename)).await
                else {
                    eprintln!("could not make file");
                    continue;
                };
                let Ok(bytes) = resp.bytes().await else {
                    eprintln!("could not get bytes");
                    continue;
                };
                file.write_all(&bytes).await?;
                file.flush().await?;
            }
            Err(e) => eprintln!("message get failed: {e}"),
        }
    }
    println!("done");
    Ok(())
}

/// If over 80% of the server votes yes, delete the server.
#[poise::command(slash_command)]
pub async fn delete_server(ctx: Context<'_>) -> Result<(), Error> {
    let Some(total_members) = ctx
        .data()
        .guild_id
        .to_guild_cached(&ctx)
        .map(|v| v.member_count)
    else {
        ctx.send(
            CreateReply::default()
                .content("Error fetching guild.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    };
    let yes_votes =
        sqlx::query!("SELECT COUNT(*) as yes_votes FROM `delete_server` WHERE `vote` = true")
            .fetch_one(&ctx.data().local_pool)
            .await?
            .yes_votes;

    let uid = ctx.author().id;

    let new_vote = true;

    let required_votes = (total_members as f32 * 0.80).ceil();

    sqlx::query!("INSERT INTO `delete_server` (`uid`, `vote`) VALUES (?, ?) ON DUPLICATE KEY UPDATE `vote` = ?", uid.to_string(), new_vote, new_vote).execute(&ctx.data().local_pool).await?;

    if new_vote == true {
        ctx.send(CreateReply::default().content(format!(
            "{:.0} left. ({:.2}%). #DELETETKGP",
            required_votes as i64 - (yes_votes + 1),
            (yes_votes as f32 + 1.0) / required_votes * 100.0
        )))
        .await?;
    } else {
        ctx.send(CreateReply::default().content(format!(
            "{:.0} left. ({:.2}%) #DELETETKGP",
            required_votes as i64 - (yes_votes - 1),
            (yes_votes as f32 - 1.0) / required_votes * 100.0
        )))
        .await?;
    }

    Ok(())
}

/// Purge a user's messages
#[poise::command(slash_command)]
pub async fn purge(
    ctx: Context<'_>,
    #[description = "The user ID to purge"] user: serenity::UserId,
    #[description = "Messages to go back"] limit: Option<usize>,
) -> Result<(), Error> {
    ctx.send(
        CreateReply::default()
            .content(format!("Deleting <@{}>'s messages", user.get()))
            .ephemeral(true),
    )
    .await?;
    let channel = ctx.channel_id();
    let limit = limit.unwrap_or(100);
    let mut msgs = channel.messages_iter(&ctx).boxed();
    let mut count = 0;
    while let Some(message) = msgs.next().await {
        count += 1;
        if count > limit {
            break;
        }
        let Ok(message) = message else {
            continue;
        };
        if message.author.id == user {
            let _ = message
                .delete(&ctx)
                .await
                .inspect_err(|e| log::error!("Could not delete user's message: {:?}", e));
        }
    }
    log::info!("Deleted {count} messages from <@{}>", user.get());
    Ok(())
}

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
            class.emoji(),
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
            log::info!(
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
                        .inspect_err(|e| log::info!("claim fail: {e:?}"))
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

    log::info!(
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

/// Lookup your tkgp psychostats data
#[poise::command(slash_command, user_cooldown = 15)]
pub async fn stats(
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
    server: String,
    #[description = "The player cap 24 <= p <= 32"] count: i32,
) -> Result<(), Error> {
    ctx.defer().await?;
    let re = Regex::new(r#""maxplayers" is "(\d+)""#).unwrap();

    let server = ctx.data().server(&server)?;

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
        .files
        .add_or_edit_line("tf/cfg/server.cfg", "sm_reserved_slots", &rs)
        .await?;
    server
        .files
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
    server: Option<String>,
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
    server: String,
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
    server: String,
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
    let server = ctx.data().server(&server)?;
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
    server: Option<String>,
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
    server: String,
    #[description = "Optional message to attach"] message: Option<String>,
) -> Result<(), Error> {
    // check cooldown
    match ctx.data().can_seed(ctx.data().server(&server)?.addr).await {
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

    let server = ctx.data().server(&server)?;
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
                status.as_discord_output(&server.emoji, false),
            ))
            .allowed_mentions(CreateAllowedMentions::new().roles(vec![seeder_role.get()])),
    )
    .await?;
    // reset cooldown
    ctx.data().reset_seed_cooldown(server.addr).await;

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
    server: Option<String>,
    #[description = "Display user IDs?"] show_uids: Option<bool>,
) -> Result<(), Error> {
    // get all the servers to include in the result
    let mut servers = if let Some(server) = server {
        vec![ctx.data().server(&server)?]
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
    let (tx, rx) = mpsc::channel(100);
    for server in servers {
        let server = server.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut rcon = server.controller.write().await;
            let Ok(state) = rcon.status().await else {
                return;
            };
            let _ = tx.send((state, server.emoji)).await;
        });
    }
    drop(tx);

    let mut res = common::util::recv_timeout(rx, Duration::from_millis(2500)).await;
    res.sort_by(|(_, a), (_, b)| a.cmp(b));
    for (state, emoji) in res {
        output += &state.as_discord_output(&emoji, show_uids);
    }

    // delete last status msg
    let msgs = ctx
        .channel_id()
        .messages(ctx.http(), GetMessages::new().limit(30))
        .await?;
    let bid = ctx.cache().current_user().id;
    for msg in &msgs {
        if msg.author.id == bid
            && (msg.content.starts_with("🅰️")
                || msg.content.starts_with("🅱️")
                || msg.content.starts_with("💀"))
        {
            let _ = msg
                .delete(ctx.http())
                .await
                .inspect_err(|e| eprintln!("could not delete old status msg: {e}"));
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
    ctx.defer().await?;
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
                log::info!("Error fetching users: {:?}", e);
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
