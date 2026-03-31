//! Storage helpers for data-fabrication challenge.
//!
//! Provides namespaced storage operations using host functions:
//! - `eval`: evaluation state (scores, last_epoch)
//! - `leaderboard`: rankings and score aggregation
//! - `agents`: agent code and metadata storage

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;
use platform_challenge_sdk_wasm::host_functions::{host_storage_get, host_storage_set};

// ════════════════════════════════════════════════════════════════════════════
// Key Builders
// ════════════════════════════════════════════════════════════════════════════

/// Build a two-part key: "prefix:value"
pub fn key2(prefix: &str, a: &str) -> Vec<u8> {
    let mut k = Vec::with_capacity(prefix.len() + 1 + a.len());
    k.extend_from_slice(prefix.as_bytes());
    k.push(b':');
    k.extend_from_slice(a.as_bytes());
    k
}

/// Build a three-part key: "prefix:a:b"
fn key3(prefix: &str, a: &str, b: &str) -> Vec<u8> {
    let mut k = Vec::with_capacity(prefix.len() + 2 + a.len() + b.len());
    k.extend_from_slice(prefix.as_bytes());
    k.push(b':');
    k.extend_from_slice(a.as_bytes());
    k.push(b':');
    k.extend_from_slice(b.as_bytes());
    k
}

/// Build a hotkey+epoch key: "prefix:hotkey:epoch"
fn key_hotkey_epoch(prefix: &str, hotkey: &str, epoch: u64) -> Vec<u8> {
    let mut epoch_str = String::new();
    let _ = write!(epoch_str, "{}", epoch);
    key3(prefix, hotkey, &epoch_str)
}

/// Build a global key (just the name)
pub fn global_key(name: &str) -> Vec<u8> {
    Vec::from(name.as_bytes())
}

// ════════════════════════════════════════════════════════════════════════════
// Low-level Storage Operations
// ════════════════════════════════════════════════════════════════════════════

/// Get raw bytes from storage by key.
pub fn get_raw(key: &[u8]) -> Option<Vec<u8>> {
    let data = host_storage_get(key).ok()?;
    if data.is_empty() {
        None
    } else {
        Some(data)
    }
}

/// Set raw bytes in storage by key. Returns true on success.
pub fn set_raw(key: &[u8], value: &[u8]) -> bool {
    host_storage_set(key, value).is_ok()
}

/// Get a bincode-serialized value from storage.
pub fn get_bincode<T: serde::de::DeserializeOwned>(key: &[u8]) -> Option<T> {
    let data = get_raw(key)?;
    bincode::deserialize(&data).ok()
}

/// Set a bincode-serialized value in storage. Returns true on success.
pub fn set_bincode<T: serde::Serialize>(key: &[u8], value: &T) -> bool {
    match bincode::serialize(value) {
        Ok(data) => set_raw(key, &data),
        Err(_) => false,
    }
}

/// Get a u64 value from storage (stored as little-endian bytes).
pub fn get_u64(key: &[u8]) -> Option<u64> {
    let data = get_raw(key)?;
    if data.len() < 8 {
        return None;
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[..8]);
    Some(u64::from_le_bytes(buf))
}

/// Set a u64 value in storage (stored as little-endian bytes). Returns true on success.
pub fn set_u64(key: &[u8], value: u64) -> bool {
    set_raw(key, &value.to_le_bytes())
}

/// Get an f64 value from storage (stored as little-endian bytes).
pub fn get_f64(key: &[u8]) -> Option<f64> {
    let data = get_raw(key)?;
    if data.len() < 8 {
        return None;
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[..8]);
    Some(f64::from_le_bytes(buf))
}

/// Set an f64 value in storage (stored as little-endian bytes). Returns true on success.
pub fn set_f64(key: &[u8], value: f64) -> bool {
    set_raw(key, &value.to_le_bytes())
}

