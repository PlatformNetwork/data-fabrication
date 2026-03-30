//! Types for LLM evaluation scores including multi-criteria breakdown.
//!
//! This module defines the core types for scoring conversations and datasets
//! based on multiple criteria: diversity (thematic/structural), uniqueness,
//! and semantic quality.

use serde::{Deserialize, Serialize};

/// Error type for score validation failures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScoreError {
    /// Score value is below the minimum allowed (0.0).
    BelowMin { value: f64, min: f64 },
    /// Score value is above the maximum allowed (1.0).
    AboveMax { value: f64, max: f64 },
    /// Score is NaN.
    NotANumber,
}

impl core::fmt::Display for ScoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ScoreError::BelowMin { value, min } => {
                write!(f, "Score {} is below minimum allowed {}", value, min)
            }
            ScoreError::AboveMax { value, max } => {
                write!(f, "Score {} is above maximum allowed {}", value, max)
            }
            ScoreError::NotANumber => write!(f, "Score value is NaN"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ScoreError {}

/// Validates that a score is within the valid range [0.0, 1.0].
///
/// # Arguments
/// * `value` - The score value to validate.
///
/// # Returns
/// * `Ok(value)` if the score is valid.
/// * `Err(ScoreError)` if the score is outside the valid range.
///
/// # Examples
/// ```
/// use data_fabrication_core::scoring_types::{validate_score, ScoreError};
///
/// assert_eq!(validate_score(0.5), Ok(0.5));
/// assert_eq!(validate_score(0.0), Ok(0.0));
/// assert_eq!(validate_score(1.0), Ok(1.0));
/// assert!(matches!(validate_score(-0.1), Err(ScoreError::BelowMin { .. })));
/// assert!(matches!(validate_score(1.1), Err(ScoreError::AboveMax { .. })));
/// ```
pub fn validate_score(value: f64) -> Result<f64, ScoreError> {
    const MIN: f64 = 0.0;
    const MAX: f64 = 1.0;

    if value.is_nan() {
        return Err(ScoreError::NotANumber);
    }
    if value < MIN {
        return Err(ScoreError::BelowMin { value, min: MIN });
    }
    if value > MAX {
        return Err(ScoreError::AboveMax { value, max: MAX });
    }
    Ok(value)
}

/// Multi-criteria scores for evaluation.
///
/// Each criterion is scored on a scale of 0.0 to 1.0.
/// The overall score is typically a weighted average of these criteria.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriteriaScores {
    /// Thematic diversity - measures topic diversity across the dataset.
    pub diversity_thematic: f64,
    /// Structural diversity - measures format and length diversity.
    pub diversity_structural: f64,
    /// Uniqueness - measures avoidance of repeating patterns.
    pub uniqueness: f64,
    /// Semantic quality - measures semantic coherence and relevance.
    pub quality_semantic: f64,
}

impl CriteriaScores {
    /// Creates new criteria scores with validation.
    ///
    /// # Errors
    /// Returns `ScoreError` if any score is outside [0.0, 1.0].
    pub fn new(
        diversity_thematic: f64,
        diversity_structural: f64,
        uniqueness: f64,
        quality_semantic: f64,
    ) -> Result<Self, ScoreError> {
        Ok(Self {
            diversity_thematic: validate_score(diversity_thematic)?,
            diversity_structural: validate_score(diversity_structural)?,
            uniqueness: validate_score(uniqueness)?,
            quality_semantic: validate_score(quality_semantic)?,
        })
    }

    /// Calculates the weighted average using equal weights (0.25 each).
    pub fn weighted_average(&self) -> f64 {
        const WEIGHT: f64 = 0.25;
        (self.diversity_thematic
            + self.diversity_structural
            + self.uniqueness
            + self.quality_semantic)
            * WEIGHT
    }
}

/// LLM evaluation score with multi-criteria breakdown.
///
/// Represents the result of an LLM evaluating a conversation or dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmEvaluationScore {
    /// Overall score (0.0 to 1.0), typically the weighted average of criteria.
    pub overall: f64,
    /// Breakdown by individual criteria.
    pub criteria: CriteriaScores,
    /// Reasoning for the scores assigned.
    pub reasoning: String,
    /// Brief summary of the evaluation.
    pub summary: String,
}

impl LlmEvaluationScore {
    /// Creates a new LLM evaluation score with validation.
    ///
    /// # Errors
    /// Returns `ScoreError` if overall or any criterion is outside [0.0, 1.0].
    pub fn new(
        overall: f64,
        criteria: CriteriaScores,
        reasoning: String,
        summary: String,
    ) -> Result<Self, ScoreError> {
        Ok(Self {
            overall: validate_score(overall)?,
            criteria,
            reasoning,
            summary,
        })
    }

    /// Creates a score with the overall computed from criteria (equal weights).
    pub fn from_criteria(criteria: CriteriaScores, reasoning: String, summary: String) -> Self {
        let overall = criteria.weighted_average();
        Self {
            overall,
            criteria,
            reasoning,
            summary,
        }
    }
}

