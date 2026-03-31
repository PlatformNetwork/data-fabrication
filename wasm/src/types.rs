//! WASM-specific types for the data-fabrication challenge.
//!
//! These types are used for serialization between the host and WASM module.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// State of an agent's submission in the evaluation pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    /// Submission received, waiting to be evaluated.
    Pending,
    /// Currently being evaluated.
    Evaluating,
    /// Evaluation complete, score assigned.
    Scored,
    /// Evaluation failed with an error.
    Failed,
}

/// Entry on the challenge leaderboard.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    /// The miner's hotkey identifier.
    pub hotkey: String,
    /// The final score (0.0 to 1.0).
    pub score: f64,
    /// Rank on the leaderboard (1-based).
    pub rank: u32,
    /// Unix timestamp of when the score was recorded.
    pub timestamp: u64,
}

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

/// Quality metrics for a dataset used in scoring calculation.
/// Each metric is a value in the range [0.0, 1.0].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetQualityMetrics {
    /// Format validity score (schema compliance, structure correctness)
    pub format_score: f64,
    /// Content quality score (semantic coherence, relevance)
    pub quality_score: f64,
    /// Originality score (novelty, diversity of content)
    pub originality_score: f64,
}

/// Status tracking for a submission.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmissionStatus {
    /// Unique submission identifier.
    pub id: String,
    /// Current state of the submission.
    pub state: AgentState,
    /// Final score (if evaluated).
    pub score: Option<f64>,
    /// Unix timestamp when submission was created.
    pub created_at: u64,
    /// Unix timestamp of last status update.
    pub updated_at: u64,
}

/// Upload control state for sudo owner.
/// Controls whether uploads are accepted and processed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UploadState {
    /// Reject all uploads (403 Forbidden)
    Disabled,
    /// Accept uploads but queue without processing
    Pending,
    /// Full processing pipeline (default)
    Enabled,
}

impl Default for UploadState {
    fn default() -> Self { Self::Enabled }
}
