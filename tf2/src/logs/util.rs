use srcds_log_parser::MessageType;

use common::util::strip_markdown;

pub fn as_discord_message(msg: &MessageType, dom_score: Option<i32>) -> Option<String> {
    let dominator_dom_score = dom_score
        .map(|s| format!(" **({})**", s))
        .unwrap_or("".to_owned());
    let victim_dom_score = dom_score
        .map(|s| format!(" **({})**", s * -1))
        .unwrap_or("".to_owned());
    match msg {
        MessageType::ChatMessage { from, message, .. } => format!(
            "**{}** :  {}",
            strip_markdown(&from.name),
            strip_markdown(&message)
        )
        .into(),
        MessageType::Connected { user, .. } => format!(
            "+ **{}** `{}` connected.",
            strip_markdown(&user.name),
            user.steamid
        )
        .into(),
        MessageType::Disconnected { user, reason } => format!(
            "\\- **{}** `{}` disconnected: {}",
            strip_markdown(&user.name),
            user.steamid,
            strip_markdown(reason)
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
                ":crossed_swords: **{}**{} is DOMINATING **{}!**{}",
                strip_markdown(&from.name),
                dominator_dom_score,
                strip_markdown(&against.name),
                victim_dom_score
            )),
            "revenge" => Some(format!(
                ":crossed_swords: **{}** got REVENGE on **{}!**",
                strip_markdown(&from.name),
                strip_markdown(&against.name)
            )),
            _ => None,
        },
        MessageType::Unknown => "Unknown message".to_owned().into(),
        _ => None,
    }
}
