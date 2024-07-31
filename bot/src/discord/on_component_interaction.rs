use poise::{
    serenity_prelude::{
        self as serenity, ComponentInteractionDataKind, CreateInteractionResponse,
        CreateInteractionResponseMessage,
    },
    Modal,
};
use profile::{
    edits::{
        create_class_select_components, dispatch_profile_edit, open_class_select_menu,
        toggle_class, update_profile_column,
    },
    get_user_profile,
};
use serenity::ComponentInteraction;

use common::{discord::execute_modal_generic, Error};

use super::{
    commands::{birthday_check, SteamLinkCodeModal},
    PoiseData,
};

/// handle all permanent component interactions
pub async fn dispatch(
    ctx: &serenity::Context,
    data: &PoiseData,
    mci: &ComponentInteraction,
) -> Result<(), Error> {
    match mci.data.custom_id.as_str() {
        id if id.starts_with("stock-market.") => {
            stocks::interaction_dispatch(ctx, &data.local_pool, mci).await?
        }
        "birthday.submit" => birthday_check::submit_button(ctx, data, mci).await?,
        "profile.edit.select" => match &mci.data.kind {
            ComponentInteractionDataKind::StringSelect { values } => {
                let choice = values.first().ok_or("No choice")?;
                dispatch_profile_edit(ctx, mci, &data.local_pool, &data.api_state, choice).await?;
            }
            _ => {
                mci.create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
            }
        },
        "delete.msg" => match &mci.data.kind {
            ComponentInteractionDataKind::Button => {
                let _ =
                    mci.message.delete(ctx).await.inspect_err(|e| {
                        eprintln!("Could not delete from component interaction: {e}")
                    });
            }
            _ => (),
        },
        // the direct class edit button
        "profile.edit.classes" => match &mci.data.kind {
            ComponentInteractionDataKind::Button => {
                open_class_select_menu(ctx, &data.local_pool, mci).await?
            }
            _ => (),
        },
        "profile.edit.class.select" => match &mci.data.kind {
            ComponentInteractionDataKind::StringSelect { values } => {
                let choice = values.first().ok_or("No choice")?;
                toggle_class(&data.local_pool, mci.user.id, choice.parse()?).await?;
                let classes: String = get_user_profile(&data.local_pool, mci.user.id)
                    .await?
                    .get_classes()
                    .iter()
                    .map(|c| c.emoji().to_owned())
                    .collect::<Vec<String>>()
                    .join("");
                let components = create_class_select_components();
                let _ = mci
                    .create_response(
                        &ctx,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .components(components)
                                .content(format!("{} | Select each class to toggle it.", classes))
                                .ephemeral(true),
                        ),
                    )
                    .await?;
            }
            _ => {
                mci.create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
            }
        },
        "profile.edit.favorite.select" => match &mci.data.kind {
            ComponentInteractionDataKind::UserSelect { values } => {
                let choice = values.first().ok_or("No choice")?;
                update_profile_column(
                    mci.user.id,
                    "favorite_user",
                    choice.to_string(),
                    &data.local_pool,
                )
                .await?;
                mci.create_response(
                    ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(format!("Selected <@{}> as your favorite.", choice))
                            .ephemeral(true),
                    ),
                )
                .await?;
            }
            _ => {
                mci.create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
            }
        },
        "steam.link" => {
            if let Some(response) = execute_modal_generic::<SteamLinkCodeModal, _>(
                ctx,
                |resp| mci.create_response(ctx, resp),
                mci.id.to_string(),
                None,
                None,
            )
            .await?
            {
                let dm = SteamLinkCodeModal::parse(response.data.clone())?;
                let steamid64 = match data.api_state.try_link_user(dm.code).await {
                    Ok(s) => s,
                    Err(e) => {
                        response
                            .create_response(
                                ctx,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content(format!("Could not link: {e}"))
                                        .ephemeral(true),
                                ),
                            )
                            .await?;
                        return Ok(());
                    }
                };
                let profiles = data
                    .steamid_client
                    .lookup(steamid64.to_string().as_str())
                    .await?;
                let profile = profiles
                    .first()
                    .ok_or("No profile found for the returned steamid.")?;

                sqlx::query!("INSERT INTO `profiles` (`uid`, `steamid`) VALUES (?, ?) ON DUPLICATE KEY UPDATE `steamid` = ?", mci.user.id.to_string(), profile.steam3, profile.steam3).execute(&data.local_pool).await?;

                response
                    .create_response(
                        ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!("Successfully linked to {}", profile.steamidurl))
                                .ephemeral(true),
                        ),
                    )
                    .await?;
            }
        }
        _ => (),
    }
    Ok(())
}
