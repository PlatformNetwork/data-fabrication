//! WASM-specific types for the data-fabrication challenge.
//!
//! These types are used for serialization between the host and WASM module.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Submission data sent from the miner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    /// The miner's hotkey identifier
    pub hotkey: String,
    /// The epoch this submission was made for
    pub epoch: u64,
    /// SHA-256 hash of the harness code
    pub code_hash: String,
    /// The Python harness code package
    pub package: Vec<u8>,
    /// Signature from the miner
    pub signature: String,
}

/// Challenge parameters for evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeParams {
    /// Minimum number of conversations required
    pub min_conversations: u32,
    /// Maximum number of conversations allowed
    pub max_conversations: u32,
    /// Maximum dataset size in bytes
    pub max_size_bytes: u64,
    /// Model to use for evaluation (optional)
    pub model: Option<String>,
}

impl Default for ChallengeParams {
    fn default() -> Self {
        use data_fabrication_core::{
            MAX_CONVERSATION_COUNT, MAX_DATASET_SIZE_BYTES, MIN_CONVERSATION_COUNT,
        };
        Self {
            min_conversations: MIN_CONVERSATION_COUNT,
            max_conversations: MAX_CONVERSATION_COUNT,
            max_size_bytes: MAX_DATASET_SIZE_BYTES,
            model: None,
        }
    }
}

/// Result of the evaluation.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// Whether the submission passed evaluation
    pub passed: bool,
    /// Score from 0.0 to 1.0
    pub score: f64,
    /// Number of valid conversations generated
    pub conversation_count: u32,
    /// Total messages in the dataset
    pub total_messages: u32,
    /// Dataset size in bytes
    pub size_bytes: u64,
    /// Error message if failed
    pub error: Option<String>,
}
