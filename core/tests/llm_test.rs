//! Integration tests for LLM client and consensus modules.
//!
//! Tests mock LLM responses, rate limit handling, score parsing,
//! aggregation, and consensus mechanisms.

use data_fabrication_core::cache::{hash_conversation, EvaluationCache, DEFAULT_TTL};
use data_fabrication_core::consensus::consensus;
use data_fabrication_core::llm_client::{LlmClient, MockLlmClient, WasmLlmClient};
use data_fabrication_core::scoring_types::{CriteriaScores, DatasetScore, LlmEvaluationScore};
use data_fabrication_core::{ConversationEntry, Message};
use std::time::Duration;

fn create_test_conversation(messages: Vec<(&str, &str)>) -> ConversationEntry {
    ConversationEntry {
        messages: messages
            .into_iter()
            .map(|(role, content)| Message {
                role: role.to_string(),
                content: content.to_string(),
                name: None,
                function_call: None,
            })
            .collect(),
        function_calls: None,
        thinking: None,
    }
}

fn create_test_score(overall: f64) -> LlmEvaluationScore {
    let criteria = CriteriaScores::new(overall, overall, overall, overall).unwrap();
    LlmEvaluationScore::from_criteria(
        criteria,
        format!("Score {:.2}", overall),
        format!("Summary for {:.2}", overall),
    )
}

// ============================================================================
// Mock LLM Response Tests
// ============================================================================

