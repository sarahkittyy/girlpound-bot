use std::env;

use dotenv::dotenv;
use rcon::Connection;

use tokio;
use tokio::net::TcpStream;

use regex::Regex;

mod discord;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub struct RconController {
    pub connection: Connection<TcpStream>,
    pub address: String,
}

impl RconController {
    async fn connect(address: &str, password: &str) -> Result<RconController, Error> {
        let connection = <Connection<TcpStream>>::builder()
            .connect(address, password)
            .await?;

        Ok(RconController {
            connection,
            address: address.to_owned(),
        })
    }

    async fn run(&mut self, cmd: &str) -> Result<String, Error> {
        self.connection.cmd(cmd).await.map_err(|e| e.into())
    }

    async fn player_count(&mut self) -> Result<i32, Error> {
        let status_msg = self.run("status").await?;

        let re = Regex::new(r"players : (\d+) humans,").unwrap();
        if let Some(caps) = re.captures(&status_msg) {
            Ok(caps[1].parse::<i32>().unwrap())
        } else {
            Err("Could not parse player count".into())
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    println!("Starting the girlpound bot...");

    let rcon_addr = env::var("RCON_ADDR").expect("Could not find env variable RCON_ADDR");
    let rcon_pass = env::var("RCON_PASS").expect("Could not find env variable RCON_PASS");

    let controller = RconController::connect(&rcon_addr, &rcon_pass)
        .await
        .expect("Could not connect to RCON");
    println!("Connected to RCON!\nStarting discord bot...");
    discord::start_bot(controller).await;
}
