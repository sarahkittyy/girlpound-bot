use poise::serenity_prelude as serenity;
use sqlx::{self, MySql, Pool};

use crate::Error;

/// The change in the total votes given a vote_on call
#[derive(Clone, Copy)]
pub struct Votes {
    pub likes: i64,
    pub dislikes: i64,
}

impl Votes {
    pub fn zero() -> Self {
        Self {
            likes: 0,
            dislikes: 0,
        }
    }
}

pub async fn get_profile_votes(
    pool: &Pool<MySql>,
    profile_uid: serenity::UserId,
) -> Result<Votes, Error> {
    let votes = sqlx::query!(
        "SELECT * FROM `profile_votes_aggregate` WHERE `profile_uid` = ?",
        profile_uid.get()
    )
    .fetch_optional(pool)
    .await?;

    Ok(votes
        .map(|v| Votes {
            likes: v.likes.unwrap_or(0),
            dislikes: v.dislikes.unwrap_or(0),
        })
        .unwrap_or(Votes::zero()))
}

/// Submits a vote to the profile, returning the change in total votes as a result
pub async fn vote_on(
    pool: &Pool<MySql>,
    profile_uid: serenity::UserId,
    voter_uid: serenity::UserId,
    like: bool,
) -> Result<Votes, Error> {
    let existing_vote = sqlx::query!(
        "SELECT * FROM `profile_votes` WHERE `profile_uid` = ? AND `voter_uid` = ?",
        profile_uid.get(),
        voter_uid.get()
    )
    .fetch_optional(pool)
    .await?;

    let new_vote = if like { 1 } else { -1 };

    // if the existing vote is identical, return
    match existing_vote {
        Some(record) if record.vote == new_vote => return Ok(Votes::zero()),
        _ => (),
    };

    let existing_vote = existing_vote.map(|e| e.vote).unwrap_or(0);

    // update the vote
    sqlx::query!(
        r#"
		INSERT INTO `profile_votes` (`profile_uid`, `voter_uid`, `vote`)
		VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE `vote` = ?
		"#,
        profile_uid.get(),
        voter_uid.get(),
        new_vote,
        new_vote
    )
    .execute(pool)
    .await?;

    // calculate the votediff
    let diff = if existing_vote == 0 {
        Votes {
            likes: if like { 1 } else { 0 },
            dislikes: if !like { 1 } else { 0 },
        }
    } else if existing_vote == -1 {
        Votes {
            likes: if like { 1 } else { 0 },
            dislikes: if like { -1 } else { 0 },
        }
    } else if existing_vote == 1 {
        Votes {
            likes: if !like { -1 } else { 0 },
            dislikes: if !like { 1 } else { 0 },
        }
    } else {
        return Err("Invalid existing_vote".into());
    };

    Ok(diff)
}
