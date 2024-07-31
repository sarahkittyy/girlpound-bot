use rand::prelude::*;
use std::{
    collections::HashMap,
    hash::Hash,
    net::{Ipv4Addr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use common::{util::parse_env, Error};

use axum::{
    self,
    extract::{RawQuery, State},
    http::StatusCode,
    response::{self, IntoResponse, Response},
    routing::get,
    Router,
};
use chrono::{DateTime, TimeDelta, Utc};
use steam_connect as steam;
use tokio::{self, net::TcpListener, sync::RwLock};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LinkCode(pub String);

#[derive(Clone)]
pub struct ApiState {
    public_url: String,
    pub link_codes: Arc<RwLock<HashMap<LinkCode, (u64, DateTime<Utc>)>>>,
}

impl ApiState {
    pub async fn gen_link_code(&self, steamid64: u64) -> (LinkCode, DateTime<Utc>) {
        const CHARS: &'static str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ123456789";
        let existing_codes = self
            .link_codes
            .read()
            .await
            .keys()
            .map(|m| m.0.clone())
            .collect::<Vec<String>>();

        let new_code = loop {
            let code: String = (0..6)
                .flat_map(|_| CHARS.chars().nth(thread_rng().gen_range(0..CHARS.len())))
                .collect();
            if !existing_codes.contains(&code) {
                break code;
            }
        };
        let issued_at = Utc::now();
        let code = LinkCode(new_code);

        let mut v = self.link_codes.write().await;
        v.insert(code.clone(), (steamid64, issued_at));
        (code, issued_at)
    }

    pub fn link_url(&self) -> String {
        format!("{}/steam-link", self.public_url)
    }

    pub async fn try_link_user(&self, code: String) -> Result<u64, Error> {
        // check if code is valid
        let mut codes = self.link_codes.write().await;
        let code = LinkCode(code);
        let v = codes.get(&code);

        match v {
            None => {
                return Err("Code not found".into());
            }
            Some((steamid64, issued_at)) => {
                let now = Utc::now();
                if issued_at
                    .checked_add_signed(TimeDelta::try_minutes(10).unwrap())
                    .is_some_and(|expires| expires < now)
                {
                    codes.remove(&code);
                    Err("Code has expired".into())
                } else {
                    Ok(*steamid64)
                }
            }
        }
    }
}

pub async fn init() -> Result<ApiState, Error> {
    let ip: Ipv4Addr = parse_env("HTTP_IP");
    let public_url: String = parse_env("HTTP_PUBLIC_URL");
    let port: u16 = parse_env("HTTP_PORT");

    let state = ApiState {
        public_url,
        link_codes: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/steam-link", get(steam_link))
        .route("/steam-callback", get(steam_callback))
        .with_state(state.clone());

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            let socket_addr = (ip, port)
                .to_socket_addrs()
                .expect("No addrs.")
                .nth(0)
                .expect("No addrs.");
            let Ok(listener) = TcpListener::bind(socket_addr).await else {
                eprintln!("Could not bind to port.");
                continue;
            };
            println!("HTTP listener bound to {}", socket_addr);
            let _ = axum::serve(listener, app.clone())
                .await
                .inspect_err(|e| eprintln!("axum server error: {e}"));
        }
    });

    Ok(state)
}

async fn steam_link(State(state): State<ApiState>) -> response::Redirect {
    let r = steam::Redirect::new(&format!("{}/steam-callback", state.public_url)).unwrap();

    response::Redirect::to(r.url().as_str())
}

fn response_doc(text: impl AsRef<str>) -> response::Html<String> {
    response::Html(format!(
        r#"
<!DOCTYPE html>
<html>
<head>
	<title>fluffycat.gay api</title>
	<style>
		.center {{
			position: absolute;
			left: 50%;
			top: 30%;
			transform: translateX(-50%) translateY(-50%);
		}}
	</style>
</head>
<body>
	<h1 class="center">{}</h1>
</body>
</html>
	"#,
        text.as_ref()
    ))
}

#[axum::debug_handler(state = ApiState)]
async fn steam_callback(State(state): State<ApiState>, RawQuery(query): RawQuery) -> Response {
    let Some(query) = query else {
        return (
            StatusCode::BAD_REQUEST,
            response_doc(format!(
                "BAD REQUEST. You're looking for <a href=\"{}/steam-link\">/steam-link</a>",
                state.public_url
            )),
        )
            .into_response();
    };
    let Ok(v) = steam::Verify::verify_request(&query).await else {
        return response::Redirect::to("/steam-link").into_response();
    };
    let id = v.claim_id();
    let (code, issued_at) = state.gen_link_code(id).await;
    (
        StatusCode::OK,
        response_doc(format!(
            "Your steam link code is <tt>{}</tt>. It will be valid for 10 minutes ({}).",
            code.0,
            (issued_at + chrono::Duration::try_minutes(10).unwrap()).format("%d/%m/%Y %H:%M")
        )),
    )
        .into_response()
}
