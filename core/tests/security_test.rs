//! Integration tests for security-related modules.
//!
//! Tests AST validation, pattern detection, sandbox isolation, and resource limits.

use data_fabrication_core::ast_validation::{validate_python_code, SecurityViolation, Severity};
#[cfg(unix)]
use data_fabrication_core::resource_limits::{
    ResourceLimitError, ResourceLimits, MAX_CPU_TIME_SECONDS, MAX_FILE_SIZE_BYTES, MAX_MEMORY_BYTES,
};
use data_fabrication_core::sandbox::{Sandbox, SandboxConfig, HARNESS_FILENAME, OUTPUT_FILENAME};
use tempfile::TempDir;

// ============================================================================
// AST Validation Tests - Blocking exec/eval
// ============================================================================

#[test]
fn test_ast_blocks_exec_call() {
    let code = r#"
exec("import os; os.system('ls')")
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected exactly 1 violation for exec call"
    );

    let violation = &violations[0];
    assert_eq!(violation.pattern, "exec");
    assert_eq!(violation.severity, Severity::Critical);
    assert!(
        violation.line.is_some(),
        "Line number should be present for exec call"
    );
}

#[test]
fn test_ast_blocks_eval_call() {
    let code = r#"
result = eval("1 + 1")
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected exactly 1 violation for eval call"
    );

    let violation = &violations[0];
    assert_eq!(violation.pattern, "eval");
    assert_eq!(violation.severity, Severity::Critical);
}

#[test]
fn test_ast_blocks_import_builtin() {
    let code = r#"
mod = __import__("os")
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected exactly 1 violation for __import__"
    );

    let violation = &violations[0];
    assert_eq!(violation.pattern, "__import__");
    assert_eq!(violation.severity, Severity::Critical);
}

#[test]
fn test_ast_blocks_exec_in_nested_function() {
    let code = r#"
def outer():
    def inner():
        exec("x = 1")
    return inner
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(violations.len(), 1, "Should detect exec in nested function");

    let violation = &violations[0];
    assert_eq!(violation.pattern, "exec");
    assert_eq!(violation.severity, Severity::Critical);
}

// ============================================================================
// Pattern Detection Tests - os.system, subprocess.run, socket.socket
// ============================================================================

#[test]
fn test_pattern_detects_os_system() {
    let code = r#"
import os
os.system("rm -rf /")
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected exactly 1 violation for os.system"
    );

    let violation = &violations[0];
    assert_eq!(violation.pattern, "os.system");
    assert_eq!(violation.severity, Severity::Critical);
}

#[test]
fn test_pattern_detects_os_popen() {
    let code = r#"
import os
os.popen("cat /etc/passwd")
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected exactly 1 violation for os.popen"
    );

    let violation = &violations[0];
    assert_eq!(violation.pattern, "os.popen");
    assert_eq!(violation.severity, Severity::Critical);
}

#[test]
fn test_pattern_detects_subprocess_run() {
    let code = r#"
import subprocess
subprocess.run(["ls", "-la"])
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected exactly 1 violation for subprocess.run"
    );

    let violation = &violations[0];
    assert_eq!(violation.pattern, "subprocess.run");
    assert_eq!(violation.severity, Severity::Warning);
}

#[test]
fn test_pattern_detects_subprocess_call() {
    let code = r#"
import subprocess
subprocess.call(["echo", "test"])
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected exactly 1 violation for subprocess.call"
    );

    let violation = &violations[0];
    assert_eq!(violation.pattern, "subprocess.call");
    assert_eq!(violation.severity, Severity::Warning);
}

#[test]
fn test_pattern_detects_subprocess_popen() {
    let code = r#"
import subprocess
subprocess.Popen(["ls"])
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected exactly 1 violation for subprocess.Popen"
    );

    let violation = &violations[0];
    assert_eq!(violation.pattern, "subprocess.Popen");
    assert_eq!(violation.severity, Severity::Warning);
}

#[test]
fn test_pattern_detects_socket_socket() {
    let code = r#"
import socket
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected exactly 1 violation for socket.socket"
    );

    let violation = &violations[0];
    assert_eq!(violation.pattern, "socket.socket");
    assert_eq!(violation.severity, Severity::Warning);
}

