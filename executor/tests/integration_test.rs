//! Integration tests for Python harness execution.
//!
//! Tests the Python execution engine end-to-end with real Python processes.

use std::fs;
use std::path::PathBuf;

use data_executor::{ExecutorError, PythonExecutor};

fn fixtures_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path
}

fn load_fixture(name: &str) -> String {
    let path = fixtures_dir().join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture '{}': {}", name, e))
}

/// Test: Execute simple.py harness and validate successful output.
#[tokio::test]
async fn test_execute_simple_harness() {
    let executor = PythonExecutor::with_timeout(30);
    let code = load_fixture("simple.py");

    let result = executor.execute(&code).await;
    assert!(result.is_ok(), "Execution should succeed");

    let result = result.unwrap();
    assert_eq!(result.exit_code, Some(0), "Exit code should be 0");
    assert!(!result.timed_out, "Should not have timed out");
    assert!(!result.stdout.is_empty(), "Should have stdout output");

    let validation = executor.validate_output(&result.stdout);
    assert!(validation.is_ok(), "Output should be valid JSONL: {:?}", validation);
}

/// Test: Run timeout.py harness and verify timeout is enforced.
#[tokio::test]
async fn test_timeout_enforcement() {
    let executor = PythonExecutor::with_timeout(3);
    let code = load_fixture("timeout.py");

    let result = executor.execute(&code).await;
    assert!(result.is_err(), "Should return error for timeout");

    match result {
        Err(ExecutorError::Timeout { seconds }) => {
            assert_eq!(seconds, 3, "Timeout should match configured duration");
        }
        _ => panic!("Expected Timeout error, got: {:?}", result),
    }
}

/// Test: Run error.py harness and verify non-zero exit code is returned.
#[tokio::test]
async fn test_error_handling() {
    let executor = PythonExecutor::with_timeout(30);
    let code = load_fixture("error.py");

    let result = executor.execute(&code).await;
    assert!(result.is_ok(), "Execution should succeed (process runs and exits)");

    let result = result.unwrap();
    assert_ne!(result.exit_code, Some(0), "Exit code should not be 0");
    assert!(!result.stderr.is_empty(), "Should have stderr output");
}

/// Test: Validate JSON-L output structure from simple.py.
#[tokio::test]
async fn test_output_validation() {
    let executor = PythonExecutor::with_timeout(30);
    let code = load_fixture("simple.py");

    let result = executor.execute(&code).await.expect("Execution should succeed");

    let lines: Vec<&str> = result.stdout.lines().collect();
    assert!(lines.len() >= 3, "Should have at least 3 conversation entries");

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => panic!("Line {} is not valid JSON: {} - content: {}", idx + 1, e, trimmed),
        };

        let obj = parsed.as_object().expect(&format!("Line {} should be a JSON object", idx + 1));
        assert!(obj.contains_key("messages"), "Line {} should have 'messages' key", idx + 1);

        let messages = obj.get("messages").expect(&format!("Line {} should have messages", idx + 1));
        let messages_arr = messages.as_array().expect(&format!("Line {} messages should be array", idx + 1));
        assert!(messages_arr.len() >= 2, "Line {} should have at least 2 messages", idx + 1);
    }
}

/// Test: Validate empty output is rejected.
#[tokio::test]
async fn test_validate_empty_output_rejected() {
    let executor = PythonExecutor::new();

    let result = executor.validate_output("");
    assert!(result.is_err(), "Empty output should be invalid");

    match result {
        Err(ExecutorError::InvalidOutput { message, .. }) => {
            assert!(message.to_lowercase().contains("empty"), "Message should mention empty");
        }
        _ => panic!("Expected InvalidOutput error"),
    }
}

/// Test: Validate malformed JSON-L is rejected.
#[tokio::test]
async fn test_validate_malformed_jsonl_rejected() {
    let executor = PythonExecutor::new();

    let malformed_output = r#"{"messages":[{"role":"user","content":"Hi"}]}
{"messages": invalid json here}
{"messages":[{"role":"user","content":"Bye"},{"role":"assistant","content":"See you"}]}"#;

    let result = executor.validate_output(malformed_output);
    assert!(result.is_err(), "Malformed JSON-L should be rejected");
}

/// Test: Multiple conversation entries are parsed correctly.
#[tokio::test]
async fn test_multiple_conversations_parsed() {
    let executor = PythonExecutor::with_timeout(30);
    let code = load_fixture("simple.py");

    let result = executor.execute(&code).await.expect("Execution should succeed");

    use data_fabrication_core::JsonlParser;
    let entries = JsonlParser::parse(&result.stdout).expect("Output should parse as JSONL");

    assert!(entries.len() >= 3, "Should parse at least 3 conversation entries");

    for entry in &entries {
        assert!(entry.messages.len() >= 2, "Each conversation should have at least 2 messages");
    }
}

/// Test: Execution result contains correct timing information.
#[tokio::test]
async fn test_execution_timing() {
    let executor = PythonExecutor::with_timeout(30);
    let code = load_fixture("simple.py");

    let result = executor.execute(&code).await.expect("Execution should succeed");

    assert!(result.duration_ms > 0, "Duration should be greater than 0");
    assert!(result.duration_ms < 30000, "Duration should be less than timeout");
}

/// Test: Harness that produces multiple lines of valid JSON-L.
#[tokio::test]
async fn test_multiline_output() {
    let executor = PythonExecutor::with_timeout(30);
    let code = r#"
import json
import sys

for i in range(5):
    entry = {
        "messages": [
            {"role": "user", "content": f"Question {i}"},
            {"role": "assistant", "content": f"Answer {i}"}
        ]
    }
    print(json.dumps(entry), flush=True)

sys.exit(0)
"#;

    let result = executor.execute(code).await.expect("Execution should succeed");
    assert_eq!(result.exit_code, Some(0));

    let lines: Vec<&str> = result.stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 5, "Should produce exactly 5 lines of JSON-L");
}
