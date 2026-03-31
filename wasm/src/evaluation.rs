//! Dataset evaluation logic for JSONL conversations.
//!
//! This module provides functions to evaluate dataset submissions:
//! - JSONL parsing and validation
//! - Quality metric calculation
//! - Plagiarism/originality checking
//! - Integration with the scoring module

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use data_fabrication_core::ConversationEntry;
use platform_challenge_sdk_wasm::host_functions::host_log;

use crate::scoring::calculate_score;
use crate::types::{DatasetQualityMetrics, EvaluationResult, Submission};

/// Parse JSONL content into a vector of conversation entries.
///
/// # Arguments
/// * `content` - Raw bytes of the JSONL file
///
/// # Returns
/// * `Some(Vec<ConversationEntry>)` if parsing succeeds
/// * `None` if content is invalid or empty
pub fn parse_jsonl(content: &[u8]) -> Option<Vec<ConversationEntry>> {
    // Convert bytes to string
    let content_str = core::str::from_utf8(content).ok()?;

    if content_str.trim().is_empty() {
        host_log(2, "  parse_jsonl: empty content");
        return None;
    }

    let mut entries = Vec::new();
    let mut line_num = 0usize;

    for line in content_str.lines() {
        line_num += 1;

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Parse each line as JSON
        match parse_conversation_line(line) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                host_log(
                    3,
                    &format!("  parse_jsonl: failed at line {}: {}", line_num, e),
                );
                return None;
            }
        }
    }

    if entries.is_empty() {
        host_log(2, "  parse_jsonl: no valid entries found");
        return None;
    }

    host_log(
        2,
        &format!("  parse_jsonl: parsed {} conversations", entries.len()),
    );
    Some(entries)
}

/// Parse a single JSONL line into a ConversationEntry.
fn parse_conversation_line(line: &str) -> Result<ConversationEntry, String> {
    // Try to parse as ConversationEntry
    let entry: ConversationEntry =
        serde_json::from_str(line).map_err(|e| format!("invalid JSON: {}", e))?;

    // Validate minimum structure
    if entry.messages.is_empty() {
        return Err("messages array is empty".to_string());
    }

    // Require at least 2 messages (one turn)
    if entry.messages.len() < 2 {
        return Err(format!(
            "need at least 2 messages, got {}",
            entry.messages.len()
        ));
    }

    // Validate role alternation (user/assistant pattern)
    let mut expected_user = true;
    for msg in &entry.messages {
        let role = msg.role.as_str();
        if expected_user && role != "user" && role != "system" {
            return Err(format!("expected 'user' or 'system' role, got '{}'", role));
        }
        if !expected_user && role != "assistant" && role != "function" {
            return Err(format!(
                "expected 'assistant' or 'function' role, got '{}'",
                role
            ));
        }
        // Alternate after first message
        if expected_user && role != "system" {
            expected_user = false;
        } else if !expected_user {
            expected_user = true;
        }
    }

    Ok(entry)
}

/// Calculate quality metrics for a dataset of conversations.
///
/// Evaluates:
/// - Format score: Schema compliance and structure validity
/// - Quality score: Content quality, message length, coherence
/// - Originality score: Uniqueness of content
///
/// # Arguments
/// * `conversations` - Slice of parsed conversation entries
///
/// # Returns
/// DatasetQualityMetrics with scores in range [0.0, 1.0]
pub fn calculate_quality(conversations: &[ConversationEntry]) -> DatasetQualityMetrics {
    if conversations.is_empty() {
        return DatasetQualityMetrics {
            format_score: 0.0,
            quality_score: 0.0,
            originality_score: 0.0,
        };
    }

    let format_score = calculate_format_score(conversations);
    let quality_score = calculate_content_quality(conversations);
    let originality_score = calculate_originality_score(conversations);

    DatasetQualityMetrics {
        format_score,
        quality_score,
        originality_score,
    }
}

