//! LLM Client for conversation evaluation.
//!
//! This module provides a dual-mode LLM client that works in both
//! WASM (via host functions) and Server (via HTTP) environments.

use crate::error::{DataFabricationError, Result};
use crate::scoring_types::{CriteriaScores, LlmEvaluationScore};
use crate::ConversationEntry;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

extern crate alloc;

/// LLM Client trait for evaluating conversations.
pub trait LlmClient: Send + Sync {
    /// Evaluate a conversation and return a score.
    fn evaluate_conversation(
        &self,
        conversation: &ConversationEntry,
    ) -> impl core::future::Future<Output = Result<LlmEvaluationScore>> + Send;
}

/// HTTP-based LLM client for server environment.
#[cfg(feature = "http-client")]
pub struct HttpLlmClient {
    endpoint: String,
    model: String,
    api_key: Option<String>,
    max_retries: u32,
    retry_delay_ms: u64,
}

#[cfg(feature = "http-client")]
use core::time::Duration;

#[cfg(feature = "http-client")]
impl HttpLlmClient {
    /// Creates a new HTTP LLM client.
    pub fn new(
        endpoint: String,
        model: String,
        api_key: Option<String>,
        max_retries: u32,
        retry_delay_ms: u64,
    ) -> Self {
        Self {
            endpoint,
            model,
            api_key,
            max_retries,
            retry_delay_ms,
        }
    }

    /// Creates a client with default retry settings (3 retries, 1s initial delay).
    pub fn with_defaults(endpoint: String, model: String, api_key: Option<String>) -> Self {
        Self::new(endpoint, model, api_key, 3, 1000)
    }

    async fn make_request(
        &self,
        conversation: &ConversationEntry,
    ) -> Result<LlmEvaluationScore> {
        let client = reqwest::Client::new();
        let mut request_builder = client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .json(&LlmRequest {
                model: &self.model,
                messages: &conversation.messages,
            });

        if let Some(ref api_key) = self.api_key {
            request_builder = request_builder.bearer_auth(api_key);
        }

        let response = request_builder
            .send()
            .await
            .map_err(|e| DataFabricationError::LlmError {
                message: format!("HTTP request failed: {}", e),
                retry_count: 0,
            })?;

        let status = response.status();
        if status.as_u16() == 429 {
            return Err(DataFabricationError::LlmError {
                message: "Rate limit exceeded".to_string(),
                retry_count: 0,
            });
        }

        if !status.is_success() {
            return Err(DataFabricationError::LlmError {
                message: format!("LLM API returned status {}", status),
                retry_count: 0,
            });
        }

        let llm_response: LlmResponse = response
            .json()
            .await
            .map_err(|e| DataFabricationError::LlmError {
                message: format!("Failed to parse LLM response: {}", e),
                retry_count: 0,
            })?;

        parse_llm_response(&llm_response)
    }

    /// Evaluate with retry logic and exponential backoff.
    pub async fn evaluate_with_retry(
        &self,
        conversation: &ConversationEntry,
    ) -> Result<LlmEvaluationScore> {
        let mut delay = self.retry_delay_ms;

        for attempt in 0..self.max_retries {
            match self.make_request(conversation).await {
                Ok(result) => return Ok(result),
                Err(DataFabricationError::LlmError { message, .. }) 
                    if message.contains("Rate limit") && attempt < self.max_retries - 1 => 
                {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    delay *= 2;
                }
                Err(e) => return Err(e),
            }
        }

        Err(DataFabricationError::LlmError {
            message: "Max retries exceeded".to_string(),
            retry_count: self.max_retries,
        })
    }
}

#[cfg(feature = "http-client")]
impl LlmClient for HttpLlmClient {
    async fn evaluate_conversation(
        &self,
        conversation: &ConversationEntry,
    ) -> Result<LlmEvaluationScore> {
        self.evaluate_with_retry(conversation).await
    }
}

/// Request body sent to LLM API.
#[derive(Debug, Serialize)]
struct LlmRequest<'a> {
    model: &'a str,
    messages: &'a Vec<crate::Message>,
}

/// Response body from LLM API.
#[derive(Debug, Deserialize)]
struct LlmResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Debug, Deserialize)]
struct MessageContent {
    content: String,
}

