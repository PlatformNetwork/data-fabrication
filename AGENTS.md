# AGENTS.md — Data Fabrication

## Project Purpose

Data Fabrication is a WASM evaluation module for AI agents on the Bittensor network via platform. Miners submit Python harnesses that generate conversation datasets. The WASM module runs inside platform validators to validate submissions, evaluate generated datasets, and compute scores. A companion native CLI (`data-cli`) provides a TUI for monitoring leaderboards, evaluation progress, and network health. A native server library (`server/`) implements the `ServerChallenge` trait for running challenge logic outside the WASM sandbox.

## Architecture Overview

```
data-fabrication/
├── Cargo.toml          # workspace with members = [".", "core", "wasm", "server", "cli", "executor"]
├── src/
│   └── lib.rs                  # Root library crate entry point
├── wasm/
│   ├── Cargo.toml      # cdylib, depends on platform-challenge-sdk-wasm
│   └── src/
│       ├── lib.rs              # Challenge impl + register_challenge!
│       └── types.rs            # Submission, ChallengeParams, etc.
├── core/
│   ├── Cargo.toml      # Shared types and traits (feature-gated: std, alloc)
│   └── src/
│       ├── lib.rs              # Root module with conditional exports
│       ├── schema.rs           # JSONL parsing, conversation schema
│       ├── config.rs           # EvaluationConfig, HarnessExecutionConfig
│       ├── scoring_types.rs    # DatasetScore, ConversationScore, LlmEvaluationScore
│       ├── error.rs            # DataFabricationError (std only)
│       ├── sandbox.rs          # Python execution sandbox (std only)
│       ├── ast_validation.rs   # Python AST validation (std only)
│       ├── ast_similarity.rs   # AST structural comparison (std only)
│       ├── llm_client.rs       # LLM client trait + implementations (std only)
│       ├── consensus.rs        # P2P consensus helpers (std only)
│       ├── cache.rs            # Evaluation result cache (std only)
│       └── resource_limits.rs  # Unix resource limits (std + unix only)
├── server/
│   ├── Cargo.toml      # lib + bin, depends on platform-challenge-sdk (server mode)
│   └── src/
│       ├── lib.rs              # ServerChallenge implementation
│       ├── main.rs             # Binary entry point
│       └── types.rs            # Server-specific types
├── executor/
│   ├── Cargo.toml      # Native binary for harness execution
│   └── src/
│       ├── lib.rs              # Re-exports executor and LLM inference
│       ├── executor.rs         # PythonExecutor, ExecutionResult
│       ├── llm_inference.rs    # LLM inference with retry, plagiarism detection
│       └── error.rs            # ExecutorError, ExecutorResult
└── cli/
    ├── Cargo.toml      # Native binary, ratatui TUI
    └── src/
        ├── main.rs     # Entry point, event loop
        ├── lib.rs      # Library exports
        └── ui.rs       # Ratatui UI rendering
```

### Package Relationships

| Package | Depends On | Purpose |
|---------|------------|---------|
| `wasm` | `core` (no-default-features) | WASM module for validators |
| `server` | `core` (std), `platform-challenge-sdk` | Native server mode |
| `executor` | `core` (std) | Native harness execution engine |
| `cli` | — (standalone) | TUI monitoring tool |

### Core Feature Flags

| Flag | Description | Dependencies |
|------|-------------|--------------|
| `default` | `["alloc"]` | — |
| `alloc` | No-std alloc support | — |
| `std` | Full std support | `thiserror`, `serde/std`, `serde_json/std`, `rustpython-parser`, `tempfile`, `log` |
| `http-client` | HTTP LLM client | `reqwest`, `tokio` |

## Build Commands

```bash
# Build WASM module (for validators)
cargo build --target wasm32-unknown-unknown -p data-fabrication-wasm

# Build WASM module (release)
cargo build --release --target wasm32-unknown-unknown -p data-fabrication-wasm

# Build native release (all packages)
cargo build --release

# Build specific package
cargo build --release -p data-fabrication-core
cargo build --release -p data-fabrication-server
cargo build --release -p data-executor
cargo build --release -p data-cli

# Check without building
cargo check -p data-fabrication-wasm
cargo check -p data-fabrication-core --features std
```

## Test Commands

```bash
# Run all tests (workspace, excluding WASM)
cargo test --workspace --exclude data-fabrication-wasm

# Run tests for specific package
cargo test -p data-fabrication-core
cargo test -p data-executor

# Run specific test
cargo test -p data-fabrication-core test_harness_submission_serialization

# Run tests with all features
cargo test -p data-fabrication-core --all-features
```

## Key Types

### Submission Types (core/src/lib.rs)

```rust
/// Miner's submission with Python harness
pub struct HarnessSubmission {
    pub hotkey: String,
    pub epoch: u64,
    pub code_hash: String,
    pub package: Vec<u8>,
}

/// Generated dataset from harness execution
pub struct GeneratedDataset {
    pub conversations: Vec<ConversationEntry>,
    pub metadata: DatasetMetadata,
    pub generation_time_ms: u64,
}
```

### Scoring Types (core/src/scoring_types.rs)

