// if you're reading this please give the owner of this repository a puppy pawjob with a twist
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::str::FromStr;
use std::sync::Arc;
use std::{env, net::Ipv4Addr};

use dotenv::dotenv;

use poise::serenity_prelude as serenity;
use tokio;

mod discord;
mod ftp;
mod logs;
mod steamid;
mod tf2_rcon;

use ftp::ServerFtp;

use logs::LogReceiver;
use tf2_rcon::RconController;

use sqlx::mysql::MySql;
use sqlx::Pool;
use tokio::sync::RwLock;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub struct ServerBuilder {
    pub name: String,
    pub emoji: String,
    pub addr: SocketAddr,
    pub rcon_pass: String,
    pub player_count_cid: Option<u64>,
    pub log_cid: Option<u64>,
    pub ftp_credentials: (String, String),
    pub allow_seed: bool,
    pub show_status: bool,
}

impl ServerBuilder {
    pub async fn build(self) -> Result<Server, Error> {
        let ftp_url: SocketAddr = (self.addr.ip(), 21).into();
        println!("Connecting to {:?}...", self.addr);
        Ok(Server {
            name: self.name,
            emoji: self.emoji,
            addr: self.addr,
            controller: Arc::new(RwLock::new(
                RconController::connect(self.addr, &self.rcon_pass).await?,
            )),
            player_count_channel: self.player_count_cid.map(serenity::ChannelId),
            log_channel: self.log_cid.map(serenity::ChannelId),
            ftp: ServerFtp::new(ftp_url, self.ftp_credentials),
            allow_seed: self.allow_seed,
            show_status: self.show_status,
        })
    }
}

/// A single tf2 server to keep track of
#[derive(Clone)]
pub struct Server {
    pub name: String,
    pub emoji: String,
    pub addr: SocketAddr,
    pub controller: Arc<RwLock<RconController>>,
    pub player_count_channel: Option<serenity::ChannelId>,
    pub log_channel: Option<serenity::ChannelId>,
    pub ftp: ServerFtp,
    pub allow_seed: bool,
    pub show_status: bool,
}

fn parse_env<T: FromStr>(name: &str) -> T {
    env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .expect(&format!("Could not find env variable {}", name))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    println!("Starting the girlpound bot...");

    let db_url: String = parse_env("DATABASE_URL");

    // migrate the db
    let pool = Pool::<MySql>::connect(&db_url).await?;
    sqlx::migrate!().run(&pool).await?;
    println!("DB Migrated.");

    let rcon_pass: String = parse_env("RCON_PASS");

    // load servers
    let tkgp4 = ServerBuilder {
        name: "#4".to_owned(),
        emoji: "🅰️".to_owned(),
        addr: "tf2.fluffycat.gay:27015"
            .to_socket_addrs()?
            .next()
            .expect("Could not resolve RCON address."),
        rcon_pass: rcon_pass.clone(),
        player_count_cid: Some(parse_env("PLAYER_COUNT_CID_4")),
        log_cid: Some(parse_env("RELAY_CID_4")),
        ftp_credentials: (parse_env("FTP_USER_4"), parse_env("FTP_PASS_4")),
        show_status: true,
        allow_seed: true,
    }
    .build()
    .await
    .expect("Could not connect to server tkgp4");
    let tkgp5 = ServerBuilder {
        name: "#5".to_owned(),
        emoji: "🅱️".to_owned(),
        addr: "tf3.fluffycat.gay:27015"
            .to_socket_addrs()?
            .next()
            .expect("Could not resolve RCON address."),
        rcon_pass: rcon_pass.clone(),
        player_count_cid: Some(parse_env("PLAYER_COUNT_CID_5")),
        log_cid: Some(parse_env("RELAY_CID_5")),
        ftp_credentials: (parse_env("FTP_USER_5"), parse_env("FTP_PASS_5")),
        show_status: true,
        allow_seed: true,
    }
    .build()
    .await
    .expect("Could not connect to server tkgp5");
    let tkgp6 = ServerBuilder {
        name: "#6".to_owned(),
        emoji: "️💀".to_owned(),
        addr: "pug.fluffycat.gay:27015"
            .to_socket_addrs()?
            .next()
            .expect("Could not resolve RCON address."),
        rcon_pass: rcon_pass.clone(),
        player_count_cid: Some(parse_env("PLAYER_COUNT_CID_6")),
        log_cid: Some(parse_env("RELAY_CID_6")),
        ftp_credentials: (parse_env("FTP_USER_6"), parse_env("FTP_PASS_6")),
        show_status: false,
        allow_seed: false,
    }
    .build()
    .await
    .expect("Could not connect to server tkgp6");

    let mut servers = HashMap::new();
    servers.insert(tkgp4.addr, tkgp4);
    servers.insert(tkgp5.addr, tkgp5);
    servers.insert(tkgp6.addr, tkgp6);

    println!("{} servers loaded.", servers.len());

    println!("Launching UDP log receiver...");
    let logs_addr: Ipv4Addr = parse_env("SRCDS_LOG_ADDR");
    let logs_port: u16 = parse_env("SRCDS_LOG_PORT");
    let log_receiver = LogReceiver::connect(logs_addr, logs_port)
        .await
        .expect("Could not bind log receiver");

    println!("Starting discord bot...");
    discord::start_bot(pool, log_receiver, servers).await;
    Ok(())
}
