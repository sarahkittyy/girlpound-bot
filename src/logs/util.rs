use srcds_log_parser::MessageType;

pub fn as_discord_message(msg: &MessageType, dom_score: Option<i32>) -> Option<String> {
    let dominator_dom_score = dom_score
        .map(|s| format!(" **({})**", s))
        .unwrap_or("".to_owned());
    let victim_dom_score = dom_score
        .map(|s| format!(" **({})**", s * -1))
        .unwrap_or("".to_owned());
    match msg {
        MessageType::ChatMessage { from, message, .. } => {
            format!("`{}: {}`", safe_strip(&from.name), safe_strip(&message)).into()
        }
        MessageType::Connected { user, .. } => {
            format!("+ `{} {} connected.`", safe_strip(&user.name), user.steamid).into()
        }
        MessageType::Disconnected { user, reason } => format!(
            "\\- `{} {} disconnected: {}`",
            safe_strip(&user.name),
            user.steamid,
            reason
        )
        .into(),
        /*MessageType::JoinedTeam { user, team } => format!(
            "+ `{} {} joined team {}`",
            safe_strip(&user.name),
            user.steamid,
            team
        )
        .into(),*/
        MessageType::StartedMap { name, .. } => format!(":map: Changed map: `{}`", name).into(),
        MessageType::InterPlayerAction {
            from,
            against,
            action,
        } => match action.as_str() {
            "domination" => Some(format!(
                ":crossed_swords: `{}`{} is DOMINATING `{}!`{}",
                safe_strip(&from.name),
                dominator_dom_score,
                safe_strip(&against.name),
                victim_dom_score
            )),
            "revenge" => Some(format!(
                ":crossed_swords: `{}` got REVENGE on `{}!`",
                safe_strip(&from.name),
                safe_strip(&against.name)
            )),
            _ => None,
        },
        MessageType::Unknown => "Unknown message".to_owned().into(),
        _ => None,
    }
}

pub fn safe_strip(s: &str) -> String {
    s.replace("`", "")
}
