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
                    // Find winner of NSFWBet
                    let mut bets = data.nsfwbets.write().await;
                    let resp = if let Some(bet) = bets.get_pool_mut(new.user.id)
                        && let Some(winner) = bet.get_winner(total_s)
                    {
                        // total catcoin
                        let pool_coin: u64 = bet.wager * bet.bets.iter().len() as u64;

                        // grant winner catcoin
                        grant_catcoin(&data.local_pool, winner.0, pool_coin).await?;

                        // get response data
                        let resp = format!(
                            "{} has assigned themselves the NSFW role. Time since joining: `{}`\nClosest guess: `{}` by {} ({} **+{}**)",
                            new.mention(),
                            hhmmss(total_s.try_into()?),
                            hhmmss(winner.1),
                            winner.0.mention(),
                            emoji("catcoin"),
                            pool_coin
                        );

                        bets.remove_pool(new.user.id);

                        resp
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
