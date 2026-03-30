//! Error types for data-fabrication challenge.
//!
//! This module defines comprehensive error types used across all packages
//! in the data-fabrication workspace.

use std::fmt;

/// The main error type for data-fabrication operations.
#[derive(Debug)]
pub enum DataFabricationError {
    /// Errors related to schema validation and parsing.
    SchemaError {
        message: String,
        line: Option<usize>,
    },

    /// Errors from external process execution.
    ExecutionError {
        message: String,
        exit_code: Option<i32>,
    },

    /// Security violations detected during script execution.
    SecurityViolation {
        pattern: String,
        severity: String,
        line: Option<usize>,
    },

    /// Errors from LLM API calls.
    LlmError { message: String, retry_count: u32 },

    /// Errors from consensus validation failures.
    ConsensusError { message: String, scores: Vec<f64> },

    /// Configuration errors.
    ConfigError { message: String },

    /// I/O errors with additional context.
    IoError { message: String, source: String },

    /// Timeout errors for operations that exceeded time limits.
    TimeoutError {
        elapsed_seconds: u64,
        limit_seconds: u64,
    },

    /// JSON serialization/deserialization errors.
    JsonError { message: String },
}

impl fmt::Display for DataFabricationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaError { message, line } => match line {
                Some(l) => write!(f, "Schema error at line {}: {}", l, message),
                None => write!(f, "Schema error: {}", message),
            },
            Self::ExecutionError { message, exit_code } => match exit_code {
                Some(code) => write!(f, "Execution error (exit code {}): {}", code, message),
                None => write!(f, "Execution error: {}", message),
            },
            Self::SecurityViolation {
                pattern,
                severity,
                line,
            } => match line {
                Some(l) => write!(
                    f,
                    "Security violation ({}) at line {}: {}",
                    severity, l, pattern
                ),
                None => write!(f, "Security violation ({}): {}", severity, pattern),
            },
            Self::LlmError {
                message,
                retry_count,
            } => {
                write!(f, "LLM error (retry {}): {}", retry_count, message)
            }
            Self::ConsensusError { message, scores } => {
                let scores_str: Vec<String> = scores.iter().map(|s| format!("{:.2}", s)).collect();
                write!(
                    f,
                    "Consensus error: {} (scores: [{}])",
                    message,
                    scores_str.join(", ")
                )
            }
            Self::ConfigError { message } => {
                write!(f, "Configuration error: {}", message)
            }
            Self::IoError { message, source } => {
                write!(f, "I/O error: {} (source: {})", message, source)
            }
            Self::TimeoutError {
                elapsed_seconds,
                limit_seconds,
            } => {
                write!(
                    f,
                    "Timeout error: operation took {}s (limit: {}s)",
                    elapsed_seconds, limit_seconds
                )
            }
            Self::JsonError { message } => {
                write!(f, "JSON error: {}", message)
            }
        }
    }
}

impl std::error::Error for DataFabricationError {}

impl From<serde_json::Error> for DataFabricationError {
    fn from(err: serde_json::Error) -> Self {
        Self::JsonError {
            message: err.to_string(),
        }
    }
}

impl From<std::io::Error> for DataFabricationError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError {
            message: err.to_string(),
            source: format!("{:?}", err.kind()),
        }
    }
}

/// A specialized `Result` type for data-fabrication operations.
pub type Result<T> = std::result::Result<T, DataFabricationError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_error_display_with_line() {
        let err = DataFabricationError::SchemaError {
            message: "Invalid type".to_string(),
            line: Some(42),
        };
        assert_eq!(format!("{}", err), "Schema error at line 42: Invalid type");
    }

    #[test]
    fn test_schema_error_display_without_line() {
        let err = DataFabricationError::SchemaError {
            message: "Missing field".to_string(),
            line: None,
        };
        assert_eq!(format!("{}", err), "Schema error: Missing field");
    }

    #[test]
    fn test_execution_error_display_with_exit_code() {
        let err = DataFabricationError::ExecutionError {
            message: "Process failed".to_string(),
            exit_code: Some(1),
        };
        assert_eq!(
            format!("{}", err),
            "Execution error (exit code 1): Process failed"
        );
    }

    #[test]
    fn test_execution_error_display_without_exit_code() {
        let err = DataFabricationError::ExecutionError {
            message: "Process terminated".to_string(),
            exit_code: None,
        };
        assert_eq!(format!("{}", err), "Execution error: Process terminated");
    }

    #[test]
    fn test_security_violation_display_with_line() {
        let err = DataFabricationError::SecurityViolation {
            pattern: "os.system()".to_string(),
            severity: "high".to_string(),
            line: Some(10),
        };
        assert_eq!(
            format!("{}", err),
            "Security violation (high) at line 10: os.system()"
        );
    }

    #[test]
    fn test_security_violation_display_without_line() {
        let err = DataFabricationError::SecurityViolation {
            pattern: "import subprocess".to_string(),
            severity: "critical".to_string(),
            line: None,
        };
        assert_eq!(
            format!("{}", err),
            "Security violation (critical): import subprocess"
        );
    }

    #[test]
    fn test_llm_error_display() {
        let err = DataFabricationError::LlmError {
            message: "Rate limit exceeded".to_string(),
            retry_count: 3,
        };
        assert_eq!(
            format!("{}", err),
            "LLM error (retry 3): Rate limit exceeded"
        );
    }

    #[test]
    fn test_consensus_error_display() {
        let err = DataFabricationError::ConsensusError {
            message: "Validation failed".to_string(),
            scores: vec![0.5, 0.6, 0.55],
        };
        assert_eq!(
            format!("{}", err),
            "Consensus error: Validation failed (scores: [0.50, 0.60, 0.55])"
        );
    }

    #[test]
    fn test_config_error_display() {
        let err = DataFabricationError::ConfigError {
            message: "Missing configuration".to_string(),
        };
        assert_eq!(
            format!("{}", err),
            "Configuration error: Missing configuration"
        );
    }

    #[test]
    fn test_io_error_display() {
        let err = DataFabricationError::IoError {
            message: "File not found".to_string(),
            source: "NotFound".to_string(),
        };
        assert_eq!(
            format!("{}", err),
            "I/O error: File not found (source: NotFound)"
        );
    }

    #[test]
    fn test_timeout_error_display() {
        let err = DataFabricationError::TimeoutError {
            elapsed_seconds: 65,
            limit_seconds: 60,
        };
        assert_eq!(
            format!("{}", err),
            "Timeout error: operation took 65s (limit: 60s)"
        );
    }

    #[test]
    fn test_json_error_display() {
        let err = DataFabricationError::JsonError {
            message: "Invalid JSON".to_string(),
        };
        assert_eq!(format!("{}", err), "JSON error: Invalid JSON");
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let err: DataFabricationError = json_err.into();
        match err {
            DataFabricationError::JsonError { .. } => (),
            _ => panic!("Expected JsonError variant"),
        }
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let err: DataFabricationError = io_err.into();
        match err {
            DataFabricationError::IoError { message, .. } => {
                assert!(message.contains("test"));
            }
            _ => panic!("Expected IoError variant"),
        }
    }
}
