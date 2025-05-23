use std::collections::HashMap;

use crate::discord::Context;
use common::{util::hhmmss, Error};
use poise::{
    self,
    serenity_prelude::{Color, CreateEmbed, CreateEmbedFooter},
    CreateReply,
};

/// Fetch the top tkgp seeders.
#[poise::command(slash_command, global_cooldown = 5)]
pub async fn seederboard(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let top_seeders =
        sqlx::query!("SELECT * FROM `seederboard` ORDER BY `seconds_seeded` DESC LIMIT 10")
            .fetch_all(&ctx.data().local_pool)
            .await
            .inspect_err(|e| log::error!("error in seederboard query: {e:?}"))?;

    let comma_separated_steamids = top_seeders
        .iter()
        .map(|s| s.steamid.clone())
        .collect::<Vec<String>>()
        .join(",");

    // convert steamids
    let id3_to_id64: HashMap<String, String> = ctx
        .data()
        .steamid_client
        .lookup(&comma_separated_steamids)
        .await
        .inspect_err(|e| log::error!("error in steamid lookup: {e:?}"))?
        .into_iter()
        .map(|profile| (profile.steam3, profile.steamid64))
        .collect();

    let comma_separated_steamid64s = top_seeders
        .iter()
        .flat_map(|seeder| id3_to_id64.get(&seeder.steamid).cloned())
        .collect::<Vec<String>>()
        .join(",");

    // fetch profiles
    let profiles = ctx
        .data()
        .steamid_client
        .get_player_summaries(&comma_separated_steamid64s)
        .await
        .inspect_err(|e| log::error!("error in summaries: {e:?}"))?
        .into_iter()
        .map(|summary| (summary.steamid.clone(), summary))
        .collect::<HashMap<_, _>>();

    let leaderboard: String = top_seeders
        .iter()
        .enumerate()
        .flat_map(|(i, seeder)| -> Option<String> {
            let id64 = id3_to_id64.get(&seeder.steamid)?;
            let profile = profiles.get(id64)?;
            let total_s = seeder.seconds_seeded;
            Some(format!(
                "{}. `{}` - `{}`",
                i + 1,
                profile.personaname,
                hhmmss(total_s.try_into().ok()?)
            ))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let embed = CreateEmbed::new()
        .title("Top seeders <3")
        .description(leaderboard)
        .footer(CreateEmbedFooter::new(
            "Counts time played on TKGP with <=12 online.",
        ))
        .color(Color::DARK_RED);

    ctx.send(CreateReply::default().embed(embed))
        .await
        .inspect_err(|e| log::error!("error in send reply: {e:?}"))?;

    Ok(())
}
