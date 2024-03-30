use crate::Error;
use futures::{future::try_join_all, TryFutureExt};
use regex::Regex;
use reqwest;
use scraper::{Html, Selector};

pub const BASEURL4: &'static str = "http://stats4.fluffycat.gay/";
pub const BASEURL5: &'static str = "http://stats5.fluffycat.gay/";

#[derive(Clone)]
pub struct PsychoStats {
    pub name: String,
    pub id: u64,
    pub rank: u64,
    pub percentile: f32,
    pub kd: f32,
}

/// returns the unique player ids for stats4 and stats5.
pub async fn find_plr(steamid: &str) -> Result<(Option<PsychoStats>, Option<PsychoStats>), Error> {
    // query stats4 and stats5
    let urls = [BASEURL4, BASEURL5]
        .into_iter()
        .map(|base| format!("{base}index.php?q={}", steamid))
        .map(|query| reqwest::get(query).and_then(|resp| resp.text()));
    let pages = try_join_all(urls).await?;

    let table_row = Selector::parse("table.ps-table > tbody > tr:nth-child(2)").unwrap();

    // total player count selector
    let overall_player_sel =
        Selector::parse("div#ps-page-title > div.inner > h2 > strong:nth-child(4)").unwrap();
    // player selector
    let player_sel = Selector::parse("a.plr").unwrap();
    // rank selector
    let rank_sel = Selector::parse("td:nth-child(1)").unwrap();
    // k/d selector
    let kd_sel = Selector::parse("td:nth-child(6)").unwrap();
    // matches the href="player.php?id=1234" attribute on the page.
    let href_re = Regex::new(r#"\?id=(\d+)"#).unwrap();

    let mut ids = vec![];

    for page in pages {
        let doc = Html::parse_document(&page);
        let table = doc.select(&table_row).next(); // get the first plr result match
        let overall_plrs = doc
            .select(&overall_player_sel)
            .next()
            .map(|el| {
                let mut s = el.inner_html();
                s.retain(|c| c.is_digit(10));
                s
            })
            .and_then(|n| n.parse::<u64>().ok());
        let plr_el = table.and_then(|t| t.select(&player_sel).next());
        let plr_id = plr_el
            .and_then(|el| el.attr("href"))
            .and_then(|href| href_re.captures(href))
            .and_then(|caps| caps.get(1))
            .and_then(|id| id.as_str().parse::<u64>().ok());
        let plr_name = plr_el.map(|el| el.inner_html());
        let plr_rank = table
            .and_then(|t| t.select(&rank_sel).next())
            .map(|el| el.inner_html())
            .and_then(|rank| rank.parse::<u64>().ok());
        let plr_kd = table
            .and_then(|t| t.select(&kd_sel).next())
            .map(|el| el.inner_html())
            .and_then(|kd| kd.parse::<f32>().ok());
        let plr = (|| {
            let rank = plr_rank?;
            let total = overall_plrs?;
            Some(PsychoStats {
                name: plr_name?,
                id: plr_id?,
                percentile: (rank as f32 / total as f32) * 100.,
                rank,
                kd: plr_kd?,
            })
        })();
        ids.push(plr);
    }

    let [id4, id5] = &ids[..2] else {
        Err("Could not find both player ids.")?
    };

    Ok((id4.clone(), id5.clone()))
}
