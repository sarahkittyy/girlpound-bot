use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;

use common::Error;

pub const BASEURL: &'static str = "https://stats.fluffycat.gay/";

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct PlayerLookupData {
    pub rankinginfo: RankingInfo,
    pub playerlist: PlayerList,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct RankingInfo {
    pub totalplayers: i32,
    pub activeplayers: i32,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct PlayerList {
    #[serde(rename = "player")]
    pub players: Vec<Player>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Player {
    pub id: u64,
    pub name: String,
    pub uniqueid: String,
    pub avatar: String,
    pub activity: f32,
    pub rank: u64,
}

impl PlayerLookupData {
    /// Get the first matched player, if any
    pub fn get_first_player(&self) -> Option<&Player> {
        self.playerlist.players.first()
    }
}

pub async fn get_player(steamid: &str) -> Result<PlayerLookupData, Error> {
    let url = format!("{BASEURL}/api/playerlist/tf/uniqueid/{steamid}");
    let raw_xml = reqwest::get(url).and_then(|resp| resp.text()).await?;
    let pld: PlayerLookupData = from_str(&raw_xml)?;

    Ok(pld)
}
