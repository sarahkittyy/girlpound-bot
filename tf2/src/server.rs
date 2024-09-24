use common::Error;
use poise::serenity_prelude::ChannelId;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;

use crate::{RconController, ServerFtp};

/// Factory struct for the tf2 server data
pub struct ServerBuilder {
    pub name: String,
    pub emoji: String,
    pub addr: SocketAddr,
    pub rcon_pass: String,
    pub player_count_cid: Option<u64>,
    pub log_cid: Option<u64>,
    pub ftp_credentials: (String, String),
    pub allow_seed: bool,
    pub show_status: bool,
    pub control_mapfile: bool,
    pub wacky_server: bool,
}

impl ServerBuilder {
    pub async fn build(self) -> Result<Server, Error> {
        let ftp_url: SocketAddr = (self.addr.ip(), 21).into();
        log::info!("Connecting to {:?}...", self.addr);
        Ok(Server {
            name: self.name,
            emoji: self.emoji,
            addr: self.addr,
            controller: Arc::new(RwLock::new(
                RconController::connect(self.addr, &self.rcon_pass).await?,
            )),
            player_count_channel: self.player_count_cid.map(ChannelId::new),
            log_channel: self.log_cid.map(ChannelId::new),
            ftp: ServerFtp::new(ftp_url, self.ftp_credentials),
            allow_seed: self.allow_seed,
            show_status: self.show_status,
            control_mapfile: self.control_mapfile,
            wacky_server: self.wacky_server,
        })
    }
}

/// A single tf2 server to keep track of
#[derive(Clone)]
pub struct Server {
    pub name: String,
    pub emoji: String,
    pub addr: SocketAddr,
    pub controller: Arc<RwLock<RconController>>,
    pub player_count_channel: Option<ChannelId>,
    pub log_channel: Option<ChannelId>,
    pub ftp: ServerFtp,
    pub allow_seed: bool,
    pub show_status: bool,
    pub control_mapfile: bool,
    pub wacky_server: bool,
}

impl Server {
    /// Retrieve this server's maps
    pub async fn maps(&self) -> Result<Vec<String>, Error> {
        self.ftp.fetch_file_lines("tf/cfg/mapcycle.txt").await
    }
    /// Retrieve this server's wacky maps
    pub async fn wacky_maps(&self) -> Result<Vec<String>, Error> {
        if !self.wacky_server {
            return Err("Not a wacky server!".into());
        }
        self.ftp.fetch_file_lines("tf/cfg/mapcycle-wacky.txt").await
    }
}
