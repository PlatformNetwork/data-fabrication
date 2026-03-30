# Data Fabrication Challenge - Plan

## TL;DR

> **Quick Summary**: Adapt term-challenge into a new challenge where miners submit Python harnesses that generate conversation datasets, evaluated by LLM for diversity, uniqueness, and quality.
>
> **Deliverables**:
> - Complete challenge module (WASM + Server)
> - Python harness executor with sandboxing
> - LLM evaluation pipeline for dataset quality
> - CLI monitoring tool (TUI)
> - Comprehensive test suite
>
> **Estimated Effort**: Large (15-20 tasks)
> **Parallel Execution**: YES - 5-7 tasks per wave
> **Critical Path**: Core types → Executor → LLM eval → Integration

---

## Context

### Original Request
Créer un nouveau challenge "data-fabrication" basé sur term-challenge où les mineurs soumettent des harness Python qui génèrent des datasets de conversations. Les datasets sont évalués par LLM sur la diversité, l'unicité et la qualité.

### Interview Summary

**Key Discussions**:
- **Source challenge**: term-challenge (/root/term-challenge) à adapter
- **Target repo**: /root/data-fabrication (actuellement vide)
- **Miner soumet**: Harness Python 3.11+ upload direct sur executor
- **Dataset généré**: JSON-L, OpenAI-style avec function_calls et thinking
- **Volume**: 10-50 conversations, 20-100 MB max, 2h timeout
- **Types de conversations**: Chat + Code/Programming
- **Tours minimum**: 2 tours (1 échange) par conversation
- **Packages autorisés**: Tous (validation LLM du code pour anti-triche)
- **Déterminisme**: Non-déterministe accepté
- **Score**: Par conversation avec breakdown 4 critères, puis moyenne dataset
- **Consensus**: Moyenne des scores validators
- **Anti-triche**: LLM review du code harness
- **Tests**: Rust standard

**Research Findings**:
- term-challenge has dual-mode architecture (WASM + Server)
- Executor runs Python code with security sandboxing
- LLM evaluation via platform host functions
- Scoring with WTA + decay mechanism
- AST validation for Python code security

### Metis Review

**Identified Gaps** (addressed):
- **Harness interface**: Clarifié - Arguments CLI
- **Determinism**: Clarifié - Non-déterministe accepté
- **Package whitelist**: Clarifié - Tout autorisé avec LLM review
- **LLM score format**: Clarifié - JSON avec breakdown 4 critères
- **Anti-cheat**: Clarifié - LLM review du code harness
- **Consensus**: Clarifié - Moyenne des validators
- **Partial failure**: Clarifié - Score proportionnel
- **LLM costs**: Clarifié - Validators paient

**Technical Risks**:
- Python security (mitigated by AST validation + seccomp + rlimits)
- LLM API rate limits (mitigated by retry + backoff)
- Non-deterministic scoring (mitigated by consensus)
- Cost explosion (mitigated by caching + batch eval)

---

## Work Objectives

### Core Objective
Transformer term-challenge en data-fabrication challenge où l'objectif passe de "résoudre des tâches de code" à "générer des datasets de conversations de haute qualité".

### Concrete Deliverables
1. **Workspace structure**: Cargo.toml, core/, wasm/, server/, cli/, executor/, src/
2. **Core types**: HarnessSubmission, GeneratedDataset, ConversationEntry, LlmEvaluationScore
3. **WASM module**: Challenge trait implementation avec evaluate/validate
4. **Executor**: Python execution sandbox avec security hardening
5. **LLM evaluation**: Multi-criteria scoring via chutes.ai
6. **CLI**: term-cli style monitoring TUI
7. **Tests**: Unit tests, integration tests, golden datasets

### Definition of Done
- [ ] `cargo build --release --target wasm32-unknown-unknown` → compile
- [ ] `cargo test --all` → pass
- [ ] Executor peut exécuter un harness Python de test
- [ ] LLM evaluation retourne un score valide JSON
- [ ] CLI affiche le leaderboard

### Must Have
- Dual-mode architecture (WASM + Server)
- AST validation before Python execution
- LLM review of harness code
- Multi-criteria evaluation (diversity, uniqueness, quality, structure)
- Per-conversation scoring with aggregation
- Executor sandboxing (rlimit, seccomp, namespace)
- Consensus mechanism for validators
- CLI monitoring tool

### Must NOT Have (Guardrails from Metis)
- ❌ Custom DSL or mini-language for harness (use pure Python)
- ❌ Web UI or dashboard (CLI only)
- ❌ Dataset post-processing/validation beyond LLM scoring
- ❌ Complex harness dependency resolution
- ❌ Dataset filtering/deduplication tools
- ❌ Multiple evaluation models (start with single model)
- ❌ Dataset versioning beyond required storage
- ❌ Real-time progress streaming
- ❌ Fancy dataset visualization

---

## Verification Strategy

### Test Decision
- **Infrastructure exists**: NO (fresh repo)
- **Automated tests**: YES (TDD approach)
- **Framework**: Rust standard testing (cargo test)
- **TDD**: Each task follows RED → GREEN → REFACTOR

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Python execution**: Use Bash (cargo test) — Run harness, capture output, validate JSON-L
- **LLM evaluation**: Mock LLM responses in tests
- **Security validation**: Test AST validation blocks dangerous imports
- **Scoring**: Property-based tests with proptest

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Foundation - 7 tasks):
├── Task 1: Workspace initialization + Cargo.toml [quick]
├── Task 2: Core types (Submission, Dataset, Conversation) [quick]
├── Task 3: JSON-L schema validation types [quick]
├── Task 4: LLM score types (multi-criteria breakdown) [quick]
├── Task 5: Configuration types [quick]
└── Task 6: Error types + Result aliases [quick]

Wave 2 (Security Layer - 5 tasks):
├── Task 7: AST validation module (import whitelist) [deep]
├── Task 8: Dangerous pattern detection [deep]
├── Task 9: Resource limit configuration [quick]
├── Task 10: Sandbox execution helpers [deep]
└── Task 11: Security test suite [quick]

Wave 3 (Executor - 6 tasks):
├── Task 12: Python execution engine [unspecified-high]
├── Task 13: Output capture and validation [quick]
├── Task 14: Timeout enforcement [quick]
├── Task 15: Error handling and recovery [quick]
├── Task 16: Executor integration tests [unspecified-high]
└── Task 17: Executor CLI endpoint [quick]

Wave 4 (LLM Evaluation - 6 tasks):
├── Task 18: LLM client wrapper (host functions) [deep]
├── Task 19: Score parsing and validation [quick]
├── Task 20: Multi-criteria aggregation [quick]
├── Task 21: Consensus mechanism [deep]
├── Task 22: LLM evaluation tests [deep]
└── Task 23: Evaluation caching [quick]

