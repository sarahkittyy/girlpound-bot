use poise::serenity_prelude as serenity;
use poise::{CreateReply, Modal};
use serenity::{CreateActionRow, CreateButton, CreateEmbed, CreateMessage};

use crate::{discord::ApplicationContext, Error};

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
