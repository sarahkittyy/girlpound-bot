use chrono::NaiveDateTime;
use poise::serenity_prelude::{
    self as serenity, Color, Context, CreateEmbed, CreateEmbedAuthor, GuildId, Message,
};
use regex::Regex;
use sqlx::{MySql, Pool};

use crate::random_pulls::Rarity;
use common::Error;

use emoji::emoji;

use super::random_pulls::Reward;

#[derive(Debug, Clone)]
pub struct CatcoinPull {
    pub id: i32,
    pub uid: serenity::UserId,
    pub reward: Reward,
    pub number: i32,
    pub catcoin: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct CatcoinPullMessageData {
    pub uid: serenity::UserId,
    pub name: String,
    pub rarity: Rarity,
    pub number: i32,
    pub catcoin: i32,
    pub created_at: NaiveDateTime,
}

impl TryFrom<Message> for CatcoinPullMessageData {
    type Error = Error;
    /// Returns as id 0
    fn try_from(msg: Message) -> Result<Self, Self::Error> {
        let embed = msg.embeds.first().ok_or("No embed.")?;
        let uid = msg
            .referenced_message
            .map(|msg| msg.author.id)
            .ok_or("Not replying to anyone.")?;

        let title_re = Regex::new(r#":bangbang: (\w+) Pull: ([\w ()]+) #(\d+)"#).unwrap();
        let title = embed.title.as_deref().ok_or("No title.")?;
        let title_matches = title_re.captures(title).ok_or("No title match.")?;
        let rarity: Rarity = title_matches.get(1).ok_or("No rarity.")?.as_str().into();
        let name = title_matches.get(2).ok_or("No name.")?.as_str();
        let number: i32 = title_matches.get(3).ok_or("No number.")?.as_str().parse()?;

        let desc_re = Regex::new(r#"\*\*\+(\d+)\*\*"#).unwrap();
        let desc = embed.description.as_deref().ok_or("No desc.")?;
        let desc_matches = desc_re.captures(desc).ok_or("No desc match.")?;
        let catcoin: i32 = desc_matches.get(1).ok_or("No catcoin.")?.as_str().parse()?;

        let created_at = msg.timestamp.naive_utc();

        Ok(CatcoinPullMessageData {
            uid,
            catcoin,
            created_at,
            number,
            name: name.to_owned(),
            rarity,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PaginatedInventory {
    pub pulls: Vec<CatcoinPull>,
    uid: serenity::UserId,
    last: Option<CatcoinPull>,
}

impl PaginatedInventory {
    pub fn has_next(&self) -> bool {
        self.last.is_some()
    }

    pub async fn to_embed(&self, ctx: &Context, guild: GuildId) -> Result<CreateEmbed, Error> {
        let member = guild.member(ctx, self.uid).await?;
        let pulls: Vec<String> = self
            .pulls
            .iter()
            .map(|pull| {
                let timestamp = pull
                    .created_at
                    .signed_duration_since(NaiveDateTime::UNIX_EPOCH)
                    .num_seconds();
                format!(
                    "(**+{}** {}) **{}** {} `#{}` | <t:{}:R>",
                    pull.catcoin,
                    emoji("catcoin"),
                    pull.reward.rarity,
                    pull.reward.name,
                    pull.number,
                    timestamp
                )
            })
            .collect();
        Ok(CreateEmbed::new()
            .color(Color::BLURPLE)
            .author(CreateEmbedAuthor::new(member.display_name()).icon_url(member.face()))
            .description(format!("{}", pulls.join("\n"))))
    }

    pub async fn get(pool: &Pool<MySql>, uid: serenity::UserId) -> Result<Self, Error> {
        let res: Vec<_> = sqlx::query!(
            r#"
			SELECT 	i.id, i.uid, i.number, i.created_at, i.updated_at, i.catcoin,
					r.id as rid,
					r.name, r.file, r.rarity
			FROM `catcoin_inv` i
			INNER JOIN `catcoin_reward` r ON i.rid = r.id
			WHERE i.uid = ?
			ORDER BY r.rarity DESC, i.id DESC
			LIMIT 11
		"#,
            uid.get()
        )
        .fetch_all(pool)
        .await?;
        let pulls: Vec<CatcoinPull> = res
            .into_iter()
            .map(|r| CatcoinPull {
                id: r.id,
                uid: r
                    .uid
                    .parse()
                    .expect("Invalid UID format in catcoin inventory."),
                number: r.number,
                catcoin: r.catcoin,
                reward: Reward {
                    id: r.rid,
                    name: r.name,
                    file: r.file,
                    rarity: r.rarity.into(),
                },
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();
        Ok(Self {
            uid,
            last: pulls.get(10).cloned(),
            pulls: pulls.into_iter().take(10).collect(),
        })
    }

    pub async fn next(&self, pool: &Pool<MySql>) -> Result<Option<Self>, Error> {
        let Some(last) = &self.last else {
            return Ok(None);
        };
        let res: Vec<_> = sqlx::query!(
            r#"
			SELECT 	i.id, i.uid, i.number, i.created_at, i.updated_at, i.catcoin,
					r.id as rid,
					r.name, r.file, r.rarity
			FROM `catcoin_inv` i
			INNER JOIN `catcoin_reward` r ON i.rid = r.id
			WHERE i.uid = ? AND (r.rarity, i.id) <= (?, ?)
			ORDER BY r.rarity DESC, i.id DESC
			LIMIT 11
		"#,
            self.uid.get(),
            last.reward.rarity.to_string(),
            last.id,
        )
        .fetch_all(pool)
        .await?;
        let pulls: Vec<CatcoinPull> = res
            .into_iter()
            .map(|r| CatcoinPull {
                id: r.id,
                uid: r
                    .uid
                    .parse()
                    .expect("Invalid UID format in catcoin inventory."),
                number: r.number,
                catcoin: r.catcoin,
                reward: Reward {
                    id: r.rid,
                    name: r.name,
                    file: r.file,
                    rarity: r.rarity.into(),
                },
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();
        Ok(Some(Self {
            uid: self.uid,
            last: pulls.get(10).cloned(),
            pulls: pulls.into_iter().take(10).collect(),
        }))
    }
}

pub async fn get_reward_by_name(
    pool: &Pool<MySql>,
    reward_name: &str,
) -> Result<Option<Reward>, Error> {
    let res: Option<Reward> = sqlx::query_as!(
        Reward,
        r#"
		SELECT * FROM `catcoin_reward`
		WHERE `name` = ?
	"#,
        reward_name
    )
    .fetch_optional(pool)
    .await?;

    Ok(res)
}

/// Redeem old pull
pub async fn claim_old_pull(
    pool: &Pool<MySql>,
    data: &CatcoinPullMessageData,
) -> Result<bool, Error> {
    let Some(reward) = get_reward_by_name(pool, &data.name).await? else {
        return Err("No reward.".into());
    };

    let r = sqlx::query!(
        r#"
		IF NOT EXISTS (SELECT * FROM `catcoin_inv` WHERE `rid` = ? AND `number` = ?) THEN
			INSERT INTO `catcoin_inv` (`uid`, `rid`, `number`, `catcoin`, `created_at`)
			VALUES (?, ?, ?, ?, ?);
		END IF
	"#,
        reward.id,
        data.number,
        data.uid.get(),
        reward.id,
        data.number,
        data.catcoin,
        data.created_at
    )
    .execute(pool)
    .await?;

    Ok(r.rows_affected() > 0)
}

/// Add pull to user inventory
pub async fn add_to_inventory(
    pool: &Pool<MySql>,
    uid: serenity::UserId,
    reward_id: i32,
    number: i32,
    catcoin: u64,
) -> Result<(), Error> {
    sqlx::query!(
        r#"
		INSERT INTO `catcoin_inv` (`uid`, `rid`, `number`, `catcoin`)
		VALUES (?, ?, ?, ?)
	"#,
        uid.get(),
        reward_id,
        number,
        catcoin
    )
    .execute(pool)
    .await?;
    Ok(())
}
