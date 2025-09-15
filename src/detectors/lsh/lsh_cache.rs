//! Thread-safe caching layer for LSH operations
//!
//! This module provides efficient caching for expensive operations like tokenization
//! and signature generation to eliminate redundant work in pipeline processing.

use ahash::AHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};
use tracing::debug;

/// Thread-safe cache for tokenization and signature operations
#[derive(Debug, Clone)]
pub struct LshCache {
    /// Token cache: source_hash -> tokenized shingles
    token_cache: Arc<RwLock<HashMap<u64, Vec<String>>>>,

    /// Signature cache: (source_hash, num_hashes, shingle_size) -> signature
    signature_cache: Arc<RwLock<HashMap<(u64, usize, usize), Vec<u64>>>>,

    /// Cache statistics for performance monitoring
    stats: Arc<RwLock<CacheStatistics>>,

    /// Maximum cache size to prevent memory bloat
    max_cache_size: usize,
}

/// Cache performance statistics
#[derive(Debug, Default, Clone)]
pub struct CacheStatistics {
    /// Token cache hits
    pub token_hits: usize,
    /// Token cache misses
    pub token_misses: usize,
    /// Signature cache hits
    pub signature_hits: usize,
    /// Signature cache misses
    pub signature_misses: usize,
    /// Cache evictions performed
    pub evictions: usize,
}

impl CacheStatistics {
    /// Calculate token cache hit rate
    pub fn token_hit_rate(&self) -> f64 {
        let total = self.token_hits + self.token_misses;
        if total == 0 {
            0.0
        } else {
            self.token_hits as f64 / total as f64
        }
    }

    /// Calculate signature cache hit rate
    pub fn signature_hit_rate(&self) -> f64 {
        let total = self.signature_hits + self.signature_misses;
        if total == 0 {
            0.0
        } else {
            self.signature_hits as f64 / total as f64
        }
    }

    /// Get overall hit rate across both caches
    pub fn overall_hit_rate(&self) -> f64 {
        let total_hits = self.token_hits + self.signature_hits;
        let total_requests = total_hits + self.token_misses + self.signature_misses;
        if total_requests == 0 {
            0.0
        } else {
            total_hits as f64 / total_requests as f64
        }
    }
}

impl LshCache {
    /// Create a new LSH cache with default settings
    pub fn new() -> Self {
        Self::with_capacity(10_000) // Default max 10k entries per cache
    }

    /// Create a new LSH cache with specified capacity
    pub fn with_capacity(max_cache_size: usize) -> Self {
        Self {
            token_cache: Arc::new(RwLock::new(HashMap::with_capacity(1000))),
            signature_cache: Arc::new(RwLock::new(HashMap::with_capacity(1000))),
            stats: Arc::new(RwLock::new(CacheStatistics::default())),
            max_cache_size,
        }
    }

    /// Get cached tokens for source code, or None if not cached
    pub fn get_tokens(&self, source_code: &str) -> Option<Vec<String>> {
        let hash = self.hash_source(source_code);

        if let Ok(cache) = self.token_cache.read() {
            if let Some(tokens) = cache.get(&hash) {
                // Update statistics
                if let Ok(mut stats) = self.stats.write() {
                    stats.token_hits += 1;
                }
                debug!("Token cache hit for source hash: {:x}", hash);
                return Some(tokens.clone());
            }
        }

        // Update statistics for cache miss
        if let Ok(mut stats) = self.stats.write() {
            stats.token_misses += 1;
        }

        None
    }

    /// Cache tokens for source code
    pub fn cache_tokens(&self, source_code: &str, tokens: Vec<String>) {
        let hash = self.hash_source(source_code);

        if let Ok(mut cache) = self.token_cache.write() {
            // Check if cache is getting too large
            if cache.len() >= self.max_cache_size {
                self.evict_tokens(&mut cache);
            }

            cache.insert(hash, tokens);
            debug!("Cached tokens for source hash: {:x}", hash);
        }
    }

