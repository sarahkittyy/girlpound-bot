use std::{
    net::SocketAddr,
    time::{self, Duration},
};

use crate::{logs::safe_strip, Error, Server};

use rcon::Connection;
use regex::Regex;
use tokio::net::TcpStream;

#[derive(Debug, Clone)]
pub struct Player {
    pub name: String,
    pub connected: time::Duration,
    pub id: String,
}

#[derive(Debug, Clone)]
pub enum TimeLeft {
    LastRound,
    Time {
        remaining: String,
        rounds: Option<i32>,
    },
}

#[derive(Debug, Clone)]
pub enum NextMap {
    PendingVote,
    Map(String),
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub players: Vec<Player>,
    pub max_players: i32,
    pub map: String,
    pub timeleft: TimeLeft,
    pub nextmap: NextMap,
}

fn hhmmss(duration: &Duration) -> String {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

impl GameState {
    pub fn as_discord_output(&self, server: &Server, show_uids: bool) -> String {
        let list = self
            .players
            .iter()
            .map(|p| {
                format!(
                    "{}{}",
                    safe_strip(&p.name),
                    &if show_uids {
                        " ".to_owned() + &p.id
                    } else {
                        "".to_owned()
                    }
                )
            })
            .collect::<Vec<String>>();
        let longest_online = self.players.iter().max_by_key(|p| p.connected);
        let timeleft = match &self.timeleft {
            TimeLeft::LastRound => "This is the last round!".to_owned(),
            TimeLeft::Time { remaining, rounds } => {
                if let Some(rounds_left) = rounds {
                    format!("Time left: `{remaining}`, or {rounds_left} more round(s).")
                } else {
                    format!("Time left: `{remaining}`")
                }
            }
        };
        format!(
            "{0} `{1}/{2}` on `{3}`\n{4}\n{5}{6}\n{7}",
            server.emoji,
            self.players.len(),
            self.max_players,
            self.map,
            timeleft,
            if let NextMap::Map(map) = &self.nextmap {
                format!("Next map: `{map}`\n")
            } else {
                "".to_owned()
            },
            if let Some(longest_online) = longest_online {
                format!(
                    "Oldest player: `{}` for `{}`",
                    safe_strip(&longest_online.name),
                    hhmmss(&longest_online.connected)
                )
            } else {
                "".to_owned()
            },
            if !list.is_empty() {
                format!("`{}`\n", list.join(if show_uids { "\n" } else { " | " }))
            } else {
                "".to_owned()
            }
        )
    }
}

pub struct RconController {
    pub connection: Connection<TcpStream>,
    pub address: SocketAddr,
    pub password: String,
}

impl RconController {
    /// initialize the controller
    pub async fn connect(address: SocketAddr, password: &str) -> Result<Self, Error> {
        let connection = <Connection<TcpStream>>::builder()
            .connect(address, password)
            .await?;

        let rc = RconController {
            connection,
            address,
            password: password.to_owned(),
        };
        Ok(rc)
    }

    /// reconnect to tf2 on failure
    pub async fn reconnect(&mut self) -> Result<(), Error> {
        self.connection = <Connection<TcpStream>>::builder()
            .connect(&self.address, &self.password)
            .await?;

        Ok(())
    }

    /// fetch the value of a convar
    pub async fn convar(&mut self, convar: &str) -> Result<String, Error> {
        let result = self.run(convar).await?;
        let re = Regex::new(r#"".*" = "(.*?)""#).unwrap();
        if let Some(caps) = re.captures(&result) {
            Ok(caps[1].to_owned())
        } else {
            Err("Could not parse convar result".into())
        }
    }

    /// run an rcon command and return the output
    pub async fn run(&mut self, cmd: &str) -> Result<String, Error> {
        match self.connection.cmd(cmd).await {
            Ok(msg) => Ok(msg),
            Err(e) => {
                self.reconnect().await?;
                Err(format!("Failed to connect, retrying. Error {}", e))?
            }
        }
    }

    /// fetch the results of the status command
    pub async fn status(&mut self) -> Result<GameState, Error> {
        let status_msg = self.run("status").await?;
        let max_players_cvar = self.convar("sv_visiblemaxplayers").await?;
        let max_players: i32 = max_players_cvar.parse()?;
        let (timeleft, nextmap) = self.timeleft_nextmap().await?;

        let players = Self::parse_player_list(&status_msg)?
            .into_iter()
            .filter(|p| p.name != "tiny kitty TV")
            .collect();
        let map = Self::parse_current_map(&status_msg)?;

        let gs = GameState {
            players,
            map,
            max_players,
            timeleft,
            nextmap,
        };
        Ok(gs)
    }

    /// fetch the time remaining & next map
    pub async fn timeleft_nextmap(&mut self) -> Result<(TimeLeft, NextMap), Error> {
        let response_str = self.run("timeleft; nextmap").await?;
        let mut response = response_str.split('\n');
        let tl_response = response.next().ok_or("Invalid tlnm response")?;
        let nm_response = response.next().ok_or("Invalid tlnm response")?;
        let timeleft_re =
            Regex::new(r#"\[SM\] (?:This is the (last round)|Time remaining for map:\s+(\d+:\d+)(?:, or change map after (\d))?)"#)
                .unwrap();
        let timeleft_caps = timeleft_re
            .captures(&tl_response)
            .ok_or("Could not match timeleft")?;
        let timeleft = if let Some(_) = timeleft_caps.get(1) {
            TimeLeft::LastRound
        } else if let Some(remaining) = timeleft_caps.get(2) {
            let rounds: Option<i32> = timeleft_caps
                .get(3)
                .and_then(|s| s.as_str().parse::<i32>().ok());
            TimeLeft::Time {
                remaining: remaining.as_str().to_owned(),
                rounds,
            }
        } else {
            return Err("Could not get time left".into());
        };

        let nextmap_re = Regex::new(r#"\[SM\] (?:(Pending Vote)|Next Map: (.*))"#).unwrap();
        let nextmap_caps = nextmap_re
            .captures(&nm_response)
            .ok_or("Could not match nextmap")?;
        let nextmap = if let Some(_) = nextmap_caps.get(1) {
            NextMap::PendingVote
        } else if let Some(map) = nextmap_caps.get(2) {
            NextMap::Map(map.as_str().to_owned())
        } else {
            return Err("Could not get next map".into());
        };

        Ok((timeleft, nextmap))
    }

    fn parse_player_list(status_msg: &str) -> Result<Vec<Player>, Error> {
        let re = Regex::new(r#"\d+\s+"(.+)"\s+(\[U:.*\])\s+(\d+):(\d+)(?::(\d+))?"#).unwrap();
        let mut players = Vec::new();
        for caps in re.captures_iter(status_msg) {
            let id = caps[2].to_owned();
            let h = caps[3].parse::<u64>()?;
            let m = caps[4].parse::<u64>()?;
            let s: Option<u64> = caps.get(5).and_then(|s| s.as_str().parse::<u64>().ok());

            let connected = time::Duration::from_secs(if let Some(s) = s {
                h * 3600 + m * 60 + s
            } else {
                h * 60 + m
            });

            players.push(Player {
                name: caps[1].to_owned(),
                id,
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
