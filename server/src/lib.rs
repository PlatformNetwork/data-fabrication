//! Data Fabrication Challenge Server
//!
//! Implements the ServerChallenge trait for evaluating Python harnesses
//! that generate conversation datasets.

pub mod types;

use platform_challenge_sdk::error::ChallengeError;
use platform_challenge_sdk::routes::{ChallengeRoute, RouteRequest, RouteResponse};
use platform_challenge_sdk::server::{
    ChallengeContext, ConfigLimits, ConfigResponse, EvaluationRequest, EvaluationResponse,
    ValidationRequest, ValidationResponse,
};
use serde_json::json;

use types::{Submission, MAX_PACKAGE_SIZE};

/// The data fabrication challenge server implementation.
pub struct DataFabricationServer {
    challenge_id: String,
}

impl DataFabricationServer {
    pub fn new(challenge_id: impl Into<String>) -> Self {
        Self {
            challenge_id: challenge_id.into(),
        }
    }
}

impl Default for DataFabricationServer {
    fn default() -> Self {
        Self::new("data-fabrication")
    }
}

#[async_trait::async_trait]
impl platform_challenge_sdk::server::ServerChallenge for DataFabricationServer {
    fn challenge_id(&self) -> &str {
        &self.challenge_id
    }

    fn name(&self) -> &str {
        "Data Fabrication"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    async fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> Result<EvaluationResponse, ChallengeError> {
        let submission: Submission = serde_json::from_value(request.data.clone()).map_err(|e| {
            ChallengeError::Evaluation(format!("Invalid submission data: {}", e))
        })?;

        if submission.hotkey.is_empty() {
            return Ok(EvaluationResponse::error(
                &request.request_id,
                "Missing hotkey",
            ));
        }

        if submission.package.is_empty() {
            return Ok(EvaluationResponse::error(
                &request.request_id,
                "Missing package",
            ));
        }

        if submission.package.len() > MAX_PACKAGE_SIZE {
            return Ok(EvaluationResponse::error(
                &request.request_id,
                format!(
                    "Package too large: {} bytes (max {})",
                    submission.package.len(),
                    MAX_PACKAGE_SIZE
                ),
            ));
        }

        let params = submission.challenge_params.clone().unwrap_or_default();
        let _ = params;

        Ok(EvaluationResponse::success(
            &request.request_id,
            0.0,
            json!({
                "hotkey": submission.hotkey,
                "epoch": submission.epoch,
                "code_hash": submission.code_hash,
                "message": "Evaluation accepted - full implementation pending",
            }),
        ))
    }

    async fn validate(
        &self,
        request: ValidationRequest,
    ) -> Result<ValidationResponse, ChallengeError> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        let submission: Result<Submission, _> = serde_json::from_value(request.data.clone());
        match submission {
            Ok(sub) => {
                if sub.hotkey.is_empty() {
                    errors.push("Missing hotkey".to_string());
                }
                if sub.package.is_empty() {
                    errors.push("Missing package".to_string());
                }
                if sub.package.len() > MAX_PACKAGE_SIZE {
                    errors.push(format!(
                        "Package too large: {} bytes (max {})",
                        sub.package.len(),
                        MAX_PACKAGE_SIZE
                    ));
                }
                if sub.code_hash.is_empty() {
                    warnings.push("Missing code_hash - integrity cannot be verified".to_string());
                }
            }
            Err(e) => {
                errors.push(format!("Invalid submission format: {}", e));
            }
        }

        Ok(ValidationResponse {
            valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    fn config(&self) -> ConfigResponse {
        ConfigResponse {
            challenge_id: self.challenge_id().to_string(),
            name: self.name().to_string(),
            version: self.version().to_string(),
            config_schema: Some(json!({
                "type": "object",
                "properties": {
                    "llm_review_enabled": {"type": "boolean"},
                    "llm_judge_enabled": {"type": "boolean"},
                    "execution_timeout_secs": {"type": "integer", "minimum": 1, "maximum": 300},
                    "min_conversations": {"type": "integer", "minimum": 1},
                    "max_conversations": {"type": "integer", "maximum": 10000}
                }
            })),
            features: vec![
                "ast_validation".to_string(),
                "sandboxed_execution".to_string(),
                "quality_scoring".to_string(),
            ],
            limits: ConfigLimits {
                max_submission_size: Some(MAX_PACKAGE_SIZE as u64),
                max_evaluation_time: Some(300),
                max_cost: None,
            },
        }
    }

    fn routes(&self) -> Vec<ChallengeRoute> {
        vec![
            ChallengeRoute::get("/leaderboard", "Get current leaderboard"),
            ChallengeRoute::get("/status/:hotkey", "Get evaluation status for a miner"),
            ChallengeRoute::get("/stats", "Get challenge statistics"),
        ]
    }

    async fn handle_route(&self, _ctx: &ChallengeContext, request: RouteRequest) -> RouteResponse {
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/leaderboard") => RouteResponse::json(json!({
                "entries": [],
                "message": "No submissions yet"
            })),
            ("GET", "/stats") => RouteResponse::json(json!({
                "total_submissions": 0,
                "active_miners": 0,
                "validator_count": 0
            })),
            _ => RouteResponse::not_found(),
        }
    }
}
