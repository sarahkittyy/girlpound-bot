use chrono::{Duration, TimeDelta, Utc};
use poise::serenity_prelude as serenity;
use serenity::{CreateMessage, Member, Mentionable};

use crate::Error;

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
                    let total_s = since_join.num_seconds();
                    let s = total_s % 60;
                    let m = (total_s / 60) % 60;
                    let h = (total_s / 60) / 60;
                    let resp = format!(
                            "{} has assigned themselves the NSFW role. Time since joining: `{:0>2}:{:0>2}:{:0>2}`",
                            new.mention(),
                            h, m, s
                        );
                    data.general_channel
                        .send_message(&ctx, CreateMessage::new().content(resp))
                        .await?;
                }
            }
        }
    }
    Ok(())
}
