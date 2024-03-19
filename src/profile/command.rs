use std::time::Duration;

use poise::{
    serenity_prelude::{
        self as serenity, ButtonStyle, ComponentInteraction, ComponentInteractionCollector,
        CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, GetMessages,
    },
    CreateReply,
};

use super::vote::vote_on;
use crate::{
    discord::Context,
    profile::{get_user_profile, vote::get_profile_votes},
    Error,
};

/// TKGP Profile
#[poise::command(slash_command, channel_cooldown = 10, global_cooldown = 2)]
pub async fn profile(
    ctx: Context<'_>,
    #[description = "The user to retrieve"] member: Option<serenity::Member>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let uuid = ctx.id();

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

    let like_id = format!("{uuid}-like");
    let dislike_id = format!("{uuid}-dislike");
    let edit_id = format!("{uuid}-edit");
    let reload_id = format!("{uuid}-reload");

    // delete last profile msg
    let msgs = ctx
        .channel_id()
        .messages(ctx.http(), GetMessages::new().limit(45))
        .await?;
    let bid = ctx.cache().current_user().id;
    for msg in &msgs {
        if msg.author.id == bid
            && msg
                .embeds
                .first()
                .is_some_and(|e| e.fields.first().is_some_and(|f| f.name == "Votes"))
            && msg
                .interaction
                .as_ref()
                .is_some_and(|i| i.user.id == ctx.author().id)
        {
            msg.delete(ctx.http()).await?;
            break;
        }
    }

    // buttons
    let components = vec![CreateActionRow::Buttons(vec![
        // like
        CreateButton::new(like_id.clone())
            .style(ButtonStyle::Success)
            .emoji('👍'),
        // dislike
        CreateButton::new(dislike_id.clone())
            .style(ButtonStyle::Danger)
            .emoji('👎'),
        // edit
        CreateButton::new(edit_id.clone())
            .style(ButtonStyle::Primary)
            .label("Edit")
            .emoji('📝'),
        // reload
        CreateButton::new(reload_id.clone())
            .style(ButtonStyle::Secondary)
            .emoji('🔃'),
    ])];

    let mut profile = get_user_profile(&ctx.data().local_pool, member.user.id).await?;
    let mut votes = get_profile_votes(&ctx.data().local_pool, member.user.id).await?;
    let msg = ctx
        .send(
            CreateReply::default()
                .embed(profile.to_embed(&ctx, votes.clone()).await?)
                .components(components),
        )
        .await?;

    while let Some(mci) = ComponentInteractionCollector::new(ctx)
        .channel_id(ctx.channel_id())
        .timeout(Duration::from_secs(30))
        .filter(move |mci| mci.data.custom_id.starts_with(&uuid.to_string()))
        .await
    {
        if mci.data.custom_id == like_id || mci.data.custom_id == dislike_id {
            // submit vote
            let diff = vote_on(
                &ctx.data().local_pool,
                member.user.id,
                mci.user.id,
                mci.data.custom_id == like_id,
            )
            .await?;
            // update vote count
            votes.likes += diff.likes;
            votes.dislikes += diff.dislikes;
            // acknowledge btn
            mci.create_response(
                &ctx,
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(profile.to_embed(&ctx, votes).await?),
                ),
            )
            .await?;
        } else if mci.data.custom_id == edit_id {
            if mci.user.id == member.user.id {
                open_edit_menu(ctx.clone(), &mci).await?;
            } else {
                mci.create_response(
                    &ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .ephemeral(true)
                            .content("This is not your profile! >:3"),
                    ),
                )
                .await?;
            }
        } else if mci.data.custom_id == reload_id {
            profile = get_user_profile(&ctx.data().local_pool, member.user.id).await?;
            votes = get_profile_votes(&ctx.data().local_pool, member.user.id).await?;
            mci.create_response(
                &ctx,
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(profile.to_embed(&ctx, votes).await?),
                ),
            )
            .await?;
        };
    }

    msg.delete(ctx).await?;
    Ok(())
}

async fn open_edit_menu(ctx: Context<'_>, mci: &ComponentInteraction) -> Result<(), Error> {
    let options = vec![
        //
        CreateSelectMenuOption::new("Description", "description")
            .description("Edit your bio")
            .emoji('📝'),
        CreateSelectMenuOption::new("Color", "color")
            .description("Edit your bio color")
            .emoji('🎨'),
        CreateSelectMenuOption::new("Classes", "classes")
            .description("Display your favorite TF2 classes.")
            .emoji('🔫'),
        CreateSelectMenuOption::new("Favorite Map", "favorite-map")
            .description("Display your favorite map.")
            .emoji('📄'),
        CreateSelectMenuOption::new("Url", "url")
            .description("Set a custom link to redirect to")
            .emoji('🔗'),
        CreateSelectMenuOption::new("Title", "title")
            .description("Customize the header of your profile")
            .emoji('🐈'),
        CreateSelectMenuOption::new("Image", "image")
            .description("Link an image to your profile")
            .emoji('📷'),
        CreateSelectMenuOption::new("Remove Image", "remove-image")
            .description("Remove your profile image")
            .emoji('❌'),
    ];

    let components = vec![CreateActionRow::SelectMenu(CreateSelectMenu::new(
        "profile.edit.select",
        CreateSelectMenuKind::String { options },
    ))];

    mci.create_response(
        &ctx,
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .components(components)
                .ephemeral(true),
        ),
    )
    .await?;

    Ok(())
}
