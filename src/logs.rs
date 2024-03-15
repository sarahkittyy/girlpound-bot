use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use chrono::Utc;
use tokio::{net::UdpSocket, sync::RwLock};

use srcds_log_parser::{LogMessage, MessageType};

mod util;
pub use util::{as_discord_message, safe_strip};

mod discord;
pub use discord::spawn_log_thread;

use crate::Error;

type LogCallback = Box<dyn Fn(SocketAddr, &LogMessage, &MessageType) + Send + Sync + 'static>;

/// Receives logs from srcds and sends them to the given callbacks
#[derive(Clone)]
pub struct LogReceiver {
    callbacks: Arc<RwLock<Vec<LogCallback>>>,
}

impl LogReceiver {
    /// create and bind a udp socket to listen to srcds logs
    pub async fn connect(addr: Ipv4Addr, port: u16) -> Result<Self, Error> {
        let sock = Arc::new(UdpSocket::bind((addr, port)).await?);
        let callbacks = Arc::new(RwLock::new(Vec::new()));

        let expected_password: Option<String> = std::env::var("SRCDS_LOG_PASSWORD")
            .ok()
            .and_then(|p| if p.len() > 0 { Some(p) } else { None });

        let lr = LogReceiver { callbacks };

        let _task = {
            let sock = sock.clone();
            let lr = lr.clone();
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

                    lr.broadcast_message(from, message).await;
                }
            })
        };

        Ok(lr)
    }

    pub async fn subscribe(&self, cb: LogCallback) {
        self.callbacks.write().await.push(cb);
    }

    async fn broadcast_message(&self, from: SocketAddr, msg: LogMessage) {
        let parsed = MessageType::from_message(msg.message.as_str());
        for cb in self.callbacks.read().await.iter() {
            cb(from.clone(), &msg, &parsed);
        }
    }

    pub async fn _spoof_message(&self, msg: &str) {
        let expected_password: Option<String> = std::env::var("SRCDS_LOG_PASSWORD")
            .ok()
            .and_then(|p| if p.len() > 0 { Some(p) } else { None });
        self.broadcast_message(
            "192.168.0.0:12345".parse().unwrap(),
            LogMessage {
                timestamp: Utc::now().naive_utc(),
                message: msg.to_owned(),
                secret: expected_password,
            },
        )
        .await;
    }
}