Wave 5 (WASM Module - 5 tasks):
├── Task 24: Challenge trait implementation [ultrabrain]
├── Task 25: Route handlers [quick]
├── Task 26: Storage operations [quick]
├── Task 27: WASM integration tests [deep]
└── Task 28: Register challenge macro [quick]

Wave 6 (Server + CLI - 4 tasks):
├── Task 29: ServerChallenge implementation [unspecified-high]
├── Task 30: HTTP routes [quick]
├── Task 31: CLI TUI [visual-engineering]
└── Task 32: CLI integration tests [quick]

Wave FINAL (Verification - 4 parallel reviews):
├── Task F1: Plan compliance audit [oracle]
├── Task F2: Code quality review [unspecified-high]
├── Task F3: Security penetration test [deep]
└── Task F4: Scope fidelity check [deep]
→ Present results → Get explicit user okay

Critical Path: T1 → T2 → T7 → T12 → T18 → T24 → T29 → F1-F4 → user okay
Parallel Speedup: ~65% faster than sequential
Max Concurrent: 7 (Wave 1)
```

### Dependency Matrix (abbreviated)

- **1-6**: — — All Wave 2-6
- **7-11**: 2 — 12, 16, 24
- **12-17**: 7, 9 — 18, 24, 27
- **18-23**: 12, 20 — 24, 26, 29
- **24-28**: 2, 7, 12, 18 — 29, 30
- **29-32**: 24, 27 — F1-F4

---

## TODOs

### Wave 1: Foundation

- [x] 1. **Initialize Workspace and Cargo.toml**

  **What to do**:
  - Create Cargo.toml workspace with members: [".", "core", "wasm", "server", "cli", "executor"]
  - Add workspace dependencies: platform-challenge-sdk, platform-core, bincode, serde, sled
  - Create src/lib.rs placeholder
  - Setup workspace package metadata (version, authors, license)

  **Must NOT do**:
  - ❌ Add unnecessary dependencies
  - ❌ Configure build scripts yet (defer to WASM task)

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Simple file creation, straightforward structure
  - **Skills**: []
    - No special skills needed

  **Parallelization**:
  - **Can Run In Parallel**: NO (blocks all other tasks)
  - **Parallel Group**: Sequential (first task)
  - **Blocks**: All Wave 2-6 tasks
  - **Blocked By**: None

  **References**:
  - `/root/term-challenge/Cargo.toml` — Workspace structure template
  - Platform SDK docs — Dependency versions

  **Acceptance Criteria**:
  - [ ] Cargo.toml exists at /root/data-fabrication/Cargo.toml
  - [ ] `cargo check` passes in workspace root

  **QA Scenarios**:
  ```
  Scenario: Workspace builds successfully
    Tool: Bash
    Preconditions: Cargo.toml created
    Steps:
      1. cd /root/data-fabrication
      2. cargo check --workspace
    Expected Result: No compilation errors (warnings OK)
    Failure Indicators: "error: could not find Cargo.toml" or compilation error
    Evidence: .sisyphus/evidence/task-01-workspace-check.log

  Scenario: All members are defined
    Tool: Bash
    Steps:
      1. grep -E "members\s*=" Cargo.toml
    Expected Result: Output contains ["core", "wasm", "server", "cli", "executor"]
    Evidence: .sisyphus/evidence/task-01-members.txt
  ```

  **Commit**: YES
  - Message: `feat: initialize data-fabrication workspace`
  - Files: Cargo.toml, src/lib.rs

- [x] 2. **Create Core Types - Submission and Dataset**

  **What to do**:
  - Create core/src/lib.rs with domain types
  - Define HarnessSubmission struct (hotkey, epoch, code_hash, package)
  - Define GeneratedDataset struct (conversations, metadata, generation_time)
  - Define ConversationEntry struct (messages with function_calls, thinking)
  - Add serde derives and tests

  **Must NOT do**:
  - ❌ Add WASM-specific types here (keep in wasm/src/types.rs)
  - ❌ Include business logic (just types)

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Type definitions, straightforward serde usage
  - **Skills**: []
    - No special skills needed

  **Parallelization**:
  - **Can Run In Parallel**: YES (with tasks 3-6)
  - **Parallel Group**: Wave 1
  - **Blocks**: Tasks in Waves 2-6 that depend on types
  - **Blocked By**: Task 1

  **References**:
  - `/root/term-challenge/wasm/src/types.rs` — Submission pattern
  - OpenAI API docs — Conversation message format with function_calls

  **Acceptance Criteria**:
  - [ ] core/src/lib.rs exists with types
  - [ ] Types have proper serde derives
  - [ ] `cargo test --package data-fabrication-core` passes

  **QA Scenarios**:
  ```
  Scenario: Types serialize/deserialize correctly
    Tool: Bash
    Preconditions: Types defined
    Steps:
      1. cargo test --package data-fabrication-core test_types_serialize
    Expected Result: Test passes
    Evidence: .sisyphus/evidence/task-02-types-serialize.log

  Scenario: ConversationEntry matches OpenAI format
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_conversation_openai_format
    Expected Result: Test validates JSON schema matches OpenAI spec
    Evidence: .sisyphus/evidence/task-02-openai-format.log
  ```

  **Commit**: YES (groups with 3-6)
  - Message: `feat(core): add Submission and Dataset types`

- [x] 3. **Create JSON-L Schema Validation Types**

  **What to do**:
  - Create core/src/schema.rs for JSON-L validation
  - Define ConversationSchema with validation logic
  - Implement JSON-L line-by-line parser
  - Add error types for schema violations
  - Create tests with valid and invalid JSON-L

  **Must NOT do**:
  - ❌ Implement full OpenAI schema validation (just basic structure)
  - ❌ Add complex validation rules yet

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Schema validation helpers, standard serde usage
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 1)
  - **Blocked By**: Task 1

  **References**:
  - `/root/term-challenge/src/dataset/types.rs` — Dataset validation patterns
  - serde_json docs — Line-by-line parsing

  **Acceptance Criteria**:
  - [ ] core/src/schema.rs exists
  - [ ] Tests pass for valid JSON-L parsing
  - [ ] Tests pass for invalid JSON-L rejection

  **QA Scenarios**:
  ```
  Scenario: Valid JSON-L parses successfully
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_jsonl_valid
    Expected Result: Parses 10 conversations without error
    Evidence: .sisyphus/evidence/task-03-jsonl-valid.log

  Scenario: Invalid JSON-L rejected with error
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_jsonl_invalid
    Expected Result: Returns SchemaError with line number
    Evidence: .sisyphus/evidence/task-03-jsonl-invalid.log
  ```

  **Commit**: YES (groups with Wave 1)

- [x] 4. **Create LLM Score Types - Multi-Criteria Breakdown**

  **What to do**:
  - Create core/src/scoring_types.rs
  - Define LlmEvaluationScore struct:
    - overall: f64 (0.0-1.0)
    - criteria: CriteriaScores (diversity_thematic, diversity_structural, uniqueness, quality_semantic)
    - reasoning: String
    - summary: String
  - Define ConversationScore and DatasetScore aggregation
  - Add validation for score ranges (0.0-1.0)

  **Must NOT do**:
  - ❌ Implement scoring logic yet (just types)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 1)
  - **Blocked By**: Task 1

  **References**:
  - `/root/term-challenge/server/src/scoring.rs` — Aggregation patterns

  **Acceptance Criteria**:
  - [ ] Types defined with proper validation
  - [ ] Tests for score aggregation logic

  **QA Scenarios**:
  ```
  Scenario: Score aggregation works correctly
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_score_aggregation
    Expected Result: Averages 4 criteria correctly
    Evidence: .sisyphus/evidence/task-04-score-aggregation.log
  ```

  **Commit**: YES (Wave 1)

- [x] 5. **Create Configuration Types**

  **What to do**:
  - Create core/src/config.rs
  - Define HarnessExecutionConfig struct:
    - seed: u64
    - conversation_count: u32 (10-50)
    - timeout_seconds: u64 (7200 = 2h)
    - max_dataset_size_bytes: u64 (100MB)
  - Define EvaluationConfig for LLM parameters
  - Add validation for config values

  **Must NOT do**:
  - ❌ Add WASM-specific config (keep in wasm/)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 1)
  - **Blocked By**: Task 1

  **References**:
  - `/root/term-challenge/server/src/types.rs` — Config patterns

  **Acceptance Criteria**:
  - [ ] Config types defined
  - [ ] Validation tests pass

  **QA Scenarios**:
  ```
  Scenario: Config validation rejects invalid values
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_config_validation
    Expected Result: Rejects conversation_count < 10 or > 50
    Evidence: .sisyphus/evidence/task-05-config-validation.log
  ```

  **Commit**: YES (Wave 1)

- [x] 6. **Create Error Types and Result Aliases**

  **What to do**:
  - Create core/src/error.rs
  - Define DataFabricationError enum with variants:
    - SchemaError, ExecutionError, SecurityViolation, LlmError, ConsensusError
  - Implement std::error::Error trait
  - Create Result<T> = std::result::Result<T, DataFabricationError>
  - Add From impls for underlying errors

  **Must NOT do**:
  - ❌ Add unwrap() or expect() in library paths

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 1)
  - **Blocked By**: Task 1

  **References**:
  - `/root/term-challenge/server/src/types.rs` — Error patterns

  **Acceptance Criteria**:
  - [ ] Error enum defined with all variants
  - [ ] Tests for error conversion

  **Commit**: YES (Wave 1 completion)
  - Message: `feat(core): add error types and config`

---

### Wave 2: Security Layer

- [x] 7. **Create AST Validation Module - Import Whitelist**

  **What to do**:
  - Create core/src/ast_validation.rs
  - Use rustpython-parser crate to parse Python code
  - Implement import whitelist checker:
    - Initially allow all imports (validation via LLM review)
    - Check for dangerous patterns: exec, eval, compile, __import__
  - Define SecurityViolation struct with line/column info

  **Must NOT do**:
  - ❌ Block all imports (user wants "tout autorisé")
  - ❌ Skip validation entirely

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: AST parsing is complex, needs careful implementation
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with tasks 8-11)
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 12 (executor)
  - **Blocked By**: Tasks 1-6

  **References**:
  - `/root/term-challenge/server/src/ast_validation.rs` — AST validation patterns
  - rustpython-parser crate docs — Python AST parsing

  **Acceptance Criteria**:
  - [ ] AST parser parses Python source code
  - [ ] Detects dangerous builtins (exec, eval, compile)
  - [ ] Tests pass for safe and unsafe code

  **QA Scenarios**:
  ```
  Scenario: Safe code passes validation
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_ast_safe_code
    Expected Result: No violations detected
    Evidence: .sisyphus/evidence/task-07-ast-safe.log

  Scenario: Dangerous code blocked
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_ast_dangerous_code
    Expected Result: Returns SecurityViolation for exec()
    Evidence: .sisyphus/evidence/task-07-ast-dangerous.log
  ```

  **Commit**: YES (groups with Wave 2)
  - Message: `feat(security): add AST validation module`

- [x] 8. **Create Dangerous Pattern Detection**

  **What to do**:
  - Add pattern detection in ast_validation.rs:
    - Shell injection patterns: subprocess, os.system, os.popen
    - Network patterns: socket, urllib, requests (detect, don't block yet)
    - File system escape: os.chdir, shutil.rmtree, open(..., 'w')
  - Create DangerousPattern enum
  - Add severity levels (Critical, Warning, Info)

  **Must NOT do**:
  - ❌ Block network access (harness may need LLM API calls)
  - ❌ Block all file operations (harness needs to write output)

  **Recommended Agent Profile**:
  - **Category**: `deep`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 2)
  - **Blocked By**: Tasks 1-6

  **References**:
  - `/root/term-challenge/server/src/ast_validation.rs` — Pattern matching

  **Acceptance Criteria**:
  - [ ] Pattern detection returns severity levels
  - [ ] Tests for each pattern type

  **QA Scenarios**:
  ```
  Scenario: Detect subprocess usage with severity
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_pattern_subprocess
    Expected Result: Returns DangerousPattern::ShellInjection(Warning)
    Evidence: .sisyphus/evidence/task-08-pattern-subprocess.log
  ```

  **Commit**: YES (Wave 2)

- [x] 9. **Create Resource Limit Configuration**

  **What to do**:
  - Create core/src/resource_limits.rs
  - Define ResourceLimits struct:
    - cpu_time_seconds: u64 (7200 = 2h)
    - memory_bytes: u64 (2GB)
    - max_processes: u32 (4)
    - max_file_size: u64 (100MB)
  - Implement rlimit conversion helpers
  - Add validation for limit values

  **Must NOT do**:
  - ❌ Apply limits yet (Task 10 will do this)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 2)
  - **Blocked By**: Tasks 1-6

  **References**:
  - rlimit crate docs — Resource limiting

  **Acceptance Criteria**:
  - [ ] ResourceLimits type defined
  - [ ] Validation tests pass

  **Commit**: YES (Wave 2)

- [x] 10. **Create Sandbox Execution Helpers**

  **What to do**:
  - Create core/src/sandbox.rs
  - Implement sandbox setup:
    - tempfile::TempDir for isolated filesystem
    - rlimit::setrlimit for resource limits
    - Namespace isolation (optional, document for future)
  - Create SandboxConfig and SandboxResult types
  - Add cleanup guarantees (RAII pattern)

  **Must NOT do**:
  - ❌ Implement seccomp yet (complex, defer)
  - ❌ Spawn processes (Task 12 will do this)

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Sandbox security needs careful thought
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 2)
  - **Blocked By**: Tasks 1-6, 9

  **References**:
  - tempfile crate docs — Temporary directories
  - rlimit crate docs — Resource limits
  - `/root/term-challenge/executor/src/executor.rs` — Execution patterns

  **Acceptance Criteria**:
  - [ ] Sandbox helper creates temp directory
  - [ ] Cleans up on drop
  - [ ] Tests for isolation

  **QA Scenarios**:
  ```
  Scenario: Temporary directory created and cleaned
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_sandbox_tempdir
    Expected Result: TempDir created, then cleaned on drop
    Evidence: .sisyphus/evidence/task-10-sandbox-tempdir.log
  ```

  **Commit**: YES (Wave 2)

- [x] 11. **Create Security Test Suite**

  **What to do**:
  - Create core/tests/security_test.rs
  - Write tests for:
    - AST validation blocks exec/eval
    - Pattern detection identifies dangerous code
    - Sandbox isolates filesystem
    - Resource limits applied correctly
  - Add property-based tests with proptest

  **Must NOT do**:
  - ❌ Test executor integration yet (Task 16)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 2)
  - **Blocked By**: Tasks 7-10

  **Acceptance Criteria**:
  - [ ] All security tests pass
  - [ ] At least 10 test cases

  **QA Scenarios**:
  ```
  Scenario: Security test suite runs
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core --test security_test
    Expected Result: All tests pass, 0 failures
    Evidence: .sisyphus/evidence/task-11-security-tests.log
  ```

  **Commit**: YES (Wave 2 completion)
  - Message: `feat(security): add comprehensive test suite`

---

### Wave 3: Executor

- [x] 12. **Create Python Execution Engine**

  **What to do**:
  - Create executor/src/executor.rs
  - Implement PythonProcess struct:
    - Takes harness.py path, CLI args, timeout
    - Uses std::process::Command for execution
    - Captures stdout/stderr to output.jsonl
  - Apply resource limits from Task 9 via sandbox
  - Implement timeout handling (kill process after 2h)
  - Handle process failures (exit code != 0)

  **Must NOT do**:
  - ❌ Execute without sandbox (security risk)
  - ❌ Allow infinite timeout (always enforce 2h max)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Complex process management, error handling, security
  - **Skills**: []
    - No special skills needed

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on security layer)
  - **Parallel Group**: Sequential Wave 3
  - **Blocks**: Tasks 18-23 (LLM eval needs executor)
  - **Blocked By**: Tasks 7-11

  **References**:
  - `/root/term-challenge/executor/src/executor.rs` — Process execution patterns
  - std::process::Command docs — Process spawning
  - rlimit crate docs — Apply limits

  **Acceptance Criteria**:
  - [ ] Executor can run test harness successfully
  - [ ] Output captured to file
  - [ ] Timeout enforced (test with sleep)
  - [ ] `cargo test --package data-fabrication-executor` passes

  **QA Scenarios**:
  ```
  Scenario: Execute simple harness successfully
    Tool: Bash
    Preconditions: Test harness with `print('{"id": 1}')`
    Steps:
      1. cargo test --package data-fabrication-executor test_execute_simple
    Expected Result: Process completes, output.jsonl contains valid JSON-L
    Evidence: .sisyphus/evidence/task-12-exec-simple.log

  Scenario: Timeout kills long-running process
    Tool: Bash
    Preconditions: Test harness with `while True: pass`
    Steps:
      1. cargo test --package data-fabrication-executor test_timeout_enforcement
    Expected Result: Process killed after timeout, error returned
    Evidence: .sisyphus/evidence/task-12-exec-timeout.log

  Scenario: Memory limit enforced
    Tool: Bash
    Preconditions: Test harness allocating 10GB
    Steps:
      1. cargo test --package data-fabrication-executor test_memory_limit
    Expected Result: Process OOM killed, error returned
    Evidence: .sisyphus/evidence/task-12-exec-memory.log
  ```

  **Commit**: YES
  - Message: `feat(executor): add Python execution engine`

- [x] 13. **Create Output Capture and Validation**

  **What to do**:
  - Create executor/src/output.rs
  - Implement output capture:
    - Read output.jsonl after execution
    - Validate JSON-L format (Task 3 schema)
    - Check conversation count (10-50)
    - Check file size (max 100MB)
  - Create ExecutionResult struct (success, conversations, error)

  **Must NOT do**:
  - ❌ Validate conversation content (LLM will do this)
  - ❌ Skip schema validation

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with tasks 14-17)
  - **Blocked By**: Task 12

  **References**:
  - `/root/term-challenge/executor/src/task.rs` — Output handling

  **Acceptance Criteria**:
  - [ ] Output validation catches malformed JSON-L
  - [ ] Validates size and count limits

  **QA Scenarios**:
  ```
  Scenario: Valid output passes validation
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-executor test_output_valid
    Expected Result: Returns ExecutionResult::Success with conversation count
    Evidence: .sisyphus/evidence/task-13-output-valid.log

  Scenario: Malformed JSON-L rejected
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-executor test_output_malformed
    Expected Result: Returns ExecutionResult::Error
    Evidence: .sisyphus/evidence/task-13-output-malformed.log

  Scenario: Size limit enforced
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-executor test_output_size_limit
    Expected Result: Rejects output > 100MB
    Evidence: .sisyphus/evidence/task-13-output-size.log
  ```

  **Commit**: YES (groups with Wave 3)

- [x] 14. **Create Timeout Enforcement**

  **What to do**:
  - Create executor/src/timeout.rs or enhance executor.rs
  - Implement timeout handling:
    - Use std::process::Child::wait_with_timeout (or custom impl)
    - Send SIGKILL after timeout
    - Log timeout events
  - Make timeout configurable per execution (default 7200s)
  - Add grace period (5s warning before kill)

  **Must NOT do**:
  - ❌ Allow disabling timeout
  - ❌ Use SIGTERM before SIGKILL (fast kill)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 3)
  - **Blocked By**: Task 12

  **References**:
  - Unix signal handling in Rust — kill process

  **Acceptance Criteria**:
  - [ ] Timeout enforced exactly at configured time
  - [ ] Process guaranteed to be killed

  **QA Scenarios**:
  ```
  Scenario: Timeout kills process at exactly 7200s
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-executor test_timeout_exact
    Expected Result: Process killed at 7200s ± 1s
    Evidence: .sisyphus/evidence/task-14-timeout-exact.log
  ```

  **Commit**: YES (Wave 3)

- [x] 15. **Create Error Handling and Recovery**

  **What to do**:
  - Create executor/src/error.rs
  - Define ExecutorError enum:
    - ProcessSpawn, Timeout, MemoryExceeded, InvalidOutput, SandboxViolation
  - Implement error recovery:
    - Clean up zombie processes
    - Remove temp directories
    - Log errors with context
  - Add retry logic for transient errors (up to 2 retries)

  **Must NOT do**:
  - ❌ Panic on error (must return Result)
  - ❌ Leak resources on error

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 3)
  - **Blocked By**: Task 12

  **References**:
  - `/root/term-challenge/executor/src/types.rs` — Error patterns

  **Acceptance Criteria**:
  - [ ] All errors have proper context
  - [ ] Resources cleaned up on error

  **QA Scenarios**:
  ```
  Scenario: Zombie process cleaned up on error
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-executor test_zombie_cleanup
    Expected Result: No zombie processes after error
    Evidence: .sisyphus/evidence/task-15-zombie-cleanup.log
  ```

  **Commit**: YES (Wave 3)

- [x] 16. **Create Executor Integration Tests**

  **What to do**:
  - Create executor/tests/integration_test.rs
  - Write end-to-end tests:
    - Execute real Python harness
    - Generate 30 conversations
    - Validate output
    - Test timeout scenario
    - Test memory limit scenario
  - Create test harness fixtures in tests/fixtures/

  **Must NOT do**:
  - ❌ Mock Python execution (use real Python)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Integration tests need careful setup
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 3)
  - **Blocked By**: Tasks 12-15

  **Acceptance Criteria**:
  - [ ] All integration tests pass
  - [ ] Test harness generates valid JSON-L

  **QA Scenarios**:
  ```
  Scenario: Full executor integration test
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-executor --test integration_test
    Expected Result: All 5 integration tests pass
    Evidence: .sisyphus/evidence/task-16-integration.log
  ```

  **Commit**: YES (Wave 3)

- [x] 17. **Create Executor CLI Endpoint**

  **What to do**:
  - Create executor/src/cli.rs
  - Implement CLI for manual testing:
    - `data-fabrication-executor --harness harness.py --output output.jsonl --timeout 7200`
    - Print execution result to stdout
    - Exit code 0 for success, 1 for failure
  - Add --verbose flag for debugging

  **Must NOT do**:
  - ❌ Add complex CLI features (just execution)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 3)
  - **Blocked By**: Task 12

  **References**:
  - clap crate docs — CLI argument parsing

  **Acceptance Criteria**:
  - [ ] CLI binary compiles
  - [ ] Can execute test harness

  **QA Scenarios**:
  ```
  Scenario: Execute harness via CLI
    Tool: Bash
    Steps:
      1. cargo run --package data-fabrication-executor -- --harness tests/fixtures/simple.py --output /tmp/out.jsonl
      2. cat /tmp/out.jsonl | head -1
    Expected Result: Valid JSON-L output
    Evidence: .sisyphus/evidence/task-17-cli.log
  ```

  **Commit**: YES (Wave 3 completion)
  - Message: `feat(executor): add CLI and integration tests`

---

### Wave 4: LLM Evaluation

- [x] 18. **Create LLM Client Wrapper (Host Functions)**

  **What to do**:
  - Create core/src/llm_client.rs (shared between WASM and server)
  - For WASM: Use host_llm_chat_completion from platform-challenge-sdk-wasm
  - For Server: Use reqwest to call chutes.ai API
  - Implement LlmClient trait:
    - `async fn evaluate_conversation(&self, conversation: &ConversationEntry) -> Result<LlmEvaluationScore>`
  - Handle rate limits with retry + backoff
  - Parse JSON response

  **Must NOT do**:
  - ❌ Call external APIs directly in WASM (must use host functions)
  - ❌ Skip error handling for API failures

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Dual-mode implementation (WASM + HTTP), complex async handling
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (foundation for LLM eval)
  - **Parallel Group**: Sequential Wave 4
  - **Blocks**: Tasks 21-23 (consensus, caching)
  - **Blocked By**: Tasks 12-17

  **References**:
  - `/root/term-challenge/wasm/src/llm_review.rs` — Host function usage
  - `/root/term-challenge/server/src/llm_review.rs` — HTTP client usage
  - chutes.ai API docs — API endpoint

  **Acceptance Criteria**:
  - [ ] WASM client uses host_llm_chat_completion
  - [ ] Server client uses reqwest
  - [ ] Retries on 429 rate limit
  - [ ] Parses JSON score response

  **QA Scenarios**:
  ```
  Scenario: WASM client calls host function
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-wasm test_llm_client_wasm
    Expected Result: Mock host function called, returns valid score
    Evidence: .sisyphus/evidence/task-18-llm-wasm.log

  Scenario: Server client handles rate limit
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-server test_llm_client_rate_limit
    Expected Result: Retries with backoff, eventually succeeds
    Evidence: .sisyphus/evidence/task-18-llm-rate-limit.log
  ```

  **Commit**: YES
  - Message: `feat(llm): add dual-mode client wrapper`

- [x] 19. **Create Score Parsing and Validation**

  **What to do**:
  - Create core/src/score_parser.rs
  - Implement JSON parsing for LLM response:
    ```json
    {
      "overall": 0.85,
      "criteria": {
        "diversity_thematic": 0.9,
        "diversity_structural": 0.8,
        "uniqueness": 0.85,
        "quality_semantic": 0.75
      },
      "reasoning": "...",
      "summary": "..."
    }
    ```
  - Validate score ranges (0.0-1.0)
  - Handle malformed JSON gracefully
  - Create ScoreParsingError type

  **Must NOT do**:
  - ❌ Accept scores outside 0.0-1.0
  - ❌ Panic on malformed JSON

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 4)
  - **Blocked By**: Task 18

  **Acceptance Criteria**:
  - [ ] Parses valid JSON successfully
  - [ ] Rejects invalid scores

  **QA Scenarios**:
  ```
  Scenario: Parse valid LLM score JSON
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_parse_valid_score
    Expected Result: Returns LlmEvaluationScore with correct values
    Evidence: .sisyphus/evidence/task-19-parse-valid.log

  Scenario: Reject score > 1.0
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_parse_invalid_score
    Expected Result: Returns ScoreParsingError::OutOfRange
    Evidence: .sisyphus/evidence/task-19-parse-invalid.log
  ```

  **Commit**: YES (Wave 4)

- [x] 20. **Create Multi-Criteria Aggregation**

  **What to do**:
  - Create core/src/score_aggregation.rs
  - Implement aggregation logic:
    - Per-conversation: weighted average of 4 criteria
    - Per-dataset: average of all conversation scores
    - Handle missing criteria with default 0.0
  - Create ScoreAggregationConfig for weights:
    - diversity_thematic: 0.25
    - diversity_structural: 0.25
    - uniqueness: 0.25
    - quality_semantic: 0.25

  **Must NOT do**:
  - ❌ Use complex weighting schemes (keep simple)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 4)
  - **Blocked By**: Task 18

  **References**:
  - `/root/term-challenge/server/src/scoring.rs` — Aggregation patterns

  **Acceptance Criteria**:
  - [ ] Aggregates conversation scores correctly
  - [ ] Aggregates dataset scores correctly

  **QA Scenarios**:
  ```
  Scenario: Aggregate dataset scores
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_aggregate_dataset
    Expected Result: Average of 10 conversation scores matches expected
    Evidence: .sisyphus/evidence/task-20-aggregate-dataset.log
  ```

  **Commit**: YES (Wave 4)

- [x] 21. **Create Consensus Mechanism**

  **What to do**:
  - Create core/src/consensus.rs
  - Implement validator consensus:
    - Collect scores from N validators
    - Calculate average: `final_score = sum(scores) / n`
    - Identify outliers (> 0.2 deviation from mean)
    - Optionally exclude outliers and recalculate
  - Create ConsensusResult struct (final_score, agreement_level)

  **Must NOT do**:
  - ❌ Require strict consensus (allow 0.2 tolerance)
  - ❌ Block on single validator disagreement

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Consensus logic needs careful thought
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 4)
  - **Blocked By**: Task 18

  **References**:
  - `/root/term-challenge/server/src/dataset.rs` — Consensus patterns

  **Acceptance Criteria**:
  - [ ] Averages validator scores correctly
  - [ ] Identifies outliers

  **QA Scenarios**:
  ```
  Scenario: Consensus reaches average
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_consensus_average
    Expected Result: Returns average of [0.8, 0.85, 0.9] = 0.85
    Evidence: .sisyphus/evidence/task-21-consensus-average.log

  Scenario: Outlier detection
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_consensus_outlier
    Expected Result: Identifies 0.5 as outlier in [0.8, 0.85, 0.9, 0.5]
    Evidence: .sisyphus/evidence/task-21-consensus-outlier.log
  ```

  **Commit**: YES (Wave 4)

- [x] 22. **Create LLM Evaluation Tests**

  **What to do**:
  - Create core/tests/llm_test.rs
  - Write tests for:
    - Mock LLM responses with valid scores
    - Mock LLM responses with malformed JSON
    - Rate limit handling
    - Aggregation correctness
    - Consensus mechanism
  - Use mockall or similar for mocking

  **Must NOT do**:
  - ❌ Call real LLM APIs in tests (use mocks)

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Complex mocking setup
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 4)
  - **Blocked By**: Tasks 18-21

  **Acceptance Criteria**:
  - [ ] All LLM tests pass
  - [ ] Mock responses cover edge cases

  **QA Scenarios**:
  ```
  Scenario: LLM evaluation test suite
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core --test llm_test
    Expected Result: All tests pass
    Evidence: .sisyphus/evidence/task-22-llm-tests.log
  ```

  **Commit**: YES (Wave 4)

- [x] 23. **Create Evaluation Caching**

  **What to do**:
  - Create core/src/cache.rs
  - Implement conversation hash caching:
    - Hash conversation content (SHA-256)
    - Cache: `hash -> LlmEvaluationScore`
    - Use sled or in-memory HashMap
    - Cache hit avoids LLM API call
  - Implement cache expiry (24h TTL)

  **Must NOT do**:
  - ❌ Cache indefinitely (stale scores)
  - ❌ Cache across validators (local cache only)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 4)
  - **Blocked By**: Task 18

  **Acceptance Criteria**:
  - [ ] Cache hit skips LLM evaluation
  - [ ] Cache respects TTL

  **QA Scenarios**:
  ```
  Scenario: Cache hit skips LLM call
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_cache_hit
    Expected Result: Returns cached score without LLM call
    Evidence: .sisyphus/evidence/task-23-cache-hit.log

  Scenario: Cache misses on new conversation
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-core test_cache_miss
    Expected Result: Calls LLM and caches result
    Evidence: .sisyphus/evidence/task-23-cache-miss.log
  ```

  **Commit**: YES (Wave 4 completion)
  - Message: `feat(llm): add evaluation caching and tests`

---

### Wave 5: WASM Module

- [x] 24. **Implement Challenge Trait for WASM**

  **What to do**:
  - Create wasm/src/lib.rs
  - Implement Challenge trait from platform-challenge-sdk-wasm:
    - `fn name(&self) -> &'static str` → "data-fabrication"
    - `fn version(&self) -> &'static str` → "0.1.0"
    - `fn evaluate(&self, input: EvaluationInput) -> EvaluationOutput`
    - `fn validate(&self, input: EvaluationInput) -> bool`
    - `fn routes(&self) -> Vec<u8>`
    - `fn handle_route(&self, request: &[u8]) -> Vec<u8>`
  - Add `#![no_std]` and `extern crate alloc`
  - Use `register_challenge!(DataFabricationChallenge)` macro

  **Must NOT do**:
  - ❌ Use std:: in WASM code (use alloc::)
  - ❌ Call host functions incorrectly (follow ABI)

  **Recommended Agent Profile**:
  - **Category**: `ultrabrain`
    - Reason: WASM Challenge trait requires understanding of no_std, host functions, ABI
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (foundation for WASM)
  - **Parallel Group**: Sequential Wave 5
  - **Blocks**: Tasks 25-28
  - **Blocked By**: Tasks 18-23 (executor + LLM eval needed)

  **References**:
  - `/root/term-challenge/wasm/src/lib.rs` — Challenge trait implementation
  - platform-challenge-sdk-wasm docs — Challenge trait definition

  **Acceptance Criteria**:
  - [ ] WASM compiles to wasm32-unknown-unknown
  - [ ] Challenge trait fully implemented
  - [ ] register_challenge! macro called

  **QA Scenarios**:
  ```
  Scenario: WASM module compiles
    Tool: Bash
    Steps:
      1. cargo build --target wasm32-unknown-unknown -p data-fabrication-wasm
    Expected Result: Compiles without error, produces .wasm file
    Evidence: .sisyphus/evidence/task-24-wasm-compile.log

  Scenario: Challenge name returns correct value
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-wasm test_challenge_name
    Expected Result: Returns "data-fabrication"
    Evidence: .sisyphus/evidence/task-24-wasm-name.log
  ```

  **Commit**: YES
  - Message: `feat(wasm): implement Challenge trait`

