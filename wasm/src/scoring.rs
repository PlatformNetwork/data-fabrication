//! Deterministic scoring functions for data-fabrication challenge.
//!
//! All functions are pure and produce identical outputs for identical inputs,
//! ensuring P2P consensus compatibility across validator nodes.

extern crate alloc;

use crate::types::DatasetQualityMetrics;

/// Weight constants for score calculation.
const FORMAT_WEIGHT: f64 = 0.2;
const QUALITY_WEIGHT: f64 = 0.4;
const ORIGINALITY_WEIGHT: f64 = 0.4;

/// Calculate overall score from quality metrics.
///
/// Formula: `format * 0.2 + quality * 0.4 + originality * 0.4`
///
/// Each component is clamped to [0.0, 1.0] before calculation.
pub fn calculate_score(metrics: &DatasetQualityMetrics) -> f64 {
    let format = metrics.format_score.clamp(0.0, 1.0);
    let quality = metrics.quality_score.clamp(0.0, 1.0);
    let originality = metrics.originality_score.clamp(0.0, 1.0);

    (format * FORMAT_WEIGHT) + (quality * QUALITY_WEIGHT) + (originality * ORIGINALITY_WEIGHT)
}

/// Aggregate multiple scores using weighted average.
///
/// Returns 0.0 for empty input to ensure deterministic behavior.
pub fn aggregate_scores(scores: &[f64]) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }

    let sum: f64 = scores.iter().sum();
    let count = scores.len() as f64;

    (sum / count).clamp(0.0, 1.0)
}

/// Normalize a score to a Bittensor weight.
///
/// Divides score by max_score and clamps to [0.0, 1.0].
/// Returns 0.0 if max_score <= 0.0.
pub fn to_weight(score: f64, max_score: f64) -> f64 {
    if max_score <= 0.0 {
        return 0.0;
    }

    (score / max_score).clamp(0.0, 1.0)
}

/// Normalize a slice of scores so they sum to 1.0.
///
/// Returns empty slice for empty input.
/// Returns equal weights if sum is zero.
pub fn normalize(scores: &[f64]) -> alloc::vec::Vec<f64> {
    if scores.is_empty() {
        return alloc::vec::Vec::new();
    }

    let sum: f64 = scores.iter().sum();

    if sum <= 0.0 {
        let equal_weight = 1.0 / scores.len() as f64;
        return alloc::vec![equal_weight; scores.len()];
    }

    scores.iter().map(|s| (s / sum).clamp(0.0, 1.0)).collect()
}

/// Apply time-based decay to a score.
///
/// Uses exponential decay: `score * 0.5^(blocks_since / half_life)`
///
/// # Arguments
/// * `score` - The original score
/// * `blocks_since` - Number of blocks since the score was achieved
/// * `half_life` - Blocks for score to decay to 50%
///
/// Returns the decayed score, minimum 0.0.
pub fn apply_decay(score: f64, blocks_since: u64, half_life: u64) -> f64 {
    if blocks_since == 0 || half_life == 0 {
        return score.clamp(0.0, 1.0);
    }

    let decay_factor = 0.5_f64.powf(blocks_since as f64 / half_life as f64);
    (score * decay_factor).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_metrics(format: f64, quality: f64, originality: f64) -> DatasetQualityMetrics {
        DatasetQualityMetrics {
            format_score: format,
            quality_score: quality,
            originality_score: originality,
        }
    }

    #[test]
    fn test_calculate_score_weights_sum_to_one() {
        let metrics = test_metrics(1.0, 1.0, 1.0);
        let score = calculate_score(&metrics);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_score_zero() {
        let metrics = test_metrics(0.0, 0.0, 0.0);
        let score = calculate_score(&metrics);
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_score_partial() {
        let metrics = test_metrics(0.5, 0.5, 0.5);
        let score = calculate_score(&metrics);
        assert!((score - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_score_clamps_negative() {
        let metrics = test_metrics(-0.5, -0.5, -0.5);
        let score = calculate_score(&metrics);
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_score_clamps_above_one() {
        let metrics = test_metrics(2.0, 2.0, 2.0);
        let score = calculate_score(&metrics);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_aggregate_scores_empty() {
        let result = aggregate_scores(&[]);
        assert!((result - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_aggregate_scores_single() {
        let result = aggregate_scores(&[0.5]);
        assert!((result - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_aggregate_scores_multiple() {
        let result = aggregate_scores(&[0.2, 0.4, 0.6, 0.8]);
        assert!((result - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_to_weight_normal() {
        let result = to_weight(0.5, 1.0);
        assert!((result - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_to_weight_zero_max() {
        let result = to_weight(0.5, 0.0);
        assert!((result - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_to_weight_negative_max() {
        let result = to_weight(0.5, -1.0);
        assert!((result - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_to_weight_clamps_above_one() {
        let result = to_weight(2.0, 1.0);
        assert!((result - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_empty() {
        let result = normalize(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_normalize_sums_to_one() {
        let result = normalize(&[1.0, 2.0, 3.0]);
        let sum: f64 = result.iter().sum();
        assert!((sum - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_zero_sum() {
        let result = normalize(&[0.0, 0.0, 0.0]);
        assert_eq!(result.len(), 3);
        let expected = 1.0 / 3.0;
        for weight in result {
            assert!((weight - expected).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_apply_decay_no_decay() {
        let result = apply_decay(1.0, 0, 100);
        assert!((result - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_apply_decay_half() {
        let result = apply_decay(1.0, 100, 100);
        assert!((result - 0.5).abs() < 0.0001);
    }

    #[test]
    fn test_apply_decay_quarter() {
        let result = apply_decay(1.0, 200, 100);
        assert!((result - 0.25).abs() < 0.0001);
    }

    #[test]
    fn test_apply_decay_zero_half_life() {
        let result = apply_decay(0.5, 100, 0);
        assert!((result - 0.5).abs() < f64::EPSILON);
    }
}
