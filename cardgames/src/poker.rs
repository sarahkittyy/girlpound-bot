use std::time::Duration;

use poise::serenity_prelude::{
    ButtonStyle, ChannelId, ComponentInteraction, ComponentInteractionCollector,
    ComponentInteractionDataKind, Context, CreateActionRow, CreateButton, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, CreateSelectMenu,
    CreateSelectMenuKind, CreateSelectMenuOption, EditMessage, Mentionable, Message, User, UserId,
};

use common::Error;
use sqlx::{MySql, Pool};

use crate::{Deck, Hand};
use catcoin::{get_catcoin, grant_catcoin, spend_catcoin};
use emoji::emoji;

pub struct PokerLobby {
    uuid: String,
    wager: u64,
    player1: Option<User>,
    player2: Option<User>,
    deck: Deck,
    player1_hand: Option<Hand>,
    player2_hand: Option<Hand>,
    player1_selected: bool,
    player2_selected: bool,
}

impl PokerLobby {
    pub fn new(wager: u64, uuid: &str) -> Self {
        let mut deck = Deck::new();
        deck.shuffle();

        Self {
            uuid: uuid.to_owned(),
            wager,
            player1: None,
            player2: None,
            deck,
            player1_hand: None,
            player2_hand: None,
            player1_selected: false,
            player2_selected: false,
        }
    }

    pub fn to_embed(&self) -> CreateEmbed {
        let mut embed = CreateEmbed::new().title(format!(
            "♠️ poker - {} {} wager ♦️",
            self.wager,
            emoji("catcoin")
        ));

        if !self.is_ready() {
            // Lobby phase
            embed = embed
                .field(
                    format!("{} defense player", emoji("defense_position")),
                    self.player1
                        .as_ref()
                        .map(|p| p.mention().to_string())
                        .unwrap_or("open".to_owned()),
                    true,
                )
                .field(
                    format!("attack player {}", emoji("attack_position")),
                    self.player2
                        .as_ref()
                        .map(|p| p.mention().to_string())
                        .unwrap_or("open".to_owned()),
                    true,
                );
        } else if !self.are_selections_done() {
            // Game in progress
            embed = embed
                .field(
                    format!("{}'s hand", self.player1.as_ref().unwrap().display_name()),
                    emoji("card_back").repeat(5), // Card back emoji
                    true,
                )
                .field(
                    format!("{}'s hand", self.player2.as_ref().unwrap().display_name()),
                    emoji("card_back").repeat(5), // Card back emoji
                    true,
                );

            // Show who has finished their selection
            let mut status = String::new();
            if self.player1_selected {
                status.push_str(&format!(
                    "{} is ready\n",
                    self.player1.as_ref().unwrap().display_name()
                ));
            }
            if self.player2_selected {
                status.push_str(&format!(
                    "{} is ready\n",
                    self.player2.as_ref().unwrap().display_name()
                ));
            }

            if !status.is_empty() {
                embed = embed.field("Status", status, false);
            }
        } else {
            // Game complete
            let player1_hand = self.player1_hand.as_ref().unwrap();
            let player2_hand = self.player2_hand.as_ref().unwrap();

            let (player1_rank, _) = player1_hand.evaluate();
            let (player2_rank, _) = player2_hand.evaluate();

            let comparison = player1_hand.compare(player2_hand);

            let winner = match comparison {
                std::cmp::Ordering::Greater => self.player1.as_ref().unwrap(),
                std::cmp::Ordering::Less => self.player2.as_ref().unwrap(),
                std::cmp::Ordering::Equal => {
                    // In case of a tie, we could use a tiebreaker or split the pot
                    // For now, let's just pick player 1 as a winner in a tie
                    self.player1.as_ref().unwrap()
                }
            };

            embed = embed
                .field(
                    format!(
                        "{}'s hand ({})",
                        self.player1.as_ref().unwrap().name,
                        player1_rank
                    ),
                    "Cards will be shown here", // This will be replaced with actual cards in a later step
                    false,
                )
                .field(
                    format!(
                        "{}'s hand ({})",
                        self.player2.as_ref().unwrap().name,
                        player2_rank
                    ),
                    "Cards will be shown here", // This will be replaced with actual cards in a later step
                    false,
                )
                .field(
                    "Winner",
                    format!(
                        "{} wins **{}** {}!",
                        winner.mention(),
                        self.wager * 2,
                        emoji("catcoin")
                    ),
                    false,
                );
        }

        embed
    }