- [x] 25. **Create Route Handlers**

  **What to do**:
  - Create wasm/src/routes.rs and wasm/src/api/handlers.rs
  - Define route definitions (27 routes like term-challenge):
    - GET /leaderboard — Return leaderboard data
    - GET /stats — Return submission stats
    - GET /dataset/:id — Return dataset info
    - POST /submit — Receive harness submission
    - GET /agent/:hotkey/logs — Return evaluation logs
    - POST /review/assign — Assign LLM review
    - POST /review/submit — Submit LLM review result
    - etc. (follow term-challenge pattern)
  - Implement handle_route() to dispatch to handlers
  - Use bincode for serialization

  **Must NOT do**:
  - ❌ Add routes not in plan (scope creep)
  - ❌ Use JSON instead of bincode

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with tasks 26-28)
  - **Parallel Group**: Wave 5
  - **Blocked By**: Task 24

  **References**:
  - `/root/term-challenge/wasm/src/routes.rs` — Route patterns
  - `/root/term-challenge/wasm/src/api/handlers.rs` — Handler implementation

  **Acceptance Criteria**:
  - [ ] Routes return serialized responses
  - [ ] POST routes accept serialized requests

  **QA Scenarios**:
  ```
  Scenario: GET /leaderboard returns data
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-wasm test_route_leaderboard
    Expected Result: Returns bincode-serialized LeaderboardResponse
    Evidence: .sisyphus/evidence/task-25-route-leaderboard.log

  Scenario: POST /submit accepts submission
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-wasm test_route_submit
    Expected Result: Returns success response
    Evidence: .sisyphus/evidence/task-25-route-submit.log
  ```

  **Commit**: YES (Wave 5)

