use std::borrow::Cow;

use crate::discord::Context;
use crate::Error;

use poise;
use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::CreateAttachment;

/// Edit the wacky map wednesday pool
#[poise::command(slash_command, subcommands("add", "rm", "list"), subcommand_required)]
pub async fn wacky(_: Context<'_>) -> Result<(), Error> {
    Ok(()) // never run
}

/// adds a map to the mapcycle-wacky.txt of all servers
#[poise::command(slash_command)]
async fn add(ctx: Context<'_>, #[description = "The map to add"] map: String) -> Result<(), Error> {
    let server = ctx.data().wacky_server()?;
    let mut maps: Vec<String> = server.wacky_maps().await?;
    maps.push(map.clone());
    maps.sort_by(|a, b| {
        let a = a.strip_prefix("workshop/").unwrap_or(a);
        let b = b.strip_prefix("workshop/").unwrap_or(b);
        a.cmp(b)
    });
    maps.dedup();

    server
        .ftp
        .upload_file("tf/cfg/mapcycle-wacky.txt", maps.join("\n").as_bytes())
        .await?;
    ctx.say(format!("Added map `{}`", map)).await?;
    Ok(())
}

/// removes a map from the mapcycle-wacky.txt of all servers
#[poise::command(slash_command)]
async fn rm(
    ctx: Context<'_>,
    #[description = "The map to remove"] map: String,
) -> Result<(), Error> {
    let server = ctx.data().wacky_server()?;
    let mut maps: Vec<String> = server.wacky_maps().await?;

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

    server
        .ftp
        .upload_file("tf/cfg/mapcycle-wacky.txt", maps.join("\n").as_bytes())
        .await?;
    ctx.send(CreateReply::default().content(format!("Removed map `{}`", map)))
        .await?;
    Ok(())
}

/// lists all maps in the mapcycle-wacky.txt
#[poise::command(slash_command)]
async fn list(
    ctx: Context<'_>,
    #[description = "Match specific maps"] filter: Option<String>,
) -> Result<(), Error> {
    let server = ctx.data().wacky_server()?;
    let maps: Vec<String> = server
        .wacky_maps()
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
