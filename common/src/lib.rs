pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub mod discord;
pub mod util;
