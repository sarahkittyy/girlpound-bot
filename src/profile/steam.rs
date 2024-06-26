use super::UserProfile;
use crate::{discord::Context, gameme, steamid::SteamPlayerSummary, Error};
use sqlx;

#[derive(Clone)]
pub struct SteamProfileData {
    // rank + seconds_seeded
    pub seederboard: Option<(i64, i64)>,
    pub worst_enemy: Option<(SteamPlayerSummary, i64)>,
    pub best_friend: Option<(SteamPlayerSummary, i64)>,
    pub stats: Option<gameme::PlayerLookupData>,
    pub summary: SteamPlayerSummary,
}

pub async fn get_steam_profile_data(
    ctx: &Context<'_>,
    profile: &UserProfile,
) -> Result<Option<SteamProfileData>, Error> {
    let Some(steamid3) = &profile.steamid else {
        return Ok(None);
    };
    // seederboard rank
    let seeding = sqlx::query!(
        r#"
		SELECT `seconds_seeded`, `rank`
		FROM (select *, RANK() OVER (ORDER BY `seconds_seeded` DESC) AS `rank` from `seederboard`) t
		WHERE `steamid` = ?
		LIMIT 1"#,
        steamid3
    )
    .fetch_optional(&ctx.data().local_pool)
    .await?;

    let steamidprofiles = ctx.data().steamid_client.lookup(&steamid3).await?;
    let steamidprofile = steamidprofiles.first().ok_or("Steam lookup failed.")?;
    let summaries = ctx
        .data()
        .steamid_client
        .get_player_summaries(&steamidprofile.steamid64)
        .await?;
    let summary = summaries.first().ok_or("Steam profile not found")?;

    let stats = gameme::get_player(&steamidprofile.steamid).await.ok();

    let best_friend = sqlx::query!("select against, abs(score) as score from (select score, gt_steamid as against from domination where lt_steamid=? order by score asc limit 1) as lts
	UNION ALL
	select against, abs(score) as score from (select score, lt_steamid as against from domination where gt_steamid=? order by score desc limit 1) as gts
	ORDER BY score DESC LIMIT 1;", steamid3, steamid3).fetch_optional(&ctx.data().local_pool).await?;
    let worst_enemy = sqlx::query!("select against, abs(score) as score from (select score, gt_steamid as against from domination where lt_steamid=? order by score desc limit 1) as lts
	UNION ALL
	select against, abs(score) as score from (select score, lt_steamid as against from domination where gt_steamid=? order by score asc limit 1) as gts
	ORDER BY score DESC LIMIT 1;", steamid3, steamid3).fetch_optional(&ctx.data().local_pool).await?;

    let worst_enemy: Option<(SteamPlayerSummary, i64)> = match worst_enemy {
        Some(e) => ctx
            .data()
            .steamid_client
            .lookup_player_summaries(&e.against)
            .await
            .ok()
            .and_then(|ss| ss.first().cloned())
            .and_then(|s| Some((s, e.score))),
        _ => None,
    };
    let best_friend: Option<(SteamPlayerSummary, i64)> = match best_friend {
        Some(e) => ctx
            .data()
            .steamid_client
            .lookup_player_summaries(&e.against)
            .await
            .ok()
            .and_then(|ss| ss.first().cloned())
            .and_then(|s| Some((s.clone(), e.score))),
        _ => None,
    };

    Ok(Some(SteamProfileData {
        seederboard: seeding.map(|s| (s.rank, s.seconds_seeded.unwrap_or(0))),
        worst_enemy,
        best_friend,
        stats,
        summary: summary.clone(),
    }))
}
