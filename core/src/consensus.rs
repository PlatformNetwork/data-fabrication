//! Consensus mechanism for validator score aggregation.

use crate::error::{DataFabricationError, Result};
use crate::scoring_types::LlmEvaluationScore;

/// Threshold for outlier detection (deviation from mean).
const OUTLIER_THRESHOLD: f64 = 0.2;

/// Result of consensus calculation.
#[derive(Debug, Clone)]
pub struct ConsensusResult {
    /// The final consensus score (average of non-outliers).
    pub final_score: f64,
    /// Level of agreement between validators (0.0-1.0).
    pub agreement_level: f64,
    /// Number of validators that contributed.
    pub validator_count: usize,
    /// Indices of scores that were identified as outliers.
    pub outlier_indices: Vec<usize>,
}

/// Calculate consensus from multiple validator scores.
///
/// Returns the average score and identifies outliers (deviation > 0.2 from mean).
pub fn consensus(scores: &[LlmEvaluationScore]) -> Result<ConsensusResult> {
    if scores.is_empty() {
        return Err(DataFabricationError::ConsensusError {
            message: "No scores provided for consensus".to_string(),
            scores: vec![],
        });
    }

    let overall_scores: Vec<f64> = scores.iter().map(|s| s.overall).collect();

    // Calculate mean
    let mean: f64 = overall_scores.iter().sum::<f64>() / overall_scores.len() as f64;

    // Identify outliers
    let outlier_indices: Vec<usize> = overall_scores
        .iter()
        .enumerate()
        .filter(|(_, &score)| (score - mean).abs() > OUTLIER_THRESHOLD)
        .map(|(i, _)| i)
        .collect();

    // Calculate final score (average of non-outliers, or all if no outliers)
    let valid_scores: Vec<f64> = overall_scores
        .iter()
        .enumerate()
        .filter(|(i, _)| !outlier_indices.contains(i))
        .map(|(_, &s)| s)
        .collect();

    let final_score = if valid_scores.is_empty() {
        mean // Fall back to mean of all if all are outliers
    } else {
        valid_scores.iter().sum::<f64>() / valid_scores.len() as f64
    };

    // Calculate agreement level (1.0 - normalized deviation)
    let max_deviation = overall_scores
        .iter()
        .map(|&s| (s - mean).abs())
        .fold(0.0, f64::max);

    let agreement_level = 1.0 - (max_deviation / OUTLIER_THRESHOLD).min(1.0);

    Ok(ConsensusResult {
        final_score,
        agreement_level,
        validator_count: scores.len(),
        outlier_indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring_types::CriteriaScores;

    fn create_score(overall: f64) -> LlmEvaluationScore {
        let criteria = CriteriaScores::new(overall, overall, overall, overall).unwrap();
        LlmEvaluationScore::from_criteria(
            criteria,
            format!("Score {:.2}", overall),
            format!("Summary {:.2}", overall),
        )
    }

    #[test]
    fn test_consensus_single_score() {
        let scores = vec![create_score(0.85)];
        let result = consensus(&scores).unwrap();

        assert!((result.final_score - 0.85).abs() < 0.001);
        assert_eq!(result.validator_count, 1);
        assert!(result.outlier_indices.is_empty());
    }

    #[test]
    fn test_consensus_average() {
        let scores = vec![create_score(0.8), create_score(0.85), create_score(0.9)];
        let result = consensus(&scores).unwrap();

        assert!((result.final_score - 0.85).abs() < 0.01);
        assert_eq!(result.validator_count, 3);
        assert!(result.outlier_indices.is_empty());
    }

    #[test]
    fn test_consensus_identifies_outlier() {
        let scores = vec![
            create_score(0.8),
            create_score(0.85),
            create_score(0.9),
            create_score(0.5),
        ];
        let result = consensus(&scores).unwrap();

        assert!(result.outlier_indices.contains(&3));
        assert!((result.final_score - 0.85).abs() < 0.01);
    }

    #[test]
    fn test_consensus_empty_fails() {
        let scores: Vec<LlmEvaluationScore> = vec![];
        let result = consensus(&scores);

        assert!(result.is_err());
        match result {
            Err(DataFabricationError::ConsensusError { message, scores }) => {
                assert!(message.contains("No scores"));
                assert!(scores.is_empty());
            }
            _ => panic!("Expected ConsensusError"),
        }
    }

    #[test]
    fn test_consensus_agreement_level_high() {
        let scores = vec![create_score(0.80), create_score(0.82), create_score(0.81)];
        let result = consensus(&scores).unwrap();

        assert!(result.agreement_level > 0.5);
        assert!(result.agreement_level <= 1.0);
    }

    #[test]
    fn test_consensus_agreement_level_low() {
        let scores = vec![create_score(0.5), create_score(0.9)];
        let result = consensus(&scores).unwrap();

        assert!((result.agreement_level).abs() < 0.001);
    }

    #[test]
    fn test_consensus_all_outliers() {
        let scores = vec![create_score(0.1), create_score(0.5), create_score(0.9)];
        let result = consensus(&scores).unwrap();

        assert!(result.final_score >= 0.0 && result.final_score <= 1.0);
    }
}