/// Calculate format validity score.
/// Checks schema compliance and structural correctness.
fn calculate_format_score(conversations: &[ConversationEntry]) -> f64 {
    if conversations.is_empty() {
        return 0.0;
    }

    let mut valid_count = 0usize;

    for entry in conversations {
        // Check messages array
        if entry.messages.is_empty() {
            continue;
        }

        // Check minimum message length requirement (at least 2 messages)
        if entry.messages.len() < 2 {
            continue;
        }

        // Validate all messages have content
        let all_have_content = entry
            .messages
            .iter()
            .all(|m| !m.content.is_empty() || m.function_call.is_some());

        if !all_have_content {
            continue;
        }

        // Validate role alternation
        let roles_valid = validate_role_sequence(&entry.messages);
        if !roles_valid {
            continue;
        }

        valid_count += 1;
    }

    (valid_count as f64 / conversations.len() as f64).clamp(0.0, 1.0)
}

/// Validate that role sequence alternates correctly.
fn validate_role_sequence(messages: &[data_fabrication_core::Message]) -> bool {
    if messages.is_empty() {
        return false;
    }

    // First message can be 'system', 'user', or 'assistant'
    let first_role = messages[0].role.as_str();
    if first_role != "system" && first_role != "user" && first_role != "assistant" {
        return false;
    }

    // Check remaining messages for valid role sequence
    for i in 1..messages.len() {
        let prev_role = messages[i - 1].role.as_str();
        let curr_role = messages[i].role.as_str();

        match (prev_role, curr_role) {
            ("system", "user") | ("system", "assistant") => {}
            ("user", "assistant") => {}
            ("user", "function") => {}
            ("assistant", "user") => {}
            ("assistant", "function") => {}
            ("function", "user") | ("function", "assistant") => {}
            _ => return false,
        }
    }

    true
}

/// Calculate content quality score.
/// Evaluates message lengths, coherence, and response quality.
fn calculate_content_quality(conversations: &[ConversationEntry]) -> f64 {
    if conversations.is_empty() {
        return 0.0;
    }

    let mut total_score = 0.0;

    for entry in conversations {
        let entry_score = evaluate_conversation_quality(entry);
        total_score += entry_score;
    }

    (total_score / conversations.len() as f64).clamp(0.0, 1.0)
}

/// Evaluate quality of a single conversation.
fn evaluate_conversation_quality(entry: &ConversationEntry) -> f64 {
    let mut score = 0.0;

    // Message count score (more turns = better, up to a point)
    let msg_count = entry.messages.len();
    let turn_score = if msg_count >= 4 && msg_count <= 20 {
        0.3 // Optimal range
    } else if msg_count >= 2 {
        0.2 // Minimum acceptable
    } else {
        0.0 // Too short
    };
    score += turn_score;

    // Average message length score
    let total_content_len: usize = entry.messages.iter().map(|m| m.content.len()).sum();
    let avg_len = total_content_len / msg_count.max(1);

    let length_score = if avg_len >= 50 && avg_len <= 500 {
        0.3 // Good message length
    } else if avg_len >= 10 {
        0.15 // Acceptable
    } else {
        0.0 // Too short
    };
    score += length_score;

    // Response diversity score (check for repetitive content)
    let diversity_score = calculate_response_diversity(entry);
    score += diversity_score * 0.4;

    score.clamp(0.0, 1.0)
}

/// Calculate response diversity within a conversation.
fn calculate_response_diversity(entry: &ConversationEntry) -> f64 {
    if entry.messages.len() < 2 {
        return 0.0;
    }

    // Check for duplicate content
    let content_set: Vec<&str> = entry.messages.iter().map(|m| m.content.as_str()).collect();

    let unique_count = count_unique_content(&content_set);
    let uniqueness_ratio = unique_count as f64 / content_set.len() as f64;

    // Penalize high repetition
    if uniqueness_ratio < 0.5 {
        return uniqueness_ratio * 0.5;
    }

    uniqueness_ratio
}