/// Get a string value from storage.
pub fn get_string(key: &[u8]) -> Option<String> {
    let data = get_raw(key)?;
    String::from_utf8(data).ok()
}

/// Set a string value in storage. Returns true on success.
pub fn set_string(key: &[u8], value: &str) -> bool {
    set_raw(key, value.as_bytes())
}

// ════════════════════════════════════════════════════════════════════════════
// Evaluation Namespace (scores, last_epoch)
// ════════════════════════════════════════════════════════════════════════════

/// Evaluation state storage (scores, epochs, weights).
pub mod eval {
    use super::*;

    /// Get the last submission epoch for a hotkey.
    pub fn get_last_epoch(hotkey: &str) -> Option<u64> {
        get_u64(&key2("last_submission", hotkey))
    }

    /// Set the last submission epoch for a hotkey.
    pub fn set_last_epoch(hotkey: &str, epoch: u64) -> bool {
        set_u64(&key2("last_submission", hotkey), epoch)
    }

    /// Get the score for an agent by hotkey.
    pub fn get_score(hotkey: &str) -> Option<f64> {
        get_f64(&key2("eval:score", hotkey))
    }

    /// Set the score for an agent by hotkey.
    pub fn set_score(hotkey: &str, score: f64) -> bool {
        set_f64(&key2("eval:score", hotkey), score)
    }

    /// Get score by agent hash.
    pub fn get_score_by_hash(agent_hash: &str) -> Option<f64> {
        get_f64(&key2("score_by_hash", agent_hash))
    }

    /// Set score by agent hash and link to hotkey.
    pub fn set_score_by_hash(agent_hash: &str, hotkey: &str, score: f64) -> bool {
        if !set_f64(&key2("score_by_hash", agent_hash), score) {
            return false;
        }
        // Link agent_hash -> hotkey
        set_string(&key2("score_hash_hotkey", agent_hash), hotkey);
        // Update index
        let idx_key = global_key("score_hash_index");
        let mut index: Vec<String> = get_bincode(&idx_key).unwrap_or_default();
        if !index.iter().any(|h| h == agent_hash) {
            index.push(String::from(agent_hash));
            let _ = set_bincode(&idx_key, &index);
        }
        true
    }

    /// Delete a score by agent hash.
    pub fn delete_score(agent_hash: &str) {
        set_raw(&key2("score_by_hash", agent_hash), &[]);
        set_raw(&key2("score_hash_hotkey", agent_hash), &[]);
        let idx_key = global_key("score_hash_index");
        let mut index: Vec<String> = get_bincode(&idx_key).unwrap_or_default();
        index.retain(|h| h != agent_hash);
        let _ = set_bincode(&idx_key, &index);
    }

    /// Get all scores as (agent_hash, hotkey, score) triples.
    pub fn get_all_scores() -> Vec<(String, String, f64)> {
        let index: Vec<String> = get_bincode(&global_key("score_hash_index")).unwrap_or_default();
        let mut result = Vec::new();
        for agent_hash in &index {
            if let Some(score) = get_f64(&key2("score_by_hash", agent_hash)) {
                let hotkey = get_string(&key2("score_hash_hotkey", agent_hash)).unwrap_or_default();
                if !hotkey.is_empty() {
                    result.push((agent_hash.clone(), hotkey, score));
                }
            }
        }
        result
    }

    /// Store WTA weight for a hotkey.
    pub fn set_wta_weight(hotkey: &str, weight: f64) -> bool {
        set_f64(&key2("wta_weight", hotkey), weight)
    }

    /// Get WTA weight for a hotkey.
    pub fn get_wta_weight(hotkey: &str) -> Option<f64> {
        get_f64(&key2("wta_weight", hotkey))
    }

    /// Record submission mapping (hotkey, epoch -> agent_hash).
    pub fn store_record(hotkey: &str, epoch: u64, agent_hash: &str) -> bool {
        set_string(&key_hotkey_epoch("submission", hotkey, epoch), agent_hash)
    }