    /// Get cached signature, or None if not cached
    pub fn get_signature(
        &self,
        source_code: &str,
        num_hashes: usize,
        shingle_size: usize,
    ) -> Option<Vec<u64>> {
        let source_hash = self.hash_source(source_code);
        let key = (source_hash, num_hashes, shingle_size);

        if let Ok(cache) = self.signature_cache.read() {
            if let Some(signature) = cache.get(&key) {
                // Update statistics
                if let Ok(mut stats) = self.stats.write() {
                    stats.signature_hits += 1;
                }
                debug!("Signature cache hit for key: {:?}", key);
                return Some(signature.clone());
            }
        }

        // Update statistics for cache miss
        if let Ok(mut stats) = self.stats.write() {
            stats.signature_misses += 1;
        }

        None
    }

    /// Cache signature for source code and parameters
    pub fn cache_signature(
        &self,
        source_code: &str,
        num_hashes: usize,
        shingle_size: usize,
        signature: Vec<u64>,
    ) {
        let source_hash = self.hash_source(source_code);
        let key = (source_hash, num_hashes, shingle_size);

        if let Ok(mut cache) = self.signature_cache.write() {
            // Check if cache is getting too large
            if cache.len() >= self.max_cache_size {
                self.evict_signatures(&mut cache);
            }

            cache.insert(key, signature);
            debug!("Cached signature for key: {:?}", key);
        }
    }

    /// Get cache statistics
    pub fn get_statistics(&self) -> CacheStatistics {
        if let Ok(stats) = self.stats.read() {
            stats.clone()
        } else {
            // If lock is poisoned, return default stats
            CacheStatistics::default()
        }
    }

    /// Reset cache statistics
    pub fn reset_statistics(&self) {
        if let Ok(mut stats) = self.stats.write() {
            *stats = CacheStatistics::default();
        }
    }

    /// Clear all caches
    pub fn clear(&self) {
        if let Ok(mut token_cache) = self.token_cache.write() {
            token_cache.clear();
        }
        if let Ok(mut signature_cache) = self.signature_cache.write() {
            signature_cache.clear();
        }
        if let Ok(mut stats) = self.stats.write() {
            *stats = CacheStatistics::default();
        }
        debug!("Cleared all LSH caches");
    }

    /// Get cache sizes for monitoring
    pub fn cache_sizes(&self) -> (usize, usize) {
        let token_size = self.token_cache.read().map(|c| c.len()).unwrap_or(0);
        let signature_size = self.signature_cache.read().map(|c| c.len()).unwrap_or(0);
        (token_size, signature_size)
    }

    /// Hash source code for cache key generation
    fn hash_source(&self, source_code: &str) -> u64 {
        let mut hasher = AHasher::default();
        source_code.hash(&mut hasher);
        hasher.finish()
    }

    /// Evict entries from token cache when it gets too large
    /// Uses a simple strategy: remove 25% of entries
    fn evict_tokens(&self, cache: &mut HashMap<u64, Vec<String>>) {
        let target_size = (self.max_cache_size * 3) / 4; // Remove 25%
        let current_size = cache.len();

        if current_size > target_size {
            let keys_to_remove: Vec<u64> = cache
                .keys()
                .take(current_size - target_size)
                .cloned()
                .collect();

            for key in keys_to_remove {
                cache.remove(&key);
            }

            // Update eviction statistics
            if let Ok(mut stats) = self.stats.write() {
                stats.evictions += 1;
            }

            debug!(
                "Evicted tokens: {} -> {} entries",
                current_size,
                cache.len()
            );
        }
    }

    /// Evict entries from signature cache when it gets too large
    fn evict_signatures(&self, cache: &mut HashMap<(u64, usize, usize), Vec<u64>>) {
        let target_size = (self.max_cache_size * 3) / 4; // Remove 25%
        let current_size = cache.len();

        if current_size > target_size {
            let keys_to_remove: Vec<(u64, usize, usize)> = cache
                .keys()
                .take(current_size - target_size)
                .cloned()
                .collect();

            for key in keys_to_remove {
                cache.remove(&key);
            }

            // Update eviction statistics
            if let Ok(mut stats) = self.stats.write() {
                stats.evictions += 1;
            }

            debug!(
                "Evicted signatures: {} -> {} entries",
                current_size,
                cache.len()
            );
        }
    }
}

