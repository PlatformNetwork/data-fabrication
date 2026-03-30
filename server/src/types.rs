//! Server-specific types for the data-fabrication challenge.

use serde::{Deserialize, Serialize};

/// Submission for the data-fabrication challenge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    /// The miner's hotkey identifier
    pub hotkey: String,
    /// The epoch this submission was made for
    pub epoch: u64,
    /// SHA-256 hash of the Python code for integrity verification
    pub code_hash: String,
    /// The Python harness code as bytes
    pub package: Vec<u8>,
    /// Optional submission name for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_name: Option<String>,
    /// Optional challenge-specific parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub challenge_params: Option<ChallengeParams>,
}

/// Challenge-specific parameters for evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChallengeParams {
    /// Whether to enable LLM-based quality review
    pub llm_review_enabled: Option<bool>,
    /// Whether to use LLM as a judge for scoring
    pub llm_judge_enabled: Option<bool>,
    /// Override LLM API URL
    pub llm_api_url: Option<String>,
    /// Override LLM API key
    pub llm_api_key: Option<String>,
    /// Override LLM model name
    pub llm_model: Option<String>,
    /// Minimum conversation count required
    pub min_conversations: Option<u64>,
    /// Maximum conversation count allowed
    pub max_conversations: Option<u64>,
    /// Timeout for harness execution in seconds
    pub execution_timeout_secs: Option<u64>,
}

/// Result of evaluating a submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// Number of conversations generated
    pub conversation_count: u64,
    /// Total number of messages across all conversations
    pub total_messages: u64,
    /// Size of the generated dataset in bytes
    pub dataset_size_bytes: u64,
    /// Quality score (0.0 - 1.0)
    pub quality_score: f64,
    /// Consensus score
    pub consensus_score: f64,
    /// Final combined score
    pub final_score: f64,
    /// Whether AST validation passed
    pub ast_passed: bool,
    /// Any security violations found
    pub violations: Vec<String>,
}

/// Log entry for a submission evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationLog {
    /// Hotkey of the submitter
    pub hotkey: String,
    /// Epoch of submission
    pub epoch: u64,
    /// Result of the evaluation
    pub result: Option<EvaluationResult>,
    /// Any errors that occurred
    pub errors: Vec<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Leaderboard entry for the data-fabrication challenge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    /// Rank in the leaderboard (1-indexed)
    pub rank: u32,
    /// Hotkey of the miner
    pub hotkey: String,
    /// Score
    pub score: f64,
    /// Epoch of best submission
    pub epoch: u64,
    /// Number of conversations generated
    pub conversation_count: u64,
    /// Optional submission name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_name: Option<String>,
}

/// Status of an ongoing evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationStatus {
    /// Hotkey being evaluated
    pub hotkey: String,
    /// Epoch of the evaluation
    pub epoch: u64,
    /// Current phase of evaluation
    pub phase: EvaluationPhase,
    /// Steps completed and their status
    pub steps: Vec<EvaluationStep>,
}

/// Phase of evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationPhase {
    /// Waiting to start
    Pending,
    /// Validating AST
    AstValidation,
    /// Executing harness
    Execution,
    /// Scoring results
    Scoring,
    /// Storing results
    Storing,
    /// Evaluation complete
    Complete,
    /// Evaluation failed
    Failed,
}

/// A step in the evaluation process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationStep {
    /// Name of the step
    pub name: String,
    /// Status of the step
    pub status: StepStatus,
    /// Optional detail message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Status of an evaluation step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Waiting to run
    Pending,
    /// Currently running
    Running,
    /// Completed successfully
    Complete,
    /// Skipped
    Skipped,
    /// Failed
    Failed,
}

// Constants
/// Maximum size of submitted package (1MB)
pub const MAX_PACKAGE_SIZE: usize = 1_048_576;
/// Maximum dataset size allowed (100MB)
pub const MAX_DATASET_SIZE: usize = 100 * 1_024 * 1_024;
/// Default timeout for harness execution (60 seconds)
pub const DEFAULT_EXECUTION_TIMEOUT_SECS: u64 = 60;
