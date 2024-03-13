use crate::{discord::Context, Error};

use poise::CreateReply;
use reqwest;
use serde::Deserialize;

use uwuifier::uwuify_str_sse;

use rand::prelude::*;

#[derive(Deserialize)]
struct BibleVerseApiResponse {
    pub reference: String,
    pub text: String,
}

/// Get a random bible verse!
#[poise::command(slash_command, channel_cooldown = 10)]
pub async fn bibleverse(
    ctx: Context<'_>,
    #[description = "Preach from the book of ultralations."] ultra: Option<bool>,
) -> Result<(), Error> {
    const URL: &'static str = "https://bible-api.com/?random=verse";

    let resp = reqwest::get(URL).await?;
    let verse = resp.json::<BibleVerseApiResponse>().await?;

    let ultra = ultra.unwrap_or(false);

    if ultra {
        let r = ctx.send(CreateReply::default().content("\"Game is game rise and grind to hustle everyday so that one day you may get yourself a cute trans girl in your dms begging for your presence everyday..\" (Ultralations 6:23)"))
			.await?;
        r.message().await?.react(&ctx, '🙏').await?;
        return Ok(());
    }

    let text = if random::<f32>() > 0.7 {
        uwuify_str_sse(&verse.text.trim())
    } else {
        verse.text.trim().to_owned()
    };

    let r = ctx
        .send(CreateReply::default().content(format!("\"{}\" ({}).", text, verse.reference)))
        .await?;
    r.message().await?.react(&ctx, '🙏').await?;
    Ok(())
}
