use std::{
    cmp::Ordering,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use poise::serenity_prelude::{
    self as serenity, ButtonStyle, ChannelId, ComponentInteraction, ComponentInteractionCollector,
    ComponentInteractionDataKind, Context, CreateActionRow, CreateButton, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, CreateSelectMenu,
    CreateSelectMenuKind, CreateSelectMenuOption, EditMessage, Mentionable, Message, User, UserId,
};

use common::Error;
use rand::{thread_rng, Rng};
use rand_distr::{Distribution, Normal};
use sqlx::{MySql, Pool};

use crate::{format_rank_tie, Deck, Hand};
use catcoin::{get_catcoin, grant_catcoin, spend_catcoin};
use emoji::emoji;

pub async fn on_message(
    ctx: &serenity::Context,
    pool: &Pool<MySql>,
    msg: &serenity::Message,
) -> Result<(), Error> {
    let wager: u64 = {
        let mut rng = thread_rng();

        // should message spawn a game?
        if !rng.gen_ratio(1, 2000) {
            return Ok(());
        }

        // determine wager
        let dist: Normal<f32> = Normal::new(50.0, 50.0).unwrap();
        dist.sample(&mut rng).max(5.0).round().abs() as u64
    };

    create_poker_game(ctx, pool, msg.channel_id, msg.id.get(), wager).await?;

    Ok(())
}

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
    player1_redrawn_count: usize,
    player2_redrawn_count: usize,
    draw_timeout: Option<u64>, // Unix timestamp when selection time expires
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
            player1_redrawn_count: 0,
            player2_redrawn_count: 0,
            draw_timeout: None,
        }
    }

    // Create an embed that shows both players' hands with their ranks
    pub fn create_revealed_hands_embed(&self) -> CreateEmbed {
        let mut embed = CreateEmbed::new().title(format!(
            "♠️ poker - {} {} wager ♦️",
            self.wager,
            emoji("catcoin")
        ));

        let player1_hand = self.player1_hand.as_ref().unwrap();
        let player2_hand = self.player2_hand.as_ref().unwrap();

        let (player1_rank, player1_tiebreakers) = player1_hand.evaluate();
        let (player2_rank, player2_tiebreakers) = player2_hand.evaluate();

        embed = embed
            .field(
                format!(
                    "{}'s hand ({})",
                    self.player1.as_ref().unwrap().display_name(),
                    if player1_rank == player2_rank {
                        format_rank_tie(player1_rank, &player1_tiebreakers)
                    } else {
                        player1_rank.to_string()
                    }
                ),
                self.format_hand(player1_hand),
                true,
            )
            .field(
                format!(
                    "{}'s hand ({})",
                    self.player2.as_ref().unwrap().display_name(),
                    if player1_rank == player2_rank {
                        format_rank_tie(player2_rank, &player2_tiebreakers)
                    } else {
                        player2_rank.to_string()
                    }
                ),
                self.format_hand(player2_hand),
                true,
            );

        embed
    }

    // Send a reply message announcing the winner
    pub async fn send_winner_announcement(
        &self,
        ctx: &Context,
        winner_id: Option<UserId>,
        original_msg: &Message,
    ) -> Result<(), Error> {
        let player1_hand = self.player1_hand.as_ref().unwrap();
        let player2_hand = self.player2_hand.as_ref().unwrap();

        let (player1_rank, player1_tiebreakers) = player1_hand.evaluate();
        let (player2_rank, player2_tiebreakers) = player2_hand.evaluate();

        let mut winner_embed = CreateEmbed::new()
            .title("🏆 poker results 🏆")
            .color(0xFFD700); // Gold color

        if let Some(winner_id) = winner_id {
            let winner = if winner_id == self.player1.as_ref().unwrap().id {
                (
                    self.player1.as_ref().unwrap(),
                    player1_rank,
                    player1_tiebreakers.clone(),
                )
            } else {
                (
                    self.player2.as_ref().unwrap(),
                    player2_rank,
                    player2_tiebreakers.clone(),
                )
            };

            // Get loser info
            let loser = if winner_id == self.player1.as_ref().unwrap().id {
                (
                    self.player2.as_ref().unwrap(),
                    player2_rank,
                    player2_tiebreakers,
                )
            } else {
                (
                    self.player1.as_ref().unwrap(),
                    player1_rank,
                    player1_tiebreakers,
                )
            };

            // Create a more detailed description when both players have the same hand rank
            let description = format!(
                "{} stole +**{}** {} with **{}** over {}'s **{}**",
                winner.0.mention(),
                self.wager,
                emoji("catcoin"),
                if player1_rank == player2_rank {
                    format_rank_tie(winner.1, &winner.2)
                } else {
                    winner.1.to_string()
                },
                loser.0.mention(),
                if player1_rank == player2_rank {
                    format_rank_tie(loser.1, &loser.2)
                } else {
                    loser.1.to_string()
                }
            );

            winner_embed = winner_embed.description(description);
        } else {
            // For ties, mention it's a tie but player 1 wins by default
            winner_embed = winner_embed.description(format!(
                "draw!! {} catcoins have been returned >w<",
                emoji("catcoin")
            ));
        }

        // Send the winner announcement as a reply to the original message
        original_msg
            .channel_id
            .send_message(
                ctx,
                CreateMessage::new()
                    .reference_message(original_msg)
                    .embed(winner_embed),
            )
            .await?;

        Ok(())
    }

    // Abort the game with a reason, refund players if needed
    pub async fn abort_game(
        &self,
        ctx: &Context,
        pool: &Pool<MySql>,
        msg: &Message,
        reason: &str,
    ) -> Result<(), Error> {
        // Refund catcoin to players who joined if wagers were deducted
        // (Only applies when game has started but selections aren't done)
        if self.is_ready() && self.draw_timeout.is_some() && !self.are_selections_done() {
            let mut tx = pool.begin().await?;

            // Refund player 1
            if let Some(player1) = &self.player1 {
                grant_catcoin(&mut *tx, player1.id, self.wager).await?;
            }

            // Refund player 2
            if let Some(player2) = &self.player2 {
                grant_catcoin(&mut *tx, player2.id, self.wager).await?;
            }

            tx.commit().await?;

            // Send abort notification message to channel
            msg.channel_id
                .send_message(
                    ctx,
                    CreateMessage::new()
                        .content(format!("poker game aborted: {}. wagers refunded.", reason)),
                )
                .await?;
        } else {
            // Just notify about the abort
            msg.channel_id
                .send_message(
                    ctx,
                    CreateMessage::new().content(format!("poker game aborted: {}.", reason)),
                )
                .await?;
        }

        // Delete the original message
        msg.delete(ctx).await?;

        Ok(())
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
        } else {
            // Game in progress
            let player1_status = if self.player1_selected && self.player1_redrawn_count > 0 {
                format!(
                    " (redrew {} card{})",
                    self.player1_redrawn_count,
                    if self.player1_redrawn_count == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            } else if self.player1_selected {
                " (ready)".to_string()
            } else {
                "".to_string()
            };

            let player2_status = if self.player2_selected && self.player2_redrawn_count > 0 {
                format!(
                    " (redrew {} card{})",
                    self.player2_redrawn_count,
                    if self.player2_redrawn_count == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            } else if self.player2_selected {
                " (ready)".to_string()
            } else {
                "".to_string()
            };

            embed = embed
                .field(
                    format!(
                        "{}'s hand{}",
                        self.player1.as_ref().unwrap().display_name(),
                        player1_status
                    ),
                    emoji("card_back").repeat(5), // Card back emoji
                    true,
                )
                .field(
                    format!(
                        "{}'s hand{}",
                        self.player2.as_ref().unwrap().display_name(),
                        player2_status
                    ),
                    emoji("card_back").repeat(5), // Card back emoji
                    true,
                )
                .description(format!(
                    "hands finalized {}",
                    self.format_timeout_timestamp()
                ));
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

    // Deducts wager from both players, sets selection timeouts, and deals initial hands
    pub async fn deduct_wagers_and_set_timeouts(
        &mut self,
        pool: &Pool<MySql>,
        ctx: &Context,
        msg: &Message,
    ) -> Result<bool, Error> {
        if !self.is_ready() {
            return Ok(false);
        }

        let mut tx = pool.begin().await?;

        if !spend_catcoin(&mut *tx, self.player1.as_ref().unwrap().id, self.wager).await? {
            tx.rollback().await?;
            msg.channel_id
                .send_message(
                    ctx,
                    CreateMessage::new().content(format!(
                        "{} twied to cheat. game aborted",
                        self.player1.as_ref().unwrap().mention()
                    )),
                )
                .await?;
            msg.delete(ctx).await?;
            return Ok(false);
        }

        if !spend_catcoin(&mut *tx, self.player2.as_ref().unwrap().id, self.wager).await? {
            tx.rollback().await?;
            msg.channel_id
                .send_message(
                    ctx,
                    CreateMessage::new().content(format!(
                        "{} twied to cheat. game aborted",
                        self.player2.as_ref().unwrap().mention()
                    )),
                )
                .await?;
            msg.delete(ctx).await?;
            return Ok(false);
        }

        tx.commit().await?;

        // Set timeouts for both players after successful catcoin deduction
        self.set_selection_timeout();

        // Deal initial hands after catcoin deduction is successful
        self.deal_initial_hands();

        Ok(true)
    }

    // Sets 30-second timeouts for both players
    fn set_selection_timeout(&mut self) {
        // Get current time in seconds
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        // Add 30 seconds for the timeout
        let timeout = now + 30;

        self.draw_timeout = Some(timeout);
    }

    // Format a discord relative timestamp for the timeout
    fn format_timeout_timestamp(&self) -> String {
        if let Some(timestamp) = self.draw_timeout {
            format!("<t:{}:R>", timestamp)
        } else {
            "".to_string()
        }
    }

    // Checks if a player's selection time has expired
    fn is_player_timeout_expired(&self) -> bool {
        let Some(deadline) = self.draw_timeout else {
            return false;
        };

        // Get current time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        now >= deadline
    }

    // Auto-selects for players who haven't made a choice when their time expires
    fn auto_select_on_timeout(&mut self) {
        if self.is_player_timeout_expired() {
            self.player1_selected = true;
            self.player2_selected = true;
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
                .label("redraw cawds")
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

        let mut options = hand
            .cards
            .iter()
            .enumerate()
            .map(|(i, card)| {
                CreateSelectMenuOption::new(
                    format!("{}. {} of {}", i + 1, card.rank, card.suit),
                    format!("card{}", i),
                )
            })
            .collect::<Vec<_>>();

        options.push(CreateSelectMenuOption::new("none", "none"));

        CreateActionRow::SelectMenu(
            CreateSelectMenu::new(custom_id, CreateSelectMenuKind::String { options })
                .placeholder("select cards to redraw (or none)")
                .min_values(0)
                .max_values(6),
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
                if selections.contains(&"none".to_owned()) {
                    self.player1_selected = true;
                    return;
                }
                // Update redrawn count before redrawing
                self.player1_redrawn_count = indices.len();
                hand.redraw(&mut self.deck, &indices);
                hand.sort();
                self.player1_selected = true;
            }
        } else if player_num == 2 && !self.player2_selected {
            if let Some(hand) = self.player2_hand.as_mut() {
                if selections.contains(&"none".to_owned()) {
                    self.player2_selected = true;
                    return;
                }
                // Update redrawn count before redrawing
                self.player2_redrawn_count = indices.len();
                hand.redraw(&mut self.deck, &indices);
                hand.sort();
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

    // Update embed and components of a message
    pub async fn update_embed(
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

    // Respond to an interaction with an error message
    pub async fn err_respond(
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

    // Set up a shorter timeout for the component collector to allow regular timeout checks
    let collector_timeout = Duration::from_secs(3); // Check every 10 seconds

    // Wait for players to join and handle gameplay
    loop {
        let interaction = ComponentInteractionCollector::new(ctx)
            .channel_id(channel)
            .timeout(collector_timeout)
            .filter(move |mci| mci.data.custom_id.starts_with(&uuid.to_string()))
            .await;

        // Check timeouts and auto-select for players who didn't make a choice in time
        if poker.is_ready() && !poker.are_selections_done() && poker.is_player_timeout_expired() {
            poker.auto_select_on_timeout();
            update_embed(ctx, &mut msg, &poker).await?;

            // If both players have made their selections (due to auto-selection), break out
            if poker.are_selections_done() {
                break;
            }
        }

        // Abort if we've been waiting too long for someone to join
        // This prevents the game from running forever if nobody joins
        if !poker.is_ready() && interaction.is_none() {
            let elapsed = msg.timestamp.unix_timestamp() + 600 < chrono::Utc::now().timestamp();
            if elapsed {
                poker
                    .abort_game(ctx, pool, &msg, "timed out waiting for players")
                    .await?;
                return Ok(());
            }
        }

        // If no interaction occurred in this timeout window, continue checking
        let Some(mci) = interaction else {
            continue;
        };

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

            // If this was the second player joining, deduct catcoin and set timeouts immediately
            if poker.is_ready() {
                if !poker
                    .deduct_wagers_and_set_timeouts(pool, ctx, &msg)
                    .await?
                {
                    // If deduction failed, the method already handled the error messaging
                    continue;
                }
            }

            update_embed(ctx, &mut msg, &poker).await?;
            mci.create_response(ctx, CreateInteractionResponse::Acknowledge)
                .await?;
        } else if mci.data.custom_id == view_id {
            let Some(player_num) = poker.get_player_num(user.id) else {
                err_respond(ctx, &mci, "nope not u!!").await?;
                continue;
            };

            let Some(hand) = poker.get_hand(player_num).cloned() else {
                err_respond(ctx, &mci, "hands haven't been dealt yet, please try again").await?;
                continue;
            };

            // Send ephemeral message with player's cards, selection menu, and timeout
            let hand_text = poker.format_hand_with_rank(&hand);
            let timeout_text = format!(
                "\n\nselection time ends {}",
                poker.format_timeout_timestamp()
            );

            mci.create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("{}{}", hand_text, timeout_text))
                        .ephemeral(true)
                        .components(vec![poker.get_card_select_menu(player_num)]),
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

            if selections.contains(&"none".to_owned()) {
                mci.create_response(ctx, CreateInteractionResponse::Acknowledge)
                    .await?;
            } else {
                mci.create_response(
                    ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(format!(
                                "you redrew {} card(s).\n{}",
                                selections.len(),
                                hand_text
                            ))
                            .ephemeral(true),
                    ),
                )
                .await?;
            }

            update_embed(ctx, &mut msg, &poker).await?;

            // If both players have made their selections (after this interaction), break out
            if poker.is_ready() && poker.are_selections_done() {
                break;
            }
        }
    }

    // If game didn't complete (timeout, etc.), clean up
    if !poker.is_ready() || !poker.are_selections_done() {
        poker
            .abort_game(ctx, pool, &msg, "game broke >//< aborted..")
            .await?;
        return Ok(());
    }

    // Determine winner and update embed
    let player1_hand = poker.player1_hand.as_ref().unwrap();
    let player2_hand = poker.player2_hand.as_ref().unwrap();

    let comparison = player1_hand.compare(player2_hand);

    let winner_id: Option<UserId> = match comparison {
        Ordering::Greater => {
            let winner = poker.player1.as_ref().unwrap().id;
            grant_catcoin(pool, winner, wager * 2).await?;
            Some(winner)
        }
        Ordering::Less => {
            let winner = poker.player2.as_ref().unwrap().id;
            grant_catcoin(pool, winner, wager * 2).await?;
            Some(winner)
        }
        Ordering::Equal => {
            let a = poker.player2.as_ref().unwrap().id;
            let b = poker.player1.as_ref().unwrap().id;
            grant_catcoin(pool, a, wager).await?;
            grant_catcoin(pool, b, wager).await?;
            None
        }
    };

    // Create a results embed for final hand reveal
    let revealed_hands_embed = poker.create_revealed_hands_embed();

    // Update original message with revealed hands
    msg.edit(
        ctx,
        EditMessage::new()
            .embed(revealed_hands_embed)
            .components(vec![]),
    )
    .await?;

    // Create and send winner announcement as reply
    poker.send_winner_announcement(ctx, winner_id, &msg).await?;

    Ok(())
}
