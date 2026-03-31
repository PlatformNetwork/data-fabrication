pub mod status;
pub mod submit;
pub mod ui;

pub use status::status_command;
pub use submit::{parse_keypair, submit_command};
pub use ui::App;

#[derive(Debug, Clone, PartialEq)]
pub struct LeaderboardEntry {
    pub rank: u32,
    pub hotkey: String,
    pub score: f64,
    pub epoch: u64,
}
