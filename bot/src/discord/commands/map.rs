use std::borrow::Cow;

use crate::discord::Context;
use common::Error;
use tf2::Server;

use poise;
use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::CreateAttachment;

/// mapcycle.txt related configuration
#[poise::command(slash_command, subcommands("add", "rm", "list"), subcommand_required)]
pub async fn map(_: Context<'_>) -> Result<(), Error> {
    Ok(()) // never run
}

/// adds a map to the mapcycle.txt of all servers
#[poise::command(slash_command)]
async fn add(ctx: Context<'_>, #[description = "The map to add"] map: String) -> Result<(), Error> {
    ctx.defer().await?;
    let servers: Vec<Server> = ctx
        .data()
        .servers
        .values()
        .filter(|s| s.control_mapfile)
        .cloned()
        .collect();
    let server = servers.first().ok_or("No servers")?;
    let mut maps: Vec<String> = server.maps().await?;
    maps.push(map.clone());
    maps.sort_by(|a, b| {
        let a = a.strip_prefix("workshop/").unwrap_or(a);
        let b = b.strip_prefix("workshop/").unwrap_or(b);
        a.cmp(b)
    });
    maps.dedup();

    for server in servers {
        server
            .files
            .upload_file("tf/cfg/mapcycle.txt", maps.join("\n").as_bytes())
            .await?;
    }
    ctx.say(format!("Added map `{map}`")).await?;
    Ok(())
}

/// removes a map from the mapcycle.txt of all servers
#[poise::command(slash_command)]
async fn rm(
    ctx: Context<'_>,
    #[description = "The map to remove"] map: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let servers: Vec<Server> = ctx
        .data()
        .servers
        .values()
        .filter(|s| s.control_mapfile)
        .cloned()
        .collect();
    let server = servers.first().ok_or("No servers")?;
    let mut maps: Vec<String> = server.maps().await?;

    let Some((index, map)) = maps
        .iter()
        .cloned()
        .enumerate()
        .find(|(_, m)| m.contains(&map))
    else {
        ctx.send(CreateReply::default().content(format!("Could not find map with filter `{map}`")))
            .await?;
        return Ok(());
    };

    maps.remove(index);

    for server in servers {
        server
            .files
            .upload_file("tf/cfg/mapcycle.txt", maps.join("\n").as_bytes())
            .await?;
    }
    ctx.send(CreateReply::default().content(format!("Removed map `{}`", map)))
        .await?;
    Ok(())
}

/// lists all maps in the mapcycle.txt
#[poise::command(slash_command)]
async fn list(
    ctx: Context<'_>,
    #[description = "Match specific maps"] filter: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let server = ctx
        .data()
        .servers
        .values()
        .filter(|s| s.control_mapfile)
        .next()
        .ok_or("No servers")?;
    let maps: Vec<String> = server
        .maps()
        .await?
        .into_iter()
        .filter(|s| filter.as_ref().map(|f| s.contains(f)).unwrap_or(true))
        .collect();
    let data = Cow::Owned(maps.join("\n").as_bytes().to_vec());
    ctx.send(
        CreateReply::default().attachment(CreateAttachment::bytes(data, "message.txt".to_owned())),
    )
    .await?;
    Ok(())
}