#[test]
fn test_pattern_detects_all_subprocess_methods() {
    let code = r#"
import subprocess
subprocess.run(["ls"])
subprocess.call(["ls"])
subprocess.Popen(["ls"])
subprocess.check_output(["ls"])
subprocess.check_call(["ls"])
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        5,
        "Expected 5 violations for all subprocess methods"
    );

    for violation in &violations {
        assert_eq!(violation.severity, Severity::Warning);
        assert!(violation.pattern.starts_with("subprocess."));
    }
}

// ============================================================================
// getattr Bypass Detection Tests
// ============================================================================

#[test]
fn test_getattr_bypass_builtins_exec() {
    let code = r#"
fn = getattr(__builtins__, 'exec')
fn("print('pwned')")
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected 1 violation for getattr bypass"
    );

    let violation = &violations[0];
    assert!(violation.pattern.contains("getattr"));
    assert!(violation.pattern.contains("exec"));
    assert_eq!(violation.severity, Severity::Critical);
}

#[test]
fn test_getattr_bypass_builtins_eval() {
    let code = r#"
fn = getattr(__builtins__, 'eval')
fn("1 + 1")
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected 1 violation for getattr bypass"
    );

    let violation = &violations[0];
    assert!(violation.pattern.contains("getattr"));
    assert!(violation.pattern.contains("eval"));
    assert_eq!(violation.severity, Severity::Critical);
}

#[test]
fn test_getattr_bypass_os_system() {
    let code = r#"
import os
fn = getattr(os, 'system')
fn("ls")
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected 1 violation for getattr bypass"
    );

    let violation = &violations[0];
    assert!(violation.pattern.contains("getattr"));
    assert!(violation.pattern.contains("os"));
    assert!(violation.pattern.contains("system"));
    assert_eq!(violation.severity, Severity::Critical);
}

#[test]
fn test_getattr_bypass_subprocess_run() {
    let code = r#"
import subprocess
fn = getattr(subprocess, 'run')
fn(["ls"])
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        1,
        "Expected 1 violation for getattr bypass"
    );

    let violation = &violations[0];
    assert!(violation.pattern.contains("getattr"));
    assert!(violation.pattern.contains("subprocess"));
    assert!(violation.pattern.contains("run"));
    assert_eq!(violation.severity, Severity::Warning);
}

#[test]
fn test_getattr_dynamic_not_flagged() {
    let code = r#"
import os
attr_name = "something_safe"
fn = getattr(os, attr_name)
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert!(
        violations.is_empty(),
        "Dynamic getattr should not be flagged as a bypass"
    );
}

// ============================================================================
// Sandbox Isolation Tests
// ============================================================================

#[test]
fn test_sandbox_creates_temp_directory() {
    let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");
    let path = sandbox.workdir_path();
    assert!(
        path.exists(),
        "Working directory should exist after creation"
    );
    assert!(path.is_dir(), "Working directory should be a directory");
}

#[test]
fn test_sandbox_cleans_up_on_drop() {
    let path = {
        let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");
        sandbox.workdir_path()
    };
    // Sandbox is dropped here
    assert!(
        !path.exists(),
        "Working directory should be cleaned up after drop"
    );
}

#[test]
fn test_sandbox_isolation_multiple_instances() {
    let sandbox1 = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox1");
    let sandbox2 = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox2");

    let path1 = sandbox1.workdir_path();
    let path2 = sandbox2.workdir_path();

    assert_ne!(
        path1, path2,
        "Each sandbox should have its own isolated directory"
    );

    // Both should exist at the same time
    assert!(path1.exists());
    assert!(path2.exists());
}

#[test]
fn test_sandbox_custom_working_directory() {
    let parent_dir = TempDir::new().expect("Failed to create parent temp dir");
    let parent_path = parent_dir.path().to_path_buf();

    let config = SandboxConfig {
        #[cfg(unix)]
        limits: data_fabrication_core::resource_limits::ResourceLimits::default(),
        working_directory: Some(parent_path.clone()),
    };

    let sandbox = Sandbox::new(config).expect("Failed to create sandbox");
    let sandbox_path = sandbox.workdir_path();

    assert!(
        sandbox_path.starts_with(&parent_path),
        "Sandbox directory should be under custom working directory"
    );
    assert!(sandbox_path.exists(), "Sandbox directory should exist");
}

#[test]
fn test_sandbox_write_and_read_file() {
    let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");

    let test_content = b"Hello, Sandbox!";
    let file_path = sandbox
        .create_file("test.txt", test_content)
        .expect("Failed to create test file");

    assert!(file_path.exists(), "Created file should exist");

    let read_content = sandbox
        .read_file("test.txt")
        .expect("Failed to read test file");
    assert_eq!(read_content, test_content, "File content should match");
}

