use chrono::NaiveDateTime;
use poise::serenity_prelude::{self as serenity, Color, CreateEmbed, CreateEmbedAuthor};
use sqlx::{MySql, Pool};

use crate::{discord::Context, Error};

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
pub struct PaginatedInventory {
    pub pulls: Vec<CatcoinPull>,
    uid: serenity::UserId,
    last: Option<CatcoinPull>,
}

impl PaginatedInventory {
    pub fn has_next(&self) -> bool {
        self.last.is_some()
    }

    pub async fn to_embed(&self, ctx: &Context<'_>) -> Result<CreateEmbed, Error> {
        let member = ctx.data().guild_id.member(ctx, self.uid).await?;
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
                    ctx.data().catcoin_emoji,
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
