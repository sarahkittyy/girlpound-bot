use std::{collections::HashMap, sync::Arc};

use chrono::{NaiveDateTime, Utc};
use common::Error;
use poise::serenity_prelude::{self as serenity, ChannelId, EditMessage, Message, RoleId, UserId};
use tokio::sync::RwLock;

// when a user sends a message with an image/embed in the channel, their "genimg" role is removed for 24 hours
pub struct GenImg {
    pub genimg_role: RoleId,       // role that grants image perms
    pub gen_channel: ChannelId,    // gen channel
    pub member_role: RoleId,       // role that grants access to post images at all
    pub ignore_roles: Vec<RoleId>, // roles that do not need genimg permissions, so are ignored.
    pub last_sent: HashMap<UserId, NaiveDateTime>,
}

impl GenImg {
    pub fn new(
        genimg_role: RoleId,
        gen_channel: ChannelId,
        member_role: RoleId,
        ignore_roles: Vec<RoleId>,
    ) -> Self {
        Self {
            genimg_role,
            gen_channel,
            member_role,
            ignore_roles,
            last_sent: HashMap::new(),
        }
    }
}

pub async fn on_message(
    ctx: &serenity::Context,
    genimg: Arc<RwLock<GenImg>>,
    mut msg: Message,
) -> Result<(), Error> {
    let mut genimg = genimg.write().await;
    let member = msg.member(ctx).await?;
    let has_genimg_role = member.roles.contains(&genimg.genimg_role);

    // ignore unverified users
    if !member.roles.contains(&genimg.member_role) {
        // just incase something goes wrong
        if has_genimg_role {
            member.remove_role(ctx, genimg.genimg_role).await?;
        }
        return Ok(());
    }

    // ignore users with roles that don't need genimg perms
    if member
        .roles
        .iter()
        .any(|role| genimg.ignore_roles.contains(role))
    {
        // just in case something goes wrong
        if has_genimg_role {
            member.remove_role(ctx, genimg.genimg_role).await?;
        }
        return Ok(());
    }

    // give them the role if it's been enough time
    let last_sent = genimg
        .last_sent
        .entry(msg.author.id)
        .or_insert_with(|| chrono::DateTime::<Utc>::MIN_UTC.naive_utc())
        .clone();
    if !has_genimg_role && last_sent + chrono::Duration::hours(12) < chrono::Utc::now().naive_utc()
    {
        member.add_role(ctx, genimg.genimg_role).await?;
        return Ok(());
    }

    // only revoke the role in gen chat
    if msg.channel_id != genimg.gen_channel {
        return Ok(());
    }

    if has_genimg_role {
        // if they have the role, and sent an embed / image, remove the role, put them on timer

        if msg.attachments.len() > 0 || msg.embeds.len() > 0 {
            // remove the role
            member.remove_role(ctx, genimg.genimg_role).await?;
            // put them on timer
            genimg
                .last_sent
                .insert(msg.author.id, chrono::Utc::now().naive_utc());
        }
    }

    Ok(())
}
