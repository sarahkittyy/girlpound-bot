use std::{collections::VecDeque, fmt::Display, net::Ipv4Addr, sync::Arc};

use chrono::{DateTime, NaiveDateTime};
use tokio::{net::UdpSocket, sync::RwLock, task::JoinHandle};

use crate::Error;

const PACKET_HEADER: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
const MAGIC_NOPASSWORD_BYTE: u8 = 0x52; // R
const MAGIC_PASSWORD_BYTE: u8 = 0x53; // S
const MAGIC_STRING_END: u8 = 0x4C; // L

#[derive(Debug)]
pub struct LogMessage {
    timestamp: DateTime<chrono::Utc>,
    message: String,
    password: Option<String>,
}

impl Display for LogMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} | {}",
            self.timestamp.format("%m/%d %H:%M:%S"),
            self.message
        )
    }
}

pub struct LogReceiver {
    sock: Arc<UdpSocket>,
    task: JoinHandle<()>,
    messages: Arc<RwLock<VecDeque<LogMessage>>>,
}

impl LogReceiver {
    /// create and bind a udp socket to listen to srcds logs
    pub async fn connect(addr: Ipv4Addr, port: u16) -> Result<Self, Error> {
        let sock = Arc::new(UdpSocket::bind((addr, port)).await?);
        let messages = Arc::new(RwLock::new(VecDeque::new()));
        let task = {
            let sock = sock.clone();
            let messages = messages.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                loop {
                    let (len, _) = sock.recv_from(&mut buf).await.unwrap();
                    let message = match try_parse_packet(&buf[..len]) {
                        Ok(m) => m,
                        Err(e) => {
                            println!("Could not parse packet: {e:?}");
                            continue;
                        }
                    };
                    messages.write().await.push_back(message);
                }
            })
        };

        Ok(LogReceiver {
            sock,
            task,
            messages,
        })
    }

    /// retrieve all log messages from the queue
    pub async fn drain(&mut self) -> Vec<LogMessage> {
        let mut messages = self.messages.write().await;
        messages.drain(..).collect()
    }
}

#[derive(Debug)]
enum PacketParseError {
    TooShort,
    InvalidHeader,
    BadPasswordByte,
    NoMagicStringEnd,
    BadTimestamp,
}

impl Display for PacketParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PacketParseError")
    }
}

impl std::error::Error for PacketParseError {}

fn try_parse_packet(data: &[u8]) -> Result<LogMessage, Error> {
    if data.len() < 16 {
        return Err(PacketParseError::TooShort.into());
    }
    // magic first 4 bytes
    let header = &data[..4];
    if header != PACKET_HEADER {
        return Err(PacketParseError::InvalidHeader.into());
    }
    // password byte
    let password_byte = data[4];
    let (password, rest) = if password_byte == MAGIC_PASSWORD_BYTE {
        let password_end = data[5..]
            .iter()
            .position(|&x| x == MAGIC_STRING_END)
            .ok_or(PacketParseError::NoMagicStringEnd)?;
        let password = String::from_utf8_lossy(&data[5..5 + password_end]).to_string();
        (Some(password), &data[5 + password_end..])
    } else if password_byte == MAGIC_NOPASSWORD_BYTE {
        (None, &data[5..])
    } else {
        return Err(PacketParseError::BadPasswordByte.into());
    };
    // magic string ending
    if rest[0] != MAGIC_STRING_END {
        return Err(PacketParseError::NoMagicStringEnd.into());
    }
    // header parsed, now this starts from the timestamp
    let rest = &rest[2..];
    let message = String::from_utf8_lossy(rest).to_string();
    let (timestamp, rest) = NaiveDateTime::parse_and_remainder(&message, "%m/%d/%Y - %H:%M:%S: ")
        .map_err(|_| PacketParseError::BadTimestamp)?;

    Ok(LogMessage {
        timestamp: timestamp.and_utc(),
        message: rest[0..rest.len() - 2].to_owned(),
        password,
    })
}
