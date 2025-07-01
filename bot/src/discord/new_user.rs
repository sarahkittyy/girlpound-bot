use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use catcoin::{get_catcoin, grant_catcoin, spend_catcoin};
use poise::{
    Modal,
    serenity_prelude::{
        self as serenity, ComponentInteraction, CreateActionRow, CreateButton,
        CreateInteractionResponse, CreateInteractionResponseMessage, EditMessage, MessageId,
        ReactionType, UserId,
    },
};
use regex::Regex;
use serenity::{ChannelId, CreateMessage, Member, Mentionable};

use common::{Error, discord::execute_modal_generic};
use rand::prelude::*;

use crate::discord::PoiseData;

#[derive(Debug)]
pub enum BetResult {
    Success,
    Error(String),
}

#[derive(Debug)]
pub struct NSFWWinResult {
    pub user_id: UserId,
    pub actual_seconds: u64,
    pub winner_id: UserId,
    pub winner_guess: u64,
    pub pool_coin: u64,
}

#[derive(Debug)]
pub struct HourPassedResult {
    pub user_id: UserId,
    pub winners: Vec<UserId>,
    pub win_amount: u64,
}

pub struct NSFWBets {
    pub pools: HashMap<UserId, NSFWBet>,
}

impl NSFWBets {
    pub fn new() -> Self {
        NSFWBets {
            pools: HashMap::new(),
        }
    }

    pub fn create_pool(
        &mut self,
        user: UserId,
        wager: u64,
        message_id: MessageId,
        channel_id: ChannelId,
    ) {
        self.pools
            .insert(user, NSFWBet::new(user, wager, message_id, channel_id));
    }

    pub fn get_pool_mut(&mut self, user: UserId) -> Option<&mut NSFWBet> {
        self.pools.get_mut(&user)
    }

    pub fn get_pool(&self, user: UserId) -> Option<&NSFWBet> {
        self.pools.get(&user)
    }

    fn get_bet_button(&self, uid: UserId) -> Result<Vec<CreateActionRow>, Error> {
        let button = CreateButton::new("newuser.bet")
            .label(&format!(
                "Bet ({} coin)",
                self.get_pool(uid)
                    .ok_or("Could not find NSFWBet pool.")?
                    .wager
            ))
            .emoji(ReactionType::try_from(emoji::emoji("catcoin"))?);
        return Ok(vec![CreateActionRow::Buttons(vec![button])]);
    }

    pub fn remove_pool(&mut self, user: UserId) {
        self.pools.remove(&user);
    }

