use std::{net::SocketAddr, time};

use common::{
    util::{hhmmss, remove_backticks},
    Error,
};

use chrono::{DateTime, TimeDelta, Utc};
use rcon::Connection;
use regex::Regex;
use steam::SteamIDClient;
use tokio::net::TcpStream;

use crate::Server;

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
    pub timeleft: Option<TimeLeft>,
    pub nextmap: Option<NextMap>,
}

impl GameState {
    pub fn as_discord_output(&self, emoji: &str, show_uids: bool) -> String {
        let list = self
            .players
            .iter()
            .map(|p| {
                format!(
                    "{}{}",
                    remove_backticks(&p.name),
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
            Some(TimeLeft::LastRound) => "This is the last round!\n".to_owned(),
            Some(TimeLeft::Time { remaining, rounds }) => {
                if let Some(rounds_left) = rounds {
                    format!("Time left: `{remaining}`, or {rounds_left} more round(s).\n")
                } else {
                    format!("Time left: `{remaining}`\n")
                }
            }
            None => "".to_owned(),
        };
        let nextmap = match &self.nextmap {
            Some(NextMap::Map(map)) => format!("Next map: `{map}`\n"),
            _ => "".to_owned(),
        };
        format!(
            "{0} `{1}/{2}` on `{3}`\n{4}{5}{6}{7}",
            emoji,
            self.players.len(),
            self.max_players,
            self.map,
            // 4
            timeleft,
            nextmap,
            // 6
            if let Some(longest_online) = longest_online {
                format!(
                    "Oldest player: `{}` for `{}`\n",
                    remove_backticks(&longest_online.name),
                    hhmmss(longest_online.connected.as_secs())
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
    pub gamestate_cache: Option<(DateTime<Utc>, GameState)>,
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
            gamestate_cache: None,
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
        if self
            .gamestate_cache
            .as_ref()
            .is_some_and(|(last_updated, _)| {
                last_updated.signed_duration_since(Utc::now()).abs()
                    < TimeDelta::try_seconds(4).unwrap()
            })
        {
            return Ok(self.gamestate_cache.as_ref().unwrap().1.clone());
        }
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
        self.gamestate_cache = Some((Utc::now(), gs.clone()));
        Ok(gs)
    }

    /// fetch the time remaining & next map
    pub async fn timeleft_nextmap(&mut self) -> Result<(Option<TimeLeft>, Option<NextMap>), Error> {
        let response_str = self.run("timeleft; nextmap").await?;
        let mut response = response_str.split('\n');
        let tl_response = response.next().ok_or("Invalid tlnm response")?;
        let nm_response = response.next().ok_or("Invalid tlnm response")?;
        let timeleft_re =
            Regex::new(r#"\[SM\] (?:This is the (last round)|Time remaining for map:\s+(\d+:\d+)(?:, or change map after (\d))?)"#)
                .unwrap();
        let timeleft_caps = timeleft_re.captures(&tl_response);
        let timeleft = if let Some(_) = timeleft_caps.as_ref().and_then(|caps| caps.get(1)) {
            Some(TimeLeft::LastRound)
        } else if let Some(remaining) = timeleft_caps.as_ref().and_then(|caps| caps.get(2)) {
            let rounds: Option<i32> = timeleft_caps
                .as_ref()
                .and_then(|caps| caps.get(3))
                .and_then(|s| s.as_str().parse::<i32>().ok());
            Some(TimeLeft::Time {
                remaining: remaining.as_str().to_owned(),
                rounds,
            })
        } else {
            None
        };

        let nextmap_re = Regex::new(r#"\[SM\] (?:(Pending Vote)|Next Map: (.*))"#).unwrap();
        let nextmap_caps = nextmap_re.captures(&nm_response);
        let nextmap = if let Some(_) = nextmap_caps.as_ref().and_then(|caps| caps.get(1)) {
            Some(NextMap::PendingVote)
        } else if let Some(map) = nextmap_caps.as_ref().and_then(|caps| caps.get(2)) {
            Some(NextMap::Map(map.as_str().to_owned()))
        } else {
            None
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

pub async fn banid(
    client: &SteamIDClient,
    id: &str,
    server: &Server,
    minutes: u32,
    reason: &str,
) -> String {
    //let sid_re = Regex::new(r#"(?:(STEAM_\d+:\d+:\d+)|\[?(.:1:\d+)]?)"#).unwrap();
    let Ok(profile) = client
        .lookup(&id)
        .await
        .and_then(|profiles| profiles.first().cloned().ok_or("No profile found".into()))
    else {
        return format!("Could not resolve given SteamID to a profile.");
    };
    let cmd = format!(
        "sm_addban {} {} {}; kickid \"{}\" {}",
        minutes, &profile.steamid, reason, &profile.steam3, reason
    );
    let _ = rcon_user_output(&[server], cmd).await;
    let time = if minutes == 0 {
        "permanent".to_owned()
    } else {
        minutes.to_string()
    };
    return format!(
        "Banned https://steamcommunity.com/profiles/{} (time: {}) (reason: {})",
        &profile.steamid64, time, reason
    );
}

pub async fn rcon_user_output(servers: &[&Server], cmd: String) -> String {
    let mut outputs: Vec<String> = vec![];
    for server in servers {
        let mut rcon = server.controller.write().await;
        let output = match rcon.run(&cmd).await {
            Ok(output) => {
                if output.is_empty() {
                    ":white_check_mark:".to_owned()
                } else {
                    format!(" `{}`", output.trim())
                }
            }
            Err(e) => e.to_string(),
        };
        outputs.push(format!("{}{}", server.emoji, output))
    }
    outputs.sort();
    outputs.join("\n")
}
