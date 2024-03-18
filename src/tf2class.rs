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

    pub fn from_number(n: u8) -> Self {
        if n > 8 {
            panic!("N should be between 0 and 8 inclusive");
        }
        unsafe { *TF2Class::all().get(n as usize).unwrap_unchecked() }
    }
}
