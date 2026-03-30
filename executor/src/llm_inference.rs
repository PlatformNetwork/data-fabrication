//! LLM inference for plagiarism detection.
//!
//! Provides LLM-based evaluation of code similarity with retry logic.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for LLM inference
#[derive(Debug, Clone)]
pub struct LlmInferenceConfig {
    pub endpoint: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

impl Default for LlmInferenceConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434/api/chat".to_string(),
            model: "llama3".to_string(),
            timeout_seconds: 60,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// Verdict from LLM plagiarism evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlagiarismVerdict {
    pub is_plagiarism: bool,
    pub confidence: f64,
    pub reasoning: String,
}

impl PlagiarismVerdict {
    /// Validates that the verdict has sensible values
    pub fn validate(&self) -> Result<(), LlmInferenceError> {
        if self.confidence < 0.0 || self.confidence > 1.0 {
            return Err(LlmInferenceError::InvalidResponse(
                format!("Confidence {} out of range [0, 1]", self.confidence)
            ));
        }
        if self.reasoning.is_empty() {
            return Err(LlmInferenceError::InvalidResponse(
                "Empty reasoning field".to_string()
            ));
        }
        Ok(())
    }
}

/// LLM inference errors
#[derive(Debug, Clone)]
pub enum LlmInferenceError {
    HttpError(String),
    Timeout,
    InvalidResponse(String),
    MaxRetriesExceeded,
}

impl fmt::Display for LlmInferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            Self::Timeout => write!(f, "Request timed out"),
            Self::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            Self::MaxRetriesExceeded => write!(f, "Maximum retries exceeded"),
        }
    }
}

impl std::error::Error for LlmInferenceError {}

/// LLM inference request for plagiarism evaluation
#[derive(Debug, Serialize)]
struct PlagiarismRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: String,
}

/// LLM inference client with retry logic
pub struct LlmInference {
    config: LlmInferenceConfig,
    client: reqwest::Client,
}

impl LlmInference {
    /// Creates a new LLM inference client
    pub fn new(config: LlmInferenceConfig) -> Result<Self, LlmInferenceError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|e| LlmInferenceError::HttpError(e.to_string()))?;
        Ok(Self { config, client })
    }

    pub fn config(&self) -> &LlmInferenceConfig {
        &self.config
    }

    /// Evaluate plagiarism with retry logic and exponential backoff
    pub async fn evaluate_plagiarism_with_retry(
        &self,
        code_a: &str,
        code_b: &str,
        similarity: f64,
    ) -> Result<PlagiarismVerdict, LlmInferenceError> {
        let mut last_error = None;
        let mut delay = self.config.retry_delay_ms;

        for attempt in 0..=self.config.max_retries {
            match self.evaluate_plagiarism_once(code_a, code_b, similarity).await {
                Ok(verdict) => {
                    verdict.validate()?;
                    return Ok(verdict);
                }
                Err(e) => {
                    last_error = Some(e.clone());
                    
                    if matches!(e, LlmInferenceError::InvalidResponse(_)) {
                        return Err(e);
                    }
                    
                    if attempt < self.config.max_retries {
                        sleep(Duration::from_millis(delay)).await;
                        delay *= 2;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(LlmInferenceError::MaxRetriesExceeded))
    }

    async fn evaluate_plagiarism_once(
        &self,
        code_a: &str,
        code_b: &str,
        similarity: f64,
    ) -> Result<PlagiarismVerdict, LlmInferenceError> {
        let prompt = self.build_prompt(code_a, code_b, similarity);
        
        let request = PlagiarismRequest {
            model: &self.config.model,
            messages: vec![Message {
                role: "user",
                content: prompt,
            }],
            stream: false,
        };

        let response = self.client
            .post(&self.config.endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LlmInferenceError::Timeout
                } else {
                    LlmInferenceError::HttpError(e.to_string())
                }
            })?;

        let status = response.status();
        let body = response.text().await
            .map_err(|e| LlmInferenceError::HttpError(e.to_string()))?;

        if !status.is_success() {
            return Err(LlmInferenceError::HttpError(
                format!("HTTP {}: {}", status, body)
            ));
        }

        self.parse_response(&body)
    }

    fn build_prompt(&self, code_a: &str, code_b: &str, similarity: f64) -> String {
        format!(
            "You are a plagiarism detection assistant.\n\n\
            Compare these two Python code snippets and determine if one is plagiarized from the other.\n\n\
            Code A:\n{}\n\n\
            Code B:\n{}\n\n\
            Structural similarity score: {:.1}%\n\n\
            Analyze the code structure, variable naming patterns, and logic flow.\n\
            Respond with a JSON object containing:\n\
            - is_plagiarism: boolean\n\
            - confidence: number between 0 and 1\n\
            - reasoning: string\n\n\
            JSON response:",
            code_a, code_b, similarity * 100.0
        )
    }

    fn parse_response(&self, body: &str) -> Result<PlagiarismVerdict, LlmInferenceError> {
        // Extract JSON from response
        let json_str = body.trim();
        
        // Try to find JSON object in the response
        let start = json_str.find('{').ok_or_else(|| {
            LlmInferenceError::InvalidResponse("No JSON object found".to_string())
        })?;
        let end = json_str.rfind('}').ok_or_else(|| {
            LlmInferenceError::InvalidResponse("No JSON object end found".to_string())
        })?;
        let json_obj = &json_str[start..=end];
        
        serde_json::from_str(json_obj).map_err(|e| {
            LlmInferenceError::InvalidResponse(format!("JSON parse error: {}", e))
        })
    }

    /// Simple evaluation without LLM (based on similarity threshold)
    pub async fn evaluate_plagiarism(
        &self,
        _code_a: &str,
        _code_b: &str,
        similarity: f64,
    ) -> Result<PlagiarismVerdict, LlmInferenceError> {
        Ok(PlagiarismVerdict {
            is_plagiarism: similarity > 0.8,
            confidence: similarity,
            reasoning: "Placeholder: use evaluate_plagiarism_with_retry".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = LlmInferenceConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 1000);
    }

    #[test]
    fn test_verdict_validation() {
        let valid = PlagiarismVerdict {
            is_plagiarism: true,
            confidence: 0.9,
            reasoning: "test".to_string(),
        };
        assert!(valid.validate().is_ok());

        let invalid = PlagiarismVerdict {
            is_plagiarism: true,
            confidence: 1.5,
            reasoning: "test".to_string(),
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", LlmInferenceError::Timeout), "Request timed out");
        assert_eq!(format!("{}", LlmInferenceError::MaxRetriesExceeded), "Maximum retries exceeded");
    }
}
