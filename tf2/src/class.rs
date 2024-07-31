use std::{collections::HashMap, hash::Hash, sync::OnceLock};

use common::util::parse_env;

/// All tf2 classes
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum TF2Class {
    Scout,
    Soldier,
    Pyro,
    Demo,
    Heavy,
    Engineer,
    Medic,
    Sniper,
    Spy,
}

static EMOJIS: OnceLock<HashMap<TF2Class, String>> = OnceLock::new();
fn get_emojis() -> HashMap<TF2Class, String> {
    let class_emojis: Vec<String> = parse_env::<String>("CLASS_EMOJIS") //
        .split(",")
        .map(str::to_owned)
        .collect();
    let class_emojis: HashMap<TF2Class, String> = TF2Class::all()
        .to_vec()
        .into_iter()
        .zip(class_emojis.into_iter())
        .collect();

    class_emojis
}

impl TF2Class {
    /// returns all classes, in order
    pub fn all() -> [Self; 9] {
        use TF2Class::*;
        [
            Scout, Soldier, Pyro, Demo, Heavy, Engineer, Medic, Sniper, Spy,
        ]
    }

    pub fn emoji(&self) -> &'static str {
        EMOJIS
            .get_or_init(get_emojis)
            .get(&self)
            .expect("Missing TF2Class emoji.")
    }

    pub fn emojis() -> &'static HashMap<TF2Class, String> {
        EMOJIS.get_or_init(get_emojis)
    }

    pub fn as_number(&self) -> u8 {
        use TF2Class::*;
        match self {
            Scout => 0,
            Soldier => 1,
            Pyro => 2,
            Demo => 3,
            Heavy => 4,
            Engineer => 5,
            Medic => 6,
            Sniper => 7,
            Spy => 8,
        }
    }

    pub fn from_number(n: u8) -> Self {
        if n > 8 {
            panic!("N should be between 0 and 8 inclusive");
        }
        unsafe { *TF2Class::all().get(n as usize).unwrap_unchecked() }
    }
}
