use poise::serenity_prelude::{
    self as serenity, ComponentInteractionDataKind, CreateInteractionResponse,
};
use serenity::ComponentInteraction;

use crate::{
    profile::edits::{dispatch_profile_edit, toggle_class},
    Error,
};

use super::{commands::birthday_check, PoiseData};

/// handle all permanent component interactions
pub async fn dispatch(
    ctx: &serenity::Context,
    data: &PoiseData,
    mci: &ComponentInteraction,
) -> Result<(), Error> {
    match mci.data.custom_id.as_str() {
        "birthday.submit" => birthday_check::submit_button(ctx, data, mci).await?,
        "profile.edit.select" => match &mci.data.kind {
            ComponentInteractionDataKind::StringSelect { values } => {
                let choice = values.first().ok_or("No choice")?;
                dispatch_profile_edit(ctx, mci, data, choice).await?;
            }
            _ => {
                mci.create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
            }
        },
        "profile.edit.class.select" => match &mci.data.kind {
            ComponentInteractionDataKind::StringSelect { values } => {
                let choice = values.first().ok_or("No choice")?;
                toggle_class(&data.local_pool, mci.user.id, choice.parse()?).await?;
                mci.create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
            }
            _ => {
                mci.create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
            }
        },
        _ => (),
    }
    Ok(())
}
