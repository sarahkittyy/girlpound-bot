use poise::{serenity_prelude as serenity, CreateReply};

use crate::{discord::Context, profile::get_user_profile, Error};

/// TKGP Profile
#[poise::command(slash_command, user_cooldown = 8, global_cooldown = 2)]
pub async fn profile(
    ctx: Context<'_>,
    #[description = "The user to retrieve"] member: Option<serenity::Member>,
) -> Result<(), Error> {
    let member = if let Some(member) = member {
        member
    } else if let Some(member) = ctx.author_member().await {
        member.into_owned()
    } else {
        ctx.send(
            CreateReply::default()
                .content("Could not find a user's profile to fetch!")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    };
    let profile = get_user_profile(&ctx.data().local_pool, member.user.id).await?;
    ctx.send(CreateReply::default().embed(profile.to_embed(&ctx, &ctx.data().local_pool).await?))
        .await?;
    Ok(())
}
