use catcoin::grant_catcoin;
use chrono::{Duration, TimeDelta, Utc};
use emoji::emoji;
use poise::serenity_prelude as serenity;
use serenity::{CreateMessage, Member, Mentionable};

use crate::{Error, util::hhmmss};

use super::PoiseData;

/// Checks if the user has received the nsfw role maximum 1 hour since joining, and if so, posts about it in gen.
pub async fn try_callout_nsfw_role(
    ctx: &serenity::Context,
    data: &PoiseData,
    old: &Option<Member>,
    new: &Option<Member>,
) -> Result<(), Error> {
    if let Some(old) = old {
        if let Some(new) = new {
            if let Some(joined_at) = new.joined_at {
                let since_join: Duration = joined_at.signed_duration_since(Utc::now()).abs();
                if !old.roles.contains(&data.horny_role)
                    && new.roles.contains(&data.horny_role)
                    && since_join <= TimeDelta::try_hours(1).unwrap()
                    && data.horny_callouts.write().await.insert(new.user.id.get())
                {
                    let total_s = since_join.num_seconds() as u64;
                    // Handle NSFW role assignment
                    let resp = if let Ok(Some(result)) = data.nsfwbets
                        .write()
                        .await
                        .on_nsfw_role_assigned(ctx, new.user.id, total_s, &data.local_pool)
                        .await
                    {
                        format!(
                            "{} has assigned themselves the NSFW role. Time since joining: `{}`\nClosest guess: `{}` by {} ({} **+{}**)",
                            new.mention(),
                            hhmmss(total_s.try_into()?),
                            hhmmss(result.winner_guess),
                            result.winner_id.mention(),
                            emoji("catcoin"),
                            result.pool_coin
                        )
                    } else {
                        format!(
                            "{} has assigned themselves the NSFW role. Time since joining: `{}`",
                            new.mention(),
                            hhmmss(total_s.try_into()?)
                        )
                    };

                    let _resp = data
                        .general_channel
                        .send_message(&ctx, CreateMessage::new().content(resp))
                        .await?;
                }
            }
        }
    }
    Ok(())
}
