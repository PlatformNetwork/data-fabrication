//! Resource limit configuration for sandboxed Python execution.
//!
//! Defines Unix resource limits (rlimit) to constrain harness execution:
//! - CPU time: prevent infinite loops
//! - Memory: prevent memory exhaustion
//! - Processes: prevent fork bombs
//! - File size: prevent disk exhaustion

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

#[cfg(unix)]
use rlimit::{setrlimit, Resource};

/// Maximum CPU time in seconds (2 hours).
pub const MAX_CPU_TIME_SECONDS: u64 = 7200;

/// Maximum memory in bytes (4 GB).
pub const MAX_MEMORY_BYTES: u64 = 4_000_000_000;

/// Maximum number of processes.
pub const MAX_PROCESSES: u32 = 4;

/// Maximum file size in bytes (200 MB).
pub const MAX_FILE_SIZE_BYTES: u64 = 200_000_000;

/// Default memory limit (2 GB).
pub const DEFAULT_MEMORY_BYTES: u64 = 2_147_483_648;

/// Default file size limit (100 MB).
pub const DEFAULT_FILE_SIZE_BYTES: u64 = 104_857_600;

/// Resource limits for sandboxed harness execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Maximum CPU time in seconds (RLIMIT_CPU).
    /// Hard limit: 7200 seconds (2 hours).
    pub cpu_time_seconds: u64,

    /// Maximum memory in bytes (RLIMIT_AS).
    /// Default: 2 GB, Max: 4 GB.
    pub memory_bytes: u64,

    /// Maximum number of processes (RLIMIT_NPROC).
    /// Default: 4, Min: 1.
    pub max_processes: u32,

    /// Maximum file size in bytes (RLIMIT_FSIZE).
    /// Default: 100 MB, Max: 200 MB.
    pub max_file_size: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_time_seconds: MAX_CPU_TIME_SECONDS,
            memory_bytes: DEFAULT_MEMORY_BYTES,
            max_processes: MAX_PROCESSES,
            max_file_size: DEFAULT_FILE_SIZE_BYTES,
        }
    }
}

impl ResourceLimits {
    /// Creates new resource limits with validation.
    ///
    /// # Errors
    ///
    /// Returns `ResourceLimitError` if any limit is invalid.
    pub fn new(
        cpu_time_seconds: u64,
        memory_bytes: u64,
        max_processes: u32,
        max_file_size: u64,
    ) -> Result<Self, ResourceLimitError> {
        let limits = Self {
            cpu_time_seconds,
            memory_bytes,
            max_processes,
            max_file_size,
        };
        limits.validate()?;
        Ok(limits)
    }

    /// Validates all resource limit values.
    ///
    /// # Errors
    ///
    /// Returns `ResourceLimitError` if:
    /// - `cpu_time_seconds` exceeds 7200
    /// - `memory_bytes` exceeds 4 GB
    /// - `max_processes` is zero
    /// - `max_file_size` exceeds 200 MB
    pub fn validate(&self) -> Result<(), ResourceLimitError> {
        if self.cpu_time_seconds > MAX_CPU_TIME_SECONDS {
            return Err(ResourceLimitError::CpuTimeTooHigh {
                actual: self.cpu_time_seconds,
                maximum: MAX_CPU_TIME_SECONDS,
            });
        }

        if self.memory_bytes > MAX_MEMORY_BYTES {
            return Err(ResourceLimitError::MemoryTooHigh {
                actual: self.memory_bytes,
                maximum: MAX_MEMORY_BYTES,
            });
        }

        if self.max_processes == 0 {
            return Err(ResourceLimitError::ProcessesTooLow {
                actual: self.max_processes,
                minimum: 1,
            });
        }

        if self.max_file_size > MAX_FILE_SIZE_BYTES {
            return Err(ResourceLimitError::FileSizeTooHigh {
                actual: self.max_file_size,
                maximum: MAX_FILE_SIZE_BYTES,
            });
        }

        Ok(())
    }

