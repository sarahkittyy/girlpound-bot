use rand::prelude::*;
use std::time::Duration;

use poise::{
    self,
    serenity_prelude::{
        self as serenity, ChannelType, ComponentInteractionCollector, ComponentInteractionDataKind,
        CreateActionRow, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateMessage, CreateSelectMenu, CreateSelectMenuOption, Member, Mentionable, UserId,
    },
    CreateReply,
};

use crate::{discord::Context, Error};

async fn prompt(
    ctx: &Context<'_>,
    msg: &str,
    user: UserId,
    options: Vec<CreateSelectMenuOption>,
) -> Result<String, Error> {
    let uuid = ctx.id();

    // send msg
    let menu = CreateSelectMenu::new(
        format!("{uuid}-choice"),
        serenity::CreateSelectMenuKind::String { options },
    );
    let row = vec![CreateActionRow::SelectMenu(menu)];
    ctx.channel_id()
        .send_message(ctx, CreateMessage::new().components(row).content(msg))
        .await?;

    // listen for responses
    while let Some(mci) = ComponentInteractionCollector::new(ctx)
        .channel_id(ctx.channel_id())
        .timeout(Duration::from_secs(60))
        .filter(move |mci| mci.data.custom_id.starts_with(&uuid.to_string()))
        .await
    {
        if mci.user.id != user {
            mci.create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Not you!! >_<")
                        .ephemeral(true),
                ),
            )
            .await?;
            continue;
        }
        let res = match &mci.data.kind {
            ComponentInteractionDataKind::StringSelect { values } => {
                values.first().cloned().ok_or("No choice given.".into())
            }
            _ => Err("Invalid interaction data kind.".into()),
        };
        mci.create_response(ctx, CreateInteractionResponse::Acknowledge)
            .await?;
        return res;
    }
    return Err("/teamcaptain response missing, aborted.".into());
}

/// Set up pug team captains and display prompts for them to pick users
#[poise::command(slash_command)]
pub async fn teamcaptain(
    ctx: Context<'_>,
    #[description = "Pug lobby voice channel with all the players"] channel: serenity::GuildChannel,
) -> Result<(), Error> {
    if !matches!(channel.kind, ChannelType::Voice) {
        ctx.reply(format!("Channel {} is not a voice channel!", channel.name))
            .await?;
        return Ok(());
    }
    let mut members = channel.members(ctx)?;
    if members.len() < 2 {
        ctx.send(
            CreateReply::default().content("Must be at least 2 people in the VC to be captains!"),
        )
        .await?;
        return Ok(());
    }
    let invoker = ctx.author();

    ctx.send(
        CreateReply::default()
            .content(":white_check_mark:")
            .ephemeral(true),
    )
    .await?;

    // prompt for red captain
    let options = |members: &Vec<Member>| -> Vec<CreateSelectMenuOption> {
        members
            .iter()
            .map(|m| CreateSelectMenuOption::new(m.display_name(), m.user.id.to_string()))
            .collect()
    };
    let red_captain: UserId = prompt(
        &ctx,
        &format!("<@{}> Pick the RED team captain.", invoker.id),
        invoker.id,
        options(&members),
    )
    .await?
    .parse()?;
    members.retain(|m| m.user.id != red_captain);
    let blu_captain: UserId = prompt(
        &ctx,
        &format!("<@{}> Pick the BLU team captain.", invoker.id),
        invoker.id,
        options(&members),
    )
    .await?
    .parse()?;
    members.retain(|m| m.user.id != blu_captain);

    let mut red: Vec<UserId> = vec![red_captain.clone()];
    let mut blu: Vec<UserId> = vec![blu_captain.clone()];
    let mut pick_red: bool = random();

    loop {
        if members.len() == 0 {
            break;
        }
        let captain = if pick_red { red_captain } else { blu_captain };
        let team: &mut Vec<UserId> = if pick_red { &mut red } else { &mut blu };
        let pick: UserId = prompt(
            &ctx,
            &format!("<@{}> Your pick! :3", captain),
            captain,
            options(&members),
        )
        .await?
        .parse()?;
        members.retain(|m| m.user.id != pick);
        team.push(pick);
        pick_red = !pick_red;
    }

    let embed = CreateEmbed::new() //
        .field(
            "🔴 RED",
            red.into_iter()
                .map(|uid| uid.mention().to_string())
                .collect::<Vec<String>>()
                .join("\n"),
            true,
        )
        .field(
            "🔵 BLU",
            blu.into_iter()
                .map(|uid| uid.mention().to_string())
                .collect::<Vec<String>>()
                .join("\n"),
            true,
        );

    ctx.channel_id()
        .send_message(
            &ctx,
            CreateMessage::default()
                .content("Teams have been selected!")
                .embed(embed),
        )
        .await?;

    Ok(())
}
