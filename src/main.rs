use std::{env, net::Ipv4Addr};

use dotenv::dotenv;

use tokio;

mod discord;
mod logs;
mod tf2_rcon;

use logs::LogReceiver;
use tf2_rcon::RconController;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    dotenv().ok();
    println!("Starting the girlpound bot...");

    let rcon_addr = env::var("RCON_ADDR").expect("Could not find env variable RCON_ADDR");
    let rcon_pass = env::var("RCON_PASS").expect("Could not find env variable RCON_PASS");

    let controller = RconController::connect(&rcon_addr, &rcon_pass)
        .await
        .expect("Could not connect to RCON");
    println!("Connected to RCON!");
    println!("Launching UDP log receiver...");
    let logs_addr: Ipv4Addr = env::var("SRCDS_LOG_ADDR")
        .ok()
        .and_then(|a| a.parse().ok())
        .expect("Invalid env variable SRCDS_LOG_ADDR");
    let logs_port: u16 = env::var("SRCDS_LOG_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .expect("Invalid env variable SRCDS_LOG_PORT");
    let log_receiver = LogReceiver::connect(logs_addr, logs_port)
        .await
        .expect("Could not bind log receiver");
    println!("Starting discord bot...");
    discord::start_bot(controller, log_receiver).await;
}
