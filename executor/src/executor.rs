//! Python harness execution engine.

use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::Context;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use data_fabrication_core::{Sandbox, SandboxConfig, JsonlParser, validate_python_code, Severity};

use crate::error::{ExecutorError, ExecutorResult};

/// Maximum output size to prevent memory exhaustion (10 MB).
const MAX_OUTPUT_SIZE: usize = 10 * 1024 * 1024;

/// Result of a Python harness execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Standard output from the Python process.
    pub stdout: String,
    /// Standard error from the Python process.
    pub stderr: String,
    /// Exit code of the process (None if killed).
    pub exit_code: Option<i32>,
    /// Time taken for execution in milliseconds.
    pub duration_ms: u64,
    /// Whether the execution timed out.
    pub timed_out: bool,
}

/// Python harness executor with sandbox isolation.
pub struct PythonExecutor {
    /// Timeout duration for execution.
    timeout_duration: Duration,
    /// Maximum output size in bytes.
    max_output_size: usize,
}

impl Default for PythonExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonExecutor {
    /// Creates a new executor with default settings.
    pub fn new() -> Self {
        Self {
            timeout_duration: Duration::from_secs(3600),
            max_output_size: MAX_OUTPUT_SIZE,
        }
    }

    /// Creates a new executor with custom timeout.
    pub fn with_timeout(seconds: u64) -> Self {
        Self {
            timeout_duration: Duration::from_secs(seconds.min(7200)),
            max_output_size: MAX_OUTPUT_SIZE,
        }
    }

    /// Creates a new executor with custom settings.
    pub fn with_config(timeout_seconds: u64, max_output_bytes: usize) -> Self {
        Self {
            timeout_duration: Duration::from_secs(timeout_seconds.min(7200)),
            max_output_size: max_output_bytes.min(MAX_OUTPUT_SIZE),
        }
    }

