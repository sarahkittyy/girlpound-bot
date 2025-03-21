use std::{collections::HashMap, hash::Hash};

use emoji;

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

impl TF2Class {
    /// returns all classes, in order
    pub fn all() -> [Self; 9] {
        use TF2Class::*;
        [
            Scout, Soldier, Pyro, Demo, Heavy, Engineer, Medic, Sniper, Spy,
        ]
    }

    pub fn emoji(&self) -> &'static str {
        emoji::emoji(&self.to_string())
    }

    pub fn emojis() -> HashMap<TF2Class, String> {
        return [
            (TF2Class::Scout, emoji::emoji("scout").to_string()),
            (TF2Class::Soldier, emoji::emoji("soldier").to_string()),
            (TF2Class::Pyro, emoji::emoji("pyro").to_string()),
            (TF2Class::Demo, emoji::emoji("demoman").to_string()),
            (TF2Class::Heavy, emoji::emoji("heavy").to_string()),
            (TF2Class::Engineer, emoji::emoji("engineer").to_string()),
            (TF2Class::Medic, emoji::emoji("medic").to_string()),
            (TF2Class::Sniper, emoji::emoji("sniper").to_string()),
            (TF2Class::Spy, emoji::emoji("spy").to_string()),
        ]
        .into();
    }

    pub fn to_string(&self) -> String {
        use TF2Class::*;
        match self {
            Scout => "scout".to_string(),
            Soldier => "soldier".to_string(),
            Pyro => "pyro".to_string(),
            Demo => "demoman".to_string(),
            Heavy => "heavy".to_string(),
            Engineer => "engineer".to_string(),
            Medic => "medic".to_string(),
            Sniper => "sniper".to_string(),
            Spy => "spy".to_string(),
        }
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
