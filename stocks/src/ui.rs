use std::time::Duration;

use poise::serenity_prelude::{
    self as serenity, ActionRowComponent, ButtonStyle, ComponentInteraction, CreateActionRow,
    CreateButton, CreateEmbed, CreateInputText, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage, CreateModal, CreateSelectMenu,
    CreateSelectMenuKind, CreateSelectMenuOption, InputTextStyle, ModalInteraction,
    ModalInteractionCollector,
};

use common::Error;
use sqlx::{MySql, Pool};

use super::{company::Company, market_time};

/// Primary embed sent with the state of the market
pub async fn market_hub() -> CreateMessage {
    let now = market_time().read().await.format("%m-%d");

    let hub = CreateEmbed::new()
        .title(format!("Market Hub [{}]", now))
        .description("TODO");

    let components = vec![CreateActionRow::Buttons(vec![
        //
        CreateButton::new("stock-market.buy")
            .emoji('🛒')
            .label("Invest")
            .style(ButtonStyle::Primary),
    ])];

    CreateMessage::new().embed(hub).components(components)
}

/// Company picker
pub async fn choose_company(
    ctx: &serenity::Context,
    pool: &Pool<MySql>,
    mci: &ComponentInteraction,
) -> Result<(), Error> {
    let companies = Company::fetch_all(pool).await?;
    let options: Vec<CreateSelectMenuOption> = companies
        .iter()
        .map(|company| {
            CreateSelectMenuOption::new(
                format!("{} ({})", company.name, company.tag),
                company.id.to_string(),
            )
        })
        .collect();
    let select_menu = CreateSelectMenu::new(
        "stock-market.buy-from",
        CreateSelectMenuKind::String { options },
    )
    .placeholder("Select a company.");

    let row = vec![CreateActionRow::SelectMenu(select_menu)];

    mci.create_response(
        ctx,
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .components(row)
                .ephemeral(true),
        ),
    )
    .await?;

    Ok(())
}

/// Returns how many stocks to buy
pub async fn buy_stocks_modal(
    ctx: &serenity::Context,
    mci: &ComponentInteraction,
    company: &Company,
) -> Result<(ModalInteraction, i32), Error> {
    let uuid = mci.id;
    let modal_id = format!("{uuid}-buy-stocks");
    let components = vec![CreateActionRow::InputText(CreateInputText::new(
        InputTextStyle::Short,
        format!("1 stock = {} CC", company.price),
        "amount",
    ))];
    let modal = CreateModal::new(
        modal_id.clone(),
        format!("How many {} stocks to buy?", company.tag),
    )
    .components(components);
    mci.create_response(ctx, CreateInteractionResponse::Modal(modal))
        .await?;

    if let Some(response) = ModalInteractionCollector::new(ctx)
        .filter(move |d| d.data.custom_id == modal_id)
        .timeout(Duration::from_secs(3600))
        .await
    {
        let Some(ActionRowComponent::InputText(it)) = response
            .data
            .components
            .first()
            .and_then(|c| c.components.first())
        else {
            return Err("Invalid Modal Components.".into());
        };
        let Some(Ok(amount)) = it.value.as_ref().map(|v| v.parse::<u32>()) else {
            return Err("Inputted value must be a positive whole number.".into());
        };
        Ok((response, amount as i32))
    } else {
        Err("No response received.".into())
    }
}
