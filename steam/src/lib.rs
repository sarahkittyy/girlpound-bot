use std::str::FromStr;

use poise::serenity_prelude as serenity;
use regex::Regex;
use reqwest;
use serenity::CreateEmbed;

use common::Error;
use serde::{Deserialize, Serialize};

const STEAMID_BASEURL: &'static str = "https://steamidapi.uk/v2/";
const STEAM_BASEURL: &'static str = "https://api.steampowered.com/";
const STEAM_VANITY_ROUTE: &'static str = "ISteamUser/ResolveVanityURL/v0001/";
const STEAM_INFO_ROUTE: &'static str = "ISteamUser/GetPlayerSummaries/v0002/";

mod profile;
pub use profile::SteamProfileData;

pub struct SteamIDClient {
    myid: u64,
    steamid_api_key: String,
    steam_api_key: String,
    client: reqwest::Client,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SteamIDProfile {
    pub steamid64: String,
    pub steamid: String,
    pub steam3: String,
    pub steamidurl: String,
    pub inviteurl: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SteamPlayerSummary {
    pub steamid: String,
    pub avatarmedium: String,
    pub personaname: String,
    pub profileurl: String,
}

enum SteamProfileURL {
    Vanity(String),
    SteamID64(u64),
}

impl FromStr for SteamProfileURL {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // parse input url
        let regex =
            Regex::new(r#"(?:https?://)?steamcommunity\.com/(id|profiles)/([^/]+)/?"#).unwrap();
        let caps = regex
            .captures(s.trim())
            .ok_or("Is not a valid steam profile url.")?;
        // /id/ is vanity, /profiles/ with digits is just directly a steamid64
        let idtype = caps.get(1).unwrap().as_str();
        let id = caps.get(2).unwrap().as_str();

        match idtype {
            "profiles" => Ok(SteamProfileURL::SteamID64(id.parse()?)),
            "id" => Ok(SteamProfileURL::Vanity(id.to_owned())),
            _ => Err("Invalid profile type. Valid: /profiles/, /id/".into()),
        }
    }
}

impl SteamIDProfile {
    pub fn to_embed(&self) -> CreateEmbed {
        let mut embed = CreateEmbed::new();
        embed = embed.field("SteamID64", &self.steamid64, false);
        embed = embed.field("SteamID", &self.steamid, false);
        embed = embed.field("Steam3", &self.steam3, false);
        embed = embed.field("SteamID URL", &self.steamidurl, false);
        if let Some(inviteurl) = &self.inviteurl {
            embed = embed.field("Invite URL", inviteurl, false);
        }
        embed
    }
}

impl SteamIDClient {
    pub fn new(myid: u64, steamid_api_key: String, steam_api_key: String) -> Self {
        Self {
            myid,
            steamid_api_key,
            steam_api_key,
            client: reqwest::Client::new(),
        }
    }

    /// returns a steam user's player info
    pub async fn get_player_summaries(
        &self,
        steamids: &str,
    ) -> Result<Vec<SteamPlayerSummary>, Error> {
        let path = format!("{}{}", STEAM_BASEURL, STEAM_INFO_ROUTE);
        let resp = self
            .client
            .get(path)
            .query(&[
                ("key", self.steam_api_key.clone()),
                ("steamids", steamids.to_string()),
            ])
            .send()
            .await?;
        let body = resp.text().await?;
        let response: serde_json::Value = serde_json::from_str(&body)?;
        let players = response
            .get("response")
            .and_then(|r| r.get("players"))
            .and_then(|players| players.as_array())
            .ok_or("Could not parse response body.")?;

        Ok(players
            .into_iter()
            .map(|v| serde_json::from_value(v.clone()))
            .flatten()
            .collect())
    }

    pub async fn lookup_player_summaries(
        &self,
        input: &str,
    ) -> Result<Vec<SteamPlayerSummary>, Error> {
        let looked = self.lookup(input).await?;
        self.get_player_summaries(
            &looked
                .iter()
                .map(|l| l.steamid64.clone())
                .collect::<Vec<String>>()
                .join(","),
        )
        .await
    }

    /// resolves a steam profile url to a steamid64
    async fn resolve_vanity(&self, url: SteamProfileURL) -> Result<u64, Error> {
        let vanityurl = match url {
            SteamProfileURL::SteamID64(id) => return Ok(id),
            SteamProfileURL::Vanity(v) => v,
        };

        // otherwise, fetch conversion api
        let path = format!("{}{}", STEAM_BASEURL, STEAM_VANITY_ROUTE);
        let resp = self
            .client
            .get(path)
            .query(&[
                ("key", self.steam_api_key.clone()),
                ("vanityurl", vanityurl),
            ])
            .send()
            .await?;
        let body = resp.text().await?;
        let response: serde_json::Value = serde_json::from_str(&body)?;

        if let Some(steamid) = response.get("response").and_then(|r| r.get("steamid")) {
            Ok(steamid
                .as_str()
                .and_then(|s| s.parse().ok())
                .ok_or("Could not parse steamid response field")?)
        } else if let Some(error) = response.get("response").and_then(|r| r.get("message")) {
            Err(format!("Steam API error: {}", error).into())
        } else {
            Err("Invalid response from steam api.".into())
        }
    }

    /// Convert between steam urls.
    pub async fn lookup(&self, input: &str) -> Result<Vec<SteamIDProfile>, Error> {
        let input = if let Ok(url) = input.parse::<SteamProfileURL>() {
            self.resolve_vanity(url).await?.to_string()
        } else {
            input.to_string()
        };

        let resp = self
            .client
            .get(format!("{}{}", STEAMID_BASEURL, "convert.php"))
            .query(&[
                ("myid", &self.myid.to_string()),
                ("apikey", &self.steamid_api_key),
                ("input", &input),
            ])
            .send()
            .await?;
        // check for errors
        let body = resp.text().await?;
        let response: serde_json::Value = serde_json::from_str(&body)?;
        let response = if let Some(errormsg) = response.get("error").and_then(|e| e.get("errormsg"))
        {
            return Err(format!("steamid.uk error: {}", errormsg).into());
        } else if let Some(converted) = response.get("converted") {
            if converted.is_object() {
                Ok(vec![serde_json::from_value(converted.clone())?])
            } else if converted.is_array() {
                Ok(serde_json::from_value(converted.clone())?)
            } else {
                Err("Invalid response from SteamID API".to_string())?
            }
        } else {
            Err("Invalid response from SteamID API".to_string())?
        };
        response
    }
}
