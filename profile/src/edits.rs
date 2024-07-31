use std::{collections::HashMap, fmt::Display};

use poise::{
    serenity_prelude::{
        self as serenity, ComponentInteraction, CreateActionRow, CreateInteractionResponse,
        CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuOption, EmojiId,
    },
    Modal,
};
use regex::Regex;
use sqlx::{self, MySql, Pool};

use api::ApiState;
use common::{
    discord::{execute_modal_generic, get_steam_link_content},
    Error,
};
use tf2::TF2Class;

use crate::get_user_profile;

#[derive(Debug, Modal)]
#[name = "Edit user description"]
pub struct DescriptionModal {
    #[name = "New bio."]
    #[placeholder = "Cat ipsum dolor sit amet, american shorthair, but puma leopard."]
    #[paragraph]
    #[max_length = 256]
    pub description: String,
}

#[derive(Debug, Modal)]
#[name = "Edit profile URL"]
pub struct ProfileURLModal {
    #[name = "The url for the embed title to link to"]
    #[placeholder = "https://fluffycat.gay"]
    #[max_length = 256]
    pub url: String,
}

#[derive(Debug, Modal)]
#[name = "Edit profile header"]
pub struct TitleModal {
    #[name = "The title. Use % in place of your name."]
    #[placeholder = "%'s profile"]
    #[max_length = 256]
    pub title: String,
}

#[derive(Debug, Modal)]
#[name = "Edit profile image"]
pub struct ImageModal {
    #[name = "The url to an image to embed in your profile."]
    #[placeholder = "https://fluffycat.gay/img/plush_atk.png"]
    #[max_length = 256]
    pub image_url: String,
}

#[derive(Debug, Modal)]
#[name = "Edit your favorite map"]
pub struct FavoriteMapModal {
    #[name = "The map that you like the best."]
    #[placeholder = "ctf_2fort"]
    #[min_length = 3]
    #[max_length = 32]
    pub map: String,
}

#[derive(Debug, Modal)]
#[name = "Pick your bio color"]
pub struct ColorModal {
    #[name = "The color hex (www.color-hex.com)"]
    #[placeholder = "#365AA1"]
    #[min_length = 7]
    #[max_length = 7]
    pub color: String,
}

pub fn create_class_select_components() -> Vec<CreateActionRow> {
    let emoji = Regex::new(r#"<(a?):([A-Za-z0-9_-]+):(\d+)>"#).unwrap();
    let emojis: HashMap<TF2Class, EmojiId> = TF2Class::emojis()
        .iter()
        .flat_map(|(c, v)| -> Option<(TF2Class, EmojiId)> {
            emoji.captures(v).and_then(|caps| {
                let id = caps.get(3).unwrap();
                Some((c.clone(), EmojiId::new(id.as_str().parse().ok()?)))
            })
        })
        .collect();

    let options = vec![
        //
        CreateSelectMenuOption::new("Scout", "0")
            .description("Next time eat a salad")
            .emoji(*emojis.get(&TF2Class::Scout).unwrap()),
        CreateSelectMenuOption::new("Soldier", "1")
            .description("You were good son real good. maybe even the best.")
            .emoji(*emojis.get(&TF2Class::Soldier).unwrap()),
        CreateSelectMenuOption::new("Pyro", "2")
            .description("Mmph!")
            .emoji(*emojis.get(&TF2Class::Pyro).unwrap()),
        CreateSelectMenuOption::new("Demo", "3")
            .description("anyofyouthinkyerebetternmeyergoinhavnotherthingcomin")
            .emoji(*emojis.get(&TF2Class::Demo).unwrap()),
        CreateSelectMenuOption::new("Heavy", "4")
            .description("Poot Dispenser Here")
            .emoji(*emojis.get(&TF2Class::Heavy).unwrap()),
        CreateSelectMenuOption::new("Engineer", "5")
            .description("Nope")
            .emoji(*emojis.get(&TF2Class::Engineer).unwrap()),
        CreateSelectMenuOption::new("Medic", "6")
            .description("Ze healing is not as rewarding as ze hurting")
            .emoji(*emojis.get(&TF2Class::Medic).unwrap()),
        CreateSelectMenuOption::new("Sniper", "7")
            .description("Wave goodbye to ya head wanker!")
            .emoji(*emojis.get(&TF2Class::Sniper).unwrap()),
        CreateSelectMenuOption::new("Spy", "8")
            .description("Pornography")
            .emoji(*emojis.get(&TF2Class::Spy).unwrap()),
    ];
    let components = vec![CreateActionRow::SelectMenu(CreateSelectMenu::new(
        "profile.edit.class.select",
        serenity::CreateSelectMenuKind::String { options },
    ))];

    components
}

pub async fn open_class_select_menu(
    ctx: &serenity::Context,
    pool: &Pool<MySql>,
    mci: &ComponentInteraction,
) -> Result<(), Error> {
    let components = create_class_select_components();
    let classes: String = get_user_profile(pool, mci.user.id)
        .await?
        .get_classes()
        .iter()
        .map(|c| c.emoji().to_owned())
        .collect::<Vec<String>>()
        .join("");
    let _ = mci
        .create_response(
            &ctx,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .components(components)
                    .content(format!("{} | Select each class to toggle it.", classes))
                    .ephemeral(true),
            ),
        )
        .await?;

    Ok(())
}

