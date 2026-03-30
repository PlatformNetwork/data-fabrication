//! Integration tests for the data-cli TUI application.
//!
//! Tests the data structures and logic without requiring terminal rendering.

use data_cli::ui::App;
use data_cli::LeaderboardEntry;

/// Test that App::default() creates a valid structure with initial state.
#[test]
fn test_app_creation() {
    let app = App::default();

    // App should start with empty leaderboard
    assert!(
        app.leaderboard.is_empty(),
        "Leaderboard should be empty on creation"
    );

    // Initial scroll offset should be 0
    assert_eq!(app.scroll_offset, 0, "Scroll offset should start at 0");

    // Should not be in quit state
    assert!(!app.should_quit, "should_quit should be false initially");

    // Submission and miner counts should be 0 initially
    assert_eq!(
        app.submission_count, 0,
        "Submission count should be 0 initially"
    );
    assert_eq!(app.miner_count, 0, "Miner count should be 0 initially");
}

/// Test that App::new() is equivalent to App::default().
#[test]
fn test_app_new_is_default() {
    let app_new = App::new();
    let app_default = App::default();

    assert_eq!(app_new.leaderboard, app_default.leaderboard);
    assert_eq!(app_new.scroll_offset, app_default.scroll_offset);
    assert_eq!(app_new.should_quit, app_default.should_quit);
    assert_eq!(app_new.submission_count, app_default.submission_count);
    assert_eq!(app_new.miner_count, app_default.miner_count);
}

/// Test that LeaderboardEntry creates with correct fields.
#[test]
fn test_leaderboard_entry() {
    let entry = LeaderboardEntry {
        rank: 1,
        hotkey: "5GrwvaEF5zXb26Fz9rcQpDWS7hZkXhCuJDBu3qM9YxnU".to_string(),
        score: 0.9876,
        epoch: 127,
    };

    assert_eq!(entry.rank, 1, "Rank should match");
    assert_eq!(
        entry.hotkey, "5GrwvaEF5zXb26Fz9rcQpDWS7hZkXhCuJDBu3qM9YxnU",
        "Hotkey should match"
    );
    assert!(
        (entry.score - 0.9876).abs() < f64::EPSILON,
        "Score should match"
    );
    assert_eq!(entry.epoch, 127, "Epoch should match");
}

/// Test that LeaderboardEntry can be cloned.
#[test]
fn test_leaderboard_entry_clone() {
    let entry = LeaderboardEntry {
        rank: 5,
        hotkey: "test_hotkey".to_string(),
        score: 0.5,
        epoch: 100,
    };

    let cloned = entry.clone();
    assert_eq!(entry.rank, cloned.rank);
    assert_eq!(entry.hotkey, cloned.hotkey);
    assert_eq!(entry.score, cloned.score);
    assert_eq!(entry.epoch, cloned.epoch);
}

/// Test that entries sort by score correctly (descending order).
#[test]
fn test_leaderboard_sorting() {
    let mut entries = vec![
        LeaderboardEntry {
            rank: 3,
            hotkey: "low".to_string(),
            score: 0.5,
            epoch: 1,
        },
        LeaderboardEntry {
            rank: 1,
            hotkey: "high".to_string(),
            score: 0.95,
            epoch: 1,
        },
        LeaderboardEntry {
            rank: 2,
            hotkey: "medium".to_string(),
            score: 0.75,
            epoch: 1,
        },
    ];

    // Sort by score descending
    entries.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    assert_eq!(entries[0].hotkey, "high", "Highest score should be first");
    assert_eq!(entries[1].hotkey, "medium", "Medium score should be second");
    assert_eq!(entries[2].hotkey, "low", "Lowest score should be last");
}

/// Test that App::refresh() generates at least 10 mock leaderboard entries.
#[test]
fn test_mock_leaderboard_data() {
    let mut app = App::default();
    app.refresh();

    // Should have at least 10 entries after refresh
    assert!(
        app.leaderboard.len() >= 10,
        "Leaderboard should have at least 10 entries after refresh"
    );

    // Verify entries are valid
    for entry in &app.leaderboard {
        assert!(entry.rank > 0, "Rank should be positive");
        assert!(!entry.hotkey.is_empty(), "Hotkey should not be empty");
        assert!(
            entry.score >= 0.0 && entry.score <= 1.0,
            "Score should be between 0 and 1"
        );
        assert!(entry.epoch > 0, "Epoch should be positive");
    }
}

/// Test that mock leaderboard entries have sorted ranks.
#[test]
fn test_mock_leaderboard_rank_ordering() {
    let mut app = App::default();
    app.refresh();

    // Verify ranks are in ascending order
    for i in 1..app.leaderboard.len() {
        assert!(
            app.leaderboard[i].rank > app.leaderboard[i - 1].rank,
            "Ranks should be in ascending order"
        );
    }
}

/// Test that mock leaderboard entries have descending scores.
#[test]
fn test_mock_leaderboard_score_ordering() {
    let mut app = App::default();
    app.refresh();

    // Verify scores are in descending order
    for i in 1..app.leaderboard.len() {
        assert!(
            app.leaderboard[i - 1].score >= app.leaderboard[i].score,
            "Scores should be in descending order"
        );
    }
}

/// Test that ui module is accessible.
#[test]
fn test_ui_module_exists() {
    let _app: App = App::new();
    // If this compiles, the ui module exists and App is accessible
}

/// Test scroll up functionality.
#[test]
fn test_scroll_up() {
    let mut app = App::default();
    app.scroll_offset = 5;
    app.scroll_up();

    assert_eq!(app.scroll_offset, 4, "Scroll up should decrement offset");

    // Test that scroll_offset is saturating (doesn't underflow)
    app.scroll_offset = 0;
    app.scroll_up();
    assert_eq!(
        app.scroll_offset, 0,
        "Scroll offset should stay at 0 (saturating)"
    );
}

/// Test scroll down functionality.
#[test]
fn test_scroll_down() {
    let mut app = App::default();
    app.scroll_offset = 0;
    app.scroll_down();

    assert_eq!(app.scroll_offset, 1, "Scroll down should increment offset");
}

/// Test refresh updates submission and miner counts.
#[test]
fn test_refresh_updates_counts() {
    let mut app = App::default();
    app.refresh();

    assert!(
        app.submission_count > 0,
        "Submission count should be set after refresh"
    );
    assert!(
        app.miner_count > 0,
        "Miner count should be set after refresh"
    );
    assert_eq!(
        app.miner_count as usize,
        app.leaderboard.len(),
        "Miner count should equal leaderboard length"
    );
}

/// Test that LeaderboardEntry Debug trait works.
#[test]
fn test_leaderboard_entry_debug() {
    let entry = LeaderboardEntry {
        rank: 1,
        hotkey: "test".to_string(),
        score: 0.5,
        epoch: 100,
    };

    let debug_str = format!("{:?}", entry);
    assert!(
        debug_str.contains("rank"),
        "Debug output should contain rank"
    );
    assert!(
        debug_str.contains("hotkey"),
        "Debug output should contain hotkey"
    );
    assert!(
        debug_str.contains("score"),
        "Debug output should contain score"
    );
    assert!(
        debug_str.contains("epoch"),
        "Debug output should contain epoch"
    );
}
