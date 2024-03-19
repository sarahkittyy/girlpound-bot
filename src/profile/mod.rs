use crate::{discord::Context, tf2class::TF2Class, Error};
use poise::serenity_prelude::{self as serenity};
use sqlx::{self, MySql, Pool};

use self::vote::Votes;

pub mod command;
pub mod edits;
pub mod vote;

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct UserProfile {
    pub uid: String, // discord id
    pub title: String,
    pub url: Option<String>,
    pub steamid: Option<String>,     // steam3 id
    pub description: Option<String>, // user bio
    pub image: Option<String>,       // customizable image
    pub classes: u16,
    pub favorite_map: Option<String>,
}

impl UserProfile {
    pub fn new(uid: String) -> Self {
        Self {
            uid,
            title: "%'s profile".to_owned(),
            url: None,
            steamid: None,
            description: None,
            image: None,
            classes: 0,
            favorite_map: None,
        }
    }

    pub async fn to_embed(
        &self,
        ctx: &Context<'_>,
        votes: Votes,
    ) -> Result<serenity::CreateEmbed, Error> {
        let user = serenity::UserId::new(self.uid.parse()?)
            .to_user(&ctx)
            .await?;
        let nickname = user
            .nick_in(&ctx, ctx.data().guild_id)
            .await
            .unwrap_or_else(|| user.name.clone());
        let description = format!(
            "{}",
            self.description
                .as_deref()
                .unwrap_or("*No description set*")
        );
        let pfp = user.avatar_url().unwrap_or(user.default_avatar_url());
        let mut e = serenity::CreateEmbed::new() //
            .title(self.title.replace("%", &nickname))
            .thumbnail(pfp)
            .description(description);
        // link
        if let Some(url) = &self.url {
            e = e.url(url);
        };
        // image
        if let Some(image) = &self.image {
            e = e.image(image);
        };

        // votes
        e = e.field(
            "Votes",
            format!("👍`{}`|`{}`👎", votes.likes, votes.dislikes),
            true,
        );

        // classes
        let mut classes = vec![];
        for i in 0..9u8 {
            if get_bit(self.classes, i) {
                let emoji = ctx
                    .data()
                    .class_emojis
                    .get(&TF2Class::from_number(i))
                    .cloned()
                    .unwrap();
                classes.push(emoji);
            }
        }
        if classes.len() > 0 {
            e = e.field("Classes", classes.join(""), true);
        }

        // fav map
        if let Some(map) = &self.favorite_map {
            e = e.field("Favorite Map", map, true);
        }

        Ok(e)
    }
}

fn get_bit(value: u16, bit: u8) -> bool {
    value & (1 << bit) > 0
}

/// retrieve a profile by discord user id
pub async fn get_user_profile(
    pool: &Pool<MySql>,
    uid: serenity::UserId,
) -> Result<UserProfile, Error> {
    let prof = sqlx::query_as!(
        UserProfile,
        "SELECT * FROM `profiles` WHERE `uid` = ?",
        uid.get()
    )
    .fetch_optional(pool)
    .await?;

    Ok(prof.unwrap_or_else(|| UserProfile::new(uid.get().to_string())))
}