    fn try_register_player(&mut self, user: User) -> Result<(), &'static str> {
        if self.player1.is_none() {
            self.player1 = Some(user);
        } else if self.player2.is_none() {
            self.player2 = Some(user);
        } else {
            return Err("da lobby is full >//<");
        }
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.player1.is_some() && self.player2.is_some()
    }

    fn are_selections_done(&self) -> bool {
        self.player1_selected && self.player2_selected
    }

    fn deal_initial_hands(&mut self) {
        if self.is_ready() {
            self.player1_hand = Some(self.deck.deal_hand());
            self.player2_hand = Some(self.deck.deal_hand());
        }
    }

    fn to_components(&self) -> Vec<CreateActionRow> {
        if !self.is_ready() {
            // Lobby phase - join button

            if !self.is_ready() {
                let button = CreateButton::new(format!("{}-join", self.uuid))
                    .label("join ^-^")
                    .style(ButtonStyle::Primary)
                    .emoji('🃏');
                vec![CreateActionRow::Buttons(vec![button])]
            } else {
                vec![]
            }
        } else if !self.are_selections_done() {
            // Game in progress - view cards and select cards to redraw
            let view = CreateButton::new(format!("{}-view", self.uuid))
                .label("view cawds")
                .style(ButtonStyle::Success)
                .emoji('👀');

            vec![CreateActionRow::Buttons(vec![view])]
        } else {
            // Game complete - no buttons needed
            vec![]
        }
    }

    fn get_card_select_menu(&self, player_num: u8) -> CreateActionRow {
        let custom_id = format!("{}-select", self.uuid);

        let Some(hand) = self.get_hand(player_num) else {
            panic!("cannot get card select manu if no hand is dealt");
        };

        let options = hand
            .cards
            .iter()
            .enumerate()
            .map(|(i, card)| {
                CreateSelectMenuOption::new(
                    format!("{} of {}", card.rank, card.suit),
                    format!("card{}", i),
                )
                .description(format!("select to redraw card {}", i + 1))
            })
            .collect::<Vec<_>>();

        CreateActionRow::SelectMenu(
            CreateSelectMenu::new(custom_id, CreateSelectMenuKind::String { options })
                .placeholder("select cards to redraw (or none)")
                .min_values(0)
                .max_values(5),
        )
    }

    fn format_hand(&self, hand: &Hand) -> String {
        let mut result = String::new();

        // ranks
        for card in hand.cards.iter() {
            result.push_str(&format!("{}", card.rank.emoji(card.suit.is_red())));
        }
        result.push_str("\n");
        // suits
        for card in hand.cards.iter() {
            result.push_str(&format!("{}", card.suit.emoji()));
        }

        result
    }

    fn format_hand_with_rank(&self, hand: &Hand) -> String {
        let mut result = String::new();

        // hand display
        result.push_str(&self.format_hand(hand));

        // hand evaluation
        let (rank, _) = hand.evaluate();
        result.push_str(&format!("\n**{}**", rank));

        result
    }

    fn has_selected(&self, player_num: u8) -> bool {
        match player_num {
            1 => self.player1_selected,
            2 => self.player2_selected,
            _ => false,
        }
    }

    fn process_card_selection(&mut self, player_num: u8, selections: &[String]) {
        let indices: Vec<usize> = selections
            .iter()
            .filter_map(|s| {
                if s.starts_with("card") {
                    s[4..].parse::<usize>().ok()
                } else {
                    None
                }
            })
            .collect();

        if player_num == 1 && !self.player1_selected {
            if let Some(hand) = self.player1_hand.as_mut() {
                hand.redraw(&mut self.deck, &indices);
                self.player1_selected = true;
            }
        } else if player_num == 2 && !self.player2_selected {
            if let Some(hand) = self.player2_hand.as_mut() {
                hand.redraw(&mut self.deck, &indices);
                self.player2_selected = true;
            }
        }
    }

    fn get_player_num(&self, user: UserId) -> Option<u8> {
        if self.player1.as_ref().map(|p| p.id) == Some(user) {
            Some(1)
        } else if self.player2.as_ref().map(|p| p.id) == Some(user) {
            Some(2)
        } else {
            None
        }
    }

    fn get_hand(&self, player_num: u8) -> Option<&Hand> {
        match player_num {
            1 => self.player1_hand.as_ref(),
            2 => self.player2_hand.as_ref(),
            _ => None,
        }
    }

    fn get_hand_or_deal(&mut self, player_num: u8) -> &Hand {
        match player_num {
            1 => {
                if self.player1_hand.is_none() {
                    self.deal_initial_hands();
                }
                self.player1_hand.as_ref().unwrap()
            }
            2 => {
                if self.player2_hand.is_none() {
                    self.deal_initial_hands();
                }
                self.player2_hand.as_ref().unwrap()
            }
            _ => panic!("Invalid player number"),
        }
    }

    fn update_embed_with_final_hands(&self, mut embed: CreateEmbed) -> CreateEmbed {
        let player1_hand = self.player1_hand.as_ref().unwrap();
        let player2_hand = self.player2_hand.as_ref().unwrap();

        let (player1_rank, _) = player1_hand.evaluate();
        let (player2_rank, _) = player2_hand.evaluate();

        embed = embed.field(
            format!(
                "{}'s hand ({})",
                self.player1.as_ref().unwrap().name,
                player1_rank
            ),
            self.format_hand(player1_hand),
            false,
        );

        embed = embed.field(
            format!(
                "{}'s hand ({})",
                self.player2.as_ref().unwrap().name,
                player2_rank
            ),
            self.format_hand(player2_hand),
            false,
        );

        let comparison = player1_hand.compare(player2_hand);
        let winner = match comparison {
            std::cmp::Ordering::Greater => self.player1.as_ref().unwrap(),
            std::cmp::Ordering::Less => self.player2.as_ref().unwrap(),
            std::cmp::Ordering::Equal => self.player1.as_ref().unwrap(), // Default to player 1 on tie
        };

        embed = embed.field(
            "Winner",
            format!(
                "{} wins **{}** {}!",
                winner.mention(),
                self.wager * 2,
                emoji("catcoin")
            ),
            false,
        );

        embed
    }
}

