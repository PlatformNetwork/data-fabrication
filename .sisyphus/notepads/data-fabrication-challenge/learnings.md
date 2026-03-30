# Learnings - Data Fabrication Challenge

## Overview
Adaptation du challenge term-challenge vers data-fabrication challenge.
Objectif: Mineurs soumettent des harness Python qui génèrent des datasets de conversations.

## Architecture Clés
- **Dual-mode**: WASM (no_std) + Server (std)
- **Executor**: Python execution avec sandbox (AST validation + rlimit)
- **LLM Eval**: Multi-criteria scoring (diversity, uniqueness, quality, structure)
- **Scoring**: Per-conversation score → moyenne dataset → consensus validators

## Patterns de term-challenge
- Workspace: [".", "core", "wasm", "server", "cli", "executor"]
- Dependencies: platform-challenge-sdk, platform-core, bincode, serde, sled
- Host functions: host_storage_*, host_http_*, host_llm_*, host_consensus_*
- Challenge trait: name(), version(), evaluate(), validate(), routes(), handle_route()

## Conventions
- No unwrap()/expect() in library paths
- Use bincode for WASM serialization
- Use sled for server storage
- Error types in core/src/error.rs

## Timestamps
- 2026-03-30T15:19: Plan created, session started
- 2026-03-30T15:21: Workspace initialized

## Dependency Versions (from term-challenge)
- platform-challenge-sdk: git = "https://github.com/PlatformNetwork/platform.git", rev = "8b84dde8" (for WASM)
- platform-challenge-sdk: git = "https://github.com/PlatformNetwork/platform-v2", rev = "8dd66cdd" (for server)
- bincode: "1.3"
- serde: "1.0" with derive feature
- sled: "0.34"
- sp-core: "31.0"

## WASM Crate Specifics
- crate-type = ["cdylib", "rlib"]
- platform-challenge-sdk-wasm uses different git repo than server SDK
- serde/bincode need default-features = false with alloc feature for no_std

## Workspace Structure
```
data-fabrication/
├── Cargo.toml (workspace root)
├── src/lib.rs (root crate placeholder)
├── core/
│   ├── Cargo.toml
│   └── src/lib.rs
├── wasm/
│   ├── Cargo.toml
│   └── src/lib.rs
├── server/
│   ├── Cargo.toml
│   ├── src/lib.rs
│   └── src/main.rs
├── cli/
│   ├── Cargo.toml
│   └── src/main.rs
└── executor/
    ├── Cargo.toml
    └── src/main.rs
```

