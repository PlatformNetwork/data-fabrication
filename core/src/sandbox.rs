//! Sandbox execution helpers for Python isolation.
//!
//! Provides filesystem isolation via temporary directories and resource
//! limits via rlimit to constrain untrusted Python code execution.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::error::{DataFabricationError, Result};

#[cfg(unix)]
use crate::resource_limits::ResourceLimits;

/// Output filename for harness execution results.
pub const OUTPUT_FILENAME: &str = "output.jsonl";

/// Harness filename for the Python script.
pub const HARNESS_FILENAME: &str = "harness.py";

/// Configuration for creating a sandbox environment.
#[derive(Debug, Clone)]
#[cfg(unix)]
pub struct SandboxConfig {
    /// Resource limits to apply (CPU, memory, file size, processes).
    pub limits: ResourceLimits,
    /// Optional custom working directory. If None, creates a temp directory.
    pub working_directory: Option<PathBuf>,
}

#[cfg(unix)]
impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            limits: ResourceLimits::default(),
            working_directory: None,
        }
    }
}

/// Simplified sandbox config for non-Unix platforms.
#[derive(Debug, Clone)]
#[cfg(not(unix))]
pub struct SandboxConfig {
    /// Optional custom working directory. If None, creates a temp directory.
    pub working_directory: Option<PathBuf>,
}

#[cfg(not(unix))]
impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            working_directory: None,
        }
    }
}

/// Result of sandbox creation with paths to key resources.
#[derive(Debug, Clone)]
pub struct SandboxResult {
    /// Absolute path to the sandbox working directory.
    pub workdir_path: PathBuf,
    /// Absolute path to the output file for harness results.
    pub output_file: PathBuf,
}

/// Isolated execution sandbox with automatic cleanup.
///
/// Uses RAII pattern to ensure the temporary directory is cleaned up
/// when the sandbox is dropped, even on panic.
pub struct Sandbox {
    workdir: TempDir,
    #[cfg(unix)]
    limits: ResourceLimits,
    created_at: Instant,
}

impl Sandbox {
    /// Creates a new sandbox environment (Unix).
    #[cfg(unix)]
    pub fn new(config: SandboxConfig) -> Result<Self> {
        let workdir = create_tempdir(config.working_directory.as_ref())?;

        let sandbox = Self {
            workdir,
            limits: config.limits,
            created_at: Instant::now(),
        };

        sandbox.apply_limits()?;

        Ok(sandbox)
    }

    /// Creates a new sandbox environment (non-Unix).
    #[cfg(not(unix))]
    pub fn new(config: SandboxConfig) -> Result<Self> {
        let workdir = create_tempdir(config.working_directory.as_deref())?;

        Ok(Self {
            workdir,
            created_at: Instant::now(),
        })
    }

    /// Applies resource limits to the current process (Unix only).
    #[cfg(unix)]
    fn apply_limits(&self) -> Result<()> {
        use rlimit::{setrlimit, Resource};

        setrlimit(
            Resource::CPU,
            self.limits.cpu_time_seconds,
            self.limits.cpu_time_seconds,
        )
        .map_err(|e| DataFabricationError::SecurityViolation {
            pattern: format!("Failed to set CPU limit: {}", e),
            severity: "critical".to_string(),
            line: None,
        })?;

        setrlimit(
            Resource::AS,
            self.limits.memory_bytes,
            self.limits.memory_bytes,
        )
        .map_err(|e| DataFabricationError::SecurityViolation {
            pattern: format!("Failed to set memory limit: {}", e),
            severity: "critical".to_string(),
            line: None,
        })?;

        setrlimit(
            Resource::FSIZE,
            self.limits.max_file_size,
            self.limits.max_file_size,
        )
        .map_err(|e| DataFabricationError::SecurityViolation {
            pattern: format!("Failed to set file size limit: {}", e),
            severity: "critical".to_string(),
            line: None,
        })?;

        setrlimit(
            Resource::NPROC,
            self.limits.max_processes as u64,
            self.limits.max_processes as u64,
        )
        .map_err(|e| DataFabricationError::SecurityViolation {
            pattern: format!("Failed to set process limit: {}", e),
            severity: "critical".to_string(),
            line: None,
        })?;

        Ok(())
    }

    /// No-op on non-Unix platforms (resource limits not available).
    #[cfg(not(unix))]
    fn apply_limits(&self) -> Result<()> {
        Ok(())
    }

    pub fn workdir_path(&self) -> PathBuf {
        self.workdir.path().to_path_buf()
    }

    pub fn output_path(&self) -> PathBuf {
        self.workdir.path().join(OUTPUT_FILENAME)
    }

    pub fn harness_path(&self) -> PathBuf {
        self.workdir.path().join(HARNESS_FILENAME)
    }

    pub fn write_harness(&self, code: &[u8]) -> Result<PathBuf> {
        let harness_path = self.harness_path();
        fs::write(&harness_path, code).map_err(|e| DataFabricationError::IoError {
            message: format!("Failed to write harness: {}", e),
            source: format!("{:?}", e.kind()),
        })?;
        Ok(harness_path)
    }

    pub fn elapsed(&self) -> Duration {
        self.created_at.elapsed()
    }

