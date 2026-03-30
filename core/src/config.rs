//! Configuration types for harness execution and LLM evaluation.
//!
//! This module provides validated configuration structures used across
//! the data-fabrication challenge system.

use alloc::string::String;
use core::fmt;
use serde::{Deserialize, Serialize};

/// Maximum allowed timeout in seconds (2 hours).
pub const MAX_TIMEOUT_SECONDS: u64 = 7200;

/// Minimum number of conversations per dataset.
pub const MIN_CONVERSATION_COUNT: u32 = 10;

/// Maximum number of conversations per dataset.
pub const MAX_CONVERSATION_COUNT: u32 = 50;

/// Maximum dataset size in bytes (100 MB).
pub const MAX_DATASET_SIZE_BYTES: u64 = 104_857_600;

/// Default memory limit in bytes (2 GB).
pub const DEFAULT_MEMORY_LIMIT_BYTES: u64 = 2_147_483_648;

/// Configuration for harness execution.
///
/// Controls the parameters for running Python harness scripts that generate
/// conversation datasets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarnessExecutionConfig {
    /// Random seed for reproducibility of dataset generation.
    pub seed: u64,

    /// Number of conversations to generate (10-50).
    pub conversation_count: u32,

    /// Maximum execution time in seconds (max 7200 = 2h).
    pub timeout_seconds: u64,

    /// Maximum allowed dataset size in bytes (max 100 MB).
    pub max_dataset_size_bytes: u64,

    /// Memory limit for harness execution in bytes (default 2 GB).
    pub memory_limit_bytes: u64,
}

impl Default for HarnessExecutionConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            conversation_count: 20,
            timeout_seconds: 3600,              // 1 hour default
            max_dataset_size_bytes: 50_000_000, // 50 MB default
            memory_limit_bytes: DEFAULT_MEMORY_LIMIT_BYTES,
        }
    }
}

impl HarnessExecutionConfig {
    /// Validates all configuration values.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if any value is out of the valid range:
    /// - `conversation_count` must be between 10 and 50
    /// - `timeout_seconds` must not exceed 7200
    /// - `max_dataset_size_bytes` must not exceed 100 MB
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.conversation_count < MIN_CONVERSATION_COUNT {
            return Err(ConfigError::ConversationCountTooLow {
                actual: self.conversation_count,
                minimum: MIN_CONVERSATION_COUNT,
            });
        }

        if self.conversation_count > MAX_CONVERSATION_COUNT {
            return Err(ConfigError::ConversationCountTooHigh {
                actual: self.conversation_count,
                maximum: MAX_CONVERSATION_COUNT,
            });
        }

        if self.timeout_seconds > MAX_TIMEOUT_SECONDS {
            return Err(ConfigError::TimeoutTooHigh {
                actual: self.timeout_seconds,
                maximum: MAX_TIMEOUT_SECONDS,
            });
        }

        if self.max_dataset_size_bytes > MAX_DATASET_SIZE_BYTES {
            return Err(ConfigError::DatasetSizeTooLarge {
                actual: self.max_dataset_size_bytes,
                maximum: MAX_DATASET_SIZE_BYTES,
            });
        }

        Ok(())
    }
}

/// Configuration for LLM evaluation.
///
/// Controls the parameters for LLM-based quality scoring of generated datasets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationConfig {
    /// LLM model identifier for evaluation.
    pub llm_model: String,

    /// LLM API endpoint URL.
    pub llm_endpoint: String,

    /// Maximum number of retry attempts for LLM calls.
    pub max_retries: u32,

    /// Delay between retries in milliseconds.
    pub retry_delay_ms: u64,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            llm_model: String::from("claude-3-sonnet"),
            llm_endpoint: String::from("https://chutes.ai/v1"),
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// Error type for configuration validation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Conversation count is below minimum threshold.
    ConversationCountTooLow { actual: u32, minimum: u32 },

    /// Conversation count exceeds maximum threshold.
    ConversationCountTooHigh { actual: u32, maximum: u32 },

    /// Timeout exceeds maximum allowed value.
    TimeoutTooHigh { actual: u64, maximum: u64 },

    /// Dataset size exceeds maximum allowed value.
    DatasetSizeTooLarge { actual: u64, maximum: u64 },
}