## Error Types Created (2026-03-30T15:30)
- Created `core/src/error.rs` with `DataFabricationError` enum
- Error module is conditional on `std` feature (WASM doesn't need full error handling)
- Added `thiserror = { version = "2.0", optional = true }` dependency
- Variants: SchemaError, ExecutionError, SecurityViolation, LlmError, ConsensusError, ConfigError, IoError, TimeoutError, JsonError
- Implemented `From<serde_json::Error>` and `From<std::io::Error>` for ergonomic conversions
- All 40 tests pass with `--features std`
- Package compiles both with and without std feature

## Core Types Created (2026-03-30T15:28)
- Created `core/src/lib.rs` with domain types for data-fabrication
- Types defined:
  - `HarnessSubmission`: Miner's Python harness submission (hotkey, epoch, code_hash, package)
  - `GeneratedDataset`: Output from running harness (conversations, metadata, generation_time_ms)
  - `ConversationEntry`: OpenAI-style conversation format (messages, function_calls, thinking)
  - `Message`: Single message in conversation (role, content, name, function_call)
  - `FunctionCall`: Function call details (name, arguments as JSON string)
  - `DatasetMetadata`: Dataset metadata (conversation_count, total_messages, size_bytes, model, generation_params)
  - `GenerationParams`: Generation parameters (temperature, top_p, max_tokens)
- All types derive `Debug, Clone, Serialize, Deserialize`
- unit tests for JSON serialization/deserialization pass (6 tests in lib.rs)
- Used `skip_serializing_if = "Option::is_none"` for optional fields to match OpenAI format
- `extern crate alloc` needed for no_std support
- serde_json added as dev-dependency for tests

## Config Types Created (2026-03-30T15:35)
- Created `core/src/config.rs` with validated configuration types
- Types defined:
  - `HarnessExecutionConfig`: Controls harness execution (seed, conversation_count 10-50, timeout max 7200s, dataset size max 100MB, memory limit default 2GB)
  - `EvaluationConfig`: LLM evaluation settings (model, endpoint, retries, delay)
  - `ConfigError`: Validation error enum with descriptive variants
- Constants exposed: MAX_TIMEOUT_SECONDS, MIN/MAX_CONVERSATION_COUNT, MAX_DATASET_SIZE_BYTES, DEFAULT_MEMORY_LIMIT_BYTES
- Validation returns descriptive errors with actual/expected values
- Both configs have `Default` impl with sensible values
- Added tests for: valid config, boundary values, all validation failures, serialization
- ScoreError in scoring_types.rs cannot derive Eq (f64 doesn't implement Eq due to NaN) - fixed by removing Eq derive
- Pre-existing test failure in schema::tests::test_jsonl_invalid_json (unrelated to config)

## Scoring Types (2026-03-30T15:30)
- ScoreError cannot derive Eq because f64 fields don't implement Eq (floats have NaN)
- Use PartialEq only for types containing f64
- Weighted average uses equal weights: 0.25 each for 4 criteria
- DatasetScore::new returns Option to handle empty scores vec

## JSON-L Schema Validation (Task 3)
- Schema types reuse existing `ConversationEntry` and `Message` from lib.rs
- SchemaError enum with line numbers for debugging
- JsonlParser parses line-by-line, skips empty lines
- Minimum 2 messages per conversation (1 turn = 1 exchange)
- Tests cover: valid JSON-L, invalid JSON, missing field, empty messages
- `serde_json` added with `alloc` feature for no_std support

## Resource Limits (Task 6) - 2026-03-30
- Created `core/src/resource_limits.rs` for Unix sandbox constraints
- Types defined:
  - `ResourceLimits`: Struct with cpu_time_seconds (7200s max), memory_bytes (4GB max), max_processes (min 1), max_file_size (200MB max)
  - `ResourceLimitError`: Validation error enum with descriptive variants
- Constants exposed: MAX_CPU_TIME_SECONDS, MAX_MEMORY_BYTES, MAX_PROCESSES, MAX_FILE_SIZE_BYTES, DEFAULT_MEMORY_BYTES, DEFAULT_FILE_SIZE_BYTES
- `rlimit` dependency added under `[target.'cfg(unix)'.dependencies]` in Cargo.toml
- `to_rlimit()` method converts to `Vec<(rlimit::Resource, u64)>` for Unix syscall mapping
- `apply()` method sets rlimits on current process (Unix-only)
- Module is conditional on `#[cfg(all(feature = "std", unix))]`
- `rlimit::Resource` variants used: CPU, AS (address space), NPROC, FSIZE
- LSP false positives on `write!` macro returns (rust-analyzer cfg(unix) limitation)
- Tests verified via standalone compilation (7 tests pass)
- Pre-existing errors in ast_validation.rs block cargo test for whole package

## Python Execution Engine (Task 12) - 2026-03-30
- Created `executor/src/error.rs` with `ExecutorError` enum:
  - ProcessSpawn, Timeout, MemoryExceeded, InvalidOutput, SandboxViolation, IoError variants
  - Implements `Display`, `Error`, and `From<std::io::Error>`
- Created `executor/src/executor.rs` with `PythonExecutor`:
  - Uses `tokio::process::Command` for async process spawning
  - Applies timeout via `tokio::time::timeout` wrapper
  - Captures stdout/stderr with 10MB max limit to prevent memory exhaustion
  - Uses `data_fabrication_core::Sandbox` for isolation
  - Validates output using `JsonlParser`
- Created `executor/src/lib.rs` with module exports
- Updated `executor/src/main.rs` as entry point
- Added `data-fabrication-core = { path = "../core", features = ["std"] }` dependency
- 19 tests pass including:
  - `test_execute_simple_harness` — runs valid Python and validates JSONL output
  - `test_timeout_kills_process` — runs infinite loop, verifies timeout error
  - `test_invalid_python_fails` — runs syntax error, verifies non-zero exit code
- Timeout capped at 7200s (2 hours max per config)
- Output truncated at 10MB to prevent memory exhaustion

## LLM Client Wrapper (Task 18) - 2026-03-30
- Created `core/src/llm_client.rs` with dual-mode LLM client support
- `LlmClient` trait with async `evaluate_conversation()` method using RPITIT (return position impl trait in trait)
- Implementations:
  - `HttpLlmClient` (cfg: http-client): Uses reqwest for HTTP calls with retry logic
  - `MockLlmClient`: For testing with predefined responses
  - `WasmLlmClient`: Stub that returns error (no host function access in dev env)
- Retry logic with exponential backoff: initial delay * 2^attempt
- JSON response parsing into `LlmEvaluationScore` with criteria validation
- `reqwest` dependency added as optional with `rustls-tls` feature
- `tokio` added as optional (for http-client) and as dev-dependency (for tests)
- Feature flag `http-client` enables both reqwest and tokio
- Tests use tokio runtime explicitly since trait uses async
- 10 new tests added: mock client tests, JSON parsing tests, rate limit detection
- All 96 tests pass with `--features std`
- Build succeeds with `--features std,http-client`

## AST Validation Security Gap Fix (2026-03-30)
- Fixed critical security gap: pattern constants (SHELL_PATTERNS_CRITICAL, etc.) were defined but never used
- Added `Expr::Attribute` handling in `check_dangerous_call()` for `os.system()`, `subprocess.run()`, etc.
- Added `check_getattr_bypass()` function to detect `getattr(__builtins__, 'exec')` and `getattr(os, 'system')` bypass attempts
- Key implementation details:
  - `extract_module_attr()` uses recursion to handle chained attributes like `urllib.request.urlopen`
  - `check_getattr_bypass()` only flags static string arguments (dynamic calls are not analyzed)
  - Pattern constants are wired with correct severity: Critical (os.system, os.popen), Warning (subprocess, socket), Info (http clients)
- All 30 ast_validation tests pass
- Pre-existing sandbox memory allocation issue (unrelated to this fix)