    /// Converts resource limits to rlimit resource tuples.
    ///
    /// Returns a vector of (resource, limit) pairs for use with rlimit.
    #[cfg(unix)]
    pub fn to_rlimit(&self) -> Vec<(Resource, u64)> {
        vec![
            (Resource::CPU, self.cpu_time_seconds),
            (Resource::AS, self.memory_bytes),
            (Resource::NPROC, self.max_processes as u64),
            (Resource::FSIZE, self.max_file_size),
        ]
    }

    /// Applies resource limits to the current process.
    ///
    /// # Errors
    ///
    /// Returns `ResourceLimitError` if setting any limit fails.
    ///
    /// # Safety
    ///
    /// This modifies process-wide limits. Should only be called
    /// before spawning constrained child processes.
    #[cfg(unix)]
    pub fn apply(&self) -> Result<(), ResourceLimitError> {
        self.validate()?;

        for (resource, limit) in self.to_rlimit() {
            if resource.is_supported() {
                setrlimit(resource, limit, limit).map_err(|e| ResourceLimitError::ApplyFailed {
                    resource: format!("{:?}", resource),
                    message: e.to_string(),
                })?;
            }
        }

        Ok(())
    }
}

/// Error type for resource limit validation and application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceLimitError {
    /// CPU time exceeds maximum allowed value.
    CpuTimeTooHigh { actual: u64, maximum: u64 },

    /// Memory exceeds maximum allowed value.
    MemoryTooHigh { actual: u64, maximum: u64 },

    /// Process count is below minimum.
    ProcessesTooLow { actual: u32, minimum: u32 },

    /// File size exceeds maximum allowed value.
    FileSizeTooHigh { actual: u64, maximum: u64 },

    /// Failed to apply a resource limit.
    #[cfg(unix)]
    ApplyFailed { resource: String, message: String },
}

impl ResourceLimitError {
    /// Returns a static error message for this error variant.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CpuTimeTooHigh { .. } => "CPU time limit exceeds maximum",
            Self::MemoryTooHigh { .. } => "memory limit exceeds maximum",
            Self::ProcessesTooLow { .. } => "process count is below minimum",
            Self::FileSizeTooHigh { .. } => "file size limit exceeds maximum",
            #[cfg(unix)]
            Self::ApplyFailed { .. } => "failed to apply resource limit",
        }
    }
}

