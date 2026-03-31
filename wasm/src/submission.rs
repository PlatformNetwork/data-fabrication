//! Submission processing logic for the data-fabrication challenge.
//!
//! Provides functions to process, validate, and track submission state.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::storage::{get_bincode, get_string, set_bincode, set_string};
use crate::types::{AgentState, Submission};

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

/// Submission state storage namespace.
mod submission_state {
    use super::*;

    /// Key prefix for submission states.
    const PREFIX: &str = "sub_state";

    /// Get state key for a submission ID.
    fn state_key(id: &str) -> Vec<u8> {
        crate::storage::key2(PREFIX, id)
    }

    /// Get submission ID key for a hotkey+epoch combo.
    fn id_key(hotkey: &str, epoch: u64) -> Vec<u8> {
        let mut epoch_str = String::new();
        let _ = core::fmt::write(&mut epoch_str, format_args!("{}", epoch));
        key3("sub_id", hotkey, &epoch_str)
    }

    /// Store submission state.
    pub fn set_state(id: &str, state: AgentState) -> bool {
        set_bincode(&state_key(id), &state)
    }

    /// Get submission state.
    pub fn get_state(id: &str) -> Option<AgentState> {
        get_bincode(&state_key(id))
    }

    /// Store submission ID for hotkey+epoch.
    pub fn set_submission_id(hotkey: &str, epoch: u64, id: &str) -> bool {
        set_string(&id_key(hotkey, epoch), id)
    }

    /// Get submission ID for hotkey+epoch.
    pub fn get_submission_id(hotkey: &str, epoch: u64) -> Option<String> {
        get_string(&id_key(hotkey, epoch))
    }
}

/// Process a submission and return its unique ID.
///
/// Validates the submission structure, stores it, and returns a unique
/// identifier for tracking purposes.
///
/// # Arguments
/// * `submission` - The submission to process
///
/// # Returns
/// * `Ok(String)` - The unique submission ID on success
/// * `Err(&'static str)` - Error message if processing fails
pub fn process_submission(submission: &Submission) -> Result<String, &'static str> {
    // Validate first
    if !validate_submission(submission) {
        return Err("invalid submission structure");
    }

    // Generate a unique submission ID: hotkey:epoch
    let id = generate_submission_id(&submission.hotkey, submission.epoch);

    // Check if already submitted for this epoch
    if submission_state::get_submission_id(&submission.hotkey, submission.epoch).is_some() {
        return Err("duplicate submission for epoch");
    }

    // Store the submission ID mapping
    if !submission_state::set_submission_id(&submission.hotkey, submission.epoch, &id) {
        return Err("failed to store submission ID");
    }

    // Initialize state as Pending
    if !update_submission_state(&id, AgentState::Pending) {
        return Err("failed to initialize submission state");
    }

    Ok(id)
}

/// Validate submission structure.
///
/// Checks that all required fields are present and non-empty.
///
/// # Arguments
/// * `submission` - The submission to validate
///
/// # Returns
/// * `true` - If submission is valid
/// * `false` - If submission is invalid
pub fn validate_submission(submission: &Submission) -> bool {
    // Check hotkey is non-empty
    if submission.hotkey.is_empty() {
        return false;
    }

    // Check code hash is non-empty
    if submission.code_hash.is_empty() {
        return false;
    }

    // Check package is non-empty (must contain actual code)
    if submission.package.is_empty() {
        return false;
    }

    // Check signature is non-empty
    if submission.signature.is_empty() {
        return false;
    }

    true
}

/// Get the current state of a submission.
///
/// # Arguments
/// * `id` - The submission ID to query
///
/// # Returns
/// * `Some(AgentState)` - The current state if found
/// * `None` - If submission ID not found
pub fn get_submission_state(id: &str) -> Option<AgentState> {
    if id.is_empty() {
        return None;
    }
    submission_state::get_state(id)
}

/// Update the state of a submission.
///
/// # Arguments
/// * `id` - The submission ID to update
/// * `state` - The new state to set
///
/// # Returns
/// * `true` - If update was successful
/// * `false` - If update failed
pub fn update_submission_state(id: &str, state: AgentState) -> bool {
    if id.is_empty() {
        return false;
    }
    submission_state::set_state(id, state)
}

/// Generate a unique submission ID from hotkey and epoch.
///
/// Format: "hotkey:epoch"
fn generate_submission_id(hotkey: &str, epoch: u64) -> String {
    let mut id = String::with_capacity(hotkey.len() + 12);
    id.push_str(hotkey);
    id.push(':');
    let _ = core::fmt::write(&mut id, format_args!("{}", epoch));
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_validate_submission_empty_hotkey() {
        let submission = Submission {
            hotkey: String::new(),
            epoch: 1,
            code_hash: String::from("abc123"),
            package: vec![1, 2, 3],
            signature: String::from("sig"),
        };
        assert!(!validate_submission(&submission));
    }

    #[test]
    fn test_validate_submission_empty_package() {
        let submission = Submission {
            hotkey: String::from("hotkey123"),
            epoch: 1,
            code_hash: String::from("abc123"),
            package: Vec::new(),
            signature: String::from("sig"),
        };
        assert!(!validate_submission(&submission));
    }

    #[test]
    fn test_validate_submission_valid() {
        let submission = Submission {
            hotkey: String::from("hotkey123"),
            epoch: 1,
            code_hash: String::from("abc123"),
            package: vec![1, 2, 3],
            signature: String::from("sig"),
        };
        assert!(validate_submission(&submission));
    }

    #[test]
    fn test_generate_submission_id() {
        let id = generate_submission_id("hotkey123", 42);
        assert_eq!(id, "hotkey123:42");
    }
}