- `DatasetScore` — Overall dataset quality score
- `ConversationScore` — Per-conversation quality metrics
- `LlmEvaluationScore` — LLM-based evaluation results
- `CriteriaScores` — Multi-criteria scoring breakdown

### Sandbox Configuration (core/src/config.rs)

```rust
pub struct EvaluationConfig {
    pub memory_limit_bytes: u64,
    pub timeout_seconds: u64,
    pub max_conversation_count: u64,
    pub max_dataset_size_bytes: u64,
}

pub struct HarnessExecutionConfig {
    pub python_path: String,
    pub working_directory: String,
    pub env_vars: HashMap<String, String>,
}
```

## Development Workflow

### Adding a New Module to Core

1. Create the module file in `core/src/`
2. Add module declaration in `core/src/lib.rs`:
   - Without std: `pub mod my_module;`
   - With std only: `#[cfg(feature = "std")] pub mod my_module;`
   - With std + unix: `#[cfg(all(feature = "std", unix))] pub mod my_module;`
3. Add any new dependencies to `core/Cargo.toml` with appropriate feature gates

### WASM Module Development

The WASM module (`wasm/src/lib.rs`) must remain `#![no_std]` compatible:

- Use `alloc::` collections: `alloc::vec::Vec`, `alloc::string::String`
- Use `bincode` with `default-features = false`
- Dependencies must also be `no_std` compatible
- Cannot access filesystem, network, or threads directly
- Use host functions for external I/O (provided by platform-challenge-sdk-wasm)

### Testing Strategy

| Test Type | Location | Command |
|-----------|----------|---------|
| Unit tests | `core/src/*.rs` | `cargo test -p data-fabrication-core` |
| Integration tests | `core/tests/*.rs` | `cargo test -p data-fabrication-core` |
| Executor tests | `executor/tests/*.rs` | `cargo test -p data-executor` |
| CLI tests | `cli/tests/*.rs` | `cargo test -p data-cli` |

### Linting and Formatting

```bash
# Format all code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check

# Run clippy
cargo clippy --workspace --exclude data-fabrication-wasm -- -D warnings

# WASM clippy (requires target)
cargo clippy -p data-fabrication-wasm --target wasm32-unknown-unknown -- -D warnings
```

## CLI

The `data-cli` crate is a native binary that provides a terminal user interface for monitoring the data-fabrication network.

### Design

- **Framework**: Built with [ratatui](https://ratatui.rs/) for TUI rendering
- **Transport**: Connects to validators via HTTP JSON-RPC
- **Target**: Standard `x86_64` / `aarch64` native targets (not WASM)

### Keyboard Shortcuts

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Switch between tabs |
| `r` | Refresh data |
| `q` | Quit |

## CRITICAL RULES

1. **No `std` in WASM code.** The `wasm/` module compiles with `#![no_std]`. Use `alloc::` equivalents.
2. **Core uses feature gates.** All std-dependent code checks `#[cfg(feature = "std")]`.
3. **WASM uses `no-default-features`.** When depending on core in WASM, use `default-features = false, features = ["alloc"]`.
4. **No `.unwrap()` or `.expect()` in library paths.** Use pattern matching or `unwrap_or_default()`.
5. **Keep WASM minimal.** Do not add heavy dependencies to the WASM crate.
6. **Do NOT break the WASM ABI.** The `register_challenge!` macro and Challenge trait must remain compatible.

> **Note:** The `cli/`, `executor/`, and `server/` crates are native code exempt from the `no_std` rules. They use full `std` features.

## DO / DO NOT

### DO
- Use `alloc::` types in WASM code (`alloc::vec::Vec`, `alloc::string::String`)
- Use `serde` with `default-features = false, features = ["derive", "alloc"]` in WASM
- Use `bincode` with `default-features = false` for WASM serialization
- Feature-gate std-dependent imports in core with `#[cfg(feature = "std")]`
- Keep the `register_challenge!` macro ABI intact

### DO NOT
- Do NOT use `std::`, `println!`, `std::collections::HashMap` in WASM code
- Do NOT add heavy dependencies to the WASM crate
- Do NOT break the Challenge trait interface
- Do NOT use `#[allow(dead_code)]` broadly — fix or remove unused code

## Executor

The `data-executor` crate handles native harness execution and LLM inference:

### Components

| Module | Purpose |
|--------|---------|
| `executor.rs` | Python execution in sandboxed environment |
| `llm_inference.rs` | LLM API calls with retry logic, plagiarism detection |
| `error.rs` | Error types for execution failures |

### Key Features

- **Sandboxed execution**: Python harness runs with resource limits
- **LLM inference**: Quality evaluation via external LLM APIs
- **Plagiarism detection**: AST-based similarity comparison
- **Retry logic**: Automatic retry on transient failures

### Build Commands

```bash
# Build executor
cargo build -p data-executor

# Build executor (release)
cargo build --release -p data-executor

# Run executor tests
cargo test -p data-executor
```

## Server

The `data-fabrication-server` crate implements `ServerChallenge` trait:

### Key Features

- Uses `platform-challenge-sdk` for server mode
- Storage via sled key-value store
- HTTP routes via axum
- Reuses `core` types with `std` feature

### Build Commands

```bash
# Build server
cargo build -p data-fabrication-server

# Run server (requires configuration)
cargo run -p data-fabrication-server
```
