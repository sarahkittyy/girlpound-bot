use crate::Error;

use rcon::Connection;
use regex::Regex;
use tokio::net::TcpStream;

pub struct Player {
    pub name: String,
}

pub struct GameState {
    pub players: Vec<Player>,
    pub map: String,
}

pub struct RconController {
    pub connection: Connection<TcpStream>,
    pub address: String,
    pub password: String,
}

impl RconController {
    /// initialize the controller
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

    /// reconnect to tf2 on failure
    pub async fn reconnect(&mut self) -> Result<(), Error> {
        self.connection = <Connection<TcpStream>>::builder()
            .connect(&self.address, &self.password)
            .await?;

        Ok(())
    }

    /// run an rcon command and return the output
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

    pub async fn status(&mut self) -> Result<GameState, Error> {
        let status_msg = self.run("status").await?;

        let players = Self::parse_player_list(&status_msg)?.into_iter().filter(|p| p.name != "kitty girl TV").collect();
        let map = Self::parse_current_map(&status_msg)?;

        Ok(GameState { players, map })
    }

    fn parse_player_list(status_msg: &str) -> Result<Vec<Player>, Error> {
        let re = Regex::new(r#"\d+\s"(.+)""#).unwrap();
        let mut players = Vec::new();
        for caps in re.captures_iter(status_msg) {
            players.push(Player {
                name: caps[1].to_owned(),
            });
        }

        Ok(players)
    }

    fn parse_current_map(status_msg: &str) -> Result<String, Error> {
        let re = Regex::new(r#"map\s+:\s+(.+) at:"#).unwrap();
        if let Some(caps) = re.captures(status_msg) {
            Ok(caps[1].to_owned())
        } else {
            Err("Could not parse current map".into())
        }
    }
}
