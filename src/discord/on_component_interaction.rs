use chrono::NaiveDate;
use poise::{
    serenity_prelude::{
        self as serenity, CreateInteractionResponse, CreateInteractionResponseMessage,
    },
    Modal,
};
use regex::Regex;
use serenity::ComponentInteraction;

use crate::Error;

use super::{commands::AgeModal, PoiseData};

async fn execute_modal_generic<
    M: Modal,
    F: std::future::Future<Output = Result<(), serenity::Error>>,
>(
    ctx: &serenity::Context,
    create_interaction_response: impl FnOnce(serenity::CreateInteractionResponse) -> F,
    modal_custom_id: String,
    defaults: Option<M>,
    timeout: Option<std::time::Duration>,
) -> Result<Option<serenity::ModalInteraction>, Error> {
    // Send modal
    create_interaction_response(M::create(defaults, modal_custom_id.clone())).await?;

    // Wait for user to submit
    let response = serenity::collector::ModalInteractionCollector::new(&ctx.shard)
        .filter(move |d| d.data.custom_id == modal_custom_id)
        .timeout(timeout.unwrap_or(std::time::Duration::from_secs(3600)))
        .await;
    Ok(response)

    /*// Send acknowledgement so that the pop-up is closed
    response
        .create_response(ctx, serenity::CreateInteractionResponse::Acknowledge)
        .await?;

    Ok(Some(
        M::parse(response.data.clone()).map_err(serenity::Error::Other)?,
    ))*/
}

/// handle all permanent component interactions
pub async fn dispatch(
    ctx: &serenity::Context,
    data: &PoiseData,
    mci: &ComponentInteraction,
) -> Result<(), Error> {
    let Some(guild_id) = mci.guild_id else {
        return Ok(());
    };
    match mci.data.custom_id.as_str() {
        "birthday.submit" => {
            if let Some(response) = execute_modal_generic::<AgeModal, _>(
                ctx,
                |resp| mci.create_response(ctx, resp),
                mci.id.to_string(),
                None,
                None,
            )
            .await?
            {
                let invalid_birthday = || {
                    response.create_response(
                        &ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("Invalid birthday format. Try again. Example: 01/01/1995")
                                .ephemeral(true),
                        ),
                    )
                };
                // parse modal response
                let am = AgeModal::parse(response.data.clone())?;

                // parse month/day/year
                let re = Regex::new(r#"^(\d{1,2})/(\d{1,2})/(\d{4})$"#)
                    .expect("regex error in dispatch() for some reason");
                let Some(caps) = re.captures(&am.birthday) else {
                    invalid_birthday().await?;
                    return Ok(());
                };
                let (_, [month, day, year]) = caps.extract::<3>();
                let month: u32 = month.parse()?;
                let day: u32 = day.parse()?;
                let year: i32 = year.parse()?;
                let Some(birthday) = NaiveDate::from_ymd_opt(year, month, day) else {
                    invalid_birthday().await?;
                    return Ok(());
                };

                let today = chrono::Utc::now().date_naive();
                let Some(age) = today.years_since(birthday) else {
                    invalid_birthday().await?;
                    return Ok(());
                };
                response
                    .create_response(&ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
                if age < 18 {
                    // banned
                    ctx.http
                        .ban_user(guild_id, mci.user.id, 7, Some("tkgp: under 18"))
                        .await?;
                } else {
                    // allowed in
                    ctx.http
                        .add_member_role(
                            guild_id,
                            mci.user.id,
                            data.member_role,
                            Some("passed the age check"),
                        )
                        .await?;
                }
            }
        }
        _ => (),
    }
    Ok(())
}
