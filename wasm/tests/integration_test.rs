//! Integration tests for the data-fabrication WASM challenge module.

use bincode::Options;
use data_fabrication_wasm::DataFabricationChallenge;
use platform_challenge_sdk_wasm::{Challenge, EvaluationInput, EvaluationOutput};
use serde::{Deserialize, Serialize};

const MAX_SUBMISSION_SIZE: u64 = 4 * 1024 * 1024;
const MAX_PARAMS_SIZE: u64 = 1 * 1024 * 1024;

fn bincode_options_submission() -> impl bincode::Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_SUBMISSION_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

fn bincode_options_params() -> impl bincode::Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_PARAMS_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Submission {
    hotkey: String,
    epoch: u64,
    code_hash: String,
    package: Vec<u8>,
    signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChallengeParams {
    min_conversations: u32,
    max_conversations: u32,
    max_size_bytes: u64,
    model: Option<String>,
}

impl Default for ChallengeParams {
    fn default() -> Self {
        Self {
            min_conversations: 1,
            max_conversations: 1000,
            max_size_bytes: 10_000_000,
            model: None,
        }
    }
}

fn create_valid_submission() -> Submission {
    Submission {
        hotkey: "5GrwvaEF5zXD".to_string(),
        epoch: 42,
        code_hash: "abc123def456".to_string(),
        package: vec![1, 2, 3, 4, 5],
        signature: "test-signature-123".to_string(),
    }
}

fn create_valid_params() -> ChallengeParams {
    ChallengeParams {
        min_conversations: 10,
        max_conversations: 100,
        max_size_bytes: 1_000_000,
        model: Some("test-model".to_string()),
    }
}

fn create_evaluation_input(submission: &Submission, params: &ChallengeParams) -> EvaluationInput {
    let agent_data = bincode_options_submission().serialize(submission).unwrap();
    let params_bytes = bincode_options_params().serialize(params).unwrap();
    EvaluationInput {
        agent_data,
        challenge_id: "data-fabrication".to_string(),
        params: params_bytes,
        task_definition: None,
        environment_config: None,
    }
}

#[test]
fn test_challenge_name_returns_data_fabrication() {
    let challenge = DataFabricationChallenge::new();
    assert_eq!(challenge.name(), "data-fabrication");
}

#[test]
fn test_challenge_version_returns_0_1_0() {
    let challenge = DataFabricationChallenge::new();
    assert_eq!(challenge.version(), "0.1.0");
}

#[test]
fn test_validate_accepts_valid_submission() {
    let challenge = DataFabricationChallenge::new();
    let submission = create_valid_submission();
    let params = create_valid_params();
    let input = create_evaluation_input(&submission, &params);

    assert!(challenge.validate(input));
}

#[test]
fn test_validate_rejects_empty_hotkey() {
    let challenge = DataFabricationChallenge::new();
    let mut submission = create_valid_submission();
    submission.hotkey = String::new();

    let params = create_valid_params();
    let input = create_evaluation_input(&submission, &params);

    assert!(!challenge.validate(input));
}

#[test]
fn test_validate_rejects_empty_code_hash() {
    let challenge = DataFabricationChallenge::new();
    let mut submission = create_valid_submission();
    submission.code_hash = String::new();

    let params = create_valid_params();
    let input = create_evaluation_input(&submission, &params);

    assert!(!challenge.validate(input));
}

#[test]
fn test_validate_rejects_empty_package() {
    let challenge = DataFabricationChallenge::new();
    let mut submission = create_valid_submission();
    submission.package = Vec::new();

    let params = create_valid_params();
    let input = create_evaluation_input(&submission, &params);

    assert!(!challenge.validate(input));
}

#[test]
fn test_validate_rejects_empty_signature() {
    let challenge = DataFabricationChallenge::new();
    let mut submission = create_valid_submission();
    submission.signature = String::new();

    let params = create_valid_params();
    let input = create_evaluation_input(&submission, &params);

    assert!(!challenge.validate(input));
}

#[test]
fn test_validate_rejects_malformed_agent_data() {
    let challenge = DataFabricationChallenge::new();

    let input = EvaluationInput {
        agent_data: vec![0xFF, 0xFE, 0xFD],
        challenge_id: "data-fabrication".to_string(),
        params: bincode_options_params()
            .serialize(&create_valid_params())
            .unwrap(),
        task_definition: None,
        environment_config: None,
    };

    assert!(!challenge.validate(input));
}

#[test]
fn test_evaluate_returns_success_for_valid_submission() {
    let challenge = DataFabricationChallenge::new();
    let submission = create_valid_submission();
    let params = create_valid_params();
    let input = create_evaluation_input(&submission, &params);

    let output = challenge.evaluate(input);

    assert!(output.valid);
    assert_eq!(output.score, 0);
}

#[test]
fn test_evaluate_returns_failure_for_malformed_submission() {
    let challenge = DataFabricationChallenge::new();

    let input = EvaluationInput {
        agent_data: vec![0xFF, 0xFE, 0xFD],
        challenge_id: "data-fabrication".to_string(),
        params: bincode_options_params()
            .serialize(&create_valid_params())
            .unwrap(),
        task_definition: None,
        environment_config: None,
    };

    let output = challenge.evaluate(input);

    assert!(!output.valid);
    assert!(output.message.contains("failed to deserialize submission"));
}

#[test]
fn test_evaluate_returns_failure_for_malformed_params() {
    let challenge = DataFabricationChallenge::new();
    let submission = create_valid_submission();

    let input = EvaluationInput {
        agent_data: bincode_options_submission().serialize(&submission).unwrap(),
        challenge_id: "data-fabrication".to_string(),
        params: vec![0xFF, 0xFE, 0xFD],
        task_definition: None,
        environment_config: None,
    };

    let output = challenge.evaluate(input);

    assert!(!output.valid);
    assert!(output
        .message
        .contains("failed to deserialize challenge params"));
}

#[test]
fn test_routes_returns_empty_vec() {
    let challenge = DataFabricationChallenge::new();
    let routes = challenge.routes();

    assert!(routes.is_empty());
}

#[test]
fn test_handle_route_returns_empty_vec() {
    let challenge = DataFabricationChallenge::new();
    let request = b"test-request";

    let response = challenge.handle_route(request);

    assert!(response.is_empty());
}

#[test]
fn test_submission_serialization_roundtrip() {
    let submission = create_valid_submission();

    let encoded = bincode_options_submission().serialize(&submission).unwrap();
    let decoded: Submission = bincode_options_submission().deserialize(&encoded).unwrap();

    assert_eq!(submission.hotkey, decoded.hotkey);
    assert_eq!(submission.epoch, decoded.epoch);
    assert_eq!(submission.code_hash, decoded.code_hash);
    assert_eq!(submission.package, decoded.package);
    assert_eq!(submission.signature, decoded.signature);
}

#[test]
fn test_params_serialization_roundtrip() {
    let params = create_valid_params();

    let encoded = bincode_options_params().serialize(&params).unwrap();
    let decoded: ChallengeParams = bincode_options_params().deserialize(&encoded).unwrap();

    assert_eq!(params.min_conversations, decoded.min_conversations);
    assert_eq!(params.max_conversations, decoded.max_conversations);
    assert_eq!(params.max_size_bytes, decoded.max_size_bytes);
    assert_eq!(params.model, decoded.model);
}

#[test]
fn test_evaluate_with_default_params() {
    let challenge = DataFabricationChallenge::new();
    let submission = create_valid_submission();

    let params = ChallengeParams::default();
    let input = create_evaluation_input(&submission, &params);

    let output = challenge.evaluate(input);

    assert!(output.valid);
}

#[test]
fn test_validate_with_large_package() {
    let challenge = DataFabricationChallenge::new();
    let mut submission = create_valid_submission();
    submission.package = vec![0u8; 1024 * 1024];

    let params = create_valid_params();
    let input = create_evaluation_input(&submission, &params);

    assert!(challenge.validate(input));
}

#[test]
fn test_challenge_default_trait() {
    let challenge = DataFabricationChallenge::default();
    assert_eq!(challenge.name(), "data-fabrication");
    assert_eq!(challenge.version(), "0.1.0");
}

#[test]
fn test_evaluation_output_success_constructor() {
    let output = EvaluationOutput::success(100, "test message");

    assert!(output.valid);
    assert_eq!(output.score, 100);
    assert_eq!(output.message, "test message");
}

#[test]
fn test_evaluation_output_failure_constructor() {
    let output = EvaluationOutput::failure("error occurred");

    assert!(!output.valid);
    assert_eq!(output.score, 0);
    assert_eq!(output.message, "error occurred");
}
