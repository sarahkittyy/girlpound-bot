use std::time;

use crate::Error;

use rcon::Connection;
use regex::Regex;
use tokio::net::TcpStream;

#[derive(Debug)]
pub struct Player {
    pub name: String,
    pub connected: time::Duration,
}

#[derive(Debug)]
pub struct GameState {
    pub players: Vec<Player>,
    pub max_players: i32,
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

    pub async fn status(&mut self) -> Result<GameState, Error> {
        let status_msg = self.run("status").await?;
        let max_player_msg = self.run("sv_visiblemaxplayers").await?;
        let re = Regex::new(r#""sv_visiblemaxplayers" = "(\d+)""#).unwrap();
        let Some(max_players) = re
            .captures(&max_player_msg)
            .map(|caps| caps[1].parse::<i32>().unwrap())
        else {
            return Err("Could not parse player count".into());
        };

        let players = Self::parse_player_list(&status_msg)?
            .into_iter()
            .filter(|p| p.name != "tiny kitty TV")
            .collect();
        let map = Self::parse_current_map(&status_msg)?;

        Ok(GameState {
            players,
            map,
            max_players,
        })
    }

    fn parse_player_list(status_msg: &str) -> Result<Vec<Player>, Error> {
        let re = Regex::new(r#"\d+\s+"(.+)"\s+\[U:.*\]\s+(\d+):(\d+)(?::(\d+))?"#).unwrap();
        let mut players = Vec::new();
        for caps in re.captures_iter(status_msg) {
            let h = caps[2].parse::<u64>()?;
            let m = caps[3].parse::<u64>()?;
            let s: Option<u64> = caps.get(4).map(|s| {
                s.as_str()
                    .parse::<u64>()
                    .expect("Could not parse status message seconds connected.")
            });

            let connected = time::Duration::from_secs(if let Some(s) = s {
                h * 3600 + m * 60 + s
            } else {
                h * 60 + m
            });

            players.push(Player {
                name: caps[1].to_owned(),
                connected,
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