    /// Executes a Python harness in a sandboxed environment.
    pub async fn execute(&self, harness_code: &str) -> ExecutorResult<ExecutionResult> {
        let start = Instant::now();

        // Pre-execution security validation
        match validate_python_code(harness_code) {
            Ok(violations) => {
                // Block execution on CRITICAL violations
                for v in &violations {
                    if v.severity == Severity::Critical {
                        return Err(ExecutorError::SecurityViolation {
                            message: format!("Critical security violation: {}", v.pattern),
                            line: v.line,
                        });
                    }
                    warn!("Security warning: {}", v);
                }
            }
            Err(e) => {
                // Parse errors shouldn't block execution - let Python interpreter handle them
                warn!("Could not validate code (parse error): {}. Proceeding with execution.", e);
            }
        }

        // Create sandbox
        let sandbox = Sandbox::new(SandboxConfig::default())
            .map_err(|e| ExecutorError::IoError {
                message: format!("Failed to create sandbox: {}", e),
                source: "sandbox_creation".to_string(),
            })?;

        debug!("Created sandbox at: {:?}", sandbox.workdir_path());

        // Write harness to sandbox
        sandbox
            .write_harness(harness_code.as_bytes())
            .map_err(|e| ExecutorError::IoError {
                message: format!("Failed to write harness: {}", e),
                source: "harness_write".to_string(),
            })?;

        let harness_path = sandbox.harness_path();
        let workdir = sandbox.workdir_path();

        info!("Executing harness: {:?}", harness_path);

        // Spawn Python process
        let mut child = Command::new("python3")
            .arg(&harness_path)
            .current_dir(&workdir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn python3 process")
            .map_err(|e| ExecutorError::ProcessSpawn {
                message: e.to_string(),
            })?;

        let child_id = child.id();
        debug!("Spawned Python process with PID: {:?}", child_id);

        // Run with timeout wrapper
        let result = timeout(self.timeout_duration, async {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();

            // Read stdout
            if let Some(mut stdout_handle) = child.stdout.take() {
                let mut buf = [0u8; 8192];
                loop {
                    match stdout_handle.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if stdout.len() + n > self.max_output_size {
                                warn!("Output truncated at {} bytes", self.max_output_size);
                                break;
                            }
                            stdout.extend_from_slice(&buf[..n]);
                        }
                        Err(e) => {
                            error!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
            }

            // Read stderr
            if let Some(mut stderr_handle) = child.stderr.take() {
                let mut buf = [0u8; 8192];
                loop {
                    match stderr_handle.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if stderr.len() + n > self.max_output_size {
                                warn!("Stderr truncated at {} bytes", self.max_output_size);
                                break;
                            }
                            stderr.extend_from_slice(&buf[..n]);
                        }
                        Err(e) => {
                            error!("Error reading stderr: {}", e);
                            break;
                        }
                    }
                }
            }

            // Wait for process to complete
            let status = child.wait().await?;

            let stdout_str = String::from_utf8_lossy(&stdout).to_string();
            let stderr_str = String::from_utf8_lossy(&stderr).to_string();

            Ok::<_, std::io::Error>((stdout_str, stderr_str, status.code()))
        })
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok((stdout, stderr, exit_code))) => {
                info!(
                    "Harness execution completed in {}ms with exit code {:?}",
                    duration_ms, exit_code
                );

                Ok(ExecutionResult {
                    stdout,
                    stderr,
                    exit_code,
                    duration_ms,
                    timed_out: false,
                })
            }
            Ok(Err(e)) => {
                error!("Process error: {}", e);
                Err(ExecutorError::ProcessSpawn {
                    message: format!("Process error: {}", e),
                })
            }
            Err(_) => {
                // Timeout - process will be killed on drop
                warn!(
                    "Harness execution timed out after {}ms",
                    duration_ms
                );
                Err(ExecutorError::Timeout {
                    seconds: self.timeout_duration.as_secs(),
                })
            }
        }
    }

    /// Validates the output as JSONL format.
    pub fn validate_output(&self, output: &str) -> ExecutorResult<()> {
        if output.trim().is_empty() {
            return Err(ExecutorError::InvalidOutput {
                message: "Output is empty".to_string(),
                line: None,
            });
        }

        JsonlParser::parse(output).map_err(|e| ExecutorError::InvalidOutput {
            message: e.to_string(),
            line: None,
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_simple_harness() {
        let executor = PythonExecutor::with_timeout(30);
        let code = r#"
import json
entry = {"messages": [{"role": "user", "content": "Hello"}, {"role": "assistant", "content": "Hi there!"}]}
print(json.dumps(entry))
"#;

        let result = executor.execute(code).await;
        assert!(result.is_ok(), "Execution should succeed");

        let result = result.unwrap();
        assert_eq!(result.exit_code, Some(0), "Exit code should be 0");
        assert!(!result.timed_out, "Should not have timed out");

        // Validate output can be parsed
        let parse_result = JsonlParser::parse(&result.stdout);
        assert!(parse_result.is_ok(), "Output should be valid JSONL");
    }

    #[tokio::test]
    async fn test_timeout_kills_process() {
        let executor = PythonExecutor::with_timeout(2);
        let code = r#"
while True:
    pass
"#;

        let result = executor.execute(code).await;
        assert!(result.is_err(), "Should return error for timeout");

        match result {
            Err(ExecutorError::Timeout { seconds }) => {
                assert_eq!(seconds, 2);
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    #[tokio::test]
    async fn test_invalid_python_fails() {
        let executor = PythonExecutor::with_timeout(30);
        let code = r#"
this is not valid python syntax
"#;

        let result = executor.execute(code).await;
        assert!(result.is_ok(), "Execution should succeed (process runs)");

        let result = result.unwrap();
        assert_ne!(result.exit_code, Some(0), "Exit code should not be 0");
        assert!(!result.stderr.is_empty(), "Should have stderr");
    }

    #[tokio::test]
    async fn test_validate_output_empty() {
        let executor = PythonExecutor::new();
        let result = executor.validate_output("");
        assert!(result.is_err(), "Empty output should be invalid");

        match result {
            Err(ExecutorError::InvalidOutput { message, .. }) => {
                assert!(message.contains("empty"));
            }
            _ => panic!("Expected InvalidOutput error"),
        }
    }

    #[tokio::test]
    async fn test_validate_output_valid() {
        let executor = PythonExecutor::new();
        let output = r#"{"messages":[{"role":"user","content":"Hi"},{"role":"assistant","content":"Hello!"}]}"#;
        let result = executor.validate_output(output);
        assert!(result.is_ok(), "Valid output should pass");
    }

    #[tokio::test]
    async fn test_validate_output_invalid_json() {
        let executor = PythonExecutor::new();
        let output = r#"{"messages":[{"role":"user","content":"Hi"},{"role":"assistant","content":"Hello!"}]}{"invalid": json"#;
        let result = executor.validate_output(output);
        assert!(result.is_err(), "Invalid JSON should fail");
    }

    #[test]
    fn test_execution_result_debug() {
        let result = ExecutionResult {
            stdout: "test".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            duration_ms: 100,
            timed_out: false,
        };
        // Should not panic
        let _ = format!("{:?}", result);
    }

    #[test]
    fn test_executor_default() {
        let executor = PythonExecutor::default();
        assert_eq!(executor.timeout_duration, Duration::from_secs(3600));
        assert_eq!(executor.max_output_size, MAX_OUTPUT_SIZE);
    }

    #[test]
    fn test_executor_with_timeout() {
        let executor = PythonExecutor::with_timeout(1800);
        assert_eq!(executor.timeout_duration, Duration::from_secs(1800));

        // Test max cap
        let executor = PythonExecutor::with_timeout(10000);
        assert_eq!(executor.timeout_duration, Duration::from_secs(7200));
    }

    #[test]
    fn test_executor_with_config() {
        let executor = PythonExecutor::with_config(600, 5_000_000);
        assert_eq!(executor.timeout_duration, Duration::from_secs(600));
        assert_eq!(executor.max_output_size, 5_000_000);
    }

    #[tokio::test]
    async fn test_security_violation_blocks_exec() {
        let executor = PythonExecutor::with_timeout(30);
        let code = r#"
exec("print('hello')")
"#;

        let result = executor.execute(code).await;
        assert!(result.is_err(), "Should return error for exec()");

        match result {
            Err(ExecutorError::SecurityViolation { message, line }) => {
                assert!(message.contains("exec"));
                assert!(line.is_some());
            }
            _ => panic!("Expected SecurityViolation error"),
        }
    }

    #[tokio::test]
    async fn test_eval_blocked_as_critical() {
        let executor = PythonExecutor::with_timeout(30);
        let code = r#"
x = eval("1 + 1")
"#;

        let result = executor.execute(code).await;
        assert!(result.is_err(), "Should return error for eval()");

        match result {
            Err(ExecutorError::SecurityViolation { message, .. }) => {
                assert!(message.contains("eval"));
            }
            _ => panic!("Expected SecurityViolation error"),
        }
    }

    #[tokio::test]
    async fn test_os_system_blocked() {
        let executor = PythonExecutor::with_timeout(30);
        let code = r#"
import os
os.system("ls")
"#;

        let result = executor.execute(code).await;
        assert!(result.is_err(), "Should return error for os.system()");

        match result {
            Err(ExecutorError::SecurityViolation { message, .. }) => {
                assert!(message.contains("os.system"));
            }
            _ => panic!("Expected SecurityViolation error"),
        }
    }

    #[tokio::test]
    async fn test_warning_violation_does_not_block() {
        let executor = PythonExecutor::with_timeout(30);
        let code = r#"
import subprocess
subprocess.run(["echo", "test"])
"#;

        let result = executor.execute(code).await;
        // subprocess.run is WARNING level, should not block
        assert!(result.is_ok(), "WARNING severity should not block execution");
    }
}

// ============================================================================
// BATCH EXECUTION WITH SIMILARITY CHECKING
// ============================================================================

use data_fabrication_core::{normalize_ast, compare_structures, PlagiarismReport, check_plagiarism};

/// Extended execution result with similarity information
#[derive(Debug, Clone)]
pub struct BatchExecutionResult {
    /// Individual execution results
    pub results: Vec<ExecutionResult>,
    /// Plagiarism analysis report
    pub plagiarism_report: Option<String>,
    /// Similarity scores between all pairs
    pub similarity_scores: Vec<(usize, usize, u8)>,
}

impl PythonExecutor {
    /// Execute multiple harnesses and check for plagiarism
    pub async fn execute_batch(&self, harnesses: &[&str]) -> ExecutorResult<BatchExecutionResult> {
        let mut results = Vec::new();
        
        // Execute each harness
        for (i, code) in harnesses.iter().enumerate() {
            info!("Executing harness {} of {}", i + 1, harnesses.len());
            match self.execute(code).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!("Harness {} failed: {}", i, e);
                    return Err(e);
                }
            }
        }
        
        // Check for plagiarism
        let plagiarism_report = match check_plagiarism(harnesses) {
            Ok(report) => Some(format!("{}", report)),
            Err(e) => {
                warn!("Plagiarism check failed: {}", e);
                None
            }
        };
        
        // Calculate similarity scores
        let similarity_scores = self.calculate_all_similarities(harnesses);
        
        Ok(BatchExecutionResult {
            results,
            plagiarism_report,
            similarity_scores,
        })
    }
    
    /// Calculate similarity between all pairs of harnesses
    fn calculate_all_similarities(&self, harnesses: &[&str]) -> Vec<(usize, usize, u8)> {
        let mut scores = Vec::new();
        let normalized: Vec<_> = harnesses.iter()
            .filter_map(|h| normalize_ast(h).ok())
            .collect();
        
        for i in 0..normalized.len() {
            for j in (i + 1)..normalized.len() {
                let score = compare_structures(&normalized[i], &normalized[j]);
                scores.push((i, j, score.value()));
            }
        }
        
        scores
    }
    
    /// Check plagiarism between two specific harnesses
    pub fn check_similarity(&self, code_a: &str, code_b: &str) -> u8 {
        match (normalize_ast(code_a), normalize_ast(code_b)) {
            (Ok(ast_a), Ok(ast_b)) => compare_structures(&ast_a, &ast_b).value(),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod similarity_tests {
    use super::*;

    #[test]
    fn test_similarity_identical() {
        let executor = PythonExecutor::new();
        let code = "x = 1\ny = 2";
        let score = executor.check_similarity(code, code);
        assert_eq!(score, 100);
    }

    #[test]
    fn test_similarity_different() {
        let executor = PythonExecutor::new();
        let code_a = "x = 1";
        let code_b = "def foo(): return 42";
        let score = executor.check_similarity(code_a, code_b);
        assert!(score < 50);
    }

    #[test]
    fn test_similarity_renamed_vars() {
        let executor = PythonExecutor::new();
        let code_a = "x = 1\ny = x + 2";
        let code_b = "a = 1\nb = a + 2";
        let score = executor.check_similarity(code_a, code_b);
        assert_eq!(score, 100);
    }
}
