#![no_std]

extern crate alloc;

mod types;

use alloc::vec::Vec;
use bincode::Options;
use platform_challenge_sdk_wasm::{Challenge, EvaluationInput, EvaluationOutput};

use crate::types::{ChallengeParams, Submission};

const MAX_SUBMISSION_SIZE: u64 = 4 * 1024 * 1024;
const MAX_PARAMS_SIZE: u64 = 1 * 1024 * 1024;

fn bincode_options_submission() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_SUBMISSION_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

fn bincode_options_params() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_PARAMS_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

pub struct DataFabricationChallenge;

impl Default for DataFabricationChallenge {
    fn default() -> Self {
        Self
    }
}

impl DataFabricationChallenge {
    pub const fn new() -> Self {
        Self
    }
}

impl Challenge for DataFabricationChallenge {
    fn name(&self) -> &'static str {
        "data-fabrication"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn evaluate(&self, input: EvaluationInput) -> EvaluationOutput {
        let _submission: Submission =
            match bincode_options_submission().deserialize(&input.agent_data) {
                Ok(s) => s,
                Err(_) => return EvaluationOutput::failure("failed to deserialize submission"),
            };

        let _params: ChallengeParams = match bincode_options_params().deserialize(&input.params) {
            Ok(p) => p,
            Err(_) => return EvaluationOutput::failure("failed to deserialize challenge params"),
        };

        EvaluationOutput::success(
            0,
            "stub implementation - actual evaluation happens in executor",
        )
    }

    fn validate(&self, input: EvaluationInput) -> bool {
        let submission: Submission =
            match bincode_options_submission().deserialize(&input.agent_data) {
                Ok(s) => s,
                Err(_) => return false,
            };

        if submission.hotkey.is_empty() {
            return false;
        }

        if submission.code_hash.is_empty() {
            return false;
        }

        if submission.package.is_empty() {
            return false;
        }

        if submission.signature.is_empty() {
            return false;
        }

        true
    }

    fn routes(&self) -> Vec<u8> {
        Vec::new()
    }

    fn handle_route(&self, _request: &[u8]) -> Vec<u8> {
        Vec::new()
    }
}

platform_challenge_sdk_wasm::register_challenge!(
    DataFabricationChallenge,
    DataFabricationChallenge::new()
);