/// Count unique content strings.
fn count_unique_content(contents: &[&str]) -> usize {
    let mut unique_count = 0;
    let mut seen = Vec::new();

    for &content in contents {
        let is_duplicate = seen.iter().any(|&prev: &&str| {
            // Simple similarity check: content identical or very similar
            content == prev || is_near_duplicate(content, prev)
        });

        if !is_duplicate {
            seen.push(content);
            unique_count += 1;
        }
    }

    unique_count
}

/// Check if two strings are near-duplicates (>90% similar).
fn is_near_duplicate(a: &str, b: &str) -> bool {
    if a.len() == 0 || b.len() == 0 {
        return false;
    }

    // Simple check: if one is substring of other and lengths differ by <10%
    let len_ratio = if a.len() > b.len() {
        b.len() as f64 / a.len() as f64
    } else {
        a.len() as f64 / b.len() as f64
    };

    if len_ratio < 0.9 {
        return false;
    }

    // Check for common prefix/suffix
    let min_len = a.len().min(b.len());
    let prefix_match = a
        .as_bytes()
        .iter()
        .zip(b.as_bytes().iter())
        .take(min_len)
        .filter(|(x, y)| x == y)
        .count();

    let similarity = prefix_match as f64 / min_len as f64;
    similarity > 0.9
}

/// Calculate originality score based on content uniqueness.
fn calculate_originality_score(conversations: &[ConversationEntry]) -> f64 {
    if conversations.is_empty() {
        return 0.0;
    }

    // Check for duplicated conversations
    let mut unique_hashes: Vec<u64> = Vec::new();

    for entry in conversations {
        let hash = hash_conversation_content(entry);

        // Check if we've seen this content before
        if unique_hashes.contains(&hash) {
            // Duplicate found, reduce score
            continue;
        }
        unique_hashes.push(hash);
    }

    // Originality is ratio of unique conversations
    let uniqueness_ratio = unique_hashes.len() as f64 / conversations.len() as f64;

    // Also check cross-conversation plagiarism within the dataset
    let plagiarism_penalty = check_internal_plagiarism(conversations);

    (uniqueness_ratio * (1.0 - plagiarism_penalty)).clamp(0.0, 1.0)
}

/// Simple hash of conversation content for deduplication.
fn hash_conversation_content(entry: &ConversationEntry) -> u64 {
    // Simple FNV-1a hash
    let mut hash: u64 = 14695981039346656037; // FNV offset basis

    for msg in &entry.messages {
        for byte in msg.content.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(1099511628211); // FNV prime
        }
    }

    hash
}

/// Check for internal plagiarism within submitted conversations.
/// Returns a penalty factor [0.0, 1.0] where 1.0 = high plagiarism.
fn check_internal_plagiarism(conversations: &[ConversationEntry]) -> f64 {
    if conversations.len() < 2 {
        return 0.0;
    }

    let n = conversations.len();
    let mut similarity_sum = 0.0;
    let mut comparisons = 0;

    // Compare each pair of conversations
    for i in 0..n {
        for j in (i + 1)..n {
            let similarity = compare_conversation_similarity(&conversations[i], &conversations[j]);
            similarity_sum += similarity;
            comparisons += 1;
        }
    }

    if comparisons == 0 {
        return 0.0;
    }

    let avg_similarity = similarity_sum / comparisons as f64;

    // Penalize high average similarity
    if avg_similarity > 0.8 {
        avg_similarity * 0.5 // Heavy penalty
    } else if avg_similarity > 0.5 {
        avg_similarity * 0.2 // Moderate penalty
    } else {
        0.0 // No penalty for low similarity
    }
}