impl ConfigError {
    /// Returns a static error message for this error variant.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ConversationCountTooLow { .. } => "conversation count is below minimum",
            Self::ConversationCountTooHigh { .. } => "conversation count exceeds maximum",
            Self::TimeoutTooHigh { .. } => "timeout exceeds maximum allowed value",
            Self::DatasetSizeTooLarge { .. } => "dataset size exceeds maximum allowed value",
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConversationCountTooLow { actual, minimum } => {
                write!(
                    f,
                    "conversation count {} is below minimum {}",
                    actual, minimum
                )
            }
            Self::ConversationCountTooHigh { actual, maximum } => {
                write!(
                    f,
                    "conversation count {} exceeds maximum {}",
                    actual, maximum
                )
            }
            Self::TimeoutTooHigh { actual, maximum } => {
                write!(
                    f,
                    "timeout {} seconds exceeds maximum {} seconds",
                    actual, maximum
                )
            }
            Self::DatasetSizeTooLarge { actual, maximum } => {
                write!(
                    f,
                    "dataset size {} bytes exceeds maximum {} bytes",
                    actual, maximum
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = HarnessExecutionConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_valid_config_passes() {
        let config = HarnessExecutionConfig {
            seed: 12345,
            conversation_count: 30,
            timeout_seconds: 1800,
            max_dataset_size_bytes: 75_000_000,
            memory_limit_bytes: 1_073_741_824,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_conversation_count_too_low() {
        let config = HarnessExecutionConfig {
            conversation_count: 5,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());

        match result {
            Err(ConfigError::ConversationCountTooLow { actual, minimum }) => {
                assert_eq!(actual, 5);
                assert_eq!(minimum, MIN_CONVERSATION_COUNT);
            }
            _ => panic!("Expected ConversationCountTooLow error"),
        }
    }

    #[test]
    fn test_conversation_count_too_high() {
        let config = HarnessExecutionConfig {
            conversation_count: 100,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());

        match result {
            Err(ConfigError::ConversationCountTooHigh { actual, maximum }) => {
                assert_eq!(actual, 100);
                assert_eq!(maximum, MAX_CONVERSATION_COUNT);
            }
            _ => panic!("Expected ConversationCountTooHigh error"),
        }
    }

    #[test]
    fn test_timeout_too_high() {
        let config = HarnessExecutionConfig {
            timeout_seconds: 8000,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());

        match result {
            Err(ConfigError::TimeoutTooHigh { actual, maximum }) => {
                assert_eq!(actual, 8000);
                assert_eq!(maximum, MAX_TIMEOUT_SECONDS);
            }
            _ => panic!("Expected TimeoutTooHigh error"),
        }
    }

    #[test]
    fn test_dataset_size_too_large() {
        let config = HarnessExecutionConfig {
            max_dataset_size_bytes: 200_000_000,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());

        match result {
            Err(ConfigError::DatasetSizeTooLarge { actual, maximum }) => {
                assert_eq!(actual, 200_000_000);
                assert_eq!(maximum, MAX_DATASET_SIZE_BYTES);
            }
            _ => panic!("Expected DatasetSizeTooLarge error"),
        }
    }

    #[test]
    fn test_boundary_values_pass() {
        // Test minimum conversation count
        let config_min = HarnessExecutionConfig {
            conversation_count: MIN_CONVERSATION_COUNT,
            ..Default::default()
        };
        assert!(config_min.validate().is_ok());

        // Test maximum conversation count
        let config_max = HarnessExecutionConfig {
            conversation_count: MAX_CONVERSATION_COUNT,
            ..Default::default()
        };
        assert!(config_max.validate().is_ok());

        // Test maximum timeout
        let config_timeout = HarnessExecutionConfig {
            timeout_seconds: MAX_TIMEOUT_SECONDS,
            ..Default::default()
        };
        assert!(config_timeout.validate().is_ok());

        // Test maximum dataset size
        let config_size = HarnessExecutionConfig {
            max_dataset_size_bytes: MAX_DATASET_SIZE_BYTES,
            ..Default::default()
        };
        assert!(config_size.validate().is_ok());
    }

    #[test]
    fn test_evaluation_config_default() {
        let config = EvaluationConfig::default();
        assert_eq!(config.llm_model, "claude-3-sonnet");
        assert_eq!(config.llm_endpoint, "https://chutes.ai/v1");
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 1000);
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::ConversationCountTooLow {
            actual: 5,
            minimum: 10,
        };
        assert_eq!(err.to_string(), "conversation count 5 is below minimum 10");

        let err = ConfigError::TimeoutTooHigh {
            actual: 8000,
            maximum: 7200,
        };
        assert_eq!(
            err.to_string(),
            "timeout 8000 seconds exceeds maximum 7200 seconds"
        );
    }

    #[test]
    fn test_config_serialization() {
        let config = HarnessExecutionConfig::default();
        let serialized = bincode::serialize(&config).expect("serialization should succeed");
        let deserialized: HarnessExecutionConfig =
            bincode::deserialize(&serialized).expect("deserialization should succeed");
        assert_eq!(config, deserialized);
    }
}