    /// Disables the betting button on a message
    async fn disable_betting_button(
        &self,
        ctx: &serenity::Context,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> Result<(), Error> {
        let disabled_button = CreateButton::new("newuser.bet.disabled")
            .label("Betting Closed")
            .disabled(true)
            .emoji(ReactionType::try_from("❌")?);

        let components = vec![CreateActionRow::Buttons(vec![disabled_button])];

        channel_id
            .edit_message(ctx, message_id, EditMessage::new().components(components))
            .await?;

        Ok(())
    }

    /// Creates a betting pool for a new user and returns the button components
    pub fn on_join(
        &mut self,
        user_id: UserId,
        message_id: MessageId,
        channel_id: ChannelId,
        wager: u64,
    ) -> Result<Vec<CreateActionRow>, Error> {
        self.create_pool(user_id, wager, message_id, channel_id);
        self.get_bet_button(user_id)
    }

    /// Handles a bet attempt, returns success/error message
    pub async fn on_bet(
        &mut self,
        user_id: UserId,
        bettor_id: UserId,
        message_id: MessageId,
        guess: String,
        local_pool: &sqlx::MySqlPool,
    ) -> Result<BetResult, Error> {
        // Check if this is the user's own bet
        if bettor_id == user_id {
            return Ok(BetResult::Error("Not you!".to_string()));
        }

        // Check if pool exists and message matches
        let Some(bet) = self.get_pool_mut(user_id) else {
            return Ok(BetResult::Error(
                "Betting pool no longer exists.".to_string(),
            ));
        };

        if bet.message_id != message_id {
            return Ok(BetResult::Error(
                "This betting button is outdated.".to_string(),
            ));
        }

        // Check if user has enough coin
        let amnt = get_catcoin(local_pool, bettor_id).await?;
        if amnt.catcoin < bet.wager as i64 {
            return Ok(BetResult::Error(
                "You are way too broke to bet on this user.".to_string(),
            ));
        }

        // Parse the guess
        let total_seconds = if guess.to_ascii_lowercase() == "never" {
            None
        } else {
            // Parse time format MM:SS
            let re =
                Regex::new(r"^(\d{2}):(\d{2})$").expect("regex error in new user betting modal");
            let Some(caps) = re.captures(&guess) else {
                return Ok(BetResult::Error(
                    "Invalid time format. Try again :3. Example: 12:34".to_string(),
                ));
            };
            let (_, [minutes, seconds]) = caps.extract::<2>();
            let minutes: u64 = minutes.parse()?;
            let seconds: u64 = seconds.parse()?;
            Some(minutes * 60 + seconds)
        };

        // Spend the coin
        if !bet.has_bet_already(bettor_id) {
            if !spend_catcoin(local_pool, bettor_id, bet.wager).await? {
                return Ok(BetResult::Error(
                    "You are way too broke to bet on this user.".to_string(),
                ));
            }

            // Add the bet
            bet.add_bet(bettor_id, total_seconds);
            Ok(BetResult::Success)
        } else {
            return Ok(BetResult::Error("You have already bet.".to_string()));
        }
    }

    /// Handles when user gets NSFW role, returns winner info and message
    pub async fn on_nsfw_role_assigned(
        &mut self,
        ctx: &serenity::Context,
        user_id: UserId,
        actual_seconds: u64,
        local_pool: &sqlx::MySqlPool,
    ) -> Result<Option<NSFWWinResult>, Error> {
        let Some(bet) = self.get_pool_mut(user_id) else {
            return Ok(None);
        };

        let channel_id = bet.channel_id;
        let message_id = bet.message_id;

        // If only one person bet, silently refund them and return None (no gambling message)
        if bet.bets.len() == 1 {
            for (bettor, _) in &bet.bets {
                grant_catcoin(local_pool, *bettor, bet.wager).await?;
            }
            self.disable_betting_button(ctx, channel_id, message_id)
                .await?;
            self.remove_pool(user_id);
            return Ok(None);
        }

        if let Some((winner_id, winner_guess)) = bet.get_winner(actual_seconds) {
            // Calculate total pool
            let pool_coin: u64 = bet.wager * bet.bets.len() as u64;

            // Grant winner the coins
            grant_catcoin(local_pool, winner_id, pool_coin).await?;

            let result = NSFWWinResult {
                user_id,
                actual_seconds,
                winner_id,
                winner_guess,
                pool_coin,
            };

            // Disable the button and remove the pool
            self.disable_betting_button(ctx, channel_id, message_id)
                .await?;
            self.remove_pool(user_id);

            Ok(Some(result))
        } else {
            // No winner, disable button and remove pool
            self.disable_betting_button(ctx, channel_id, message_id)
                .await?;
            self.remove_pool(user_id);
            Ok(None)
        }
    }

    /// Handles hour timeout, awards "never" bettors and returns result
    pub async fn on_hour_passed(
        &mut self,
        ctx: &serenity::Context,
        user_id: UserId,
        local_pool: &sqlx::MySqlPool,
    ) -> Result<Option<HourPassedResult>, Error> {
        let Some(bet) = self.get_pool_mut(user_id) else {
            return Ok(None);
        };

        let channel_id = bet.channel_id;
        let message_id = bet.message_id;

        // If only one person bet, silently refund them and return None (no message)
        if bet.bets.len() == 1 {
            for (bettor, _) in &bet.bets {
                grant_catcoin(local_pool, *bettor, bet.wager).await?;
            }
            self.disable_betting_button(ctx, channel_id, message_id)
                .await?;
            self.remove_pool(user_id);
            return Ok(None);
        }

        let winners = bet.get_never_bets();

        if winners.is_empty() {
            // Disable button and remove pool
            self.disable_betting_button(ctx, channel_id, message_id)
                .await?;
            self.remove_pool(user_id);
            return Ok(None);
        }

        let pool_coin: u64 = bet.wager * bet.bets.len() as u64;
        let win_amount: u64 = (pool_coin as f64 / winners.len() as f64).ceil() as u64;

        // Award each winner
        for winner in &winners {
            grant_catcoin(local_pool, *winner, win_amount).await?;
        }

        let result = HourPassedResult {
            user_id,
            winners: winners.clone(),
            win_amount,
        };

        // Disable the button and remove the pool
        self.disable_betting_button(ctx, channel_id, message_id)
            .await?;
        self.remove_pool(user_id);

        Ok(Some(result))
    }

    /// Handles user leaving, refunds all bettors
    pub async fn on_leave(
        &mut self,
        ctx: &serenity::Context,
        user_id: UserId,
        local_pool: &sqlx::MySqlPool,
    ) -> Result<(), Error> {
        if let Some(bet) = self.get_pool(user_id) {
            let channel_id = bet.channel_id;
            let message_id = bet.message_id;

            // Refund all bettors
            for (bettor, _) in &bet.bets {
                grant_catcoin(local_pool, *bettor, bet.wager).await?;
            }

            // Disable the button and remove pool
            self.disable_betting_button(ctx, channel_id, message_id)
                .await?;
            self.remove_pool(user_id);
        }
        Ok(())
    }
}

pub struct NSFWBet {
    // map of users to their time bets (in seconds), or None for never
    pub bets: HashMap<UserId, Option<u64>>,
    // the user being gambled on
    pub uid: UserId,
    // how much coin each user is wagering
    pub wager: u64,
    // the message ID of the latest join message
    pub message_id: MessageId,
    // the channel ID where the message was sent
    pub channel_id: ChannelId,
}

impl NSFWBet {
    fn new(uid: UserId, wager: u64, message_id: MessageId, channel_id: ChannelId) -> Self {
        NSFWBet {
            bets: HashMap::new(),
            uid,
            wager,
            message_id,
            channel_id,
        }
    }

