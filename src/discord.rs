use std::env;
use std::sync::Arc;

use crate::{Error, RconController};
use poise::serenity_prelude as serenity;

use tokio::{self, sync::RwLock, time};

struct PoiseData {
    rcon_controller: Arc<RwLock<RconController>>,
}
type Context<'a> = poise::Context<'a, PoiseData, Error>;

#[poise::command(slash_command)]
async fn rcon(
    ctx: Context<'_>,
    #[description = "The command to send."] cmd: String,
) -> Result<(), Error> {
    let mut rcon = ctx.data().rcon_controller.write().await;
    let reply = rcon.run(&cmd).await;
    match reply {
        Ok(output) => ctx.say(format!("```\n{}\n```", output)).await,
        Err(e) => ctx.say(format!("RCON error: {:?}", e)).await,
    }?;
    Ok(())
}

#[poise::command(slash_command)]
async fn online(ctx: Context<'_>) -> Result<(), Error> {
    let mut rcon = ctx.data().rcon_controller.write().await;
    let count = rcon.player_count().await?;
    ctx.say(format!("There are {} players online.", count))
        .await?;
    Ok(())
}

pub async fn start_bot(rcon_controller: RconController) {
    let rcon_controller = Arc::new(RwLock::new(rcon_controller));

    let bot_token = env::var("BOT_TOKEN").expect("Could not find env variable BOT_TOKEN");
    let guild_id = env::var("GUILD_ID")
        .expect("Could not find env variable GUILD_ID")
        .parse::<u64>()
        .expect("GUILD_ID could not be parsed into u64");

    let intents = serenity::GatewayIntents::non_privileged();

    let girlpounder = {
        let rcon_controller = rcon_controller.clone();
        poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![rcon(), online()],
                ..Default::default()
            })
            .token(bot_token)
            .intents(intents)
            .setup(move |ctx, _ready, framework| {
                Box::pin(async move {
                    poise::builtins::register_in_guild(
                        ctx,
                        &framework.options().commands,
                        serenity::GuildId(guild_id),
                    )
                    .await?;

                    ctx.set_activity(serenity::Activity::playing("tf2.fluffycat.gay:7005"))
                        .await;

                    Ok(PoiseData { rcon_controller })
                })
            })
            .build()
            .await
            .expect("Failed to build girlpounder bot.")
    };

    let global_ctx = girlpounder.client().cache_and_http.clone();
    let rcon_controller = rcon_controller.clone();

    let live_player_channel: Option<serenity::ChannelId> = env::var("LIVE_PLAYER_CHANNEL_ID")
        .ok()
        .and_then(|id| id.parse::<u64>().ok().map(serenity::ChannelId));
    println!("LIVE_PLAYER_CHANNEL: {:?}", live_player_channel);

    // launch alt threads
    {
        let rcon_controller = rcon_controller.clone();

        if let Some(live_player_channel) = live_player_channel {
            let mut interval = time::interval(time::Duration::from_secs(5 * 60));
            tokio::spawn(async move {
                interval.tick().await;

                loop {
                    let Ok(player_count) = rcon_controller.write().await.player_count().await
                    else {
                        continue;
                    };
                    // edit channel name to reflect player count
                    live_player_channel
                        .edit(global_ctx.as_ref(), |c| {
                            c.name(format!("📶 {}/24 online", player_count))
                        })
                        .await
                        .expect("Could not edit channel name");
                    interval.tick().await;
                }
            });
        }
    };

    let fut = girlpounder.start();
    println!("Bot started!");
    fut.await.expect("Bot broke");
}