/// Compare similarity between two conversations.
/// Returns [0.0, 1.0] where 1.0 = identical.
fn compare_conversation_similarity(a: &ConversationEntry, b: &ConversationEntry) -> f64 {
    // Compare message counts
    let len_ratio = if a.messages.len() > b.messages.len() {
        b.messages.len() as f64 / a.messages.len() as f64
    } else {
        a.messages.len() as f64 / b.messages.len() as f64
    };

    if len_ratio < 0.5 {
        return 0.0; // Very different lengths
    }

    // Only compare first message content (most indicative of duplication)
    if a.messages.is_empty() || b.messages.is_empty() {
        return 0.0;
    }

    let a_content = &a.messages[0].content;
    let b_content = &b.messages[0].content;

    // Jaccard similarity on word sets
    let a_words: Vec<&str> = a_content.split_whitespace().collect();
    let b_words: Vec<&str> = b_content.split_whitespace().collect();

    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }

    let mut intersection = 0;
    for word in &a_words {
        if b_words.contains(word) {
            intersection += 1;
        }
    }

    let union = a_words.len() + b_words.len() - intersection;
    if union == 0 {
        return 0.0;
    }

    (intersection as f64 / union as f64).clamp(0.0, 1.0)
}

/// Check for plagiarism against known datasets.
/// This is a simplified check - real implementation would use external services.
///
/// # Arguments
/// * `conversations` - Slice of parsed conversation entries
///
/// # Returns
/// Originality score [0.0, 1.0] where 1.0 = fully original
pub fn check_plagiarism(conversations: &[ConversationEntry]) -> f64 {
    if conversations.is_empty() {
        return 0.0;
    }

    // Check for known patterns from public datasets
    let mut originality_penalties = 0.0;

    for entry in conversations {
        // Check for suspiciously short responses (common in synthetic data)
        let short_response_count = entry
            .messages
            .iter()
            .filter(|m| m.role == "assistant" && m.content.len() < 20)
            .count();

        if short_response_count > entry.messages.len() / 2 {
            originality_penalties += 0.1;
        }

        // Check for generic responses
        let generic_count = entry
            .messages
            .iter()
            .filter(|m| is_generic_response(&m.content))
            .count();

        if generic_count > entry.messages.len() / 3 {
            originality_penalties += 0.1;
        }
    }

    let penalty_per_entry = originality_penalties / conversations.len() as f64;
    (1.0 - penalty_per_entry).clamp(0.0, 1.0)
}

/// Check if content appears to be a generic template response.
fn is_generic_response(content: &str) -> bool {
    let generic_phrases = [
        "I understand.",
        "Can you provide more details?",
        "I'm here to help.",
        "Please let me know",
        "Is there anything else",
        "Thank you for your question.",
    ];

    let lower = content.to_lowercase();
    generic_phrases
        .iter()
        .any(|phrase| lower.contains(&phrase.to_lowercase()))
}

