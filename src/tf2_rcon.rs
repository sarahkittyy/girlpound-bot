use crate::Error;

use rcon::Connection;
use regex::Regex;
use tokio::net::TcpStream;

pub struct RconController {
    pub connection: Connection<TcpStream>,
    pub address: String,
    pub password: String,
}

impl RconController {
    pub async fn connect(address: &str, password: &str) -> Result<Self, Error> {
        let connection = <Connection<TcpStream>>::builder()
            .connect(address, password)
            .await?;

        Ok(RconController {
            connection,
            address: address.to_owned(),
            password: password.to_owned(),
        })
    }

    pub async fn reconnect(&mut self) -> Result<(), Error> {
        self.connection = <Connection<TcpStream>>::builder()
            .connect(&self.address, &self.password)
            .await?;

        Ok(())
    }

    pub async fn run(&mut self, cmd: &str) -> Result<String, Error> {
        self.connection.cmd(cmd).await.map_err(|e| e.into())
    }

    pub async fn player_count(&mut self) -> Result<i32, Error> {
        let status_msg = self.run("status").await?;

        let re = Regex::new(r"players : (\d+) humans,").unwrap();
        if let Some(caps) = re.captures(&status_msg) {
            Ok(caps[1].parse::<i32>().unwrap())
        } else {
            Err("Could not parse player count".into())
        }
    }

    pub async fn player_list(&mut self) -> Result<Vec<String>, Error> {
        let status_msg = self.run("status").await?;

        let re = Regex::new(r#"\d+\s"(.+)""#).unwrap();
        let mut players = Vec::new();
        for caps in re.captures_iter(&status_msg) {
            players.push(caps[1].to_owned());
        }

        Ok(players)
    }
}