/// Parse LLM response JSON into LlmEvaluationScore.
fn parse_llm_response(response: &LlmResponse) -> Result<LlmEvaluationScore> {
    let content = response.choices.first().map(|c| &c.message.content).ok_or(
        DataFabricationError::LlmError {
            message: "No choices in LLM response".to_string(),
            retry_count: 0,
        },
    )?;

    let parsed: LlmScoreJson = serde_json::from_str(content).map_err(|e| {
        DataFabricationError::LlmError {
            message: format!("Failed to parse score JSON: {}", e),
            retry_count: 0,
        }
    })?;

    let criteria = CriteriaScores::new(
        parsed.criteria.diversity_thematic,
        parsed.criteria.diversity_structural,
        parsed.criteria.uniqueness,
        parsed.criteria.quality_semantic,
    )
    .map_err(|e| DataFabricationError::LlmError {
        message: format!("Invalid criteria scores: {}", e),
        retry_count: 0,
    })?;

    Ok(LlmEvaluationScore::new(
        parsed.overall,
        criteria,
        parsed.reasoning,
        parsed.summary,
    )
    .map_err(|e| DataFabricationError::LlmError {
        message: format!("Invalid overall score: {}", e),
        retry_count: 0,
    })?)
}

/// JSON structure for LLM score parsing.
#[derive(Debug, Deserialize)]
struct LlmScoreJson {
    overall: f64,
    criteria: CriteriaJson,
    reasoning: String,
    summary: String,
}

#[derive(Debug, Deserialize)]
struct CriteriaJson {
    diversity_thematic: f64,
    diversity_structural: f64,
    uniqueness: f64,
    quality_semantic: f64,
}

/// Check if an error is a rate limit error.
fn is_rate_limit_error(error: &DataFabricationError) -> bool {
    matches!(error, DataFabricationError::LlmError { message, .. } 
        if message.contains("Rate limit"))
}

/// Mock LLM client for testing.
pub struct MockLlmClient {
    pub response: LlmEvaluationScore,
}

impl MockLlmClient {
    /// Creates a new mock client with a predefined response.
    pub fn new(response: LlmEvaluationScore) -> Self {
        Self { response }
    }

    /// Creates a mock client with a perfect score.
    pub fn perfect() -> Self {
        let criteria = CriteriaScores::new(1.0, 1.0, 1.0, 1.0).unwrap();
        let response = LlmEvaluationScore::new(
            1.0,
            criteria,
            "Perfect score".to_string(),
            "All criteria met".to_string(),
        )
        .unwrap();
        Self { response }
    }

    /// Creates a mock client with a zero score.
    pub fn zero() -> Self {
        let criteria = CriteriaScores::new(0.0, 0.0, 0.0, 0.0).unwrap();
        let response = LlmEvaluationScore::new(
            0.0,
            criteria,
            "Zero score".to_string(),
            "No criteria met".to_string(),
        )
        .unwrap();
        Self { response }
    }
}

impl LlmClient for MockLlmClient {
    async fn evaluate_conversation(
        &self,
        _conversation: &ConversationEntry,
    ) -> Result<LlmEvaluationScore> {
        Ok(self.response.clone())
    }
}

/// WASM LLM client that uses host functions (stub implementation).
/// 
/// This is a stub since we don't have actual platform SDK access in this environment.
/// In a real WASM build, this would call host functions from platform-challenge-sdk.
pub struct WasmLlmClient {
    model: String,
}

impl WasmLlmClient {
    pub fn new(model: String) -> Self {
        Self { model }
    }
}

