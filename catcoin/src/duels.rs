use std::time::{Duration, SystemTime, UNIX_EPOCH};

use poise::serenity_prelude::{
    ButtonStyle, ChannelId, ComponentInteraction, ComponentInteractionCollector, Context,
    CreateActionRow, CreateAttachment, CreateButton, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage, EditAttachments, EditMessage, Mentionable,
    Message, User,
};

use common::Error;
use rand::{random, thread_rng, Rng};
use rand_distr::{Distribution, Normal};
use sqlx::{MySql, Pool};

use crate::{attack_emoji, defense_emoji, emoji, get_catcoin, grant_catcoin, spend_catcoin};

pub async fn on_message(ctx: &Context, pool: &Pool<MySql>, msg: &Message) -> Result<(), Error> {
    let wager: u64 = {
        let mut rng = thread_rng();

        // should message spawn a duel?
        if !rng.gen_ratio(1, 1500) {
            return Ok(());
        }

        // determine wager
        let dist: Normal<f32> = Normal::new(25.0, 25.0).unwrap();
        dist.sample(&mut rng).max(5.0).round().abs() as u64
    };

    spawn_duel(ctx, pool, msg.channel_id, wager).await?;

    Ok(())
}

struct Duel {
    uuid: String,
    wager: u64,
    attack: Option<User>,
    defense: Option<User>,
}

impl Duel {
    fn new(wager: u64, uuid: &str) -> Self {
        Self {
            uuid: uuid.to_owned(),
            wager,
            attack: None,
            defense: None,
        }
    }

    fn to_embed(&self) -> CreateEmbed {
        CreateEmbed::new()
            .title(format!("‼️ {} {} duel ✨", self.wager, emoji()))
            .field(
                format!("defense {}", defense_emoji()),
                &self
                    .defense
                    .as_ref()
                    .map(|d| d.mention().to_string())
                    .unwrap_or("open".to_owned()),
                true,
            )
            .field(
                format!("{} attack", attack_emoji()),
                self.attack
                    .as_ref()
                    .map(|a| a.mention().to_string())
                    .unwrap_or("open".to_owned()),
                true,
            )
    }

    fn try_register_defense(&mut self, user: User) -> Result<(), &'static str> {
        // if the slot isn't taken
        if self.defense.is_some() {
            return Err("spot's already taken");
        }
        if self.attack.as_ref().is_some_and(|u| u.id == user.id) {
            return Err("someone else, thx");
        }
        self.defense = Some(user);
        Ok(())
    }

    fn reset(&mut self) {
        self.attack = None;
        self.defense = None;
    }

    fn try_register_attack(&mut self, user: User) -> Result<(), &'static str> {
        // if the slot isn't taken
        if self.attack.is_some() {
            return Err("spot's already taken");
        }
        if self.defense.as_ref().is_some_and(|u| u.id == user.id) {
            return Err("someone else, thx");
        }
        self.attack = Some(user);
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.defense.is_some() && self.attack.is_some()
    }

    fn to_components(&self) -> Vec<CreateActionRow> {
        let mut buttons: Vec<CreateButton> = vec![];
        if self.defense.is_none() {
            buttons.push(
                CreateButton::new(format!("{}-defense", self.uuid))
                    .label("bet defense")
                    .style(ButtonStyle::Primary),
            );
        }
        if self.attack.is_none() {
            buttons.push(
                CreateButton::new(format!("{}-attack", self.uuid))
                    .label("bet attack")
                    .style(ButtonStyle::Primary),
            );
        }
        if !buttons.is_empty() {
            vec![CreateActionRow::Buttons(buttons)]
        } else {
            vec![]
        }
    }
}