    #[cfg(unix)]
    pub fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    pub fn create_file(&self, filename: &str, content: &[u8]) -> Result<PathBuf> {
        let path = self.workdir.path().join(filename);
        fs::write(&path, content).map_err(|e| DataFabricationError::IoError {
            message: format!("Failed to create file '{}': {}", filename, e),
            source: format!("{:?}", e.kind()),
        })?;
        Ok(path)
    }

    pub fn read_file(&self, filename: &str) -> Result<Vec<u8>> {
        let path = self.workdir.path().join(filename);
        fs::read(&path).map_err(|e| DataFabricationError::IoError {
            message: format!("Failed to read file '{}': {}", filename, e),
            source: format!("{:?}", e.kind()),
        })
    }

    pub fn has_output(&self) -> bool {
        self.output_path().exists()
    }

    pub fn as_result(&self) -> SandboxResult {
        SandboxResult {
            workdir_path: self.workdir_path(),
            output_file: self.output_path(),
        }
    }
}

fn create_tempdir(working_directory: Option<&PathBuf>) -> Result<TempDir> {
    match working_directory {
        Some(dir) => {
            fs::create_dir_all(dir).map_err(|e| DataFabricationError::IoError {
                message: format!("Failed to create working directory: {}", e),
                source: format!("{:?}", e.kind()),
            })?;
            TempDir::new_in(dir).map_err(|e| DataFabricationError::IoError {
                message: format!("Failed to create temp directory: {}", e),
                source: format!("{:?}", e.kind()),
            })
        }
        None => TempDir::new().map_err(|e| DataFabricationError::IoError {
            message: format!("Failed to create temp directory: {}", e),
            source: format!("{:?}", e.kind()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_creates_temp_directory() {
        let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");
        let path = sandbox.workdir_path();
        assert!(path.exists(), "Working directory should exist");
        assert!(path.is_dir(), "Working directory should be a directory");
    }

    #[test]
    fn test_sandbox_cleans_up_on_drop() {
        let path = {
            let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");
            sandbox.workdir_path()
        };
        assert!(
            !path.exists(),
            "Working directory should be cleaned up after drop"
        );
    }

    #[test]
    fn test_multiple_sandboxes_have_different_directories() {
        let sandbox1 = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox1");
        let sandbox2 = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox2");

        let path1 = sandbox1.workdir_path();
        let path2 = sandbox2.workdir_path();

        assert_ne!(path1, path2, "Each sandbox should have its own directory");
    }

    #[test]
    fn test_sandbox_output_path() {
        let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");
        let expected_output = sandbox.workdir_path().join(OUTPUT_FILENAME);
        assert_eq!(sandbox.output_path(), expected_output);
    }

    #[test]
    fn test_sandbox_harness_path() {
        let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");
        let expected_harness = sandbox.workdir_path().join(HARNESS_FILENAME);
        assert_eq!(sandbox.harness_path(), expected_harness);
    }

    #[test]
    fn test_sandbox_elapsed() {
        let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");
        let elapsed = sandbox.elapsed();
        assert!(
            elapsed.as_millis() < 100,
            "Elapsed should be small after creation"
        );
    }

    #[test]
    fn test_sandbox_write_harness() {
        let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");

        let code = b"print('hello')";
        let path = sandbox
            .write_harness(code)
            .expect("Failed to write harness");

        assert!(path.exists(), "Harness file should exist");
        assert_eq!(path, sandbox.harness_path());

        let content = std::fs::read(&path).expect("Failed to read harness");
        assert_eq!(content, code);
    }

    #[test]
    fn test_sandbox_create_and_read_file() {
        let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");

        let content = b"test content";
        let path = sandbox
            .create_file("test.txt", content)
            .expect("Failed to create file");

        assert!(path.exists(), "Created file should exist");

        let read_content = sandbox.read_file("test.txt").expect("Failed to read file");
        assert_eq!(read_content, content);
    }

    #[test]
    fn test_sandbox_has_output() {
        let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");

        assert!(!sandbox.has_output(), "Should not have output initially");

        sandbox
            .create_file(OUTPUT_FILENAME, b"{}")
            .expect("Failed to create output");

        assert!(sandbox.has_output(), "Should have output after creation");
    }

    #[test]
    fn test_sandbox_result() {
        let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");

        let result = sandbox.as_result();
        assert_eq!(result.workdir_path, sandbox.workdir_path());
        assert_eq!(result.output_file, sandbox.output_path());
    }

    #[test]
    fn test_custom_working_directory() {
        let parent_dir = tempfile::tempdir().expect("Failed to create parent dir");
        let config = SandboxConfig {
            #[cfg(unix)]
            limits: ResourceLimits::default(),
            working_directory: Some(parent_dir.path().to_path_buf()),
        };

        let sandbox = Sandbox::new(config).expect("Failed to create sandbox");

        let path = sandbox.workdir_path();
        assert!(
            path.starts_with(parent_dir.path()),
            "Sandbox should be created in custom directory"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_restrictive_config() {
        use crate::resource_limits::{MAX_FILE_SIZE_BYTES, MAX_MEMORY_BYTES};

        let config = SandboxConfig {
            limits: ResourceLimits {
                cpu_time_seconds: 300,
                memory_bytes: 512 * 1024 * 1024,
                max_processes: 16,
                max_file_size: 10_000_000,
            },
            working_directory: None,
        };

        let sandbox = Sandbox::new(config).expect("Failed to create sandbox");

        assert_eq!(sandbox.limits().cpu_time_seconds, 300);
        assert_eq!(sandbox.limits().max_processes, 16);
    }
}
