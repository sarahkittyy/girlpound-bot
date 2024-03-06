use crate::Error;
use futures::{future::try_join_all, TryFutureExt};
use regex::Regex;
use reqwest;
use scraper::{Html, Selector};

pub const BASEURL4: &'static str = "http://stats4.fluffycat.gay/";
pub const BASEURL5: &'static str = "http://stats5.fluffycat.gay/";

/// returns the unique player ids for stats4 and stats5.
pub async fn find_plr_ids(steamid: &str) -> Result<(Option<u64>, Option<u64>), Error> {
    // query stats4 and stats5
    let urls = [BASEURL4, BASEURL5]
        .into_iter()
        .map(|base| format!("{base}index.php?q={}", steamid))
        .map(|query| reqwest::get(query).and_then(|resp| resp.text()));
    let pages = try_join_all(urls).await?;

    // player id selector
    let selector = Selector::parse("a.plr").unwrap();
    // matches the href="player.php?id=1234" attribute on the page.
    let href_re = Regex::new(r#"\?id=(\d+)"#).unwrap();

    let mut ids = vec![];

    for page in pages {
        let doc = Html::parse_document(&page);
        let plr_id = doc
            .select(&selector)
            .next() // get the first plr result match
            .and_then(|el| el.attr("href"))
            .and_then(|href| href_re.captures(href))
            .and_then(|caps| caps.get(1))
            .and_then(|id| id.as_str().parse::<u64>().ok());
        ids.push(plr_id);
    }

    let [id4, id5] = &ids[..2] else {
        Err("Could not find both player ids.")?
    };

    Ok((*id4, *id5))
}