impl fmt::Display for ResourceLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CpuTimeTooHigh { actual, maximum } => {
                write!(
                    f,
                    "CPU time {} seconds exceeds maximum {} seconds",
                    actual, maximum
                )
            }
            Self::MemoryTooHigh { actual, maximum } => {
                write!(
                    f,
                    "memory {} bytes exceeds maximum {} bytes",
                    actual, maximum
                )
            }
            Self::ProcessesTooLow { actual, minimum } => {
                write!(f, "process count {} is below minimum {}", actual, minimum)
            }
            Self::FileSizeTooHigh { actual, maximum } => {
                write!(
                    f,
                    "file size {} bytes exceeds maximum {} bytes",
                    actual, maximum
                )
            }
            #[cfg(unix)]
            Self::ApplyFailed { resource, message } => {
                write!(f, "failed to set {} limit: {}", resource, message)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ResourceLimitError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits_are_valid() {
        let limits = ResourceLimits::default();
        assert!(limits.validate().is_ok());
    }

    #[test]
    fn test_new_with_valid_limits() {
        let limits = ResourceLimits::new(3600, 1_000_000_000, 2, 50_000_000);
        assert!(limits.is_ok());

        let limits = limits.unwrap();
        assert_eq!(limits.cpu_time_seconds, 3600);
        assert_eq!(limits.memory_bytes, 1_000_000_000);
        assert_eq!(limits.max_processes, 2);
        assert_eq!(limits.max_file_size, 50_000_000);
    }

    #[test]
    fn test_cpu_time_too_high() {
        let limits = ResourceLimits {
            cpu_time_seconds: 8000,
            ..Default::default()
        };
        let result = limits.validate();
        assert!(result.is_err());

        match result {
            Err(ResourceLimitError::CpuTimeTooHigh { actual, maximum }) => {
                assert_eq!(actual, 8000);
                assert_eq!(maximum, MAX_CPU_TIME_SECONDS);
            }
            _ => panic!("Expected CpuTimeTooHigh error"),
        }
    }

    #[test]
    fn test_memory_too_high() {
        let limits = ResourceLimits {
            memory_bytes: 5_000_000_000,
            ..Default::default()
        };
        let result = limits.validate();
        assert!(result.is_err());

        match result {
            Err(ResourceLimitError::MemoryTooHigh { actual, maximum }) => {
                assert_eq!(actual, 5_000_000_000);
                assert_eq!(maximum, MAX_MEMORY_BYTES);
            }
            _ => panic!("Expected MemoryTooHigh error"),
        }
    }

    #[test]
    fn test_processes_too_low() {
        let limits = ResourceLimits {
            max_processes: 0,
            ..Default::default()
        };
        let result = limits.validate();
        assert!(result.is_err());

        match result {
            Err(ResourceLimitError::ProcessesTooLow { actual, minimum }) => {
                assert_eq!(actual, 0);
                assert_eq!(minimum, 1);
            }
            _ => panic!("Expected ProcessesTooLow error"),
        }
    }

    #[test]
    fn test_file_size_too_high() {
        let limits = ResourceLimits {
            max_file_size: 300_000_000,
            ..Default::default()
        };
        let result = limits.validate();
        assert!(result.is_err());

        match result {
            Err(ResourceLimitError::FileSizeTooHigh { actual, maximum }) => {
                assert_eq!(actual, 300_000_000);
                assert_eq!(maximum, MAX_FILE_SIZE_BYTES);
            }
            _ => panic!("Expected FileSizeTooHigh error"),
        }
    }

    #[test]
    fn test_boundary_values_pass() {
        let limits = ResourceLimits {
            cpu_time_seconds: MAX_CPU_TIME_SECONDS,
            ..Default::default()
        };
        assert!(limits.validate().is_ok());

        let limits = ResourceLimits {
            memory_bytes: MAX_MEMORY_BYTES,
            ..Default::default()
        };
        assert!(limits.validate().is_ok());

        let limits = ResourceLimits {
            max_processes: 1,
            ..Default::default()
        };
        assert!(limits.validate().is_ok());

        let limits = ResourceLimits {
            max_file_size: MAX_FILE_SIZE_BYTES,
            ..Default::default()
        };
        assert!(limits.validate().is_ok());
    }

    #[test]
    fn test_zero_limits_allowed() {
        let limits = ResourceLimits {
            cpu_time_seconds: 0,
            memory_bytes: 0,
            max_file_size: 0,
            max_processes: 1,
        };
        assert!(limits.validate().is_ok());
    }

    #[test]
    fn test_error_display() {
        let err = ResourceLimitError::CpuTimeTooHigh {
            actual: 8000,
            maximum: 7200,
        };
        assert_eq!(
            err.to_string(),
            "CPU time 8000 seconds exceeds maximum 7200 seconds"
        );

        let err = ResourceLimitError::ProcessesTooLow {
            actual: 0,
            minimum: 1,
        };
        assert_eq!(err.to_string(), "process count 0 is below minimum 1");
    }

    #[test]
    #[cfg(unix)]
    fn test_to_rlimit_conversion() {
        let limits = ResourceLimits::default();
        let rlimits = limits.to_rlimit();

        assert_eq!(rlimits.len(), 4);

        let has_cpu = rlimits.iter().any(|(r, _)| matches!(*r, Resource::CPU));
        let has_as = rlimits.iter().any(|(r, _)| matches!(*r, Resource::AS));
        let has_nproc = rlimits.iter().any(|(r, _)| matches!(*r, Resource::NPROC));
        let has_fsize = rlimits.iter().any(|(r, _)| matches!(*r, Resource::FSIZE));

        assert!(has_cpu, "Should contain CPU resource");
        assert!(has_as, "Should contain AS (memory) resource");
        assert!(has_nproc, "Should contain NPROC resource");
        assert!(has_fsize, "Should contain FSIZE resource");
    }
}
