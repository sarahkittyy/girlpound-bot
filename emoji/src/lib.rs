use common::util::parse_env;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::OnceLock};

#[derive(Deserialize, Serialize)]
struct EmojiTable {
    dictionary: HashMap<String, String>,
}

impl EmojiTable {
    fn load() -> Self {
        let emoji_file: String = parse_env("EMOJI_JSON");
        let emoji_file_data = std::fs::read_to_string(&emoji_file)
            .expect(&format!("could not read {} to emojis", &emoji_file));

        let table = EmojiTable {
            dictionary: serde_json::from_str(&emoji_file_data).unwrap_or_default(),
        };
        log::info!("loaded {} emojis", table.dictionary.len());
        table
    }
}

static EMOJI_TABLE: OnceLock<EmojiTable> = OnceLock::new();

pub fn emoji(key: &str) -> &'static str {
    EMOJI_TABLE
        .get_or_init(|| EmojiTable::load())
        .dictionary
        .get(key)
        .expect(&format!("EMOJI MISSING FOR KEY {key}!"))
        .as_str()
}