- [x] 26. **Create Storage Operations**

  **What to do**:
  - Create wasm/src/storage.rs
  - Implement storage helpers using host functions:
    - `host_storage_get(key)` → retrieve value
    - `host_storage_set(key, value)` → store value
  - Implement storage patterns:
    - Harness code storage: `harness:<hotkey>:<epoch>`
    - Dataset storage: `dataset:<hotkey>:<epoch>`
    - Score storage: `score:<hotkey>:<epoch>`
    - Logs storage: `logs:<hotkey>:<epoch>`

  **Must NOT do**:
  - ❌ Use filesystem directly (must use host functions)
  - ❌ Store unbounded data (respect size limits)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 5)
  - **Blocked By**: Task 24

  **References**:
  - `/root/term-challenge/wasm/src/agent_storage.rs` — Storage patterns
  - platform-challenge-sdk-wasm docs — Host functions

  **Acceptance Criteria**:
  - [ ] Storage helpers use host functions
  - [ ] Data serialized with bincode

  **QA Scenarios**:
  ```
  Scenario: Harness code stored and retrieved
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-wasm test_storage_harness
    Expected Result: Stored code matches retrieved code
    Evidence: .sisyphus/evidence/task-26-storage-harness.log
  ```

  **Commit**: YES (Wave 5)

