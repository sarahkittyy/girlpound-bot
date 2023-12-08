use std::borrow::Cow;
use std::io::Cursor;
use std::str::from_utf8;

use crate::discord::Context;
use crate::Error;

use crate::ftp::*;

use poise;
use poise::serenity_prelude as serenity;

/// mapcycle.txt related configuration
#[poise::command(slash_command, subcommands("add", "rm", "list"), subcommand_required)]
pub async fn map(_: Context<'_>) -> Result<(), Error> {
    Ok(()) // never run
}

/// adds a map to the mapcycle.txt of all servers
#[poise::command(slash_command)]
async fn add(ctx: Context<'_>, #[description = "The map to add"] map: String) -> Result<(), Error> {
    let server = ctx.data().servers.values().next().ok_or("No servers")?;
    let mapcyclefile = server
        .controller
        .write()
        .await
        .convar("mapcyclefile")
        .await?;
    let mut maps: Vec<String> = fetch_file(server, &format!("tf/cfg/{}", mapcyclefile))
        .await?
        .split(|&c| c == b'\n')
        .map(|v| String::from_utf8_lossy(v).trim().to_owned())
        .collect();
    maps.push(map.clone());
    maps.sort();
    maps.dedup();

    for (_addr, server) in &ctx.data().servers {
        upload_file(
            server,
            &format!("tf/cfg/{}", mapcyclefile),
            maps.join("\n").as_bytes(),
        )
        .await?;
    }
    ctx.say(":white_check_mark:").await?;
    Ok(())
}

/// removes a map from the mapcycle.txt of all servers
#[poise::command(slash_command)]
async fn rm(
    ctx: Context<'_>,
    #[description = "The map to remove"] map: String,
) -> Result<(), Error> {
    let server = ctx.data().servers.values().next().ok_or("No servers")?;
    let mapcyclefile = server
        .controller
        .write()
        .await
        .convar("mapcyclefile")
        .await?;
    let maps: Vec<String> = fetch_file(server, &format!("tf/cfg/{}", mapcyclefile))
        .await?
        .split(|&c| c == b'\n')
        .map(|v| String::from_utf8_lossy(v).trim().to_owned())
        .filter(|s| s != &map)
        .collect();

    for (_addr, server) in &ctx.data().servers {
        upload_file(
            server,
            &format!("tf/cfg/{}", mapcyclefile),
            maps.join("\n").as_bytes(),
        )
        .await?;
    }
    ctx.say(":white_check_mark:").await?;
    Ok(())
}

/// lists all maps in the mapcycle.txt
#[poise::command(slash_command)]
async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let server = ctx.data().servers.values().next().ok_or("No servers")?;
    let mapcyclefile = server
        .controller
        .write()
        .await
        .convar("mapcyclefile")
        .await?;
    let data = server
        .ftp
        .write()
        .await
        .simple_retr(&format!("tf/cfg/{}", mapcyclefile))?;
    let data = data.into_inner();
    let file = Cow::Borrowed(data.as_slice());
    ctx.send(|f| {
        f.attachment(serenity::AttachmentType::Bytes {
            data: file,
            filename: "message.txt".to_owned(),
        })
    })
    .await?;
    Ok(())
}
