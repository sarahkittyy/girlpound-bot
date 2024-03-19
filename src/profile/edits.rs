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

use crate::{
    discord::{util::execute_modal_generic, PoiseData},
    tf2class::TF2Class,
    Error,
};

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

async fn open_class_select_menu(
    ctx: &serenity::Context,
    data: &PoiseData,
    mci: &ComponentInteraction,
) -> Result<(), Error> {
    let emoji = Regex::new(r#"<(a?):([A-Za-z0-9_-]+):(\d+)>"#).unwrap();
    let emojis: HashMap<TF2Class, EmojiId> = data
        .class_emojis
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

    mci.create_response(
        &ctx,
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .components(components)
                .content("Select each class to toggle it.")
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

pub async fn dispatch_profile_edit(
    ctx: &serenity::Context,
    mci: &ComponentInteraction,
    data: &PoiseData,
    choice: &str,
) -> Result<(), Error> {
    match choice {
        "description" => {
            edit_field_modal::<DescriptionModal, _, _, &str>(
                "description",
                |dm| Ok(dm.description),
                ctx,
                mci,
                data,
            )
            .await?;
        }
        "classes" => {
            open_class_select_menu(ctx, data, mci).await?;
        }
        "favorite-map" => {
            edit_field_modal::<FavoriteMapModal, _, _, &str>(
                "favorite_map",
                |fm| Ok(fm.map),
                ctx,
                mci,
                data,
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
			}, ctx, mci, data).await?;
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
                data,
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
                data,
            )
            .await?;
        }
        "remove-image" => {
            update_profile_column::<Option<String>>(mci.user.id, "image", None, &data.local_pool)
                .await?;
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
    data: &PoiseData,
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
        update_profile_column(mci.user.id, column, value.clone(), &data.local_pool).await?;
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

async fn update_profile_column<'a, T>(
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
