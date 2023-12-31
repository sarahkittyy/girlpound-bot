use regex::Regex;
use std::net::Ipv4Addr;

use super::LogMessage;

use nom::{
    bytes::complete::*, character::complete::*, combinator::fail, sequence::Tuple, Err, IResult,
    Parser,
};

/// a parsed log message
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParsedLogMessage {
    ChatMessage { from: User, message: String },
    Connected { user: User, ip: Ipv4Addr, port: u16 },
    Disconnected { user: User, reason: String },
    JoinedTeam { user: User, team: String },
    StartedMap(String),
    Domination { from: User, to: User },
    Revenge { from: User, to: User },
    Unknown,
}

impl ParsedLogMessage {
    pub fn from_message(msg: &LogMessage) -> Self {
        let i: &str = &msg.message;
        match parse_log_message(i) {
            Ok((_, m)) => m,
            Err(_) => ParsedLogMessage::Unknown,
        }
    }

    pub fn is_unknown(&self) -> bool {
        match self {
            Self::Unknown => true,
            _ => false,
        }
    }

    pub fn as_discord_message(&self, dom_score: Option<i32>) -> Option<String> {
        let dominator_dom_score = dom_score
            .map(|s| format!(" **({})**", s))
            .unwrap_or("".to_owned());
        let victim_dom_score = dom_score
            .map(|s| format!(" **({})**", s * -1))
            .unwrap_or("".to_owned());
        match self {
            ParsedLogMessage::ChatMessage { from, message } => {
                format!("`{}: {}`", safe_strip(&from.name), safe_strip(message)).into()
            }
            ParsedLogMessage::Connected { user, .. } => {
                format!("+ `{} {} connected.`", safe_strip(&user.name), user.steamid).into()
            }
            ParsedLogMessage::Disconnected { user, reason } => format!(
                "\\- `{} {} disconnected: {}`",
                safe_strip(&user.name),
                user.steamid,
                reason
            )
            .into(),
            /*ParsedLogMessage::JoinedTeam { user, team } => format!(
                "+ `{} {} joined team {}`",
                safe_strip(&user.name),
                user.steamid,
                team
            )
            .into(),*/
            ParsedLogMessage::StartedMap(map) => format!(":map: Changed map: `{}`", map).into(),
            ParsedLogMessage::Revenge { from, to } => format!(
                ":crossed_swords: `{}` got REVENGE on `{}!`",
                safe_strip(&from.name),
                safe_strip(&to.name)
            )
            .into(),
            ParsedLogMessage::Domination { from, to } => format!(
                ":crossed_swords: `{}`{} is DOMINATING `{}!`{}",
                safe_strip(&from.name),
                dominator_dom_score,
                safe_strip(&to.name),
                victim_dom_score
            )
            .into(),
            ParsedLogMessage::Unknown => "Unknown message".to_owned().into(),
            _ => None,
        }
    }
}

pub fn safe_strip(s: &str) -> String {
    s.replace("`", "")
}

fn parse_log_message(i: &str) -> IResult<&str, ParsedLogMessage> {
    chat_message
        .or(connect_message)
        .or(disconnect_message)
        .or(start_map_message)
        .or(vengeance_message)
        .or(join_team_msg)
        .parse(i)
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct User {
    pub name: String,
    pub uid: u32,
    pub steamid: String,
    pub team: String,
}

fn join_team_msg(i: &str) -> IResult<&str, ParsedLogMessage> {
    let (i, user) = user(i)?;
    let (i, _) = tag(" joined team ")(i)?;
    let (i, (_, team, _)) = (char('"'), take_until1("\""), char('"')).parse(i)?;
    Ok((
        i,
        ParsedLogMessage::JoinedTeam {
            user,
            team: team.to_owned(),
        },
    ))
}

fn vengeance_message(i: &str) -> IResult<&str, ParsedLogMessage> {
    let (i, from) = user(i)?;
    if let Ok((i2, (_, to))) = (tag(" triggered \"domination\" against "), user).parse(i) {
        Ok((i2, ParsedLogMessage::Domination { from, to }))
    } else if let Ok((i2, (_, to))) = (tag(" triggered \"revenge\" against "), user).parse(i) {
        Ok((i2, ParsedLogMessage::Revenge { from, to }))
    } else {
        fail(i)
    }
}

fn start_map_message(i: &str) -> IResult<&str, ParsedLogMessage> {
    let (i, _) = tag("Started map ")(i)?;
    let (i, (_, map, _)) = (char('"'), take_until1("\""), char('"')).parse(i)?;
    Ok((i, ParsedLogMessage::StartedMap(map.to_owned())))
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
    let re = Regex::new(r#""(.*?)<(\d+)><(\[U:\d:\d+\])><(\w*)?>""#).unwrap();
    let Some(caps) = re.captures(i) else {
        return Err(Err::Error(nom::error::Error::new(
            i,
            nom::error::ErrorKind::Tag,
        )));
    };

    let len = caps.get(0).unwrap().len();
    let name = caps.get(1).unwrap().as_str();
    let uid = caps.get(2).unwrap().as_str();
    let steamid = caps.get(3).unwrap().as_str();
    let team = caps.get(4).unwrap().as_str();

    Ok((
        &i[len..],
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
