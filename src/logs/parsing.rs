use std::net::Ipv4Addr;

use super::LogMessage;

use nom::{bytes::complete::*, character::complete::*, sequence::Tuple, IResult, Parser};

/// a parsed log message
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParsedLogMessage {
    ChatMessage { from: User, message: String },
    Connected { user: User, ip: Ipv4Addr, port: u16 },
    Disconnected { user: User, reason: String },
    Unknown,
}

impl ParsedLogMessage {
    pub fn from_message(msg: &LogMessage) -> Self {
        let i: &str = &msg.message;
        if let Ok((_, m)) = parse_log_message(i) {
            m
        } else {
            println!("unparsed message: {}", i);
            ParsedLogMessage::Unknown
        }
    }

    pub fn is_known(&self) -> bool {
        self != &ParsedLogMessage::Unknown
    }

    pub fn as_discord_message(&self) -> String {
        match self {
            ParsedLogMessage::ChatMessage { from, message } => {
                format!("`{}: {}`", from.name, message)
            }
            ParsedLogMessage::Connected { user, .. } => {
                format!("`{} {} connected.`", user.name, user.steamid)
            }
            ParsedLogMessage::Disconnected { user, reason } => {
                format!("`{} {} disconnected: {}`", user.name, user.steamid, reason)
            }
            ParsedLogMessage::Unknown => "Unknown message".to_owned(),
        }
    }
}

fn parse_log_message(i: &str) -> IResult<&str, ParsedLogMessage> {
    chat_message
        .or(connect_message)
        .or(disconnect_message)
        .parse(i)
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct User {
    pub name: String,
    pub uid: u32,
    pub steamid: String,
    pub team: String,
}

fn ipv4(i: &str) -> IResult<&str, Ipv4Addr> {
    let (i, (a, _, b, _, c, _, d)) = (
        digit1,
        char('.'),
        digit1,
        char('.'),
        digit1,
        char('.'),
        digit1,
    )
        .parse(i)?;

    Ok((
        i,
        Ipv4Addr::new(
            a.parse().unwrap(),
            b.parse().unwrap(),
            c.parse().unwrap(),
            d.parse().unwrap(),
        ),
    ))
}

fn user(i: &str) -> IResult<&str, User> {
    let (i, _startquote) = tag("\"")(i)?;
    let (i, name) = take_until1("<")(i)?;
    let (i, (_, uid, _)) = (char('<'), digit1, char('>')).parse(i)?;
    let (i, (_, steamid, _)) = (char('<'), take_until1(">"), char('>')).parse(i)?;
    let (i, (_, team, _)) = (char('<'), take_while(char::is_alphabetic), char('>')).parse(i)?;
    let (i, _endquote) = tag("\"")(i)?;

    Ok((
        i,
        User {
            name: name.to_owned(),
            uid: uid.parse().unwrap(),
            steamid: steamid.to_owned(),
            team: team.to_owned(),
        },
    ))
}

fn disconnect_message(i: &str) -> IResult<&str, ParsedLogMessage> {
    let (i, user) = user(i)?;
    let (i, _) = tag(" disconnected (reason ")(i)?;
    let (i, (_, reason, _)) = (char('"'), take_until1("\""), tag("\")")).parse(i)?;
    Ok((
        i,
        ParsedLogMessage::Disconnected {
            user,
            reason: reason.to_owned(),
        },
    ))
}

fn connect_message(i: &str) -> IResult<&str, ParsedLogMessage> {
    let (i, user) = user(i)?;
    let (i, _) = tag(" connected, address ")(i)?;
    let (i, (_, ip, _)) = (char('"'), ipv4, char(':')).parse(i)?;
    let (i, port) = digit1(i)?;
    Ok((
        i,
        ParsedLogMessage::Connected {
            user,
            ip,
            port: port.parse().unwrap(),
        },
    ))
}

fn chat_message(i: &str) -> IResult<&str, ParsedLogMessage> {
    let (i, user) = user(i)?;
    let (i, _say) = tag(" say ")(i)?;
    let (i, (_, message, _)) = (char('"'), take_until1("\""), char('"')).parse(i)?;

    Ok((
        i,
        ParsedLogMessage::ChatMessage {
            from: user,
            message: message.to_owned(),
        },
    ))
}
