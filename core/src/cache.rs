//! LLM evaluation cache for avoiding redundant API calls.
//!
//! This module provides an in-memory cache for storing LLM evaluation results
//! with a configurable time-to-live (TTL). This helps avoid redundant LLM API
//! calls when evaluating the same conversation multiple times.

use crate::scoring_types::LlmEvaluationScore;
use crate::ConversationEntry;
use alloc::string::String;
use core::time::Duration;
use sha2::{Digest, Sha256};

#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::time::Instant;

/// Default TTL for cache entries (24 hours).
pub const DEFAULT_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Cache entry with score and expiration time.
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
struct CacheEntry {
    score: LlmEvaluationScore,
    expires_at: Instant,
}

/// In-memory cache for LLM evaluation results.
///
/// Uses SHA-256 hashing of conversation content to identify duplicate
/// conversations. Entries automatically expire after the configured TTL.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct EvaluationCache {
    /// The cache storage (hash -> entry).
    cache: HashMap<String, CacheEntry>,
    /// Time-to-live for cache entries.
    ttl: Duration,
}

#[cfg(feature = "std")]
impl EvaluationCache {
    /// Creates a new cache with default TTL (24 hours).
    pub fn new() -> Self {
        Self::with_ttl(DEFAULT_TTL)
    }

    /// Creates a new cache with custom TTL.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            ttl,
        }
    }

    /// Get a cached score if present and not expired.
    pub fn get(&mut self, conversation: &ConversationEntry) -> Option<LlmEvaluationScore> {
        let hash = hash_conversation(conversation);
        self.get_by_hash(&hash)
    }

    /// Get a cached score by hash if present and not expired.
    pub fn get_by_hash(&mut self, hash: &str) -> Option<LlmEvaluationScore> {
        let now = Instant::now();

        if let Some(entry) = self.cache.get(hash) {
            if entry.expires_at > now {
                return Some(entry.score.clone());
            } else {
                // Entry expired, remove it
                self.cache.remove(hash);
            }
        }
        None
    }

    /// Insert a score into the cache.
    pub fn insert(&mut self, conversation: &ConversationEntry, score: LlmEvaluationScore) {
        let hash = hash_conversation(conversation);
        self.insert_by_hash(&hash, score);
    }

    /// Insert a score by hash.
    pub fn insert_by_hash(&mut self, hash: &str, score: LlmEvaluationScore) {
        let entry = CacheEntry {
            score,
            expires_at: Instant::now() + self.ttl,
        };
        self.cache.insert(hash.to_string(), entry);
    }

    /// Clear all expired entries.
    pub fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.cache.retain(|_, entry| entry.expires_at > now);
    }

    /// Get the number of entries (including potentially expired).
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

#[cfg(feature = "std")]
impl Default for EvaluationCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash a conversation using SHA-256 and return hex string.
///
/// The hash is computed by serializing the conversation to JSON first,
/// ensuring consistent hashing regardless of field ordering.
pub fn hash_conversation(conversation: &ConversationEntry) -> String {
    // Serialize conversation to JSON for consistent hashing
    // Using serde_json with alloc features for no_std compatibility
    let json = serde_json::to_string(conversation).unwrap_or_default();

    // Hash using SHA-256
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let result = hasher.finalize();

    // Convert to hex string
    hex_encode(&result)
}

/// Encode bytes as hex string using alloc.
fn hex_encode(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push(HEX_CHARS[(byte >> 4) as usize] as char);
        hex.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring_types::CriteriaScores;
    use crate::Message;

    fn create_test_conversation(content: &str) -> ConversationEntry {
        ConversationEntry {
            messages: vec![Message {
                role: "user".to_string(),
                content: content.to_string(),
                name: None,
                function_call: None,
            }],
            function_calls: None,
            thinking: None,
        }
    }

    fn create_test_score(value: f64) -> LlmEvaluationScore {
        let criteria = CriteriaScores::new(value, value, value, value).unwrap();
        LlmEvaluationScore::from_criteria(
            criteria,
            "Test reasoning".to_string(),
            "Test summary".to_string(),
        )
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = EvaluationCache::new();
        let conversation = create_test_conversation("Hello, world!");
        let score = create_test_score(0.75);

        cache.insert(&conversation, score.clone());
        let cached = cache.get(&conversation);

        assert!(cached.is_some());
        let cached = cached.unwrap();
        assert!((cached.overall - score.overall).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = EvaluationCache::new();
        let conversation = create_test_conversation("Not in cache");

        let cached = cache.get(&conversation);
        assert!(cached.is_none());
    }

    #[test]
    fn test_cache_expiry() {
        // Use a very short TTL for testing
        let mut cache = EvaluationCache::with_ttl(Duration::from_millis(10));
        let conversation = create_test_conversation("Expiring content");
        let score = create_test_score(0.5);

        cache.insert(&conversation, score);

        // Wait for TTL to expire
        std::thread::sleep(Duration::from_millis(20));

        // Should return None after expiry
        let cached = cache.get(&conversation);
        assert!(cached.is_none());

        // Cache should be empty after cleanup
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_hash_conversation() {
        let conv1 = create_test_conversation("Same content");
        let conv2 = create_test_conversation("Same content");

        let hash1 = hash_conversation(&conv1);
        let hash2 = hash_conversation(&conv2);

        // Same content should produce same hash
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 = 64 hex chars
    }

    #[test]
    fn test_hash_different() {
        let conv1 = create_test_conversation("Content A");
        let conv2 = create_test_conversation("Content B");

        let hash1 = hash_conversation(&conv1);
        let hash2 = hash_conversation(&conv2);

        // Different content should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_cache_by_hash() {
        let mut cache = EvaluationCache::new();
        let hash = "test_hash_value".to_string();
        let score = create_test_score(0.9);

        cache.insert_by_hash(&hash, score.clone());
        let cached = cache.get_by_hash(&hash);

        assert!(cached.is_some());
        assert!((cached.unwrap().overall - score.overall).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cleanup_expired() {
        let mut cache = EvaluationCache::with_ttl(Duration::from_millis(10));
        let conv1 = create_test_conversation("First");
        let conv2 = create_test_conversation("Second");

        cache.insert(&conv1, create_test_score(0.5));
        cache.insert(&conv2, create_test_score(0.6));

        assert_eq!(cache.len(), 2);

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(20));

        // Explicit cleanup
        cache.cleanup_expired();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_default_ttl() {
        assert_eq!(DEFAULT_TTL, Duration::from_secs(24 * 60 * 60));
    }

    #[test]
    fn test_cache_default() {
        let cache = EvaluationCache::default();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }
}