- [x] 27. **Create WASM Integration Tests**

  **What to do**:
  - Create wasm/tests/integration_test.rs
  - Write tests for:
    - Full submission flow (validate → store → evaluate)
    - Route handling end-to-end
    - Storage roundtrip
    - Evaluation with mock datasets
  - Mock host functions for testing

  **Must NOT do**:
  - ❌ Call real host functions in unit tests (use mocks)

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Integration tests need careful mocking
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 5)
  - **Blocked By**: Tasks 24-26

  **Acceptance Criteria**:
  - [ ] All integration tests pass
  - [ ] Mock host functions work correctly

  **QA Scenarios**:
  ```
  Scenario: Full submission flow test
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-wasm --test integration_test
    Expected Result: All tests pass
    Evidence: .sisyphus/evidence/task-27-wasm-integration.log
  ```

  **Commit**: YES (Wave 5)

- [x] 28. **Register Challenge Macro**

  **What to do**:
  - In wasm/src/lib.rs, add:
    ```rust
    register_challenge!(DataFabricationChallenge);
    ```
  - Ensure WASM exports correct ABI symbols:
    - `evaluate`
    - `validate`
    - `get_name`
    - `get_version`
    - `get_routes`
    - `handle_route`
    - `alloc`
    - `dealloc`
  - Verify WASM can be loaded by platform-v2

  **Must NOT do**:
  - ❌ Modify ABI (platform-v2 expects specific symbols)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 5)
  - **Blocked By**: Task 24

  **References**:
  - `/root/term-challenge/wasm/src/lib.rs` — Macro usage
  - platform-challenge-sdk-wasm docs — ABI requirements

  **Acceptance Criteria**:
  - [ ] WASM exports expected ABI symbols
  - [ ] Challenge registered correctly

  **QA Scenarios**:
  ```
  Scenario: WASM ABI symbols present
    Tool: Bash
    Steps:
      1. wasm-objdump -x target/wasm32-unknown-unknown/release/data_fabrication_wasm.wasm | grep "export"
    Expected Result: Exports include evaluate, validate, get_name, get_version
    Evidence: .sisyphus/evidence/task-28-wasm-abi.log
  ```

  **Commit**: YES (Wave 5 completion)
  - Message: `feat(wasm): add integration tests and register`

