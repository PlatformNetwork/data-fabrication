pub mod ui;

pub use ui::App;

#[derive(Debug, Clone, PartialEq)]
pub struct LeaderboardEntry {
    pub rank: u32,
    pub hotkey: String,
    pub score: f64,
    pub epoch: u64,
}