#[test]
fn test_sandbox_output_path() {
    let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");
    let expected_output = sandbox.workdir_path().join(OUTPUT_FILENAME);
    assert_eq!(
        sandbox.output_path(),
        expected_output,
        "Output path should be correct"
    );
}

#[test]
fn test_sandbox_harness_path() {
    let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");
    let expected_harness = sandbox.workdir_path().join(HARNESS_FILENAME);
    assert_eq!(
        sandbox.harness_path(),
        expected_harness,
        "Harness path should be correct"
    );
}

#[test]
fn test_sandbox_write_harness() {
    let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");

    let harness_code = b"print('Hello, World!')";
    let harness_path = sandbox
        .write_harness(harness_code)
        .expect("Failed to write harness");

    assert!(harness_path.exists(), "Harness file should exist");
    assert_eq!(harness_path, sandbox.harness_path());

    let content = std::fs::read(&harness_path).expect("Failed to read harness");
    assert_eq!(content, harness_code, "Harness content should match");
}

#[test]
fn test_sandbox_has_output_detection() {
    let sandbox = Sandbox::new(SandboxConfig::default()).expect("Failed to create sandbox");

    assert!(
        !sandbox.has_output(),
        "Should not have output file initially"
    );

    sandbox
        .create_file(OUTPUT_FILENAME, b"{}")
        .expect("Failed to create output file");

    assert!(
        sandbox.has_output(),
        "Should detect output file after creation"
    );
}

// ============================================================================
// Resource Limits Tests (Unix only)
// ============================================================================

#[test]
#[cfg(unix)]
fn test_resource_limits_validation_passes() {
    let limits = ResourceLimits::default();
    assert!(limits.validate().is_ok(), "Default limits should be valid");
}

#[test]
#[cfg(unix)]
fn test_resource_limits_custom_valid() {
    let limits = ResourceLimits::new(3600, 1_000_000_000, 2, 50_000_000);
    assert!(limits.is_ok(), "Valid custom limits should be accepted");

    let limits = limits.expect("Should have valid limits");
    assert_eq!(limits.cpu_time_seconds, 3600);
    assert_eq!(limits.memory_bytes, 1_000_000_000);
    assert_eq!(limits.max_processes, 2);
    assert_eq!(limits.max_file_size, 50_000_000);
}

#[test]
#[cfg(unix)]
fn test_resource_limits_cpu_time_exceeds_max() {
    let limits = ResourceLimits {
        cpu_time_seconds: 8000,
        ..Default::default()
    };

    let result = limits.validate();
    assert!(result.is_err(), "CPU time above max should fail validation");

    match result {
        Err(ResourceLimitError::CpuTimeTooHigh { actual, maximum }) => {
            assert_eq!(actual, 8000);
            assert_eq!(maximum, MAX_CPU_TIME_SECONDS);
        }
        _ => panic!("Expected CpuTimeTooHigh error"),
    }
}

#[test]
#[cfg(unix)]
fn test_resource_limits_memory_exceeds_max() {
    let limits = ResourceLimits {
        memory_bytes: 5_000_000_000,
        ..Default::default()
    };

    let result = limits.validate();
    assert!(result.is_err(), "Memory above max should fail validation");

    match result {
        Err(ResourceLimitError::MemoryTooHigh { actual, maximum }) => {
            assert_eq!(actual, 5_000_000_000);
            assert_eq!(maximum, MAX_MEMORY_BYTES);
        }
        _ => panic!("Expected MemoryTooHigh error"),
    }
}

#[test]
#[cfg(unix)]
fn test_resource_limits_processes_zero() {
    let limits = ResourceLimits {
        max_processes: 0,
        ..Default::default()
    };

    let result = limits.validate();
    assert!(result.is_err(), "Zero processes should fail validation");

    match result {
        Err(ResourceLimitError::ProcessesTooLow { actual, minimum }) => {
            assert_eq!(actual, 0);
            assert_eq!(minimum, 1);
        }
        _ => panic!("Expected ProcessesTooLow error"),
    }
}

#[test]
#[cfg(unix)]
fn test_resource_limits_file_size_exceeds_max() {
    let limits = ResourceLimits {
        max_file_size: 300_000_000,
        ..Default::default()
    };

    let result = limits.validate();
    assert!(
        result.is_err(),
        "File size above max should fail validation"
    );

    match result {
        Err(ResourceLimitError::FileSizeTooHigh { actual, maximum }) => {
            assert_eq!(actual, 300_000_000);
            assert_eq!(maximum, MAX_FILE_SIZE_BYTES);
        }
        _ => panic!("Expected FileSizeTooHigh error"),
    }
}