    fn add_bet(&mut self, user: UserId, seconds: Option<u64>) {
        self.bets.insert(user, seconds);
    }

    fn has_bet_already(&self, user: UserId) -> bool {
        self.bets.contains_key(&user)
    }

    pub fn get_never_bets(&self) -> Vec<UserId> {
        self.bets
            .iter()
            .filter_map(|(user, &seconds)| if seconds.is_none() { Some(*user) } else { None })
            .collect()
    }

    pub fn get_winner(&self, actual_seconds: u64) -> Option<(UserId, u64)> {
        self.bets
            .iter()
            .filter_map(|(user, seconds)| {
                if let Some(seconds) = seconds {
                    Some((user, seconds))
                } else {
                    None
                }
            })
            .min_by_key(|&(_, &bet_seconds)| (actual_seconds as i64 - bet_seconds as i64).abs())
            .map(|(user, &seconds)| (*user, seconds))
    }
}

#[derive(Debug, Modal)]
#[name = "New User Gambling"]
pub struct BetModal {
    #[name = "Enter time (MM:SS) or 'never'"]
    #[placeholder = "12:34"]
    #[min_length = 5]
    #[max_length = 5]
    pub guess: String,
}

/// Handles bet button interactions from the dispatch system
pub async fn bet_button(
    ctx: &serenity::Context,
    data: &PoiseData,
    mci: &ComponentInteraction,
) -> Result<(), Error> {
    // Show modal for bet input
    if let Some(response) = execute_modal_generic::<BetModal, _>(
        ctx,
        |resp| mci.create_response(ctx, resp),
        mci.id.to_string(),
        None,
        None,
    )
    .await?
    {
        // Parse modal response
        let bm = BetModal::parse(response.data.clone())?;

        // Find the user being bet on from the message context
        // We need to extract user ID from the message or context
        let message = &mci.message;
        let user_id = message
            .mentions
            .first()
            .ok_or("Could not find user being bet on")?
            .id;

        // Process the bet
        let result = data
            .nsfwbets
            .write()
            .await
            .on_bet(
                user_id,
                response.user.id,
                message.id,
                bm.guess,
                &data.local_pool,
            )
            .await?;

        // Send response based on result
        let content = match result {
            BetResult::Success => "Bet placed!! >w<".to_string(),
            BetResult::Error(msg) => msg,
        };

        response
            .create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(content)
                        .ephemeral(true),
                ),
            )
            .await?;
    }

    Ok(())
}