pub async fn toggle_class(
    pool: &Pool<MySql>,
    user: serenity::UserId,
    class_index: u8,
) -> Result<(), Error> {
    sqlx::query!(
        r#"
		INSERT INTO `profiles` (`uid`, `classes`)
		VALUES (?, ?)
		ON DUPLICATE KEY UPDATE `classes` = `classes` ^ ?"#,
        user.get(),
        1 << class_index,
        1 << class_index,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn prompt_favorite_user(
    ctx: &serenity::Context,
    mci: &ComponentInteraction,
) -> Result<(), Error> {
    let components = vec![CreateActionRow::SelectMenu(CreateSelectMenu::new(
        "profile.edit.favorite.select",
        serenity::CreateSelectMenuKind::User {
            default_users: None,
        },
    ))];
    mci.create_response(
        &ctx,
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .components(components)
                .ephemeral(true)
                .content("Choose your favorite user 💖"),
        ),
    )
    .await?;

    Ok(())
}

pub async fn dispatch_profile_edit(
    ctx: &serenity::Context,
    mci: &ComponentInteraction,
    pool: &Pool<MySql>,
    api_state: &ApiState,
    choice: &str,
) -> Result<(), Error> {
    match choice {
        "description" => {
            edit_field_modal::<DescriptionModal, _, _, &str>(
                "description",
                |dm| Ok(dm.description),
                ctx,
                mci,
                pool,
            )
            .await?;
        }
        "classes" => {
            open_class_select_menu(ctx, pool, mci).await?;
        }
        "link-steam" => {
            let (embed, components) = get_steam_link_content(&api_state.link_url());
            mci.create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .embed(embed)
                        .components(components)
                        .ephemeral(true),
                ),
            )
            .await?;
        }
        "toggle-vote" => {
            toggle_bool_field(pool, mci.user.id, "hide_votes").await?;
            mci.create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Toggled vote visibility (refresh profile to update)")
                        .ephemeral(true),
                ),
            )
            .await?;
        }
        "toggle-stats" => {
            toggle_bool_field(pool, mci.user.id, "hide_stats").await?;
            mci.create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Toggled stats visibility (refresh profile to update)")
                        .ephemeral(true),
                ),
            )
            .await?;
        }
        "toggle-domination" => {
            toggle_bool_field(pool, mci.user.id, "hide_dominations").await?;
            mci.create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Toggled domination record visibility (refresh profile to update)")
                        .ephemeral(true),
                ),
            )
            .await?;
        }
        "unlink-steam" => {
            update_profile_column::<Option<String>>(mci.user.id, "steamid", None, pool).await?;
            mci.create_response(
                &ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Unlinked your steam. Do /link again if you change your mind ^-^")
                        .ephemeral(true),
                ),
            )
            .await?;
        }
        "color" => {
            edit_field_modal::<ColorModal, _, u32, &str>(
                "color",
                |cm| {
                    let re = Regex::new(r#"#([0-9A-Fa-f]{6})"#).unwrap();
                    match re.captures(&cm.color) {
                        Some(caps) => {
                            let v = caps.get(1).ok_or("Not a valid color!")?;
                            let color = u32::from_str_radix(v.as_str(), 16)
                                .map_err(|_| "Could not parse hex string")?;
                            Ok(color)
                        }
                        None => Err("Not a valid color!"),
                    }
                },
                ctx,
                mci,
                pool,
            )
            .await?;
        }
        "favorite-map" => {
            edit_field_modal::<FavoriteMapModal, _, _, &str>(
                "favorite_map",
                |fm| Ok(fm.map),
                ctx,
                mci,
                pool,
            )
            .await?;
        }
        "favorite-user" => {
            prompt_favorite_user(ctx, mci).await?;
        }
        "remove-favorite-user" => {
            update_profile_column::<Option<String>>(mci.user.id, "favorite_user", None, pool)
                .await?;
            mci.create_response(
                &ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .ephemeral(true)
                        .content("Favorite user unset. 💔"),
                ),
            )
            .await?;
        }
        "url" => {
            edit_field_modal::<ProfileURLModal, _, _, _>("url", |pm| {
				let regex = Regex::new(r#"https?:\/\/(www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&//=]*)"#).unwrap();
				if regex.is_match(&pm.url) {
					Ok(pm.url)
				} else {
					Err("Invalid URL.")
				}
			}, ctx, mci, pool).await?;
        }
        "title" => {
            edit_field_modal::<TitleModal, _, _, _>(
                "title",
                |tm| {
                    if tm.title.contains('%') {
                        Ok(tm.title)
                    } else {
                        Err("No % symbol in output.")
                    }
                },
                ctx,
                mci,
                pool,
            )
            .await?;
        }
        "image" => {
            edit_field_modal::<ImageModal, _, _, &str>(
                "image",
                |im| {
					let regex = Regex::new(r#"https?:\/\/(www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&//=]*)"#).unwrap();
					if regex.is_match(&im.image_url) {
						Ok(im.image_url)
					} else {
						Err("Invalid URL.")
					}
				},
                ctx,
                mci,
                pool,
            )
            .await?;
        }
        "remove-image" => {
            update_profile_column::<Option<String>>(mci.user.id, "image", None, pool).await?;
            mci.create_response(
                &ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Removed image from profile.")
                        .ephemeral(true),
                ),
            )
            .await?;
        }
        _ => (),
    }
    Ok(())
}