---

### Wave 6: Server + CLI

- [x] 29. **Implement ServerChallenge Trait for Server**

  **What to do**:
  - Create server/src/lib.rs
  - Implement ServerChallenge trait from platform-challenge-sdk:
    - `fn challenge_id(&self) -> &str`
    - `fn name(&self) -> &str`
    - `fn version(&self) -> &str`
    - `async fn evaluate(&self, req: EvaluationRequest) -> Result<EvaluationResponse, ChallengeError>`
    - `async fn validate(&self, req: ValidationRequest) -> Result<ValidationResponse, ChallengeError>`
    - `fn routes(&self) -> Vec<ChallengeRoute>`
    - `async fn handle_route(&self, ctx: &ChallengeContext, req: RouteRequest) -> RouteResponse`
  - Use ChallengeDatabase (sled) for storage
  - Implement async evaluation

  **Must NOT do**:
  - ❌ Use blocking calls in async functions
  - ❌ Skip error handling

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Async implementation, database integration, error handling
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (foundation for server)
  - **Parallel Group**: Sequential Wave 6
  - **Blocks**: Tasks 30-32
  - **Blocked By**: Tasks 24-28

  **References**:
  - `/root/term-challenge/server/src/lib.rs` — ServerChallenge implementation
  - platform-challenge-sdk docs — ServerChallenge trait

  **Acceptance Criteria**:
  - [ ] ServerChallenge trait implemented
  - [ ] Async evaluation works
  - [ ] Database operations work

  **QA Scenarios**:
  ```
  Scenario: Server evaluates harness submission
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-server test_evaluate
    Expected Result: Returns EvaluationResponse with score
    Evidence: .sisyphus/evidence/task-29-server-evaluate.log

  Scenario: Database stores and retrieves data
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-server test_database
    Expected Result: Stored data matches retrieved data
    Evidence: .sisyphus/evidence/task-29-server-database.log
  ```

  **Commit**: YES
  - Message: `feat(server): implement ServerChallenge trait`