/// Sends a welcome message when a user joins the server
pub async fn welcome_user(
    ctx: &serenity::Context,
    data: &PoiseData,
    new_member: &Member,
) -> Result<(), Error> {
    const INTROS: &'static [&'static str] = &[
        "welcome to tiny kitty's girl pound",
        "haiiiii ^_^ hi!! hiiiiii <3 haiiiiii hii :3",
        "gweetings fwom tiny kitty's girl pound",
        "o-omg hii.. >///<",
        "welcome to da girl pound <3",
        "hello girl pounder",
        "hii lol >w<",
        "can we run these dogshit ass pugs",
        "heyyyyyyyyyyy... <3",
        "bounce up and down on it",
        "rtv for puppy pawjob with a twist",
        "!!! meow~<3",
        "hi fag <3",
    ];

    let new_member = new_member.clone();

    if let Some(guild) = new_member.guild_id.to_guild_cached(ctx).map(|g| g.clone()) {
        if let Some(sid) = guild.system_channel_id {
            let r = (random::<f32>() * INTROS.len() as f32).floor() as usize;
            let g = (random::<f32>() * guild.emojis.len() as f32).floor() as usize;
            let emoji = guild.emojis.values().skip(g).next();
            let wager: u64 = thread_rng().gen_range(5..25);

            let mut msg = sid
                .send_message(
                    ctx,
                    CreateMessage::new().content(&format!(
                        "{} {} {} | total meowmbers: {}",
                        emoji
                            .map(|e| e.to_string())
                            .unwrap_or(":white_check_mark:".to_string()),
                        new_member.mention(),
                        INTROS[r],
                        guild.member_count
                    )),
                )
                .await?;

            // Create betting pool with message ID and add components
            let components = data.nsfwbets.write().await.on_join(
                new_member.user.id,
                msg.id,
                msg.channel_id,
                wager,
            )?;

            // Edit message to add betting button
            msg.edit(ctx, serenity::EditMessage::new().components(components))
                .await?;
            let _ = msg.react(&ctx, '🐈').await?;

            // spawn 1 hour cooldown thread
            {
                let new_member = new_member.clone();
                let ctx = ctx.clone();
                let pool = data.local_pool.clone();
                let horny_role = data.horny_role;
                let general_channel = data.general_channel.clone();
                let nsfwbets = data.nsfwbets.clone();
                tokio::task::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;

                    // First check if betting pool still exists (could have been resolved already)
                    let pool_exists = nsfwbets.read().await.get_pool(new_member.user.id).is_some();
                    if !pool_exists {
                        // Pool was already resolved (user got role, left, etc.)
                        return;
                    }

                    // Try to get current member info
                    match ctx
                        .http
                        .get_member(new_member.guild_id, new_member.user.id)
                        .await
                    {
                        Ok(member) => {
                            // User is still in server
                            if !member.roles.contains(&horny_role) {
                                // Handle hour timeout - user didn't get role
                                if let Ok(Some(result)) = nsfwbets
                                    .write()
                                    .await
                                    .on_hour_passed(&ctx, member.user.id, &pool)
                                    .await
                                {
                                    // Send message to general channel
                                    let _ = general_channel
                                        .send_message(
                                            &ctx,
                                            CreateMessage::new().content(format!(
                                                "{} lasted 1 hour. {} **+{}** to the **{}** who guessed 'never'!\n",
                                                member.mention(),
                                                emoji::emoji("catcoin"),
                                                result.win_amount,
                                                result.winners.len()
                                            )),
                                        )
                                        .await;
                                }
                            }
                            // If user has horny_role, pool was already resolved by nsfw_callout.rs
                        }
                        Err(_) => {
                            // User left the server - clean up the betting pool
                            let _ = nsfwbets
                                .write()
                                .await
                                .on_leave(&ctx, new_member.user.id, &pool)
                                .await;
                        }
                    }
                });
            }
        }
    }

    Ok(())
}