impl Default for LshCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_caching() {
        let cache = LshCache::new();
        let source_code = "def test(): return 1";
        let tokens = vec!["def".to_string(), "test".to_string(), "return".to_string()];

        // First access should be cache miss
        assert!(cache.get_tokens(source_code).is_none());

        // Cache the tokens
        cache.cache_tokens(source_code, tokens.clone());

        // Second access should be cache hit
        let cached_tokens = cache.get_tokens(source_code).unwrap();
        assert_eq!(cached_tokens, tokens);

        // Check statistics
        let stats = cache.get_statistics();
        assert_eq!(stats.token_hits, 1);
        assert_eq!(stats.token_misses, 1);
        assert_eq!(stats.token_hit_rate(), 0.5);
    }

    #[test]
    fn test_signature_caching() {
        let cache = LshCache::new();
        let source_code = "def test(): return 1";
        let signature = vec![1, 2, 3, 4, 5];
        let num_hashes = 64;
        let shingle_size = 3;

        // First access should be cache miss
        assert!(cache
            .get_signature(source_code, num_hashes, shingle_size)
            .is_none());

        // Cache the signature
        cache.cache_signature(source_code, num_hashes, shingle_size, signature.clone());

        // Second access should be cache hit
        let cached_signature = cache
            .get_signature(source_code, num_hashes, shingle_size)
            .unwrap();
        assert_eq!(cached_signature, signature);

        // Check statistics
        let stats = cache.get_statistics();
        assert_eq!(stats.signature_hits, 1);
        assert_eq!(stats.signature_misses, 1);
        assert_eq!(stats.signature_hit_rate(), 0.5);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = LshCache::with_capacity(5); // Very small cache for testing

        // Fill cache beyond capacity
        for i in 0..10 {
            let source = format!("def test_{}(): return {}", i, i);
            let tokens = vec![format!("test_{}", i)];
            cache.cache_tokens(&source, tokens);
        }

        // Check that cache size is limited
        let (token_size, _) = cache.cache_sizes();
        assert!(token_size <= 5, "Cache should be limited to max size");

        // Check that evictions occurred
        let stats = cache.get_statistics();
        assert!(stats.evictions > 0, "Should have performed evictions");
    }

    #[test]
    fn test_cache_clear() {
        let cache = LshCache::new();

        // Add some entries
        cache.cache_tokens("test1", vec!["token1".to_string()]);
        cache.cache_signature("test2", 64, 3, vec![1, 2, 3]);

        // Verify entries exist
        assert!(cache.get_tokens("test1").is_some());
        assert!(cache.get_signature("test2", 64, 3).is_some());

        // Clear cache
        cache.clear();

        // Verify entries are gone
        assert!(cache.get_tokens("test1").is_none());
        assert!(cache.get_signature("test2", 64, 3).is_none());

        let (token_size, signature_size) = cache.cache_sizes();
        assert_eq!(token_size, 0);
        assert_eq!(signature_size, 0);
    }

    #[test]
    fn test_overall_hit_rate() {
        let cache = LshCache::new();

        // Generate some cache hits and misses
        cache.get_tokens("test1"); // miss
        cache.cache_tokens("test1", vec!["token1".to_string()]);
        cache.get_tokens("test1"); // hit

        cache.get_signature("test2", 64, 3); // miss
        cache.cache_signature("test2", 64, 3, vec![1, 2, 3]);
        cache.get_signature("test2", 64, 3); // hit

        let stats = cache.get_statistics();
        assert_eq!(stats.overall_hit_rate(), 0.5); // 2 hits out of 4 total requests
    }
}
