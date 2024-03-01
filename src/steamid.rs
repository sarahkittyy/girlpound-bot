use poise::serenity_prelude as serenity;
use reqwest;
use serenity::CreateEmbed;

use crate::Error;
use serde::{Deserialize, Serialize};

const BASEURL: &'static str = "https://steamidapi.uk/v2/";

pub struct SteamIDClient {
    myid: u64,
    api_key: String,
    client: reqwest::Client,
}

#[derive(Deserialize, Serialize)]
pub struct SteamIDProfile {
    pub steamid64: String,
    pub steamid: String,
    pub steam3: String,
    pub steamidurl: String,
    pub inviteurl: Option<String>,
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
    pub fn new(myid: u64, api_key: String) -> Self {
        Self {
            myid,
            api_key,
            client: reqwest::Client::new(),
        }
    }

    pub async fn lookup(&self, input: &str) -> Result<Vec<SteamIDProfile>, Error> {
        let resp = self
            .client
            .get(format!("{}{}", BASEURL, "convert.php"))
            .query(&[
                ("myid", &self.myid.to_string()),
                ("apikey", &self.api_key),
                ("input", &input.to_owned()),
            ])
            .send()
            .await?;
        // check for errors
        let body = resp.text().await?;
        let response: serde_json::Value = serde_json::from_str(&body)?;
        let response = if let Some(errormsg) = response.get("error").and_then(|e| e.get("errormsg"))
        {
            Err(errormsg.to_string())?
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
