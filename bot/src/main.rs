// if you're reading this please give the owner of this repository a puppy pawjob with a twist
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use dotenv::dotenv;

use tokio;

use common::{
    util::{self, parse_env},
    Error,
};
use tf2::{ftp::ServerFtp, logs::LogReceiver, sftp::ServerSftp, ServerBuilder};

mod discord;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();

    let mut log_config = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {}] {}",
                chrono::Local::now().format("%m/%d %I:%M:%S %p"),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Off);

    for module in [
        "api",
        "bot",
        "catcoin",
        "common",
        "logstf",
        "profile",
        "seederboard",
        "sourcebans",
        "stats",
        "steam",
        "stocks",
        "tf2",
        "yapawards",
    ] {
        log_config = log_config.level_for(module, log::LevelFilter::Debug);
    }
    log_config.chain(std::io::stdout()).apply()?;

    log::info!("hello!!");
    log::info!("Starting the girlpound bot...");

    let rcon_pass: String = parse_env("RCON_PASS");

    // load servers
    let tkgp4_addr = "tf2.fluffycat.gay:27015"
        .to_socket_addrs()?
        .next()
        .expect("Could not resolve RCON address.");
    let tkgp4_ftp_addr = parse_env::<String>("FTP_HOST_4")
        .to_socket_addrs()?
        .next()
        .expect("Could not resolve RCON address.");

    let tkgp5_addr = "tf3.fluffycat.gay:27015"
        .to_socket_addrs()?
        .next()
        .expect("Could not resolve RCON address.");
    let tkgp5_ftp_addr = parse_env::<String>("FTP_HOST_5")
        .to_socket_addrs()?
        .next()
        .expect("Could not resolve RCON address.");

    let tkgp4 = ServerBuilder {
        name: "#4".to_owned(),
        emoji: "🅰️".to_owned(),
        addr: tkgp4_addr,
        rcon_pass: rcon_pass.clone(),
        player_count_cid: Some(parse_env("PLAYER_COUNT_CID_4")),
        log_cid: Some(parse_env("RELAY_CID_4")),
        files: Arc::new(ServerSftp::new(
            tkgp4_ftp_addr,
            parse_env("FTP_USER_4"),
            parse_env("FTP_PASS_4"),
        )),
        show_status: true,
        allow_seed: true,
        control_mapfile: true,
        wacky_server: true,
    }
    .build()
    .await
    .expect("Could not connect to server tkgp4");
    let tkgp5_addr = "tf3.fluffycat.gay:27015"
        .to_socket_addrs()?
        .next()
        .expect("Could not resolve RCON address.");
    let tkgp5 = ServerBuilder {
        name: "#5".to_owned(),
        emoji: "🅱️".to_owned(),
        addr: tkgp5_addr,
        rcon_pass: rcon_pass.clone(),
        player_count_cid: Some(parse_env("PLAYER_COUNT_CID_5")),
        log_cid: Some(parse_env("RELAY_CID_5")),
        files: Arc::new(ServerSftp::new(
            tkgp5_ftp_addr,
            parse_env("FTP_USER_5"),
            parse_env("FTP_PASS_5"),
        )),
        show_status: true,
        allow_seed: true,
        control_mapfile: true,
        wacky_server: false,
    }
    .build()
    .await
    .expect("Could not connect to server tkgp5");
    let tkgp6_addr = "pug.fluffycat.gay:27015"
        .to_socket_addrs()?
        .next()
        .expect("Could not resolve RCON address.");
    let tkgp6 = ServerBuilder {
        name: "#6".to_owned(),
        emoji: "️💀".to_owned(),
        addr: tkgp6_addr,
        rcon_pass: rcon_pass.clone(),
        player_count_cid: Some(parse_env("PLAYER_COUNT_CID_6")),
        log_cid: Some(parse_env("RELAY_CID_6")),
        files: Arc::new(ServerFtp::new(
            (tkgp6_addr.ip(), 21).into(),
            (parse_env("FTP_USER_6"), parse_env("FTP_PASS_6")),
        )),
        show_status: false,
        allow_seed: false,
        control_mapfile: false,
        wacky_server: false,
    }
    .build()
    .await
    .expect("Could not connect to server tkgp6");

    let mut servers = HashMap::new();
    servers.insert(tkgp4.addr, tkgp4);
    servers.insert(tkgp5.addr, tkgp5);
    servers.insert(tkgp6.addr, tkgp6);

    log::info!("{} servers loaded.", servers.len());

    log::info!("Launching UDP log receiver...");
    let logs_addr: Ipv4Addr = parse_env("SRCDS_LOG_ADDR");
    let logs_port: u16 = parse_env("SRCDS_LOG_PORT");
    let log_receiver = LogReceiver::connect(logs_addr, logs_port)
        .await
        .expect("Could not bind log receiver");

    log::info!("Spawning HTTP API listener...");
    let api_state = api::init().await.expect("Could not spawn api.");

    log::info!("Starting discord bot...");
    discord::start_bot(log_receiver, servers, api_state).await
}
