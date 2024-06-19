use crate::{
    catcoin::CatcoinWallet,
    discord::Context,
    gameme,
    tf2class::TF2Class,
    util::{get_bit, hhmmss, truncate},
    Error,
};
use chrono::{NaiveDateTime, Utc};
use poise::serenity_prelude::{self as serenity, CreateEmbedFooter, Mentionable};
use sqlx::{self, FromRow, MySql, Pool};
use steam::SteamProfileData;

use self::vote::Votes;

pub mod command;
pub mod edits;
pub mod steam;
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
    pub color: Option<u32>,
    pub views: u64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub hide_votes: i8,
    pub hide_dominations: i8,
    pub hide_stats: i8,
    pub favorite_user: Option<String>,
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
            views: 0,
            favorite_map: None,
            color: None,
            hide_votes: 0,
            hide_dominations: 0,
            hide_stats: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            favorite_user: None,
        }
    }

    pub fn get_classes(&self) -> Vec<TF2Class> {
        let mut classes = vec![];
        for i in 0..9u8 {
            if get_bit(self.classes, i) {
                classes.push(TF2Class::from_number(i));
            }
        }
        classes
    }

    pub async fn to_embed(
        &self,
        ctx: &Context<'_>,
        votes: Votes,
        steam_data: Option<SteamProfileData>,
        catcoin: CatcoinWallet,
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
            .title(truncate(&self.title.replace("%", &nickname), 255))
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
        if self.hide_votes == 0 {
            e = e.field(
                "🗳️ Votes",
                format!("👍`{}`|`{}`👎", votes.likes, votes.dislikes),
                true,
            );
        }

        // classes
        let classes: Vec<String> = self
            .get_classes()
            .into_iter()
            .map(|c| ctx.data().class_emojis.get(&c).unwrap().clone())
            .collect();
        if classes.len() > 0 {
            e = e.field("Classes", classes.join(""), true);
        }
        if let Some(steam_data) = steam_data {
            // steam account
            e = e.footer(
                CreateEmbedFooter::new(format!("{}", steam_data.summary.personaname))
                    .icon_url(steam_data.summary.avatarmedium),
            );
            // steam fields
            if let Some((rank, seconds)) = steam_data.seederboard {
                e = e.field(
                    "Seeding",
                    format!(
                        "`{}` **(#{})**",
                        hhmmss(seconds.try_into().unwrap_or(0)),
                        rank
                    ),
                    true,
                );
            };
            if self.hide_dominations == 0 {
                match (steam_data.best_friend, steam_data.worst_enemy) {
                    (Some(best_friend), Some(worst_enemy)) => {
                        e = e.field(
                            "⚔️ Dominations",
                            format!(
                                "`{}` **(+{})**\n`{}` **(-{})**",
                                best_friend.0.personaname,
                                best_friend.1,
                                worst_enemy.0.personaname,
                                worst_enemy.1
                            ),
                            true,
                        )
                    }
                    _ => (),
                }
            }
            if self.hide_stats == 0 {
                if let Some((stats, player)) = steam_data
                    .stats
                    .as_ref()
                    .and_then(|stats| Some((stats, stats.get_first_player()?)))
                {
                    let percentile = player.rank as f32 / stats.rankinginfo.activeplayers as f32;
                    // stats field
                    let output = format!(
                        "[**#{}** _(Top {:.1}%)_]({}playerinfo/{})",
                        player.rank,
                        percentile * 100.,
                        gameme::BASEURL,
                        player.id
                    );
                    e = e.field("Stats 📈", output, true);
                } else {
                    e = e.field("Stats 📈", "`No Data`", true);
                };
            }
        } else {
            // link footer
            e = e.footer(CreateEmbedFooter::new(
                "For more stats, link your steam! /link",
            ));
        }
        // fav map
        if let Some(map) = &self.favorite_map {
            e = e.field("Favorite Map", format!("🗺️ {map}"), true);
        }
        // fav user
        if let Some(fav_uid) = &self.favorite_user {
            let fav_uid: serenity::UserId = fav_uid.parse()?;
            if let Ok(user) = fav_uid.to_user(ctx).await {
                e = e.field("Favorite User 💖", user.mention().to_string(), true);
            }
        }
        // catcoins
        e = e.field(
            format!("Catcoin {}", ctx.data().catcoin_emoji),
            catcoin.catcoin.to_string(),
            true,
        );
        // color
        if let Some(color) = &self.color {
            e = e.color(*color);
        }
        // views
        e = e.field("Views 👀", format!("`{}`", self.views), true);

        Ok(e)
    }
}

/// add a view to the profile
pub async fn view_profile(pool: &Pool<MySql>, uid: serenity::UserId) -> Result<(), Error> {
    sqlx::query!(
        r#"
		INSERT INTO `profiles` (`uid`, `views`)
		VALUES (?, ?)
		ON DUPLICATE KEY UPDATE `views` = `views` + 1
	"#,
        uid.get(),
        1
    )
    .execute(pool)
    .await?;
    Ok(())
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

pub async fn get_user_profiles(
    pool: &Pool<MySql>,
    uids: Vec<serenity::UserId>,
) -> Result<Vec<UserProfile>, Error> {
    let params = format!("?{}", ", ?".repeat(uids.len() - 1));
    let query_str = format!("SELECT * FROM `profiles` WHERE `uid` IN ( {} )", params);
    let mut query = sqlx::query(&query_str);
    for id in uids {
        query = query.bind(id.get());
    }
    let profs: Vec<UserProfile> = query
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|v| UserProfile::from_row(&v).unwrap())
        .collect();

    Ok(profs)
}
