use crate::Error;
use poise::{
    serenity_prelude::{
        self as serenity, CreateActionRow, CreateButton, CreateEmbed, ReactionType,
    },
    Modal,
};

pub async fn execute_modal_generic<
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
}

pub fn get_steam_link_content(link_url: &str) -> (CreateEmbed, Vec<CreateActionRow>) {
    let embed = CreateEmbed::new() //
        .title("Get a code from https://link.fluffycat.gay/steam-link & enter it below.")
        .url(link_url);
    let row = vec![CreateActionRow::Buttons(vec![
        CreateButton::new_link(link_url)
            .label("Get Code")
            .emoji(ReactionType::Unicode("☁️".to_owned())),
        CreateButton::new("steam.link")
            .label("Enter Code")
            .emoji('🔗'),
    ])];
    (embed, row)
}