- [x] 30. **Create HTTP Routes**

  **What to do**:
  - Create server/src/routes.rs
  - Define HTTP routes using Axum:
    - GET /api/leaderboard — Return leaderboard
    - GET /api/submissions — Return submission list
    - POST /api/submit — Receive harness submission
    - GET /api/harness/:hotkey — Return harness code
    - GET /api/dataset/:hotkey — Return generated dataset
    - GET /api/score/:hotkey — Return evaluation score
  - Use ChallengeServer::builder() pattern
  - Add authentication middleware (SR25519 signature)

  **Must NOT do**:
  - ❌ Add routes not in WASM routes (must match)
  - ❌ Skip authentication

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (with tasks 31-32)
  - **Parallel Group**: Wave 6
  - **Blocked By**: Task 29

  **References**:
  - `/root/term-challenge/server/src/routes.rs` — Route patterns
  - Axum docs — HTTP routing

  **Acceptance Criteria**:
  - [ ] HTTP routes match WASM routes
  - [ ] Authentication middleware works

  **QA Scenarios**:
  ```
  Scenario: GET /api/leaderboard returns data
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-server test_http_leaderboard
    Expected Result: Returns 200 with JSON leaderboard data
    Evidence: .sisyphus/evidence/task-30-http-leaderboard.log

  Scenario: POST /api/submit accepts authenticated request
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-server test_http_submit
    Expected Result: Returns 200 for valid signature, 401 for invalid
    Evidence: .sisyphus/evidence/task-30-http-submit.log
  ```

  **Commit**: YES (Wave 6)

