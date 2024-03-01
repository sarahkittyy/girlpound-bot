use crate::discord::Context;
use crate::Error;

use poise;
use poise::CreateReply;

use super::util::*;

/// Control the pug / scrim server (tkgp6)
#[poise::command(slash_command)]
pub async fn pug(
    ctx: Context<'_>,
    #[description = "The map to use."]
    #[autocomplete = "pug_maps_autocomplete"]
    map: String,
    #[description = "The config file name to use (rgl_off to disable)."]
    #[autocomplete = "pug_cfgs_autocomplete"]
    cfg: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    if !ctx.data().pug_cfgs.contains(&cfg) {
        ctx.send(CreateReply::default().content("That cfg file does not exist!"))
            .await?;
        return Ok(());
    }
    let ps = ctx.data().pug_server()?;
    if !ps.maps().await?.contains(&map) {
        ctx.send(CreateReply::default().content("That map does not exist!"))
            .await?;
        return Ok(());
    }

    let mut rcon = ps.controller.write().await;
    let response: String = match rcon.run(&format!("exec {cfg}; changelevel {map}")).await {
        Ok(_) => format!("Ran `{cfg}.cfg` and changed to map `{map}`."),
        Err(e) => format!("Error: {e}"),
    };
    ctx.send(CreateReply::default().content(response)).await?;
    Ok(())
}
