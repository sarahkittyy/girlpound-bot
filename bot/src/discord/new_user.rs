use std::collections::HashMap;

use catcoin::get_catcoin;
use poise::{
    Modal,
    serenity_prelude::{
        self as serenity, ComponentInteractionCollector, CreateActionRow, CreateButton,
        CreateInteractionResponseMessage, ReactionType, UserId,
    },
};
use regex::Regex;
use serenity::{CreateMessage, Member, Mentionable};

use common::{Error, discord::execute_modal_generic};
use rand::prelude::*;

use crate::discord::PoiseData;

pub struct NSFWBets {
    pub pools: HashMap<UserId, NSFWBet>,
}

impl NSFWBets {
    pub fn new() -> Self {
        NSFWBets {
            pools: HashMap::new(),
        }
    }

    pub fn create_pool(&mut self, user: UserId, wager: u64) {
        self.pools.insert(user, NSFWBet::new(user, wager));
    }

    pub fn get_pool_mut(&mut self, user: UserId) -> Option<&mut NSFWBet> {
        self.pools.get_mut(&user)
    }

    pub fn get_pool(&self, user: UserId) -> Option<&NSFWBet> {
        self.pools.get(&user)
    }

    fn get_bet_button(&self, user: UserId) -> Result<Vec<CreateActionRow>, Error> {
        let button = CreateButton::new(&format!("{}-nsfwbet", user.get()))
            .label(&format!(
                "Bet ({} coin)",
                self.get_pool(user)
                    .ok_or("Could not find NSFWBet pool.")?
                    .wager
            ))
            .emoji(ReactionType::try_from(emoji::emoji("catcoin"))?);
        return Ok(vec![CreateActionRow::Buttons(vec![button])]);
    }

    pub fn remove_pool(&mut self, user: UserId) {
        self.pools.remove(&user);
    }
}

pub struct NSFWBet {
    // map of users to their time bets (in seconds), or None for never
    pub bets: HashMap<UserId, Option<u64>>,
    // the user being gambled on
    pub _uid: UserId,
    // how much coin each user is wagering
    pub wager: u64,
}

impl NSFWBet {
    fn new(uid: UserId, wager: u64) -> Self {
        NSFWBet {
            bets: HashMap::new(),
            _uid: uid,
            wager,
        }
    }

    fn add_bet(&mut self, user: UserId, seconds: Option<u64>) {
        self.bets.insert(user, seconds);
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
            data.nsfwbets
                .write()
                .await
                .create_pool(new_member.user.id, wager);
            let msg = sid
                .send_message(
                    ctx,
                    CreateMessage::new()
                        .content(&format!(
                            "{} {} {} | total meowmbers: {}",
                            emoji
                                .map(|e| e.to_string())
                                .unwrap_or(":white_check_mark:".to_string()),
                            new_member.mention(),
                            INTROS[r],
                            guild.member_count
                        ))
                        .components(
                            data.nsfwbets
                                .read()
                                .await
                                .get_bet_button(new_member.user.id)?,
                        ),
                )
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
                    if let Ok(member) = ctx
                        .http
                        .get_member(new_member.guild_id, new_member.user.id)
                        .await
                    {
                        if !member.roles.contains(&horny_role) {
                            // all of the voters who guessed "never" will win, and split the pool
                            let mut bets = nsfwbets.write().await;
                            if let Some(bet) = bets.get_pool_mut(member.user.id) {
                                let winners = bet.get_never_bets();

                                let pool_coin: u64 = bet.wager * bet.bets.iter().len() as u64;
                                let win_amount: u64 =
                                    (pool_coin as f64 / winners.len() as f64).ceil() as u64;

                                for winner in &winners {
                                    if let Err(e) =
                                        catcoin::grant_catcoin(&pool, *winner, win_amount).await
                                    {
                                        log::error!("Failed to grant catcoin to {}: {}", winner, e);
                                    }
                                }

                                // send message to general channel
                                let _ = general_channel
									.send_message(
										ctx,
										CreateMessage::new().content(format!(
											"{} lasted 1 hour. {} **+{}** to the **{}** who guessed 'never'!\n",
											member.mention(),
											emoji::emoji("catcoin"),
											win_amount,
											winners.len()
										)),
									)
									.await;
                                bets.remove_pool(member.user.id);
                            }
                        }
                    }
                });
            }

            // listen to betting interactions
            while let Some(mci) = ComponentInteractionCollector::new(ctx)
                .message_id(msg.id)
                .channel_id(msg.channel_id)
                .filter(move |mci| {
                    mci.data
                        .custom_id
                        .starts_with(&new_member.user.id.to_string())
                })
                .await
            {
                // send the betting modal
                if let Some(response) = execute_modal_generic::<BetModal, _>(
                    ctx,
                    |resp| mci.create_response(ctx, resp),
                    mci.id.to_string(),
                    None,
                    None,
                )
                .await?
                {
                    let too_broke = || {
                        response.create_response(
                            &ctx,
                            serenity::CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("You are way too broke to bet on this user.")
                                    .ephemeral(true),
                            ),
                        )
                    };

                    let mut bets = data.nsfwbets.write().await;
                    let bet = bets
                        .get_pool_mut(new_member.user.id)
                        .ok_or("Could not find NSFWBet pool.")?;

                    // check if the user has enough coin.
                    let amnt = get_catcoin(&data.local_pool, response.user.id).await?;
                    if amnt.catcoin < bet.wager as i64 {
                        too_broke().await?;
                        return Ok(());
                    }

                    let invalid_time = || {
                        response.create_response(
                            &ctx,
                            serenity::CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Invalid time format. Try again :3. Example: 12:34")
                                    .ephemeral(true),
                            ),
                        )
                    };

                    // parse modal response
                    let bm = BetModal::parse(response.data.clone())?;

                    let total_seconds = if bm.guess.to_ascii_lowercase() == "never" {
                        None
                    } else {
                        // parse time
                        let re = Regex::new(r#"^(\d{2}):(\d{2})$"#)
                            .expect("regex error in new user betting modal for some reason");
                        let Some(caps) = re.captures(&bm.guess) else {
                            invalid_time().await?;
                            return Ok(());
                        };
                        // get seconds
                        let (_, [minutes, seconds]) = caps.extract::<2>();
                        let minutes: u64 = minutes.parse()?;
                        let seconds: u64 = seconds.parse()?;
                        Some(minutes * 60 + seconds)
                    };

                    // transact coin
                    if !catcoin::spend_catcoin(&data.local_pool, response.user.id, bet.wager)
                        .await?
                    {
                        too_broke().await?;
                        return Ok(());
                    }
                    // add bet
                    bet.add_bet(response.user.id, total_seconds);
                    // send final response
                    response
                        .create_response(
                            &ctx,
                            serenity::CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Bet placed!! >w<")
                                    .ephemeral(true),
                            ),
                        )
                        .await?;
                }
            }
        }
    }

    Ok(())
}
