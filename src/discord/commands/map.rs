use std::borrow::Cow;

use crate::discord::Context;
use crate::{Error, Server};

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
    let servers: Vec<Server> = ctx
        .data()
        .servers
        .values()
        .filter(|s| s.control_mapfile)
        .cloned()
        .collect();
    let server = servers.first().ok_or("No servers")?;
    let mapcyclefile = "mapcycle.txt";
    let mut maps: Vec<String> = server
        .ftp
        .fetch_file(&format!("tf/cfg/{}", mapcyclefile))
        .await?
        .split(|&c| c == b'\n')
        .map(|v| String::from_utf8_lossy(v).trim().to_owned())
        .collect();
    maps.push(map.clone());
    maps.sort_by(|a, b| {
        let a = a.strip_prefix("workshop/").unwrap_or(a);
        let b = b.strip_prefix("workshop/").unwrap_or(b);
        a.cmp(b)
    });
    maps.dedup();

    for server in servers {
        server
            .ftp
            .upload_file(
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
    let servers: Vec<Server> = ctx
        .data()
        .servers
        .values()
        .filter(|s| s.control_mapfile)
        .cloned()
        .collect();
    let server = servers.first().ok_or("No servers")?;
    let mapcyclefile = "mapcycle.txt";
    let maps: Vec<String> = server
        .ftp
        .fetch_file(&format!("tf/cfg/{}", mapcyclefile))
        .await?
        .split(|&c| c == b'\n')
        .map(|v| String::from_utf8_lossy(v).trim().to_owned())
        .filter(|s| s != &map)
        .collect();

    for server in servers {
        server
            .ftp
            .upload_file(
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
async fn list(
    ctx: Context<'_>,
    #[description = "Match specific maps"] filter: Option<String>,
) -> Result<(), Error> {
    let server = ctx
        .data()
        .servers
        .values()
        .filter(|s| s.control_mapfile)
        .next()
        .ok_or("No servers")?;
    let mapcyclefile = "mapcycle.txt";
    let maps: Vec<String> = server
        .ftp
        .fetch_file(&format!("tf/cfg/{}", mapcyclefile))
        .await?
        .split(|&c| c == b'\n')
        .map(|v| String::from_utf8_lossy(v).trim().to_owned())
        .filter(|s| filter.as_ref().map(|f| s.contains(f)).unwrap_or(true))
        .collect();
    let data = Cow::Owned(maps.join("\n").as_bytes().to_vec());
    ctx.send(|f| {
        f.attachment(serenity::AttachmentType::Bytes {
            data,
            filename: "message.txt".to_owned(),
        })
    })
    .await?;
    Ok(())
}