    /// Get submission mapping (hotkey, epoch -> agent_hash).
    pub fn get_record(hotkey: &str, epoch: u64) -> Option<String> {
        get_string(&key_hotkey_epoch("submission", hotkey, epoch))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Leaderboard Namespace (rankings, aggregation)
// ════════════════════════════════════════════════════════════════════════════

/// Leaderboard entry for ranking display.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LeaderboardEntry {
    /// Rank position (1-based)
    pub rank: u32,
    /// Agent hash identifier
    pub agent_hash: String,
    /// Miner hotkey
    pub hotkey: String,
    /// Score (0.0 to 1.0)
    pub score: f64,
    /// Epoch of submission
    pub epoch: u64,
}

/// Leaderboard storage operations.
pub mod leaderboard {
    use super::*;

    /// Get the current leaderboard entries.
    pub fn get_leaderboard() -> Vec<LeaderboardEntry> {
        get_bincode(&global_key("leaderboard")).unwrap_or_default()
    }

    /// Set the leaderboard entries.
    pub fn set_leaderboard(entries: &[LeaderboardEntry]) -> bool {
        set_bincode(&global_key("leaderboard"), &entries.to_vec())
    }

    /// Get all stored leaderboard entries (unsorted).
    pub fn get_all_entries() -> Vec<LeaderboardEntry> {
        get_leaderboard()
    }

    /// Rebuild the leaderboard from all scores.
    /// Sorts by score descending and assigns ranks.
    pub fn rebuild() {
        let all_scores = eval::get_all_scores();
        let mut entries: Vec<LeaderboardEntry> = Vec::new();

        for (agent_hash, hotkey, score) in all_scores {
            let epoch = eval::get_last_epoch(&hotkey).unwrap_or(0);
            entries.push(LeaderboardEntry {
                rank: 0,
                agent_hash,
                hotkey,
                score,
                epoch,
            });
        }

        // Sort by score descending
        entries.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(core::cmp::Ordering::Equal)
        });

        // Assign ranks
        for (i, entry) in entries.iter_mut().enumerate() {
            entry.rank = (i + 1) as u32;
        }