- [x] 31. **Create CLI TUI**

  **What to do**:
  - Create cli/src/main.rs, cli/src/app.rs, cli/src/ui.rs, cli/src/rpc.rs
  - Implement TUI with Ratatui:
    - Leaderboard tab: Show scores, ranks, hotkeys
    - Evaluation tab: Show pending evaluations
    - Network tab: Show validators, epoch info
  - Add keyboard controls: Tab, arrows, r (refresh), q (quit)
  - Connect to validator via JSON-RPC 2.0

  **Must NOT do**:
  - ❌ Add complex visualizations (keep simple)
  - ❌ Block on RPC calls (use async)

  **Recommended Agent Profile**:
  - **Category**: `visual-engineering`
    - Reason: TUI requires UI/UX design with Ratatui
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 6)
  - **Blocked By**: Task 29

  **References**:
  - `/root/term-challenge/cli/src/` — CLI structure
  - Ratatui docs — TUI framework

  **Acceptance Criteria**:
  - [ ] CLI compiles to native binary
  - [ ] TUI displays leaderboard
  - [ ] Keyboard controls work

  **QA Scenarios**:
  ```
  Scenario: CLI launches and displays UI
    Tool: interactive_bash (tmux)
    Steps:
      1. cargo run --release --package data-fabrication-cli
      2. Wait for TUI to render
      3. Capture screenshot
    Expected Result: TUI visible with leaderboard tab
    Evidence: .sisyphus/evidence/task-31-cli-launch.png

  Scenario: Tab key switches tabs
    Tool: interactive_bash (tmux)
    Steps:
      1. Run CLI
      2. Press Tab
      3. Verify active tab changes
    Expected Result: Active tab switches from Leaderboard to Evaluation
    Evidence: .sisyphus/evidence/task-31-cli-tab.png
  ```

  **Commit**: YES (Wave 6)

- [x] 32. **Create CLI Integration Tests**

  **What to do**:
  - Create cli/tests/integration_test.rs
  - Write tests for:
    - RPC connection to validator
    - Leaderboard data retrieval
    - Submission command
    - Status command
  - Use mock validator server for testing

  **Must NOT do**:
  - ❌ Require real validator running (use mock)

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES (Wave 6)
  - **Blocked By**: Tasks 29-31

  **Acceptance Criteria**:
  - [ ] All integration tests pass
  - [ ] Mock validator works correctly

  **QA Scenarios**:
  ```
  Scenario: CLI integration test suite
    Tool: Bash
    Steps:
      1. cargo test --package data-fabrication-cli --test integration_test
    Expected Result: All tests pass
    Evidence: .sisyphus/evidence/task-32-cli-integration.log
  ```

  **Commit**: YES (Wave 6 completion)
  - Message: `feat(cli): add TUI and integration tests`

---

## Final Verification Wave

After ALL implementation tasks complete, run 4 parallel review agents:

- [x] F1. **Plan Compliance Audit** — APPROVE: 8/8 Must Have present, 8/8 Must NOT Have absent
- [x] F2. **Code Quality Review** — REJECT: Sandbox tests crash, security patterns not wired
- [x] F3. **Security Penetration Test** — REJECT: CRITICAL gaps in pattern detection
- [x] F4. **Scope Fidelity Check** — REJECT: 6/32 tasks missing (tests + CLI TUI)

---

## Commit Strategy

- **Wave 1 commits** (foundation):
  - `feat(core): initialize workspace with Cargo.toml`
  - `feat(core): add Submission and Dataset types`
  - `feat(core): add JSON-L schema validation`
  - `feat(core): add LLM score types`

- **Wave 2 commits** (security):
  - `feat(security): add AST validation module`
  - `feat(security): add dangerous pattern detection`
  - `feat(security): add sandbox execution helpers`

- **Wave 3 commits** (executor):
  - `feat(executor): add Python execution engine`
  - `feat(executor): add output validation`
  - `feat(executor): add timeout enforcement`

- **Wave 4 commits** (LLM):
  - `feat(llm): add LLM client wrapper`
  - `feat(llm): add multi-criteria scoring`
  - `feat(llm): add consensus mechanism`

- **Wave 5 commits** (WASM):
  - `feat(wasm): implement Challenge trait`
  - `feat(wasm): add route handlers`
  - `feat(wasm): add storage operations`

- **Wave 6 commits** (server + CLI):
  - `feat(server): implement ServerChallenge trait`
  - `feat(server): add HTTP routes`
  - `feat(cli): add TUI monitoring interface`

---

## Success Criteria

### Verification Commands
```bash
# Build WASM module
cargo build --release --target wasm32-unknown-unknown -p data-fabrication-wasm

# Build all
cargo build --release --all

# Run all tests
cargo test --all

# Run specific test suites
cargo test --package data-fabrication-core
cargo test --package data-fabrication-executor
cargo test --package data-fabrication-wasm

# Run integration tests
cargo test --test integration_test

# Security tests
cargo test --package data-fabrication-executor --test security_test
```

### Final Checklist
- [ ] All "Must Have" present and tested
- [ ] All "Must NOT Have" absent
- [ ] All tests pass (cargo test --all)
- [ ] WASM compiles successfully
- [ ] Executor can run test harness
- [ ] LLM evaluation returns valid JSON
- [ ] CLI displays leaderboard
- [ ] Security tests pass (no sandbox escapes)
- [ ] Plan compliance verified by oracle