/// Score for a single conversation in the dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationScore {
    /// Unique identifier for the conversation.
    pub conversation_id: u64,
    /// LLM evaluation score for this conversation.
    pub score: LlmEvaluationScore,
}

/// Aggregated score for an entire dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetScore {
    /// Individual scores for each conversation in the dataset.
    pub scores: Vec<ConversationScore>,
    /// Aggregated overall score (average of all conversation scores).
    pub aggregated: f64,
    /// Summary of the dataset evaluation.
    pub summary: String,
}

impl DatasetScore {
    /// Creates a new dataset score from individual conversation scores.
    ///
    /// The aggregated score is computed as the average of all conversation
    /// overall scores. Returns None if the scores vector is empty.
    pub fn new(scores: Vec<ConversationScore>, summary: String) -> Option<Self> {
        if scores.is_empty() {
            return None;
        }

        let aggregated = scores.iter().map(|s| s.score.overall).sum::<f64>() / scores.len() as f64;

        // aggregated is guaranteed to be in [0.0, 1.0] since all individual scores are
        Some(Self {
            scores,
            aggregated,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_validation_valid() {
        assert_eq!(validate_score(0.0), Ok(0.0));
        assert_eq!(validate_score(0.5), Ok(0.5));
        assert_eq!(validate_score(1.0), Ok(1.0));
    }

    #[test]
    fn test_score_validation_too_high() {
        let result = validate_score(1.5);
        assert!(matches!(
            result,
            Err(ScoreError::AboveMax {
                value: 1.5,
                max: 1.0
            })
        ));
    }

    #[test]
    fn test_score_validation_negative() {
        let result = validate_score(-0.1);
        assert!(matches!(
            result,
            Err(ScoreError::BelowMin {
                value: -0.1,
                min: 0.0
            })
        ));
    }

    #[test]
    fn test_score_validation_nan() {
        let result = validate_score(f64::NAN);
        assert!(matches!(result, Err(ScoreError::NotANumber)));
    }

    #[test]
    fn test_criteria_scores_weighted_average() {
        // When all criteria are 0.75, the average should be 0.75
        let criteria = CriteriaScores::new(0.75, 0.75, 0.75, 0.75).unwrap();
        assert!((criteria.weighted_average() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_aggregation_equals_overall() {
        // Test that weighted average of 4 criteria equals overall(from_criteria)
        let criteria = CriteriaScores::new(0.5, 0.6, 0.7, 0.8).unwrap();
        let expected = (0.5 + 0.6 + 0.7 + 0.8) / 4.0;

        let score = LlmEvaluationScore::from_criteria(
            criteria,
            "Test reasoning".to_string(),
            "Test summary".to_string(),
        );

        assert!((score.overall - expected).abs() < f64::EPSILON);
    }

    #[test]
    fn test_criteria_scores_validation() {
        // Valid criteria
        assert!(CriteriaScores::new(0.5, 0.5, 0.5, 0.5).is_ok());

        // Invalid - one criterion too high
        let result = CriteriaScores::new(0.5, 1.5, 0.5, 0.5);
        assert!(matches!(result, Err(ScoreError::AboveMax { .. })));

        // Invalid - one criterion negative
        let result = CriteriaScores::new(-0.1, 0.5, 0.5, 0.5);
        assert!(matches!(result, Err(ScoreError::BelowMin { .. })));
    }

    #[test]
    fn test_conversation_score() {
        let criteria = CriteriaScores::new(1.0, 1.0, 1.0, 1.0).unwrap();
        let llm_score = LlmEvaluationScore::from_criteria(
            criteria,
            "Perfect score".to_string(),
            "All criteria met".to_string(),
        );
        let conv_score = ConversationScore {
            conversation_id: 42,
            score: llm_score,
        };

        assert_eq!(conv_score.conversation_id, 42);
        assert!((conv_score.score.overall - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dataset_score_aggregation() {
        let criteria1 = CriteriaScores::new(0.5, 0.5, 0.5, 0.5).unwrap();
        let criteria2 = CriteriaScores::new(1.0, 1.0, 1.0, 1.0).unwrap();

        let score1 = LlmEvaluationScore::from_criteria(
            criteria1,
            "Reasoning 1".to_string(),
            "Summary 1".to_string(),
        );
        let score2 = LlmEvaluationScore::from_criteria(
            criteria2,
            "Reasoning 2".to_string(),
            "Summary 2".to_string(),
        );

        let conv1 = ConversationScore {
            conversation_id: 1,
            score: score1,
        };
        let conv2 = ConversationScore {
            conversation_id: 2,
            score: score2,
        };

        let dataset = DatasetScore::new(vec![conv1, conv2], "Dataset summary".to_string());

        assert!(dataset.is_some());
        let dataset = dataset.unwrap();
        // Average of 0.5 and 1.0 should be 0.75
        assert!((dataset.aggregated - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dataset_score_empty() {
        let dataset = DatasetScore::new(vec![], "Empty dataset".to_string());
        assert!(dataset.is_none());
    }
}