impl LlmClient for WasmLlmClient {
    async fn evaluate_conversation(
        &self,
        _conversation: &ConversationEntry,
    ) -> Result<LlmEvaluationScore> {
        Err(DataFabricationError::LlmError {
            message: format!(
                "WASM host function not available in this environment (model: {})",
                self.model
            ),
            retry_count: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_conversation() -> ConversationEntry {
        ConversationEntry {
            messages: alloc::vec![
                crate::Message {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                    name: None,
                    function_call: None,
                },
                crate::Message {
                    role: "assistant".to_string(),
                    content: "Hi there!".to_string(),
                    name: None,
                    function_call: None,
                },
            ],
            function_calls: None,
            thinking: None,
        }
    }

    #[test]
    fn test_mock_client_returns_score() {
        let criteria = CriteriaScores::new(0.9, 0.8, 0.85, 0.75).unwrap();
        let expected = LlmEvaluationScore::new(
            0.85,
            criteria.clone(),
            "Test reasoning".to_string(),
            "Test summary".to_string(),
        )
        .unwrap();

        let client = MockLlmClient::new(expected.clone());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let conversation = create_test_conversation();
        let result = rt.block_on(client.evaluate_conversation(&conversation)).unwrap();

        assert_eq!(result, expected);
    }

    #[test]
    fn test_mock_client_perfect() {
        let client = MockLlmClient::perfect();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let conversation = create_test_conversation();
        let result = rt.block_on(client.evaluate_conversation(&conversation)).unwrap();

        assert!((result.overall - 1.0).abs() < f64::EPSILON);
        assert!((result.criteria.diversity_thematic - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mock_client_zero() {
        let client = MockLlmClient::zero();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let conversation = create_test_conversation();
        let result = rt.block_on(client.evaluate_conversation(&conversation)).unwrap();

        assert!((result.overall - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_evaluation_score_parsed_correctly() {
        let response = LlmResponse {
            choices: alloc::vec![Choice {
                message: MessageContent {
                    content: r#"{
                        "overall": 0.85,
                        "criteria": {
                            "diversity_thematic": 0.9,
                            "diversity_structural": 0.8,
                            "uniqueness": 0.85,
                            "quality_semantic": 0.75
                        },
                        "reasoning": "The conversation demonstrates...",
                        "summary": "High quality conversation"
                    }"#
                    .to_string(),
                },
            }],
        };

        let result = parse_llm_response(&response).unwrap();

        assert!((result.overall - 0.85).abs() < f64::EPSILON);
        assert!((result.criteria.diversity_thematic - 0.9).abs() < f64::EPSILON);
        assert!((result.criteria.diversity_structural - 0.8).abs() < f64::EPSILON);
        assert_eq!(result.reasoning, "The conversation demonstrates...");
        assert_eq!(result.summary, "High quality conversation");
    }

    #[test]
    fn test_parse_empty_choices_fails() {
        let response = LlmResponse { choices: alloc::vec![] };

        let result = parse_llm_response(&response);
        assert!(result.is_err());
        
        if let Err(DataFabricationError::LlmError { message, .. }) = result {
            assert!(message.contains("No choices"));
        } else {
            panic!("Expected LlmError");
        }
    }

    #[test]
    fn test_parse_invalid_json_fails() {
        let response = LlmResponse {
            choices: alloc::vec![Choice {
                message: MessageContent {
                    content: "not valid json".to_string(),
                },
            }],
        };

        let result = parse_llm_response(&response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_score_fails() {
        let response = LlmResponse {
            choices: alloc::vec![Choice {
                message: MessageContent {
                    content: r#"{
                        "overall": 2.5,
                        "criteria": {
                            "diversity_thematic": 0.9,
                            "diversity_structural": 0.8,
                            "uniqueness": 0.85,
                            "quality_semantic": 0.75
                        },
                        "reasoning": "test",
                        "summary": "test"
                    }"#
                    .to_string(),
                },
            }],
        };

        let result = parse_llm_response(&response);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_rate_limit_error() {
        let rate_limit = DataFabricationError::LlmError {
            message: "Rate limit exceeded".to_string(),
            retry_count: 0,
        };
        assert!(is_rate_limit_error(&rate_limit));

        let other_error = DataFabricationError::LlmError {
            message: "Connection timeout".to_string(),
            retry_count: 0,
        };
        assert!(!is_rate_limit_error(&other_error));
    }

    #[test]
    fn test_wasm_client_returns_error() {
        let client = WasmLlmClient::new("test-model".to_string());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let conversation = create_test_conversation();
        let result = rt.block_on(client.evaluate_conversation(&conversation));

        assert!(result.is_err());
        if let Err(DataFabricationError::LlmError { message, .. }) = result {
            assert!(message.contains("WASM host function not available"));
        } else {
            panic!("Expected LlmError");
        }
    }
}