/// Evaluate a complete dataset submission.
///
/// This is the main entry point for dataset evaluation.
///
/// # Arguments
/// * `submission` - The miner's submission
///
/// # Returns
/// EvaluationResult with score and metrics
pub fn evaluate_dataset(submission: &Submission) -> EvaluationResult {
    host_log(
        2,
        &format!(
            "evaluating dataset for hotkey={}, epoch={}",
            submission.hotkey, submission.epoch
        ),
    );

    // Parse the package as JSONL content
    let conversations = match parse_jsonl(&submission.package) {
        Some(c) => c,
        None => {
            return EvaluationResult {
                passed: false,
                score: 0.0,
                conversation_count: 0,
                total_messages: 0,
                size_bytes: submission.package.len() as u64,
                error: Some("Failed to parse JSONL content".to_string()),
            };
        }
    };

    // Count total messages
    let total_messages: u32 = conversations.iter().map(|e| e.messages.len() as u32).sum();

    // Calculate quality metrics
    let metrics = calculate_quality(&conversations);

    // Check plagiarism/originality
    let plagiarism_score = check_plagiarism(&conversations);

    // Adjust originality with plagiarism check
    let adjusted_metrics = DatasetQualityMetrics {
        format_score: metrics.format_score,
        quality_score: metrics.quality_score,
        originality_score: (metrics.originality_score * plagiarism_score).clamp(0.0, 1.0),
    };

    // Calculate final score using scoring module
    let final_score = calculate_score(&adjusted_metrics);

    // Determine pass/fail threshold
    let passed = final_score >= 0.5 && !conversations.is_empty();

    host_log(
        2,
        &format!(
            "evaluation complete: passed={}, score={:.3}, conversations={}, messages={}",
            passed,
            final_score,
            conversations.len(),
            total_messages
        ),
    );

    EvaluationResult {
        passed,
        score: final_score,
        conversation_count: conversations.len() as u32,
        total_messages,
        size_bytes: submission.package.len() as u64,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn make_valid_jsonl() -> Vec<u8> {
        br#"{"messages":[{"role":"user","content":"Hello"},{"role":"assistant","content":"Hi there!"}]}
{"messages":[{"role":"user","content":"What is Rust?"},{"role":"assistant","content":"A systems programming language."}]}"#
            .to_vec()
    }

    fn make_submission(package: Vec<u8>) -> Submission {
        Submission {
            hotkey: "test_hotkey".to_string(),
            epoch: 1,
            code_hash: "test_hash".to_string(),
            package,
            signature: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_parse_jsonl_valid() {
        let content = make_valid_jsonl();
        let result = parse_jsonl(&content);
        assert!(result.is_some());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_parse_jsonl_empty() {
        let result = parse_jsonl(&[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_jsonl_invalid_json() {
        let content = b"{invalid json}".to_vec();
        let result = parse_jsonl(&content);
        assert!(result.is_none());
    }

    #[test]
    fn test_calculate_quality_empty() {
        let metrics = calculate_quality(&[]);
        assert!((metrics.format_score - 0.0).abs() < f64::EPSILON);
        assert!((metrics.quality_score - 0.0).abs() < f64::EPSILON);
        assert!((metrics.originality_score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_quality_valid() {
        let content = make_valid_jsonl();
        let conversations = parse_jsonl(&content).unwrap();
        let metrics = calculate_quality(&conversations);

        // Should have positive scores for valid data
        assert!(metrics.format_score > 0.0);
        assert!(metrics.quality_score > 0.0);
        assert!(metrics.originality_score >= 0.0);
    }

    #[test]
    fn test_evaluate_dataset_valid() {
        let package = make_valid_jsonl();
        let submission = make_submission(package);
        let result = evaluate_dataset(&submission);

        assert!(result.passed);
        assert!(result.score > 0.0);
        assert_eq!(result.conversation_count, 2);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_evaluate_dataset_invalid() {
        let package = b"not valid jsonl".to_vec();
        let submission = make_submission(package);
        let result = evaluate_dataset(&submission);

        assert!(!result.passed);
        assert!((result.score - 0.0).abs() < f64::EPSILON);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_check_plagiarism() {
        let content = make_valid_jsonl();
        let conversations = parse_jsonl(&content).unwrap();
        let score = check_plagiarism(&conversations);

        // Should return a valid score
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[test]
    fn test_is_near_duplicate() {
        assert!(is_near_duplicate("Hello world", "Hello world"));
        assert!(is_near_duplicate("Hello world", "Hello world!"));
        assert!(!is_near_duplicate(
            "Hello world",
            "Completely different text"
        ));
    }

    #[test]
    fn test_validate_role_sequence() {
        let valid_messages = vec![
            data_fabrication_core::Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
                name: None,
                function_call: None,
            },
            data_fabrication_core::Message {
                role: "assistant".to_string(),
                content: "Hi".to_string(),
                name: None,
                function_call: None,
            },
        ];
        assert!(validate_role_sequence(&valid_messages));

        let invalid_messages = vec![
            data_fabrication_core::Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
                name: None,
                function_call: None,
            },
            data_fabrication_core::Message {
                role: "user".to_string(),
                content: "Hello again".to_string(),
                name: None,
                function_call: None,
            },
        ];
        assert!(!validate_role_sequence(&invalid_messages));
    }
}
