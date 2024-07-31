use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use sqlx::{MySql, Pool, QueryBuilder};

use common::Error;
use tf2::GameState;

/// <= this amnt you are considered a seeder.
const SEEDER_POP_THRESHOLD: usize = 12;

pub struct Tracker {
    seeding: bool,
    players_online: HashMap<String, u64>, // map of steam ids <-> join times
    pool: Pool<MySql>,
    flush_cache: HashMap<String, u64>, // map of steam ids <-> seconds online, for flushing to db
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

impl Tracker {
    /// initialize the tracker with the current game's state
    pub fn new(state: GameState, pool: Pool<MySql>) -> Self {
        println!(
            "Init'd seeder tracker with {} players online",
            state.players.len()
        );
        let mut players_online = HashMap::new();

        state.players.into_iter().for_each(|p| {
            players_online.insert(p.id.clone(), now());
        });
        Self {
            seeding: players_online.len() <= SEEDER_POP_THRESHOLD,
            players_online,
            pool,
            flush_cache: HashMap::new(),
        }
    }

    /// synchronize with the current state of the game (incase of crash)
    pub fn synchronize(&mut self, state: GameState) {
        for player in &state.players {
            // add unaccounted for players
            if !self.players_online.contains_key(&player.id) {
                self.players_online.insert(player.id.clone(), now());
            }
        }
        // remove unaccounted for leavers
        let unmatched_steamids = self
            .players_online
            .keys()
            .filter(|&steamid| state.players.iter().find(|p| &p.id == steamid).is_none())
            .cloned()
            .collect::<Vec<String>>();
        for steamid in unmatched_steamids {
            self.players_online.remove(&steamid);
        }
        // are we seeding?
        self.seeding = self.players_online.len() <= SEEDER_POP_THRESHOLD;
    }

    pub async fn on_join(&mut self, steamid: String) {
        self.players_online.insert(steamid, now());

        // if we have exceeded the seeder population
        if self.seeding && self.players_online.len() > SEEDER_POP_THRESHOLD {
            // flush everyone currently online to db
            self.push_all_seed_times();
            // set as not seeding
            self.seeding = false;
        }
    }

    pub async fn on_leave(&mut self, steamid: String) {
        let Some(joined_at) = self.players_online.remove(&steamid) else {
            return;
        };

        let now = now();

        // if we were seeding...
        if self.seeding {
            let time_spent = now - joined_at;
            // count user's seed time
            self.push_user_seed_time(steamid, time_spent);
        } else if self.players_online.len() <= SEEDER_POP_THRESHOLD {
            // if we weren't seeding, and we are now
            // update all users new seed start times
            self.players_online
                .iter_mut()
                .for_each(|(_, joined_at)| *joined_at = now);
            // set seeding to true
            self.seeding = true;
        }
    }

    fn push_all_seed_times(&mut self) {
        let now = now();
        for (steamid, joined_at) in self.players_online.iter() {
            let seconds_seeded = now - joined_at;
            match self.flush_cache.get_mut(steamid) {
                Some(seconds) => {
                    *seconds += seconds_seeded;
                }
                None => {
                    self.flush_cache.insert(steamid.clone(), seconds_seeded);
                }
            };
        }
    }

    fn push_user_seed_time(&mut self, steamid: String, seconds_seeded: u64) {
        match self.flush_cache.get_mut(&steamid) {
            Some(seconds) => {
                *seconds += seconds_seeded;
            }
            None => {
                self.flush_cache.insert(steamid, seconds_seeded);
            }
        };
    }

    /// flush all current users seed times to the db
    pub async fn flush_cache_to_db(&mut self) -> Result<(), Error> {
        if self.flush_cache.len() == 0 {
            return Ok(());
        }
        println!("Flushing {} seeders to db.", self.flush_cache.len());
        let players = self.flush_cache.iter();
        let mut qb =
            QueryBuilder::new(r#"INSERT INTO `seederboard` (`steamid`, `seconds_seeded`)"#);
        qb.push_values(players, |mut b, (steamid, seconds_seeded)| {
            b //
                .push_bind(steamid)
                .push_bind(seconds_seeded);
        });
        qb.push("ON DUPLICATE KEY UPDATE `seconds_seeded` = `seconds_seeded` + VALUES(`seconds_seeded`)");
        let q = qb.build();
        q.execute(&self.pool).await?;
        self.flush_cache.clear();
        Ok(())
    }
}
