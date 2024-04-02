use poise::serenity_prelude::{self as serenity};
use serenity::{CreateMessage, Member, Mentionable};

use crate::Error;
use rand::prelude::*;

/// Sends a welcome message when a user joins the server
pub async fn welcome_user(ctx: &serenity::Context, new_member: &Member) -> Result<(), Error> {
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
    ];

    if let Some(guild) = new_member.guild_id.to_guild_cached(ctx).map(|g| g.clone()) {
        if let Some(sid) = guild.system_channel_id {
            let r = (random::<f32>() * INTROS.len() as f32).floor() as usize;
            let g = (random::<f32>() * guild.emojis.len() as f32).floor() as usize;
            let emoji = guild.emojis.values().skip(g).next();
            let msg = sid
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
            let _ = msg.react(&ctx, '🐈').await?;
        }
    }

    Ok(())
}