pub async fn create_poker_game(
    ctx: &Context,
    pool: &Pool<MySql>,
    channel: ChannelId,
    uuid: u64,
    wager: u64,
) -> Result<(), Error> {
    let mut poker = PokerLobby::new(wager, &uuid.to_string());

    let join_id = format!("{uuid}-join");
    let view_id = format!("{uuid}-view");
    let select_id = format!("{uuid}-select");

    let mut msg = channel
        .send_message(
            ctx,
            CreateMessage::new()
                .embed(poker.to_embed())
                .components(poker.to_components()),
        )
        .await?;

    async fn update_embed(
        ctx: &Context,
        msg: &mut Message,
        poker: &PokerLobby,
    ) -> Result<(), Error> {
        msg.edit(
            ctx,
            EditMessage::new()
                .embed(poker.to_embed())
                .components(poker.to_components()),
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

    // Wait for players to join
    while let Some(mci) = ComponentInteractionCollector::new(ctx)
        .channel_id(channel)
        .timeout(Duration::from_secs(600))
        .filter(move |mci| mci.data.custom_id.starts_with(&uuid.to_string()))
        .await
    {
        let user = mci.user.clone();

        if mci.data.custom_id == join_id {
            // Check if user has enough catcoin
            if get_catcoin(pool, user.id).await?.catcoin < wager as i64 {
                err_respond(ctx, &mci, "not today brokie").await?;
                continue;
            }

            if let Err(e) = poker.try_register_player(user) {
                err_respond(ctx, &mci, e).await?;
                continue;
            }

            update_embed(ctx, &mut msg, &poker).await?;
            mci.create_response(ctx, CreateInteractionResponse::Acknowledge)
                .await?;
        } else if mci.data.custom_id == view_id {
            let Some(player_num) = poker.get_player_num(user.id) else {
                err_respond(ctx, &mci, "nope not u!!").await?;
                continue;
            };

            let hand = poker.get_hand_or_deal(player_num).clone();
            // Send ephemeral message with player's cards and selection menu
            let hand_text = poker.format_hand_with_rank(&hand);

            mci.create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(hand_text)
                        .ephemeral(true)
                        .components(vec![poker.get_card_select_menu(1)]),
                ),
            )
            .await?;
        } else if mci.data.custom_id == select_id {
            let Some(player_num) = poker.get_player_num(user.id) else {
                err_respond(ctx, &mci, "nope not u!!").await?;
                continue;
            };

            if poker.has_selected(player_num) {
                err_respond(ctx, &mci, "u already picked!").await?;
                continue;
            }

            let selections = match mci.data.kind {
                ComponentInteractionDataKind::StringSelect { ref values } => values.clone(),
                _ => {
                    err_respond(ctx, &mci, "Invalid selection kind.").await?;
                    continue;
                }
            };
            poker.process_card_selection(player_num, &selections);

            let hand = poker.get_hand_or_deal(player_num).clone();

            // Get the updated hand
            let hand_text = poker.format_hand_with_rank(&hand);

            mci.create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!(
                            "u redrew {} cards. ur new hand:\n\n{}",
                            selections.len(),
                            hand_text
                        ))
                        .ephemeral(true),
                ),
            )
            .await?;

            update_embed(ctx, &mut msg, &poker).await?;
        }

        // If both players have made their selections, determine winner
        if poker.is_ready() && poker.are_selections_done() {
            break;
        }
    }

    // If game didn't complete (timeout, etc.), clean up
    if !poker.is_ready() || !poker.are_selections_done() {
        msg.delete(ctx).await?;
        return Ok(());
    }

    // Deduct wagers from both players
    {
        let mut tx = pool.begin().await?;

        if !spend_catcoin(&mut *tx, poker.player1.as_ref().unwrap().id, wager).await? {
            tx.rollback().await?;
            msg.channel_id
                .send_message(
                    ctx,
                    CreateMessage::new().content(format!(
                        "{} doesn't have enough catcoin! Game cancelled.",
                        poker.player1.as_ref().unwrap().mention()
                    )),
                )
                .await?;
            msg.delete(ctx).await?;
            return Ok(());
        }

        if !spend_catcoin(&mut *tx, poker.player2.as_ref().unwrap().id, wager).await? {
            tx.rollback().await?;
            msg.channel_id
                .send_message(
                    ctx,
                    CreateMessage::new().content(format!(
                        "{} doesn't have enough catcoin! Game cancelled.",
                        poker.player2.as_ref().unwrap().mention()
                    )),
                )
                .await?;
            msg.delete(ctx).await?;
            return Ok(());
        }

        tx.commit().await?;
    }

    // Determine winner and update embed
    let player1_hand = poker.player1_hand.as_ref().unwrap();
    let player2_hand = poker.player2_hand.as_ref().unwrap();

    let comparison = player1_hand.compare(player2_hand);

    let winner_id = match comparison {
        std::cmp::Ordering::Greater => poker.player1.as_ref().unwrap().id,
        std::cmp::Ordering::Less => poker.player2.as_ref().unwrap().id,
        std::cmp::Ordering::Equal => poker.player1.as_ref().unwrap().id, // Default to player 1 on tie
    };

    // Award prize to winner
    grant_catcoin(pool, winner_id, wager * 2).await?;

    // Update the message with final results
    let mut embed = poker.to_embed();
    embed = poker.update_embed_with_final_hands(embed);

    msg.edit(
        ctx,
        EditMessage::new().embed(embed).components(vec![]), // No components needed at the end
    )
    .await?;

    Ok(())
}