#[test]
fn test_mock_llm_returns_predefined_score() {
    let criteria = CriteriaScores::new(0.9, 0.8, 0.85, 0.75).unwrap();
    let expected_score = LlmEvaluationScore::new(
        0.85,
        criteria.clone(),
        "Test reasoning".to_string(),
        "Test summary".to_string(),
    )
    .unwrap();

    let client = MockLlmClient::new(expected_score.clone());
    let conversation =
        create_test_conversation(vec![("user", "Hello"), ("assistant", "Hi there!")]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(client.evaluate_conversation(&conversation))
        .unwrap();

    assert_eq!(result.overall, expected_score.overall);
    assert_eq!(result.reasoning, expected_score.reasoning);
    assert_eq!(result.criteria.diversity_thematic, 0.9);
}

#[test]
fn test_mock_llm_perfect_score() {
    let client = MockLlmClient::perfect();
    let conversation = create_test_conversation(vec![("user", "Test")]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(client.evaluate_conversation(&conversation))
        .unwrap();

    assert!(
        (result.overall - 1.0).abs() < f64::EPSILON,
        "Perfect client should return 1.0"
    );
    assert!(
        (result.criteria.diversity_thematic - 1.0).abs() < f64::EPSILON,
        "All criteria should be 1.0"
    );
    assert!(
        (result.criteria.diversity_structural - 1.0).abs() < f64::EPSILON,
        "All criteria should be 1.0"
    );
}

#[test]
fn test_mock_llm_zero_score() {
    let client = MockLlmClient::zero();
    let conversation = create_test_conversation(vec![("user", "Test")]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(client.evaluate_conversation(&conversation))
        .unwrap();

    assert!(
        (result.overall - 0.0).abs() < f64::EPSILON,
        "Zero client should return 0.0"
    );
    assert!(
        (result.criteria.uniqueness - 0.0).abs() < f64::EPSILON,
        "All criteria should be 0.0"
    );
}

#[test]
fn test_mock_llm_custom_score_values() {
    let criteria = CriteriaScores::new(0.5, 0.6, 0.7, 0.8).unwrap();
    let score = LlmEvaluationScore::new(
        0.65,
        criteria,
        "Mixed scores".to_string(),
        "Mix of criteria values".to_string(),
    )
    .unwrap();

    let client = MockLlmClient::new(score.clone());
    let conversation = create_test_conversation(vec![
        ("system", "You are helpful"),
        ("user", "Hello"),
        ("assistant", "Hi!"),
    ]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt
        .block_on(client.evaluate_conversation(&conversation))
        .unwrap();

    assert!((result.criteria.diversity_thematic - 0.5).abs() < f64::EPSILON);
    assert!((result.criteria.diversity_structural - 0.6).abs() < f64::EPSILON);
    assert!((result.criteria.uniqueness - 0.7).abs() < f64::EPSILON);
    assert!((result.criteria.quality_semantic - 0.8).abs() < f64::EPSILON);
}

#[test]
fn test_mock_llm_ignores_conversation_content() {
    let score1 = create_test_score(0.75);
    let client = MockLlmClient::new(score1.clone());

    let conversation1 = create_test_conversation(vec![("user", "Short")]);
    let conversation2 = create_test_conversation(vec![
        ("user", "This is a much longer message"),
        ("assistant", "And a longer response too"),
        ("user", "Follow up question"),
    ]);

    let rt = tokio::runtime::Runtime::new().unwrap();

    let result1 = rt
        .block_on(client.evaluate_conversation(&conversation1))
        .unwrap();
    let result2 = rt
        .block_on(client.evaluate_conversation(&conversation2))
        .unwrap();

    assert_eq!(
        result1.overall, result2.overall,
        "Mock should return same score regardless of conversation content"
    );
}

// ============================================================================
// WasmLlmClient Tests (Error Handling)
// ============================================================================

#[test]
fn test_wasm_client_returns_error() {
    let client = WasmLlmClient::new("test-model".to_string());
    let conversation = create_test_conversation(vec![("user", "Hello")]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(client.evaluate_conversation(&conversation));

    assert!(
        result.is_err(),
        "WASM client should return error without host functions"
    );

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("WASM host function not available"),
        "Error should mention WASM host function"
    );
}

// ============================================================================
// Score Parsing Tests
// ============================================================================

#[test]
fn test_criteria_scores_validation() {
    let valid = CriteriaScores::new(0.5, 0.6, 0.7, 0.8);
    assert!(valid.is_ok(), "Valid scores should be accepted");

    let invalid_high = CriteriaScores::new(1.5, 0.5, 0.5, 0.5);
    assert!(invalid_high.is_err(), "Score above 1.0 should be rejected");

    let invalid_low = CriteriaScores::new(-0.1, 0.5, 0.5, 0.5);
    assert!(invalid_low.is_err(), "Negative score should be rejected");
}

#[test]
fn test_llm_evaluation_score_validation() {
    let criteria = CriteriaScores::new(0.5, 0.5, 0.5, 0.5).unwrap();

    let valid = LlmEvaluationScore::new(
        0.5,
        criteria.clone(),
        "Reasoning".to_string(),
        "Summary".to_string(),
    );
    assert!(valid.is_ok(), "Valid overall score should be accepted");

    let invalid = LlmEvaluationScore::new(
        1.5,
        criteria,
        "Reasoning".to_string(),
        "Summary".to_string(),
    );
    assert!(
        invalid.is_err(),
        "Overall score above 1.0 should be rejected"
    );
}

#[test]
fn test_llm_score_from_criteria_computes_weighted_average() {
    let criteria = CriteriaScores::new(0.5, 0.6, 0.7, 0.8).unwrap();
    let expected = (0.5 + 0.6 + 0.7 + 0.8) / 4.0;

    let score = LlmEvaluationScore::from_criteria(
        criteria,
        "Auto-computed".to_string(),
        "Weighted average".to_string(),
    );

    assert!(
        (score.overall - expected).abs() < f64::EPSILON,
        "Overall should be weighted average of criteria"
    );
}

// ============================================================================
// Score Aggregation Tests
// ============================================================================

#[test]
fn test_dataset_score_aggregation_basic() {
    use data_fabrication_core::scoring_types::ConversationScore;

    let score1 = create_test_score(0.5);
    let score2 = create_test_score(1.0);

    let conv1 = ConversationScore {
        conversation_id: 1,
        score: score1,
    };
    let conv2 = ConversationScore {
        conversation_id: 2,
        score: score2,
    };

    let dataset = DatasetScore::new(vec![conv1, conv2], "Test dataset".to_string());

    assert!(dataset.is_some(), "Should aggregate non-empty scores");
    let dataset = dataset.unwrap();

    assert!(
        (dataset.aggregated - 0.75).abs() < f64::EPSILON,
        "Average of 0.5 and 1.0 should be 0.75"
    );
}

#[test]
fn test_dataset_score_empty_scores() {
    let dataset = DatasetScore::new(vec![], "Empty dataset".to_string());
    assert!(dataset.is_none(), "Empty dataset should return None");
}

#[test]
fn test_dataset_score_multiple_conversations() {
    use data_fabrication_core::scoring_types::ConversationScore;

    let scores: Vec<f64> = vec![0.3, 0.5, 0.7, 0.9];
    let conv_scores: Vec<ConversationScore> = scores
        .iter()
        .enumerate()
        .map(|(i, &s)| ConversationScore {
            conversation_id: i as u64,
            score: create_test_score(s),
        })
        .collect();

    let dataset =
        DatasetScore::new(conv_scores, "Multi-conversation".to_string()).expect("Should aggregate");

    let expected = scores.iter().sum::<f64>() / scores.len() as f64;
    assert!(
        (dataset.aggregated - expected).abs() < f64::EPSILON,
        "Aggregation should compute correct average"
    );
}

// ============================================================================
// Consensus Mechanism Tests
// ============================================================================

#[test]
fn test_consensus_single_validator() {
    let scores = vec![create_test_score(0.85)];
    let result = consensus(&scores).unwrap();

    assert!(
        (result.final_score - 0.85).abs() < 0.001,
        "Single score should pass through"
    );
    assert_eq!(result.validator_count, 1, "Should report 1 validator");
    assert!(
        result.outlier_indices.is_empty(),
        "No outliers with single score"
    );
}

#[test]
fn test_consensus_agreement_between_validators() {
    let scores = vec![
        create_test_score(0.80),
        create_test_score(0.82),
        create_test_score(0.81),
    ];
    let result = consensus(&scores).unwrap();

    assert!(
        (result.final_score - 0.81).abs() < 0.01,
        "Average of close scores"
    );
    assert_eq!(result.validator_count, 3);
    assert!(
        result.agreement_level > 0.5,
        "Close scores should have high agreement"
    );
}

#[test]
fn test_consensus_outlier_detection() {
    let scores = vec![
        create_test_score(0.80),
        create_test_score(0.85),
        create_test_score(0.82),
        create_test_score(0.5),
    ];
    let result = consensus(&scores).unwrap();

    assert!(
        result.outlier_indices.contains(&3),
        "Score 0.5 should be an outlier (deviates > 0.2 from mean ~0.74)"
    );
}

#[test]
fn test_consensus_excludes_outliers() {
    let scores = vec![
        create_test_score(0.8),
        create_test_score(0.8),
        create_test_score(0.8),
        create_test_score(0.1),
    ];
    let result = consensus(&scores).unwrap();

    assert!(
        (result.final_score - 0.8).abs() < 0.01,
        "Final score should use non-outliers"
    );
    assert!(
        result.outlier_indices.contains(&3),
        "Deviant score should be outlier"
    );
}

#[test]
fn test_consensus_empty_scores_error() {
    let scores: Vec<LlmEvaluationScore> = vec![];
    let result = consensus(&scores);

    assert!(result.is_err(), "Empty scores should return error");

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("No scores"),
        "Error should mention no scores"
    );
}

#[test]
fn test_consensus_agreement_level_high() {
    let scores = vec![
        create_test_score(0.80),
        create_test_score(0.805),
        create_test_score(0.802),
    ];
    let result = consensus(&scores).unwrap();

    assert!(
        result.agreement_level > 0.9,
        "Very close scores should have very high agreement"
    );
}

#[test]
fn test_consensus_agreement_level_low() {
    let scores = vec![create_test_score(0.0), create_test_score(1.0)];
    let result = consensus(&scores).unwrap();

    assert!(
        result.agreement_level < 0.5,
        "Widely divergent scores should have low agreement"
    );
}

#[test]
fn test_consensus_all_outliers_fallback() {
    let scores = vec![
        create_test_score(0.1),
        create_test_score(0.5),
        create_test_score(0.9),
    ];
    let result = consensus(&scores).unwrap();

    assert!(
        result.final_score >= 0.0 && result.final_score <= 1.0,
        "Should produce valid score even with all potential outliers"
    );
}

#[test]
fn test_consensus_result_fields() {
    let scores = vec![create_test_score(0.75)];
    let result = consensus(&scores).unwrap();

    assert!(result.final_score >= 0.0 && result.final_score <= 1.0);
    assert!(result.agreement_level >= 0.0 && result.agreement_level <= 1.0);
    assert_eq!(result.validator_count, 1);
}

// ============================================================================
// Cache Tests
// ============================================================================

#[test]
fn test_cache_hit_with_same_conversation() {
    let mut cache = EvaluationCache::new();
    let conversation = create_test_conversation(vec![("user", "Hello, world!")]);
    let score = create_test_score(0.75);

    cache.insert(&conversation, score.clone());
    let cached = cache.get(&conversation);

    assert!(
        cached.is_some(),
        "Should find cached score for same conversation"
    );
    assert!((cached.unwrap().overall - 0.75).abs() < f64::EPSILON);
}

#[test]
fn test_cache_miss_with_different_conversation() {
    let mut cache = EvaluationCache::new();
    let conversation1 = create_test_conversation(vec![("user", "Hello")]);
    let conversation2 = create_test_conversation(vec![("user", "Goodbye")]);

    cache.insert(&conversation1, create_test_score(0.5));

    let cached = cache.get(&conversation2);
    assert!(
        cached.is_none(),
        "Different conversation should not hit cache"
    );
}

#[test]
fn test_cache_expiry_after_ttl() {
    let mut cache = EvaluationCache::with_ttl(Duration::from_millis(10));
    let conversation = create_test_conversation(vec![("user", "Expiring")]);
    cache.insert(&conversation, create_test_score(0.5));

    std::thread::sleep(Duration::from_millis(20));

    let cached = cache.get(&conversation);
    assert!(cached.is_none(), "Cache entry should expire after TTL");
}

#[test]
fn test_cache_hash_consistency() {
    let conversation1 = create_test_conversation(vec![("user", "Same content")]);
    let conversation2 = create_test_conversation(vec![("user", "Same content")]);

    let hash1 = hash_conversation(&conversation1);
    let hash2 = hash_conversation(&conversation2);

    assert_eq!(hash1, hash2, "Same content should produce same hash");
    assert_eq!(hash1.len(), 64, "SHA-256 hash should be 64 hex characters");
}

#[test]
fn test_cache_hash_uniqueness() {
    let conversation1 = create_test_conversation(vec![("user", "Content A")]);
    let conversation2 = create_test_conversation(vec![("user", "Content B")]);

    let hash1 = hash_conversation(&conversation1);
    let hash2 = hash_conversation(&conversation2);

    assert_ne!(
        hash1, hash2,
        "Different content should produce different hashes"
    );
}

#[test]
fn test_cache_by_hash_operations() {
    let mut cache = EvaluationCache::new();
    let custom_hash = "custom_test_hash_123".to_string();
    let score = create_test_score(0.9);

    cache.insert_by_hash(&custom_hash, score.clone());
    let cached = cache.get_by_hash(&custom_hash);

    assert!(cached.is_some(), "Should find score by custom hash");
    assert!((cached.unwrap().overall - 0.9).abs() < f64::EPSILON);
}

#[test]
fn test_cache_cleanup_expired() {
    let mut cache = EvaluationCache::with_ttl(Duration::from_millis(10));
    let conv1 = create_test_conversation(vec![("user", "First")]);
    let conv2 = create_test_conversation(vec![("user", "Second")]);

    cache.insert(&conv1, create_test_score(0.5));
    cache.insert(&conv2, create_test_score(0.6));

    assert_eq!(cache.len(), 2, "Should have 2 entries");

    std::thread::sleep(Duration::from_millis(20));

    cache.cleanup_expired();
    assert_eq!(cache.len(), 0, "All entries should be expired");
    assert!(cache.is_empty());
}

#[test]
fn test_cache_default_ttl() {
    assert_eq!(
        DEFAULT_TTL,
        Duration::from_secs(24 * 60 * 60),
        "Default TTL should be 24 hours"
    );
}

#[test]
fn test_cache_len_and_empty() {
    let mut cache = EvaluationCache::new();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);

    let conv = create_test_conversation(vec![("user", "Test")]);
    cache.insert(&conv, create_test_score(0.5));

    assert!(!cache.is_empty());
    assert_eq!(cache.len(), 1);
}

// ============================================================================
// Integration: Mock LLM + Consensus
// ============================================================================

#[test]
fn test_multiple_mock_clients_consensus() {
    let client1 = MockLlmClient::new(create_test_score(0.8));
    let client2 = MockLlmClient::new(create_test_score(0.9));
    let client3 = MockLlmClient::new(create_test_score(0.85));

    let conversation =
        create_test_conversation(vec![("user", "Test consensus"), ("assistant", "Response")]);

    let rt = tokio::runtime::Runtime::new().unwrap();

    let score1 = rt
        .block_on(client1.evaluate_conversation(&conversation))
        .unwrap();
    let score2 = rt
        .block_on(client2.evaluate_conversation(&conversation))
        .unwrap();
    let score3 = rt
        .block_on(client3.evaluate_conversation(&conversation))
        .unwrap();

    let scores = vec![score1, score2, score3];
    let result = consensus(&scores).unwrap();

    assert!(
        (result.final_score - 0.85).abs() < 0.01,
        "Consensus of 0.8, 0.9, 0.85 should average to ~0.85"
    );
    assert_eq!(result.validator_count, 3);
}
