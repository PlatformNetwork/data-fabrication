//! Error types for the Python execution engine.
//!
//! Provides executor-specific error types for handling Python harness failures.

use std::fmt;

/// Error type for executor operations.
#[derive(Debug, Clone)]
pub enum ExecutorError {
    /// Failed to spawn the Python process.
    ProcessSpawn {
        /// Error message describing the spawn failure.
        message: String,
    },

    /// Process execution exceeded the timeout limit.
    Timeout {
        /// Number of seconds before timeout occurred.
        seconds: u64,
    },

    /// Memory limit was exceeded during execution.
    MemoryExceeded {
        /// Number of bytes that were attempted to be allocated.
        bytes: u64,
    },

    /// Output validation failed.
    InvalidOutput {
        /// Error message describing the validation failure.
        message: String,
        /// Optional line number where the error occurred.
        line: Option<usize>,
    },

    /// Sandbox security violation detected.
    SandboxViolation {
        /// Description of the security violation.
        message: String,
    },

    /// Security violation detected during AST validation (CRITICAL).
    SecurityViolation {
        /// Description of the security violation.
        message: String,
        /// Line number where the violation occurred.
        line: Option<usize>,
    },

    /// Security validation failed to parse or analyze code.
    SecurityValidation {
        /// Error message describing the validation failure.
        message: String,
    },

    /// I/O error with additional context.
    IoError {
        /// Human-readable error message.
        message: String,
        /// The underlying error kind or source.
        source: String,
    },
}

impl ExecutorError {
    /// Returns a static error category for this error variant.
    pub fn category(&self) -> &'static str {
        match self {
            Self::ProcessSpawn { .. } => "process_spawn",
            Self::Timeout { .. } => "timeout",
            Self::MemoryExceeded { .. } => "memory_exceeded",
            Self::InvalidOutput { .. } => "invalid_output",
            Self::SandboxViolation { .. } => "sandbox_violation",
            Self::SecurityViolation { .. } => "security_violation",
            Self::SecurityValidation { .. } => "security_validation",
            Self::IoError { .. } => "io_error",
        }
    }
}

impl fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProcessSpawn { message } => {
                write!(f, "Failed to spawn Python process: {}", message)
            }
            Self::Timeout { seconds } => {
                write!(f, "Execution timed out after {} seconds", seconds)
            }
            Self::MemoryExceeded { bytes } => {
                write!(
                    f,
                    "Memory limit exceeded: attempted to allocate {} bytes",
                    bytes
                )
            }
            Self::InvalidOutput { message, line } => match line {
                Some(l) => write!(f, "Invalid output at line {}: {}", l, message),
                None => write!(f, "Invalid output: {}", message),
            },
            Self::SandboxViolation { message } => {
                write!(f, "Sandbox violation: {}", message)
            }
            Self::SecurityViolation { message, line } => match line {
                Some(l) => write!(f, "Security violation at line {}: {}", l, message),
                None => write!(f, "Security violation: {}", message),
            },
            Self::SecurityValidation { message } => {
                write!(f, "Security validation failed: {}", message)
            }
            Self::IoError { message, source } => {
                write!(f, "I/O error: {} (source: {})", message, source)
            }
        }
    }
}

impl std::error::Error for ExecutorError {}

impl From<std::io::Error> for ExecutorError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError {
            message: err.to_string(),
            source: format!("{:?}", err.kind()),
        }
    }
}

/// Result type for executor operations.
pub type ExecutorResult<T> = std::result::Result<T, ExecutorError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_spawn_error_display() {
        let err = ExecutorError::ProcessSpawn {
            message: "python not found".to_string(),
        };
        assert_eq!(
            format!("{}", err),
            "Failed to spawn Python process: python not found"
        );
    }

    #[test]
    fn test_timeout_error_display() {
        let err = ExecutorError::Timeout { seconds: 3600 };
        assert_eq!(format!("{}", err), "Execution timed out after 3600 seconds");
    }

    #[test]
    fn test_memory_exceeded_error_display() {
        let err = ExecutorError::MemoryExceeded {
            bytes: 4_000_000_000,
        };
        assert_eq!(
            format!("{}", err),
            "Memory limit exceeded: attempted to allocate 4000000000 bytes"
        );
    }

    #[test]
    fn test_invalid_output_error_display_with_line() {
        let err = ExecutorError::InvalidOutput {
            message: "Invalid JSON".to_string(),
            line: Some(42),
        };
        assert_eq!(
            format!("{}", err),
            "Invalid output at line 42: Invalid JSON"
        );
    }

    #[test]
    fn test_invalid_output_error_display_without_line() {
        let err = ExecutorError::InvalidOutput {
            message: "Empty output".to_string(),
            line: None,
        };
        assert_eq!(format!("{}", err), "Invalid output: Empty output");
    }

    #[test]
    fn test_sandbox_violation_error_display() {
        let err = ExecutorError::SandboxViolation {
            message: "Attempted file access outside sandbox".to_string(),
        };
        assert_eq!(
            format!("{}", err),
            "Sandbox violation: Attempted file access outside sandbox"
        );
    }

    #[test]
    fn test_io_error_display() {
        let err = ExecutorError::IoError {
            message: "File not found".to_string(),
            source: "NotFound".to_string(),
        };
        assert_eq!(
            format!("{}", err),
            "I/O error: File not found (source: NotFound)"
        );
    }

    #[test]
    fn test_category() {
        assert_eq!(
            ExecutorError::ProcessSpawn {
                message: String::new()
            }
            .category(),
            "process_spawn"
        );
        assert_eq!(ExecutorError::Timeout { seconds: 0 }.category(), "timeout");
        assert_eq!(
            ExecutorError::MemoryExceeded { bytes: 0 }.category(),
            "memory_exceeded"
        );
        assert_eq!(
            ExecutorError::InvalidOutput {
                message: String::new(),
                line: None
            }
            .category(),
            "invalid_output"
        );
        assert_eq!(
            ExecutorError::SandboxViolation {
                message: String::new()
            }
            .category(),
            "sandbox_violation"
        );
        assert_eq!(
            ExecutorError::SecurityViolation {
                message: String::new(),
                line: None
            }
            .category(),
            "security_violation"
        );
        assert_eq!(
            ExecutorError::SecurityValidation {
                message: String::new()
            }
            .category(),
            "security_validation"
        );
        assert_eq!(
            ExecutorError::IoError {
                message: String::new(),
                source: String::new()
            }
            .category(),
            "io_error"
        );
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test file");
        let err: ExecutorError = io_err.into();
        match err {
            ExecutorError::IoError { message, .. } => {
                assert!(message.contains("test file"));
            }
            _ => panic!("Expected IoError variant"),
        }
    }
}