#[test]
#[cfg(unix)]
fn test_resource_limits_boundary_values() {
    // Test maximum allowed values
    let limits = ResourceLimits {
        cpu_time_seconds: MAX_CPU_TIME_SECONDS,
        memory_bytes: MAX_MEMORY_BYTES,
        max_processes: 1,
        max_file_size: MAX_FILE_SIZE_BYTES,
    };

    assert!(limits.validate().is_ok(), "Maximum values should be valid");
}

#[test]
#[cfg(unix)]
fn test_resource_limits_zero_values_allowed() {
    // CPU time, memory, and file size of 0 are allowed (unlimited)
    let limits = ResourceLimits {
        cpu_time_seconds: 0,
        memory_bytes: 0,
        max_file_size: 0,
        max_processes: 1, // Must be at least 1
    };

    assert!(
        limits.validate().is_ok(),
        "Zero values (except processes) should be valid"
    );
}

#[test]
#[cfg(unix)]
fn test_resource_limits_apply_to_sandbox() {
    let config = SandboxConfig {
        limits: ResourceLimits {
            cpu_time_seconds: 300,
            memory_bytes: 512 * 1024 * 1024,
            max_processes: 16,
            max_file_size: 10_000_000,
        },
        working_directory: None,
    };

    let sandbox = Sandbox::new(config).expect("Failed to create sandbox with custom limits");

    // Access limits through sandbox
    assert_eq!(sandbox.limits().cpu_time_seconds, 300);
    assert_eq!(sandbox.limits().max_processes, 16);
}

// ============================================================================
// Severity and Display Tests
// ============================================================================

#[test]
fn test_security_violation_display_with_location() {
    let violation = SecurityViolation::new("exec", Severity::Critical, Some(10), Some(5));
    let display = format!("{}", violation);
    assert!(display.contains("critical"));
    assert!(display.contains("line 10"));
    assert!(display.contains("col 5"));
    assert!(display.contains("exec"));
}

#[test]
fn test_security_violation_display_without_column() {
    let violation = SecurityViolation::new("eval", Severity::Warning, Some(20), None);
    let display = format!("{}", violation);
    assert!(display.contains("warning"));
    assert!(display.contains("line 20"));
    assert!(display.contains("eval"));
}

#[test]
fn test_severity_display() {
    assert_eq!(format!("{}", Severity::Critical), "critical");
    assert_eq!(format!("{}", Severity::Warning), "warning");
    assert_eq!(format!("{}", Severity::Info), "info");
}

// ============================================================================
// Combined Pattern Tests
// ============================================================================

#[test]
fn test_multiple_violations_in_single_code() {
    let code = r#"
import os
import subprocess
import socket

exec("x = 1")
eval("y = 2")
os.system("ls")
subprocess.run(["ls"])
socket.socket()
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert_eq!(
        violations.len(),
        5,
        "Expected 5 violations: exec, eval, os.system, subprocess.run, socket.socket"
    );

    let patterns: Vec<&str> = violations.iter().map(|v| v.pattern.as_str()).collect();
    assert!(patterns.contains(&"exec"), "Should detect exec");
    assert!(patterns.contains(&"eval"), "Should detect eval");
    assert!(patterns.contains(&"os.system"), "Should detect os.system");
    assert!(
        patterns.contains(&"subprocess.run"),
        "Should detect subprocess.run"
    );
    assert!(
        patterns.contains(&"socket.socket"),
        "Should detect socket.socket"
    );
}

#[test]
fn test_safe_code_has_no_violations() {
    let code = r#"
def hello_world():
    """A simple greeting function."""
    message = "Hello, World!"
    print(message)
    return message

# Simple list comprehension
squares = [x * x for x in range(10)]

# Dictionary
data = {"key": "value", "nested": [1, 2, 3]}
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert!(
        violations.is_empty(),
        "Safe code should have no violations: {:?}",
        violations
    );
}

#[test]
fn test_import_statements_allowed() {
    // Imports should be allowed per policy
    let code = r#"
import os
import subprocess
import socket
from sys import path
"#;
    let violations = validate_python_code(code).expect("Should parse valid Python");
    assert!(
        violations.is_empty(),
        "Import statements should be allowed: {:?}",
        violations
    );
}
