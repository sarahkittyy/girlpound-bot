use chrono::NaiveDate;
use poise::serenity_prelude::{
    self as serenity, ComponentInteraction, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use poise::{CreateReply, Modal};
use regex::Regex;
use serenity::{CreateActionRow, CreateButton, CreateEmbed, CreateMessage};

use crate::discord::{ApplicationContext, PoiseData};

use common::{discord::execute_modal_generic, Error};

#[derive(Debug, Modal)]
#[name = "TKGP"]
pub struct AgeModal {
    #[name = "What is your birthday (MONTH/DAY/YEAR)"]
    #[placeholder = "MM/DD/YYYY"]
    #[min_length = 8]
    #[max_length = 10]
    pub birthday: String,
}

fn birthday_embed() -> serenity::CreateEmbed {
    CreateEmbed::new()
        .title("Confirm your birthday to access the server.")
        .color(serenity::Color::GOLD)
}

pub async fn submit_button(
    ctx: &serenity::Context,
    data: &PoiseData,
    mci: &ComponentInteraction,
) -> Result<(), Error> {
    let Some(guild_id) = mci.guild_id else {
        return Ok(());
    };
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

    Ok(())
}

/// Spawns the birthday modal
#[poise::command(slash_command)]
pub async fn birthday_modal(ctx: ApplicationContext<'_>) -> Result<(), Error> {
    let btn = CreateButton::new("birthday.submit")
        .label("Click Here")
        .style(serenity::ButtonStyle::Primary)
        .emoji('📅');
    let row = CreateActionRow::Buttons(vec![btn]);
    let channel = ctx.channel_id();
    channel
        .send_message(
            &ctx,
            CreateMessage::default()
                .embed(birthday_embed())
                .components(vec![row]),
        )
        .await?;
    ctx.send(CreateReply::default().content("Posted.").ephemeral(true))
        .await?;

    Ok(())
}