        let _ = set_leaderboard(&entries);
    }

    /// Get the top N entries from leaderboard.
    pub fn get_top_n(n: usize) -> Vec<LeaderboardEntry> {
        let mut entries = get_leaderboard();
        entries.truncate(n);
        entries
    }

    /// Find entry by hotkey.
    pub fn find_by_hotkey(hotkey: &str) -> Option<LeaderboardEntry> {
        get_leaderboard().into_iter().find(|e| e.hotkey == hotkey)
    }

    /// Get the total number of entries in leaderboard.
    pub fn count() -> usize {
        get_leaderboard().len()
    }

    /// Clear the leaderboard.
    pub fn clear() {
        set_raw(&global_key("leaderboard"), &[]);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Agents Namespace (code storage, metadata)
// ════════════════════════════════════════════════════════════════════════════

const MAX_PACKAGE_SIZE: usize = 4 * 1024 * 1024; // 4MB max package size

/// Agent code and metadata storage.
pub mod agents {
    use super::*;
    use crate::types::Submission;

    /// Store agent package code for a hotkey at a specific epoch.
    pub fn store_code(hotkey: &str, epoch: u64, data: &[u8]) -> bool {
        if data.len() > MAX_PACKAGE_SIZE {
            return false;
        }
        set_raw(&key_hotkey_epoch("agent_code", hotkey, epoch), data)
    }

    /// Get agent package code for a hotkey at a specific epoch.
    pub fn get_code(hotkey: &str, epoch: u64) -> Option<Vec<u8>> {
        get_raw(&key_hotkey_epoch("agent_code", hotkey, epoch))
    }

    /// Store submission metadata for a hotkey at a specific epoch.
    pub fn store_submission(hotkey: &str, epoch: u64, submission: &Submission) -> bool {
        set_bincode(
            &key_hotkey_epoch("submission_data", hotkey, epoch),
            submission,
        )
    }

    /// Get submission metadata for a hotkey at a specific epoch.
    pub fn get_submission(hotkey: &str, epoch: u64) -> Option<Submission> {
        get_bincode(&key_hotkey_epoch("submission_data", hotkey, epoch))
    }

    /// Store the code hash for an agent submission.
    pub fn store_hash(hotkey: &str, epoch: u64, hash: &str) -> bool {
        set_string(&key_hotkey_epoch("agent_hash", hotkey, epoch), hash)
    }

    /// Get the code hash for an agent submission.
    pub fn get_hash(hotkey: &str, epoch: u64) -> Option<String> {
        get_string(&key_hotkey_epoch("agent_hash", hotkey, epoch))
    }

    /// Store evaluation status for an agent.
    pub fn store_status(hotkey: &str, epoch: u64, status: &str) -> bool {
        set_string(&key_hotkey_epoch("eval_status", hotkey, epoch), status)
    }

    /// Get evaluation status for an agent.
    pub fn get_status(hotkey: &str, epoch: u64) -> Option<String> {
        get_string(&key_hotkey_epoch("eval_status", hotkey, epoch))
    }

    /// Check if a hotkey has submitted in the given epoch.
    pub fn has_submitted(hotkey: &str, epoch: u64) -> bool {
        get_raw(&key_hotkey_epoch("agent_code", hotkey, epoch)).is_some()
    }

    /// Delete all data for a hotkey at a specific epoch.
    pub fn delete_submission(hotkey: &str, epoch: u64) {
        set_raw(&key_hotkey_epoch("agent_code", hotkey, epoch), &[]);
        set_raw(&key_hotkey_epoch("submission_data", hotkey, epoch), &[]);
        set_raw(&key_hotkey_epoch("agent_hash", hotkey, epoch), &[]);
        set_raw(&key_hotkey_epoch("eval_status", hotkey, epoch), &[]);
    }

    /// Store a signature for a submission.
    pub fn store_signature(hotkey: &str, epoch: u64, signature: &str) -> bool {
        set_string(&key_hotkey_epoch("signature", hotkey, epoch), signature)
    }

    /// Get a signature for a submission.
    pub fn get_signature(hotkey: &str, epoch: u64) -> Option<String> {
        get_string(&key_hotkey_epoch("signature", hotkey, epoch))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Global State Helpers
// ════════════════════════════════════════════════════════════════════════════

/// Get the active miner count (set by host).
pub fn get_active_miner_count() -> u64 {
    get_u64(&global_key("active_miner_count")).unwrap_or(0)
}

/// Get the validator count (set by host).
pub fn get_validator_count() -> u64 {
    get_u64(&global_key("validator_count")).unwrap_or(0)
}

/// Check if a hotkey is banned.
pub fn is_banned(hotkey: &str) -> bool {
    get_raw(&key2("banned", hotkey)).is_some()
}

/// Ban a hotkey.
pub fn ban_hotkey(hotkey: &str) -> bool {
    set_raw(&key2("banned", hotkey), &[1])
}

/// Unban a hotkey.
pub fn unban_hotkey(hotkey: &str) -> bool {
    set_raw(&key2("banned", hotkey), &[])
}

/// Store the global evaluation config.
pub fn set_eval_config(config: &[u8]) -> bool {
    set_raw(&global_key("eval_config"), config)
}

/// Get the global evaluation config.
pub fn get_eval_config() -> Option<Vec<u8>> {
    get_raw(&global_key("eval_config"))
}

/// Store current epoch number.
pub fn set_current_epoch(epoch: u64) -> bool {
    set_u64(&global_key("current_epoch"), epoch)
}

/// Get current epoch number.
pub fn get_current_epoch() -> Option<u64> {
    get_u64(&global_key("current_epoch"))
}