async fn edit_field_modal<'a, M, F, T, E>(
    column: &str,
    get_value: F,
    ctx: &serenity::Context,
    mci: &ComponentInteraction,
    pool: &Pool<MySql>,
) -> Result<(), Error>
where
    M: Modal,
    F: Fn(M) -> Result<T, E>,
    E: Display,
    T: 'a + sqlx::Type<MySql> + sqlx::Encode<'a, MySql> + Send + Clone + Display,
{
    if let Some(response) = execute_modal_generic::<M, _>(
        ctx,
        |resp| mci.create_response(ctx, resp),
        mci.id.to_string(),
        None,
        None,
    )
    .await?
    {
        // parse modal response
        let dm = M::parse(response.data.clone())?;
        let value = get_value(dm);
        // get relevant value
        let value = match value {
            Ok(value) => value,
            Err(e) => {
                response
                    .create_response(
                        &ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!("Invalid value for field `{column}`: {e}"))
                                .ephemeral(true),
                        ),
                    )
                    .await?;
                return Ok(());
            }
        };
        // push to db
        update_profile_column(mci.user.id, column, value.clone(), pool).await?;
        // respond
        response
            .create_response(
                ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!(
                            "Updated field `{}` with value `{}`.",
                            column, value
                        ))
                        .ephemeral(true),
                ),
            )
            .await?;
    }

    Ok(())
}

pub async fn update_profile_column<'a, T>(
    uid: serenity::UserId,
    column: &str,
    value: T,
    pool: &Pool<MySql>,
) -> Result<(), Error>
where
    T: 'a + sqlx::Type<MySql> + sqlx::Encode<'a, MySql> + Send + Clone,
{
    let mut qb = sqlx::QueryBuilder::new(format!(
        "INSERT INTO `profiles` (`uid`, `{column}`) VALUES ("
    ));
    qb.push_bind(uid.get());
    qb.push(", ");
    qb.push_bind(value.clone());
    qb.push(format!(") ON DUPLICATE KEY UPDATE `{column}` = "));
    qb.push_bind(value.clone());
    let q = qb.build();
    q.execute(pool).await?;
    Ok(())
}

/// toggle a boolean field on the profile
pub async fn toggle_bool_field(
    pool: &Pool<MySql>,
    profile_uid: serenity::UserId,
    column: &str,
) -> Result<(), Error> {
    let mut qb = sqlx::QueryBuilder::new(format!(
        "INSERT INTO `profiles` (`uid`, `{column}`) VALUES ("
    ));
    qb.push_bind(profile_uid.get());
    qb.push(format!(
        ", true) ON DUPLICATE KEY UPDATE `{column}` = NOT `{column}`"
    ));
    let q = qb.build();
    q.execute(pool).await?;
    Ok(())
}
