use std::time::Duration;

use poise::{
    serenity_prelude::{
        self as serenity, ButtonStyle, ComponentInteraction, ComponentInteractionCollector,
        CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, GetMessages, ReactionType,
    },
    CreateReply,
};

use crate::discord::Context;

use catcoin::get_catcoin;
use common::{discord::get_steam_link_content, Error};
use profile::{
    get_user_profile, view_profile,
    vote::{self, get_profile_votes},
    UserProfile,
};
use steam::SteamProfileData;

#[derive(Debug, poise::Modal)]
#[name = "Enter your steam link code"]
pub struct SteamLinkCodeModal {
    #[name = "The 6-digit code"]
    #[min_length = 6]
    #[max_length = 6]
    pub code: String,
}

/// TKGP Profile
#[poise::command(
    context_menu_command = "Get TKGP Profile",
    user_cooldown = 5,
    global_cooldown = 2
)]
pub async fn get_profile(
    ctx: Context<'_>,
    #[description = "The user to retrieve"] user: serenity::User,
) -> Result<(), Error> {
    let member = ctx.data().guild_id.member(&ctx, user).await?;
    send_profile(ctx, member).await
}

/// TKGP Profile
#[poise::command(slash_command, user_cooldown = 5, global_cooldown = 2)]
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
    send_profile(ctx, member).await
}

/// Link your steam account to TKGP
#[poise::command(slash_command)]
pub async fn link(ctx: Context<'_>) -> Result<(), Error> {
    let (embed, row) = get_steam_link_content(&ctx.data().api_state.link_url());
    ctx.send(CreateReply::default().embed(embed).components(row))
        .await?;

    Ok(())
}

async fn send_profile(ctx: Context<'_>, member: serenity::Member) -> Result<(), Error> {
    ctx.defer().await?;
    let uuid = ctx.id();

    // increment views
    let _ = view_profile(&ctx.data().local_pool, member.user.id)
        .await
        .inspect_err(|e| log::error!("Could not view profile: {e}"));

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

    let mut profile = get_user_profile(&ctx.data().local_pool, member.user.id).await?;
    let mut votes = get_profile_votes(&ctx.data().local_pool, member.user.id).await?;
    let mut steam_data = if let Some(ref steamid) = profile.steamid {
        SteamProfileData::get(&ctx.data().local_pool, &ctx.data().steamid_client, steamid).await?
    } else {
        None
    };
    let mut catcoin = get_catcoin(&ctx.data().local_pool, member.user.id).await?;

    // buttons
    let buttons = vec![
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
        // delete
        CreateButton::new("delete.msg")
            .style(ButtonStyle::Danger)
            .emoji(ReactionType::Unicode("🗑️".to_owned())),
    ];
    let components = vec![CreateActionRow::Buttons(buttons)];
    let msg = ctx
        .send(
            CreateReply::default()
                .embed(
                    profile
                        .to_embed(
                            ctx.serenity_context(),
                            ctx.data().guild_id,
                            votes.clone(),
                            steam_data.clone(),
                            catcoin.clone(),
                        )
                        .await?,
                )
                .components(components),
        )
        .await?;

    while let Some(mci) = ComponentInteractionCollector::new(ctx)
        .channel_id(ctx.channel_id())
        .timeout(Duration::from_secs(200))
        .filter(move |mci| mci.data.custom_id.starts_with(&uuid.to_string()))
        .await
    {
        if mci.data.custom_id == like_id || mci.data.custom_id == dislike_id {
            // submit vote
            let diff = vote::vote_on(
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
                    CreateInteractionResponseMessage::new().embed(
                        profile
                            .to_embed(
                                ctx.serenity_context(),
                                ctx.data().guild_id,
                                votes,
                                steam_data.clone(),
                                catcoin.clone(),
                            )
                            .await?,
                    ),
                ),
            )
            .await?;
        } else if mci.data.custom_id == edit_id {
            if mci.user.id == member.user.id {
                open_edit_menu(ctx.clone(), &mci, &profile).await?;
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
            steam_data = if let Some(ref steamid) = profile.steamid {
                SteamProfileData::get(&ctx.data().local_pool, &ctx.data().steamid_client, steamid)
                    .await?
            } else {
                None
            };
            catcoin = get_catcoin(&ctx.data().local_pool, member.user.id).await?;
            mci.create_response(
                &ctx,
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new().embed(
                        profile
                            .to_embed(
                                ctx.serenity_context(),
                                ctx.data().guild_id,
                                votes,
                                steam_data.clone(),
                                catcoin.clone(),
                            )
                            .await?,
                    ),
                ),
            )
            .await?;
        }
    }

    let _ = msg
        .delete(ctx)
        .await
        .inspect_err(|e| log::error!("Could not delete profile: {e}"));
    Ok(())
}

async fn open_edit_menu(
    ctx: Context<'_>,
    mci: &ComponentInteraction,
    profile: &UserProfile,
) -> Result<(), Error> {
    let mut options = vec![
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
        CreateSelectMenuOption::new("Favorite User", "favorite-user")
            .description("Choose your favorite user!")
            .emoji('💝'),
        CreateSelectMenuOption::new("Remove Favorite User", "remove-favorite-user")
            .description("Remove your favorite user")
            .emoji('💔'),
        CreateSelectMenuOption::new("Image", "image")
            .description("Link an image to your profile")
            .emoji('📷'),
        CreateSelectMenuOption::new("Remove Image", "remove-image")
            .description("Remove your profile image")
            .emoji('❌'),
        CreateSelectMenuOption::new("Toggle Vote Visibility", "toggle-vote")
            .description("Toggle if your profile votes are shown or not.")
            .emoji('🫣'),
        CreateSelectMenuOption::new("Toggle Stats Visibility", "toggle-stats")
            .description("Toggle if your steam user stats are shown.")
            .emoji('📈'),
        CreateSelectMenuOption::new("Toggle Domination Visibility", "toggle-domination")
            .description("Toggle if your server domination records are shown.")
            .emoji(ReactionType::Unicode("⚔️".to_owned())),
    ];
    if profile.steamid.is_none() {
        options.insert(
            0,
            CreateSelectMenuOption::new("Link steam", "link-steam")
                .description("Link your profile to your steam account")
                .emoji('🔗'),
        );
    } else {
        options.push(
            CreateSelectMenuOption::new("Unlink steam", "unlink-steam")
                .description("Disconnects your steam account from your TKGP profile :(")
                .emoji(ReactionType::Unicode("🗑️".to_owned())),
        );
    }

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