pub async fn spawn_duel(
    ctx: &Context,
    pool: &Pool<MySql>,
    channel: ChannelId,
    wager: u64,
) -> Result<(), Error> {
    let uuid = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut duel = Duel::new(wager, &uuid.to_string());

    let defense_uuid = format!("{uuid}-defense");
    let attack_uuid = format!("{uuid}-attack");

    let mut msg = channel
        .send_message(
            ctx,
            CreateMessage::new()
                .embed(duel.to_embed())
                .components(duel.to_components()),
        )
        .await?;

    async fn update_embed(ctx: &Context, msg: &mut Message, duel: &Duel) -> Result<(), Error> {
        msg.edit(
            ctx,
            EditMessage::new()
                .embed(duel.to_embed())
                .components(duel.to_components()),
        )
        .await?;
        Ok(())
    }

    async fn err_respond(
        ctx: &Context,
        mci: &ComponentInteraction,
        msg: &str,
    ) -> Result<(), Error> {
        mci.create_response(
            ctx,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(msg)
                    .ephemeral(true),
            ),
        )
        .await?;
        Ok(())
    }

    while let Some(mci) = ComponentInteractionCollector::new(ctx)
        .channel_id(channel)
        .timeout(Duration::from_secs(500))
        .filter(move |mci| mci.data.custom_id.starts_with(&uuid.to_string()))
        .await
    {
        let user = mci.user.clone();
        // check if the user has enough catcoin
        if get_catcoin(pool, user.id).await?.catcoin < wager as i64 {
            err_respond(ctx, &mci, "not this time, brokie").await?;
            continue;
        }
        // register them for the team
        if mci.data.custom_id == defense_uuid {
            if let Err(e) = duel.try_register_defense(user) {
                err_respond(ctx, &mci, e).await?;
                continue;
            }
            update_embed(ctx, &mut msg, &duel).await?;
            mci.create_response(ctx, CreateInteractionResponse::Acknowledge)
                .await?;
        } else if mci.data.custom_id == attack_uuid {
            if let Err(e) = duel.try_register_attack(user) {
                err_respond(ctx, &mci, e).await?;
                continue;
            }
            update_embed(ctx, &mut msg, &duel).await?;
            mci.create_response(ctx, CreateInteractionResponse::Acknowledge)
                .await?;
        }

        if duel.is_ready() {
            {
                // try and deduct the catcoin from each user's balance
                let mut tx = pool.begin().await?;
                if !spend_catcoin(&mut *tx, duel.defense.as_ref().unwrap().id, duel.wager).await? {
                    tx.rollback().await?;
                    msg.channel_id
                        .send_message(
                            ctx,
                            CreateMessage::new().content(format!(
                                "{} tried to cheat the duel! aborting.",
                                duel.defense.as_ref().unwrap().mention()
                            )),
                        )
                        .await?;
                    msg.delete(ctx).await?;
                    duel.reset();
                    msg = msg
                        .channel_id
                        .send_message(
                            ctx,
                            CreateMessage::new()
                                .embed(duel.to_embed())
                                .components(duel.to_components()),
                        )
                        .await?;
                    continue;
                }
                if !spend_catcoin(&mut *tx, duel.attack.as_ref().unwrap().id, duel.wager).await? {
                    tx.rollback().await?;
                    msg.channel_id
                        .send_message(
                            ctx,
                            CreateMessage::new().content(format!(
                                "{} tried to cheat the duel! aborting.",
                                duel.attack.as_ref().unwrap().mention()
                            )),
                        )
                        .await?;
                    msg.delete(ctx).await?;
                    duel.reset();
                    msg = msg
                        .channel_id
                        .send_message(
                            ctx,
                            CreateMessage::new()
                                .embed(duel.to_embed())
                                .components(duel.to_components()),
                        )
                        .await?;
                    continue;
                }
                tx.commit().await?;
            }
            // that succeeded, so start the duel
            break;
        }
    }

    if !duel.is_ready() {
        msg.edit(
            ctx,
            EditMessage::new()
                .content("Duel ignored.")
                .components(vec![])
                .embeds(vec![]),
        )
        .await?;
        return Ok(());
    }

    msg = channel
        .send_message(
            ctx,
            CreateMessage::new().add_file(CreateAttachment::path("public/catcoinflip.gif").await?),
        )
        .await?;
    let secs = thread_rng().gen_range(2..=5);
    tokio::time::sleep(tokio::time::Duration::from_secs(secs)).await;

    let defense_won: bool = random();

    let (winner, loser, img) = if defense_won {
        (
            duel.defense.unwrap(),
            duel.attack.unwrap(),
            "public/catcoin_defense.png",
        )
    } else {
        (
            duel.attack.unwrap(),
            duel.defense.unwrap(),
            "public/catcoin_attack.png",
        )
    };

    grant_catcoin(pool, winner.id, duel.wager * 2).await?;

    let attachment = CreateAttachment::path(img).await?;
    msg.edit(
        ctx,
        EditMessage::new()
            .attachments(EditAttachments::new().add(attachment))
            .content(format!(
                "{} beat {} in a duel and stole **+{}** {}.",
                winner.mention(),
                loser.mention(),
                duel.wager,
                emoji()
            )),
    )
    .await?;

    Ok(())
}
