use std::{
    collections::VecDeque,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use chrono::Utc;
use tokio::{net::UdpSocket, sync::RwLock};

use srcds_log_parser::LogMessage;

mod util;
pub use util::{as_discord_message, safe_strip};

use crate::Error;

const PACKET_HEADER: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
const MAGIC_NOPASSWORD_BYTE: u8 = 0x52; // R
const MAGIC_PASSWORD_BYTE: u8 = 0x53; // S
const MAGIC_STRING_END: u8 = 0x4C; // L

#[derive(Clone)]
pub struct LogReceiver {
    messages: Arc<RwLock<VecDeque<(SocketAddr, LogMessage)>>>,
}

impl LogReceiver {
    /// create and bind a udp socket to listen to srcds logs
    pub async fn connect(addr: Ipv4Addr, port: u16) -> Result<Self, Error> {
        let sock = Arc::new(UdpSocket::bind((addr, port)).await?);
        let messages = Arc::new(RwLock::new(VecDeque::new()));

        let expected_password: Option<String> = std::env::var("SRCDS_LOG_PASSWORD")
            .ok()
            .and_then(|p| if p.len() > 0 { Some(p) } else { None });

        let _task = {
            let sock = sock.clone();
            let messages = messages.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                loop {
                    let (len, from) = sock.recv_from(&mut buf).await.unwrap();
                    let message = match LogMessage::from_bytes(&buf[..len]) {
                        Ok(m) => m,
                        Err(e) => {
                            println!("Could not parse packet from {from:?} with len {len}: {e:?}");
                            continue;
                        }
                    };
                    if expected_password.is_some() && message.secret != expected_password {
                        continue;
                    }
                    messages.write().await.push_back((from, message));
                }
            })
        };

        Ok(LogReceiver { messages })
    }

    /// retrieve all log messages from the queue
    pub async fn drain(&mut self) -> Vec<(SocketAddr, LogMessage)> {
        let mut messages = self.messages.write().await;
        messages.drain(..).collect()
    }

    pub async fn _spoof_message(&self, msg: &str) {
        let expected_password: Option<String> = std::env::var("SRCDS_LOG_PASSWORD")
            .ok()
            .and_then(|p| if p.len() > 0 { Some(p) } else { None });
        self.messages.write().await.push_back((
            "192.168.0.0:12345".parse().unwrap(),
            LogMessage {
                timestamp: Utc::now().naive_utc(),
                message: msg.to_owned(),
                secret: expected_password,
            },
        ));
    }
}
