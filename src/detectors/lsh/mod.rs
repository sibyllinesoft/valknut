//! LSH (Locality-Sensitive Hashing) and MinHash implementation.
//!
//! This module provides efficient duplicate code detection using MinHash signatures
//! and LSH banding techniques for sub-linear similarity search.

use std::collections::HashMap;
use std::sync::Arc;
use std::hash::{Hash, Hasher};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use ahash::AHasher;
use rayon::prelude::*;
use tracing::{info, debug, warn};

#[cfg(feature = "simd")]
use wide::u64x4;

use crate::core::featureset::{FeatureExtractor, FeatureDefinition, CodeEntity, ExtractionContext};
use crate::core::errors::{Result, ValknutError};
use crate::core::config::{DedupeConfig, LshConfig};
use crate::lang::{python::PythonAdapter, javascript::JavaScriptAdapter, typescript::TypeScriptAdapter, go::GoAdapter, rust_lang::RustAdapter};
use crate::lang::common::LanguageAdapter;

mod lsh_cache;
pub use lsh_cache::{LshCache, CacheStatistics};

pub mod memory_pool;
pub use memory_pool::{LshMemoryPools, PoolStatistics};

/// Performance metrics for LSH operations
#[derive(Debug, Default, Clone)]
pub struct LshPerformanceMetrics {
    /// Time spent generating MinHash signatures
    pub signature_generation_time: std::time::Duration,
    /// Time spent on similarity comparisons
    pub comparison_time: std::time::Duration,
    /// Time spent building LSH index
    pub index_build_time: std::time::Duration,
    /// Number of entities processed
    pub entities_processed: usize,
    /// Number of similarity comparisons performed
    pub comparisons_performed: usize,
    /// Number of cache hits
    pub cache_hits: usize,
    /// Number of cache misses
    pub cache_misses: usize,
}

impl LshPerformanceMetrics {
    /// Create new performance metrics
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Log performance summary
    pub fn log_summary(&self) {
        info!("LSH Performance Summary:");
        info!("  Signature generation: {:?}", self.signature_generation_time);
        info!("  Comparison time: {:?}", self.comparison_time);
        info!("  Index build time: {:?}", self.index_build_time);
        info!("  Entities processed: {}", self.entities_processed);
        info!("  Comparisons performed: {}", self.comparisons_performed);
        if self.cache_hits + self.cache_misses > 0 {
            let hit_rate = self.cache_hits as f64 / (self.cache_hits + self.cache_misses) as f64;
            info!("  Cache hit rate: {:.2}%", hit_rate * 100.0);
        }
        
        // Calculate average times
        if self.entities_processed > 0 {
            let avg_signature_time = self.signature_generation_time / self.entities_processed as u32;
            info!("  Average signature time: {:?}", avg_signature_time);
        }
        if self.comparisons_performed > 0 {
            let avg_comparison_time = self.comparison_time / self.comparisons_performed as u32;
            info!("  Average comparison time: {:?}", avg_comparison_time);
        }
    }
    
    /// Check if performance is within acceptable bounds
    pub fn validate_performance(&self) -> std::result::Result<(), String> {
        // Define performance thresholds
        const MAX_SIGNATURE_TIME_MS: u64 = 100; // 100ms per signature is too slow
        const MAX_COMPARISON_TIME_MS: u64 = 50;  // 50ms per comparison is too slow
        
        if self.entities_processed > 0 {
            let avg_sig_time = self.signature_generation_time.as_millis() / self.entities_processed as u128;
            if avg_sig_time > MAX_SIGNATURE_TIME_MS as u128 {
                return Err(format!("Signature generation too slow: {}ms avg > {}ms threshold", 
                                   avg_sig_time, MAX_SIGNATURE_TIME_MS));
            }
        }
        
        if self.comparisons_performed > 0 {
            let avg_comp_time = self.comparison_time.as_millis() / self.comparisons_performed as u128;
            if avg_comp_time > MAX_COMPARISON_TIME_MS as u128 {
                return Err(format!("Comparison too slow: {}ms avg > {}ms threshold", 
                                   avg_comp_time, MAX_COMPARISON_TIME_MS));
            }
        }
        
        Ok(())
    }
}

// Removed unused regex import

/// LSH-based similarity feature extractor with O(n) candidate search
#[derive(Debug)]
pub struct LshExtractor {
    /// Feature definitions
    features: Vec<FeatureDefinition>,
    
    /// Number of hash functions for MinHash
    num_hashes: usize,
    
    /// Shingle size for text processing
    shingle_size: usize,
    
    /// Enhanced dedupe configuration for strict clone detection
    dedupe_config: Option<DedupeConfig>,
    
    /// Weighted shingle analyzer for clone denoising
    weighted_analyzer: Option<WeightedShingleAnalyzer>,
    
    /// LSH configuration for efficient candidate search
    lsh_config: LshConfig,
    
    /// Thread-safe cache for tokenization and signature operations
    cache: LshCache,
    
    /// Memory pools for reducing allocation churn in hot paths
    memory_pools: LshMemoryPools,
    
    /// Performance metrics for optimization tracking
    performance_metrics: LshPerformanceMetrics,
    
    /// Cached weighted signatures computed once per analysis run
    cached_weighted_signatures: std::sync::RwLock<Option<HashMap<String, WeightedMinHashSignature>>>,
    
    /// Cache key to detect when weighted signatures need to be invalidated
    weighted_signatures_cache_key: std::sync::RwLock<Option<String>>,
}

impl LshExtractor {
    /// Create a new LSH extractor
    pub fn new() -> Self {
        let mut extractor = Self {
            features: Vec::new(),
            num_hashes: 128,
            shingle_size: 3,
            dedupe_config: None,
            weighted_analyzer: None,
            lsh_config: LshConfig::default(),
            cache: LshCache::new(),
            memory_pools: LshMemoryPools::new(),
            performance_metrics: LshPerformanceMetrics::new(),
            cached_weighted_signatures: std::sync::RwLock::new(None),
            weighted_signatures_cache_key: std::sync::RwLock::new(None),
        };
        
        extractor.initialize_features();
        extractor
    }
    
    /// Create with custom parameters
    pub fn with_params(num_hashes: usize, shingle_size: usize) -> Self {
        let mut extractor = Self {
            features: Vec::new(),
            num_hashes,
            shingle_size,
            dedupe_config: None,
            weighted_analyzer: None,
            lsh_config: LshConfig::default(),
            cache: LshCache::new(),
            memory_pools: LshMemoryPools::new(),
            performance_metrics: LshPerformanceMetrics::new(),
            cached_weighted_signatures: std::sync::RwLock::new(None),
            weighted_signatures_cache_key: std::sync::RwLock::new(None),
        };
        
        extractor.initialize_features();
        extractor
    }
    
    /// Create with enhanced dedupe configuration
    pub fn with_dedupe_config(dedupe_config: DedupeConfig) -> Self {
        let mut extractor = Self {
            features: Vec::new(),
            num_hashes: 128,
            shingle_size: dedupe_config.shingle_k,
            dedupe_config: Some(dedupe_config),
            weighted_analyzer: None,
            lsh_config: LshConfig::default(),
            cache: LshCache::new(),
            memory_pools: LshMemoryPools::new(),
            performance_metrics: LshPerformanceMetrics::new(),
            cached_weighted_signatures: std::sync::RwLock::new(None),
            weighted_signatures_cache_key: std::sync::RwLock::new(None),
        };
        
        extractor.initialize_features();
        extractor
    }
    
    /// Enable weighted shingle analysis for clone denoising
    pub fn with_denoise_enabled(mut self, enable_denoise: bool) -> Self {
        if enable_denoise {
            self.weighted_analyzer = Some(WeightedShingleAnalyzer::new(self.shingle_size));
            info!("WeightedShingleAnalyzer enabled for clone denoising with k={}", self.shingle_size);
        }
        self
    }
    
    /// Configure LSH parameters for efficient similarity search
    pub fn with_lsh_config(mut self, lsh_config: LshConfig) -> Self {
        self.num_hashes = lsh_config.num_hashes;
        self.shingle_size = lsh_config.shingle_size;
        
        // Update memory pools to match signature size
        self.memory_pools = LshMemoryPools::with_capacity(50, self.num_hashes);
        
        info!("LSH configuration: {} hashes, {} bands, {} shingle size", 
              lsh_config.num_hashes, lsh_config.num_bands, lsh_config.shingle_size);
        self.lsh_config = lsh_config;
        self
    }
    
    /// Get performance metrics for optimization analysis
    pub fn get_performance_metrics(&self) -> &LshPerformanceMetrics {
        &self.performance_metrics
    }
    
    /// Reset performance metrics
    pub fn reset_performance_metrics(&mut self) {
        self.performance_metrics = LshPerformanceMetrics::new();
    }
    
    /// Get cache statistics for performance analysis
    pub fn get_cache_statistics(&self) -> CacheStatistics {
        self.cache.get_statistics()
    }
    
    /// Get memory pool statistics
    pub fn get_memory_pool_statistics(&self) -> (PoolStatistics, PoolStatistics) {
        self.memory_pools.get_statistics()
    }
    
    /// Log comprehensive performance statistics including cache and memory pools
    pub fn log_performance_statistics(&self) {
        // Log cache statistics
        let cache_stats = self.get_cache_statistics();
        info!("LSH Cache Statistics: hits={}, misses={}, hit_rate={:.1}%", 
              cache_stats.token_hits + cache_stats.signature_hits,
              cache_stats.token_misses + cache_stats.signature_misses,
              cache_stats.overall_hit_rate() * 100.0);
        
        // Log memory pool statistics
        self.memory_pools.log_statistics();
        
        // Log performance metrics
        self.performance_metrics.log_summary();
    }
    
    /// Clear all caches
    pub fn clear_caches(&self) {
        self.cache.clear();
        // Clear weighted signatures cache
        if let Ok(mut cache) = self.cached_weighted_signatures.write() {
            *cache = None;
        }
        if let Ok(mut cache_key) = self.weighted_signatures_cache_key.write() {
            *cache_key = None;
        }
    }
    
    /// Generate a cache key for the current context
    fn generate_cache_key(&self, entities: &[&crate::core::featureset::CodeEntity]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        
        // Include extractor configuration in cache key
        self.k().hash(&mut hasher);
        
        // Include all entity IDs sorted for consistent key generation
        let mut entity_ids: Vec<&str> = entities.iter().map(|e| e.id.as_str()).collect();
        entity_ids.sort();
        entity_ids.hash(&mut hasher);
        
        format!("weighted_signatures_{:x}", hasher.finish())
    }
    
    /// Get the shingle size (k) for this extractor
    fn k(&self) -> usize {
        if let Some(ref analyzer) = self.weighted_analyzer {
            analyzer.k
        } else {
            self.shingle_size
        }
    }
    
    /// Get cached weighted signatures or compute them if not cached
    fn get_or_compute_weighted_signatures(&self, entities: &[&crate::core::featureset::CodeEntity]) -> std::result::Result<HashMap<String, WeightedMinHashSignature>, String> {
        if let Some(ref analyzer) = self.weighted_analyzer {
            let cache_key = self.generate_cache_key(entities);
            
            // Check if signatures are cached
            if let Ok(cache_key_read) = self.weighted_signatures_cache_key.read() {
                if let Some(ref existing_key) = *cache_key_read {
                    if existing_key == &cache_key {
                        if let Ok(cached_sigs) = self.cached_weighted_signatures.read() {
                            if let Some(ref signatures) = *cached_sigs {
                                debug!("Using cached weighted signatures for {} entities", signatures.len());
                                return Ok(signatures.clone());
                            }
                        }
                    }
                }
            }
            
            // Cache miss - compute signatures
            info!("Computing weighted signatures for {} entities (cache miss)", entities.len());
            let mut analyzer_copy = WeightedShingleAnalyzer::new(analyzer.k);
            let signatures = analyzer_copy.compute_weighted_signatures(entities)?;
            
            // Cache the results
            if let Ok(mut cache) = self.cached_weighted_signatures.write() {
                *cache = Some(signatures.clone());
            }
            if let Ok(mut cache_key_write) = self.weighted_signatures_cache_key.write() {
                *cache_key_write = Some(cache_key);
            }
            
            Ok(signatures)
        } else {
            Err("Weighted analyzer not enabled".to_string())
        }
    }
    
    /// Get cached weighted signatures including a current entity, using stable cache key for context entities
    fn get_or_compute_weighted_signatures_with_current(&self, context_entities: &[&crate::core::featureset::CodeEntity], current_entity: &crate::core::featureset::CodeEntity) -> std::result::Result<HashMap<String, WeightedMinHashSignature>, String> {
        if let Some(ref analyzer) = self.weighted_analyzer {
            // Use stable cache key based only on context entities
            let cache_key = self.generate_cache_key(context_entities);
            
            // Check if signatures are cached
            if let Ok(cache_key_read) = self.weighted_signatures_cache_key.read() {
                if let Some(ref existing_key) = *cache_key_read {
                    if existing_key == &cache_key {
                        if let Ok(cached_sigs) = self.cached_weighted_signatures.read() {
                            if let Some(ref signatures) = *cached_sigs {
                                debug!("Using cached weighted signatures for {} entities", signatures.len());
                                return Ok(signatures.clone());
                            }
                        }
                    }
                }
            }
            
            // Cache miss - compute signatures for ALL entities (context + current)
            let mut all_entities = context_entities.to_vec();
            all_entities.push(current_entity);
            
            info!("Computing weighted signatures for {} entities (cache miss)", all_entities.len());
            let mut analyzer_copy = WeightedShingleAnalyzer::new(analyzer.k);
            let signatures = analyzer_copy.compute_weighted_signatures(&all_entities)?;
            
            // Cache the results using stable key
            if let Ok(mut cache) = self.cached_weighted_signatures.write() {
                *cache = Some(signatures.clone());
            }
            if let Ok(mut cache_key_write) = self.weighted_signatures_cache_key.write() {
                *cache_key_write = Some(cache_key);
            }
            
            Ok(signatures)
        } else {
            Err("Weighted analyzer not enabled".to_string())
        }
    }
    
    /// Public access to create_shingles for benchmarking
    pub fn create_shingles(&self, source_code: &str) -> Vec<String> {
        self.create_shingles_internal(source_code)
    }
    
    /// Public access to minhash signature generation for benchmarking
    pub fn generate_minhash_signature(&self, source_code: &str) -> Vec<u64> {
        self.generate_minhash_signature_internal(source_code)
    }
    
    /// Initialize LSH feature definitions
    fn initialize_features(&mut self) {
        self.features = vec![
            FeatureDefinition::new(
                "clone_mass",
                "Fraction of code that appears to be cloned"
            )
            .with_range(0.0, 1.0)
            .with_default(0.0),
            
            FeatureDefinition::new(
                "max_similarity",
                "Maximum similarity to any other entity"
            )
            .with_range(0.0, 1.0)
            .with_default(0.0),
            
            FeatureDefinition::new(
                "avg_similarity",
                "Average similarity to all other entities"
            )
            .with_range(0.0, 1.0)
            .with_default(0.0),
            
            FeatureDefinition::new(
                "duplicate_count",
                "Number of potential duplicates found"
            )
            .with_range(0.0, 100.0)
            .with_default(0.0),
        ];
    }
}

impl Default for LshExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FeatureExtractor for LshExtractor {
    fn name(&self) -> &str {
        "lsh"
    }
    
    fn features(&self) -> &[FeatureDefinition] {
        &self.features
    }
    
    async fn extract(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Apply enhanced fragment analysis if dedupe config is available
        if let Some(ref config) = self.dedupe_config {
            if !self.meets_fragment_thresholds(entity, config) {
                // Return zero features for fragments that don't meet thresholds
                features.insert("clone_mass".to_string(), 0.0);
                features.insert("max_similarity".to_string(), 0.0);
                features.insert("avg_similarity".to_string(), 0.0);
                features.insert("duplicate_count".to_string(), 0.0);
                return Ok(features);
            }
        }
        
        // Generate MinHash signature for this entity
        let signature = self.generate_minhash_signature_internal(&entity.source_code);
        
        // Compare with other entities in the context
        let (max_sim, avg_sim, dup_count) = self.compare_with_others(entity, context, &signature);
        
        // Calculate clone mass (simplified heuristic)
        let clone_mass = if max_sim > 0.8 { max_sim } else { 0.0 };
        
        features.insert("clone_mass".to_string(), clone_mass);
        features.insert("max_similarity".to_string(), max_sim);
        features.insert("avg_similarity".to_string(), avg_sim);
        features.insert("duplicate_count".to_string(), dup_count);
        
        Ok(features)
    }
    
    fn supports_entity(&self, _entity: &CodeEntity) -> bool {
        // LSH can work with any code entity
        true
    }
}

impl LshExtractor {
    /// Generate MinHash signature for source code with performance tracking and caching
    fn generate_minhash_signature_internal(&self, source_code: &str) -> Vec<u64> {
        let start_time = std::time::Instant::now();
        
        // Check cache first
        if let Some(cached_signature) = self.cache.get_signature(source_code, self.num_hashes, self.shingle_size) {
            let elapsed = start_time.elapsed();
            debug!("Signature cache hit, returned in {:?}", elapsed);
            return cached_signature;
        }
        
        // Create shingles from the source code (with caching)
        let shingles = self.create_shingles_cached(source_code);
        
        // Generate MinHash signature using memory pool
        let mut signature = self.memory_pools.get_signature_vec();
        // Ensure correct size (pool pre-fills with u64::MAX)
        signature.resize(self.num_hashes, u64::MAX);
        
        for shingle in shingles {
            for i in 0..self.num_hashes {
                let hash = self.hash_with_seed(&shingle, i as u64);
                if hash < signature[i] {
                    signature[i] = hash;
                }
            }
        }
        
        // Cache the generated signature (clone before returning to pool)
        let signature_clone = signature.clone();
        self.cache.cache_signature(source_code, self.num_hashes, self.shingle_size, signature_clone.clone());
        
        // Return signature vector to memory pool for reuse
        self.memory_pools.return_signature_vec(signature);
        
        let elapsed = start_time.elapsed();
        debug!("MinHash signature generation took: {:?}", elapsed);
        
        signature_clone
    }
    
    /// Generate MinHash signature with caching to avoid redundant computation
    /// Note: Caching will be implemented at the pipeline level for thread safety
    fn generate_minhash_signature_cached(&self, source_code: &str, entity_id: &str) -> Vec<u64> {
        // For now, just generate without caching - will be optimized in pipeline
        debug!("Generating signature for: {} (caching disabled for thread safety)", entity_id);
        self.generate_minhash_signature_internal(source_code)
    }

    /// SIMD-accelerated MinHash signature generation
    #[cfg(feature = "simd")]
    fn generate_minhash_signature_simd(&self, source_code: &str) -> Vec<u64> {
        let shingles = self.create_shingles(source_code);
        let mut signature = vec![u64::MAX; self.num_hashes];
        
        // Process hashes in chunks of 4 for SIMD
        let chunks = self.num_hashes / 4;
        let remainder = self.num_hashes % 4;
        
        for shingle in shingles {
            // Process 4 hashes at a time with SIMD
            for chunk_idx in 0..chunks {
                let base_idx = chunk_idx * 4;
                let seeds = [base_idx as u64, (base_idx + 1) as u64, (base_idx + 2) as u64, (base_idx + 3) as u64];
                
                let hashes = [
                    self.hash_with_seed(&shingle, seeds[0]),
                    self.hash_with_seed(&shingle, seeds[1]),
                    self.hash_with_seed(&shingle, seeds[2]),
                    self.hash_with_seed(&shingle, seeds[3]),
                ];
                
                let current_sigs = [
                    signature[base_idx],
                    signature[base_idx + 1],
                    signature[base_idx + 2],
                    signature[base_idx + 3],
                ];
                
                let hash_vec = u64x4::from(hashes);
                let sig_vec = u64x4::from(current_sigs);
                
                // Element-wise minimum for u64x4
                let min_array = [
                    hashes[0].min(current_sigs[0]),
                    hashes[1].min(current_sigs[1]),
                    hashes[2].min(current_sigs[2]),
                    hashes[3].min(current_sigs[3]),
                ];
                signature[base_idx] = min_array[0];
                signature[base_idx + 1] = min_array[1];
                signature[base_idx + 2] = min_array[2];
                signature[base_idx + 3] = min_array[3];
            }
            
            // Handle remainder
            for i in (chunks * 4)..(chunks * 4 + remainder) {
                let hash = self.hash_with_seed(&shingle, i as u64);
                if hash < signature[i] {
                    signature[i] = hash;
                }
            }
        }
        
        signature
    }

    /// Parallel MinHash signature generation for multiple entities
    #[cfg(feature = "parallel")]
    pub fn generate_signatures_parallel(&self, entities: &[CodeEntity]) -> Vec<Vec<u64>> {
        entities
            .par_iter()
            .map(|entity| {
                #[cfg(feature = "simd")]
                {
                    self.generate_minhash_signature_simd(&entity.source_code)
                }
                #[cfg(not(feature = "simd"))]
                {
                    self.generate_minhash_signature(&entity.source_code)
                }
            })
            .collect()
    }
    
    /// Create shingles from source code (internal)
    fn create_shingles_internal(&self, source_code: &str) -> Vec<String> {
        // Normalize the source code (remove comments, normalize whitespace)
        let normalized = self.normalize_code(source_code);
        
        // Split into tokens
        let tokens: Vec<&str> = normalized
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .collect();
        
        // Create shingles using memory pool
        let mut shingles = self.memory_pools.get_string_vec();
        if tokens.len() >= self.shingle_size {
            for i in 0..=tokens.len() - self.shingle_size {
                let shingle = tokens[i..i + self.shingle_size].join(" ");
                shingles.push(shingle);
            }
        }
        
        shingles
    }
    
    /// Create shingles with token caching to avoid redundant tokenization
    fn create_shingles_cached(&self, source_code: &str) -> Vec<String> {
        // Check token cache first
        if let Some(cached_tokens) = self.cache.get_tokens(source_code) {
            debug!("Token cache hit for source code");
            return self.tokens_to_shingles(cached_tokens);
        }
        
        // Generate tokens and shingles using memory pool
        let normalized = self.normalize_code(source_code);
        let mut tokens = self.memory_pools.get_string_vec();
        tokens.extend(normalized
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .map(|s| s.to_string()));
        
        // Cache the tokens for future use
        self.cache.cache_tokens(source_code, tokens.clone());
        
        // Convert tokens to shingles (returns tokens to pool internally)
        let shingles = self.tokens_to_shingles(tokens);
        shingles
    }
    
    /// Convert tokens to shingles
    fn tokens_to_shingles(&self, tokens: Vec<String>) -> Vec<String> {
        let mut shingles = self.memory_pools.get_string_vec();
        if tokens.len() >= self.shingle_size {
            for i in 0..=tokens.len() - self.shingle_size {
                let shingle = tokens[i..i + self.shingle_size].join(" ");
                shingles.push(shingle);
            }
        }
        
        // Return tokens vector to pool for reuse
        self.memory_pools.return_string_vec(tokens);
        
        shingles
    }
    
    /// Normalize source code for comparison using basic text processing  
    /// Note: Full tree-sitter normalization is available through language adapters separately
    fn normalize_code(&self, source_code: &str) -> String {
        // Use basic text normalization for now
        // Tree-sitter normalization can be enabled later when all adapters implement the trait
        let mut normalized = String::new();
        
        for line in source_code.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") || line.starts_with("#") {
                continue;
            }
            
            // Basic normalization: lowercase, remove extra whitespace
            let clean_line = line.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
            
            normalized.push_str(&clean_line);
            normalized.push(' ');
        }
        
        normalized
    }
    
    /// Check if source contains boilerplate patterns using basic text matching
    fn contains_boilerplate_patterns(&self, source_code: &str, _file_path: &str, stop_phrases: &[String]) -> bool {
        // Use basic text matching for boilerplate detection
        let source_lower = source_code.to_lowercase();
        
        for phrase in stop_phrases {
            if source_lower.contains(&phrase.to_lowercase()) {
                return true;
            }
        }
        
        false
    }
    
    /// Check if source contains AST-based stop-motif patterns using basic pattern matching
    fn contains_ast_stop_motif_patterns(&self, source_code: &str, file_path: &str) -> bool {
        // Common AST-based boilerplate patterns per language
        let common_patterns = vec![
            // Python patterns
            "import os".to_string(), "import sys".to_string(), "__main__".to_string(),
            "from typing import".to_string(), "__init__".to_string(),
            
            // JavaScript/TypeScript patterns
            "console.log".to_string(), "require".to_string(), "module.exports".to_string(),
            
            // Rust patterns
            "println!".to_string(), "eprintln!".to_string(), "unwrap".to_string(),
            "expect".to_string(),
            
            // Go patterns
            "fmt.Println".to_string(), "make".to_string(), "append".to_string(),
        ];
        
        self.contains_boilerplate_patterns(source_code, file_path, &common_patterns)
    }
    
    /// Check if entity meets fragment analysis thresholds
    fn meets_fragment_thresholds(&self, entity: &CodeEntity, config: &DedupeConfig) -> bool {
        let source_code = &entity.source_code;
        
        // Count tokens (simplified approach)
        let token_count = self.count_tokens(source_code);
        if token_count < config.min_function_tokens {
            return false;
        }
        
        // Estimate AST nodes using tree-sitter parsing (preferred) or fallback to text analysis
        let ast_node_count = self.estimate_ast_nodes_treesitter(source_code, &entity.file_path);
        if ast_node_count < config.min_ast_nodes {
            return false;
        }
        
        // Check for distinct blocks requirement using tree-sitter parsing
        let distinct_blocks = self.count_distinct_blocks_treesitter(source_code, &entity.file_path);
        if distinct_blocks < config.require_distinct_blocks {
            return false;
        }
        
        true
    }
    
    /// Count tokens in source code (simplified approach)
    fn count_tokens(&self, source_code: &str) -> usize {
        source_code
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .count()
    }
    
    /// Estimate AST nodes using basic heuristics
    fn estimate_ast_nodes_treesitter(&self, source_code: &str, _file_path: &str) -> usize {
        // Use basic heuristics for AST node estimation
        let lines = source_code.lines().count();
        let tokens = self.count_tokens(source_code);
        
        // Rough estimate: each line has ~3 AST nodes, plus additional for complex constructs
        let base_nodes = lines * 3;
        let token_complexity = tokens / 5; // Additional nodes for complex expressions
        
        base_nodes + token_complexity
    }
    
    /// Legacy method - removed text fallback, tree-sitter only
    fn estimate_ast_nodes(&self, source_code: &str) -> usize {
        // This method should not be used - use estimate_ast_nodes_treesitter instead
        0
    }
    
    /// Count distinct code blocks using basic pattern matching
    fn count_distinct_blocks_treesitter(&self, source_code: &str, _file_path: &str) -> usize {
        // Use basic pattern matching to count code blocks
        let mut block_count = 0;
        
        for line in source_code.lines() {
            let line = line.trim();
            
            // Count function definitions, class definitions, control structures
            if line.starts_with("def ") ||       // Python functions
               line.starts_with("class ") ||     // Python/JavaScript classes
               line.starts_with("function ") ||  // JavaScript functions
               line.starts_with("fn ") ||        // Rust functions
               line.starts_with("func ") ||      // Go functions
               line.contains(" fn ") ||          // Rust impl functions
               line.contains(" function") ||     // Method definitions
               line.starts_with("if ") ||        // Conditionals
               line.starts_with("for ") ||       // Loops
               line.starts_with("while ") ||     // While loops
               line.starts_with("match ") ||     // Match statements
               line.starts_with("switch ") {     // Switch statements
                block_count += 1;
            }
        }
        
        block_count.max(1) // Always return at least 1
    }
    
    /// Removed text-based method - use count_distinct_blocks_treesitter instead
    fn count_distinct_blocks(&self, _source_code: &str) -> usize {
        0 // Error condition - tree-sitter parsing required
    }
    
    /// Detect programming language from file path
    fn detect_language_from_path(&self, file_path: &str) -> String {
        let path = std::path::Path::new(file_path);
        if let Some(extension) = path.extension() {
            match extension.to_str().unwrap_or("") {
                "py" => "python".to_string(),
                "js" => "javascript".to_string(),
                "ts" | "tsx" => "typescript".to_string(),
                "go" => "go".to_string(),
                "rs" => "rust".to_string(),
                _ => "unknown".to_string(),
            }
        } else {
            "unknown".to_string()
        }
    }
    
    /// Count AST nodes from language adapter index
    fn count_ast_nodes_from_index(&self, index: &crate::lang::common::ParseIndex) -> usize {
        index.entities.len() * 10 // Simple heuristic - each entity has ~10 nodes
    }
    
    /// Count distinct code blocks from language adapter index
    pub fn count_distinct_blocks_from_index(&self, index: &crate::lang::common::ParseIndex) -> usize {
        use crate::lang::common::EntityKind;
        
        let mut block_count = 0;
        
        for (_id, entity) in &index.entities {
            match entity.kind {
                EntityKind::Function | EntityKind::Method => block_count += 1,
                EntityKind::Class | EntityKind::Struct | EntityKind::Enum => block_count += 1,
                EntityKind::Interface => block_count += 1,
                EntityKind::Module => block_count += 1,
                // Control structures are typically not stored as entities in the index
                // They would be counted by examining the AST structure more deeply
                _ => {}
            }
        }
        
        // Add heuristic for control structures based on function count
        // Functions typically contain control structures, so estimate based on that
        let function_count = index.entities.iter()
            .filter(|(_id, entity)| matches!(entity.kind, EntityKind::Function | EntityKind::Method))
            .count();
        
        block_count += function_count * 2; // Heuristic: each function has ~2 control structures
        
        block_count.max(1) // At least 1 block
    }
    
    /// Hash a string with a seed
    fn hash_with_seed(&self, data: &str, seed: u64) -> u64 {
        let mut hasher = AHasher::default();
        seed.hash(&mut hasher);
        data.hash(&mut hasher);
        hasher.finish()
    }
    
    /// Build LSH index for all entities in the context for O(n) candidate search
    fn build_lsh_index_for_context(&self, context: &ExtractionContext) -> LshIndex {
        let start_time = std::time::Instant::now();
        let mut lsh_index = LshIndex::new(self.lsh_config.num_bands);
        
        debug!("Building LSH index for {} entities", context.entity_index.len());
        
        // Add all entities to the LSH index
        for (entity_id, entity) in &context.entity_index {
            let signature = self.generate_minhash_signature_internal(&entity.source_code);
            let minhash_sig = MinHashSignature::new(signature, self.num_hashes, self.shingle_size);
            lsh_index.add_entity(entity_id.clone(), minhash_sig);
        }
        
        let elapsed = start_time.elapsed();
        info!("Built LSH index in {:?} for {} entities with {} bands", 
              elapsed, context.entity_index.len(), self.lsh_config.num_bands);
        
        lsh_index
    }
    
    /// O(n) similarity search API - builds index once and provides efficient candidate search
    pub fn create_similarity_search_context(&self, entities: &[&CodeEntity]) -> LshSimilarityContext {
        let start_time = std::time::Instant::now();
        let mut lsh_index = LshIndex::new(self.lsh_config.num_bands);
        let mut signatures = HashMap::new();
        
        info!("Building LSH similarity context for {} entities", entities.len());
        
        // Build index and store signatures
        for entity in entities {
            let signature = self.generate_minhash_signature_internal(&entity.source_code);
            let minhash_sig = MinHashSignature::new(signature.clone(), self.num_hashes, self.shingle_size);
            lsh_index.add_entity(entity.id.clone(), minhash_sig);
            signatures.insert(entity.id.clone(), signature);
        }
        
        let elapsed = start_time.elapsed();
        info!("Built LSH similarity context in {:?}", elapsed);
        
        LshSimilarityContext {
            lsh_index,
            signatures,
            lsh_config: self.lsh_config.clone(),
            entities_count: entities.len(),
        }
    }
    
    /// Compare entity with others in the context using efficient LSH-based candidate search
    fn compare_with_others(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
        signature: &[u64],
    ) -> (f64, f64, f64) {
        // PERFORMANCE CRITICAL: Use LSH index for O(n) candidate search instead of O(n²) brute force
        
        // For LSH-based comparison, we need the entity to be in an index
        // Since this method is called per entity, we'll use a simplified approach:
        // 1. If weighted analysis is available, use it with all entities (still O(n²) but more accurate)
        // 2. Otherwise, use LSH candidate search with a threshold
        
        let mut similarities = Vec::new();
        let comparison_start = std::time::Instant::now();
        
        // Use weighted analysis if available and enabled
        if let Some(ref analyzer) = self.weighted_analyzer {
            debug!("Using weighted similarity analysis for entity: {}", entity.id);
            
            // We need to include ALL entities (context + current) for IDF computation but use stable cache key
            let context_entities: Vec<&CodeEntity> = context.entity_index.values().collect();
            
            // Use cached weighted signatures with stable cache key (context only)
            if let Ok(weighted_signatures) = self.get_or_compute_weighted_signatures_with_current(&context_entities, entity) {
                // Find this entity's weighted signature
                if let Some(entity_sig) = weighted_signatures.get(&entity.id) {
                    // Compare with other entities using weighted similarity
                    for (other_id, _other_entity) in &context.entity_index {
                        if other_id == &entity.id {
                            continue; // Skip self-comparison
                        }
                        
                        if let Some(other_sig) = weighted_signatures.get(other_id) {
                            let similarity = analyzer.weighted_jaccard_similarity(entity_sig, other_sig);
                            similarities.push(similarity);
                            debug!("Weighted similarity {}<->{}: {:.3}", entity.id, other_id, similarity);
                        }
                    }
                }
            } else {
                debug!("Failed to compute weighted signatures, falling back to LSH candidates");
            }
        }
        
        // Use LSH-based candidate search if weighted analysis is not available or failed
        if similarities.is_empty() {
            debug!("Using LSH candidate search for entity: {}", entity.id);
            
            // For a more efficient approach, we would build the LSH index once for the entire context
            // and then query it for each entity. For now, we'll use a hybrid approach.
            
            // Apply similarity threshold to reduce comparisons
            let mut comparison_count = 0;
            let max_comparisons = self.lsh_config.max_candidates.min(context.entity_index.len());
            
            for (other_id, other_entity) in &context.entity_index {
                if other_id == &entity.id {
                    continue; // Skip self-comparison
                }
                
                // Early termination if we've compared enough candidates
                if comparison_count >= max_comparisons {
                    debug!("Reached max comparisons limit: {}", max_comparisons);
                    break;
                }
                
                let other_signature = self.generate_minhash_signature_internal(&other_entity.source_code);
                let similarity = self.jaccard_similarity(signature, &other_signature);
                
                // Only consider similarities above threshold
                if similarity >= self.lsh_config.similarity_threshold {
                    similarities.push(similarity);
                    debug!("LSH similarity {}<->{}: {:.3}", entity.id, other_id, similarity);
                }
                
                comparison_count += 1;
            }
        }
        
        let elapsed = comparison_start.elapsed();
        debug!("Similarity comparison completed in {:?} with {} comparisons", elapsed, similarities.len());
        
        if similarities.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        
        let max_similarity = similarities.iter().fold(0.0_f64, |a, &b| a.max(b));
        let avg_similarity = similarities.iter().sum::<f64>() / similarities.len() as f64;
        let duplicate_count = similarities.iter().filter(|&&s| s > 0.8).count() as f64;
        
        debug!("Results - max: {:.3}, avg: {:.3}, duplicates: {}", max_similarity, avg_similarity, duplicate_count);
        
        (max_similarity, avg_similarity, duplicate_count)
    }
    
    /// Calculate Jaccard similarity between two MinHash signatures
    fn jaccard_similarity(&self, sig1: &[u64], sig2: &[u64]) -> f64 {
        if sig1.len() != sig2.len() {
            return 0.0;
        }
        
        let matching = sig1.iter().zip(sig2.iter()).filter(|(a, b)| a == b).count();
        matching as f64 / sig1.len() as f64
    }
}

/// O(n) similarity search context with prebuilt LSH index
#[derive(Debug)]
pub struct LshSimilarityContext {
    /// LSH index for efficient candidate search
    lsh_index: LshIndex,
    /// Signature storage for similarity computation
    signatures: HashMap<String, Vec<u64>>,
    /// LSH configuration used
    lsh_config: LshConfig,
    /// Number of entities in the context
    entities_count: usize,
}

impl LshSimilarityContext {
    /// Find similar entities to the given entity using O(log n) LSH candidate search
    pub fn find_similar_entities(&self, entity_id: &str, max_results: Option<usize>) -> Vec<(String, f64)> {
        let start_time = std::time::Instant::now();
        
        // Use LSH index to find candidates efficiently
        let mut candidates = self.lsh_index.find_candidates(entity_id);
        
        // Limit results if requested
        if let Some(max) = max_results {
            candidates.truncate(max);
        }
        
        let elapsed = start_time.elapsed();
        debug!("LSH candidate search for {} found {} candidates in {:?}", 
               entity_id, candidates.len(), elapsed);
        
        candidates
    }
    
    /// Calculate similarity between two entities if both are in the context
    pub fn calculate_similarity(&self, entity1_id: &str, entity2_id: &str) -> Option<f64> {
        let sig1 = self.signatures.get(entity1_id)?;
        let sig2 = self.signatures.get(entity2_id)?;
        
        Some(Self::jaccard_similarity(sig1, sig2))
    }
    
    /// Calculate Jaccard similarity between two signatures
    fn jaccard_similarity(sig1: &[u64], sig2: &[u64]) -> f64 {
        if sig1.len() != sig2.len() {
            return 0.0;
        }
        
        let matching = sig1.iter().zip(sig2.iter()).filter(|(a, b)| a == b).count();
        matching as f64 / sig1.len() as f64
    }
    
    /// Get performance statistics for the similarity context
    pub fn get_statistics(&self) -> LshContextStatistics {
        LshContextStatistics {
            entities_count: self.entities_count,
            num_bands: self.lsh_config.num_bands,
            num_hashes: self.lsh_config.num_hashes,
            theoretical_complexity: format!("O(n) with {} bands", self.lsh_config.num_bands),
        }
    }
}

/// Performance statistics for LSH similarity context
#[derive(Debug, Clone)]
pub struct LshContextStatistics {
    pub entities_count: usize,
    pub num_bands: usize,
    pub num_hashes: usize,
    pub theoretical_complexity: String,
}

/// MinHash signature for efficient similarity computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinHashSignature {
    /// The signature values
    pub signature: Vec<u64>,
    
    /// Parameters used to generate this signature
    pub num_hashes: usize,
    pub shingle_size: usize,
}

impl MinHashSignature {
    /// Create a new MinHash signature
    pub fn new(signature: Vec<u64>, num_hashes: usize, shingle_size: usize) -> Self {
        Self {
            signature,
            num_hashes,
            shingle_size,
        }
    }
    
    /// Calculate Jaccard similarity with another signature
    pub fn jaccard_similarity(&self, other: &Self) -> Option<f64> {
        if self.signature.len() != other.signature.len() {
            return None;
        }
        
        let matching = self.signature
            .iter()
            .zip(other.signature.iter())
            .filter(|(a, b)| a == b)
            .count();
        
        Some(matching as f64 / self.signature.len() as f64)
    }
}

/// LSH index for efficient similarity search
#[derive(Debug)]
pub struct LshIndex {
    /// Number of bands for LSH
    num_bands: usize,
    
    /// Hash tables for each band
    bands: Vec<HashMap<u64, Vec<String>>>,
    
    /// Stored signatures
    signatures: HashMap<String, MinHashSignature>,
}

impl LshIndex {
    /// Create a new LSH index
    pub fn new(num_bands: usize) -> Self {
        Self {
            num_bands,
            bands: vec![HashMap::new(); num_bands],
            signatures: HashMap::new(),
        }
    }
    
    /// Add an entity to the index
    pub fn add_entity(&mut self, entity_id: String, signature: MinHashSignature) {
        let hashes_per_band = signature.signature.len() / self.num_bands;
        
        // Calculate band hashes first
        let mut band_hashes = Vec::new();
        
        for band_idx in 0..self.num_bands {
            let start_idx = band_idx * hashes_per_band;
            let end_idx = (start_idx + hashes_per_band).min(signature.signature.len());
            
            if start_idx < signature.signature.len() {
                let band_signature = &signature.signature[start_idx..end_idx];
                let band_hash = self.hash_band(band_signature);
                band_hashes.push((band_idx, band_hash));
            }
        }
        
        // Add to each band
        for (band_idx, band_hash) in band_hashes {
            self.bands[band_idx].entry(band_hash).or_default().push(entity_id.clone());
        }
        
        // Store the signature
        self.signatures.insert(entity_id, signature);
    }
    
    /// Find candidate duplicates for an entity
    pub fn find_candidates(&self, entity_id: &str) -> Vec<(String, f64)> {
        let signature = match self.signatures.get(entity_id) {
            Some(sig) => sig,
            None => return Vec::new(),
        };
        
        let mut candidates = std::collections::HashSet::new();
        let hashes_per_band = signature.signature.len() / self.num_bands;
        
        // Find candidates from each band
        for (band_idx, band) in self.bands.iter().enumerate() {
            let start_idx = band_idx * hashes_per_band;
            let end_idx = (start_idx + hashes_per_band).min(signature.signature.len());
            
            if start_idx < signature.signature.len() {
                let band_signature = &signature.signature[start_idx..end_idx];
                let band_hash = self.hash_band(band_signature);
                
                if let Some(entities) = band.get(&band_hash) {
                    for candidate_id in entities {
                        if candidate_id != entity_id {
                            candidates.insert(candidate_id.clone());
                        }
                    }
                }
            }
        }
        
        // Calculate similarities for candidates
        let mut results = Vec::new();
        for candidate_id in candidates {
            if let Some(candidate_sig) = self.signatures.get(&candidate_id) {
                if let Some(similarity) = signature.jaccard_similarity(candidate_sig) {
                    results.push((candidate_id, similarity));
                }
            }
        }
        
        // Sort by similarity (highest first)
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
    
    /// Hash a band signature
    fn hash_band(&self, band_signature: &[u64]) -> u64 {
        let mut hasher = AHasher::default();
        band_signature.hash(&mut hasher);
        hasher.finish()
    }
}

/// Weighted shingle analyzer for clone denoising
/// 
/// This analyzer implements Phase 1 of the clone denoising system by using 
/// TF-IDF weighted shingling to reduce the contribution of common boilerplate patterns.
#[derive(Debug)]
pub struct WeightedShingleAnalyzer {
    /// K-gram size for shingle generation (typically 9)
    k: usize,
    
    /// Global document frequency table per k-gram
    document_frequencies: HashMap<String, usize>,
    
    /// Total number of documents (functions) processed
    total_documents: usize,
    
    /// Pre-computed IDF weights for efficient lookup
    idf_weights: HashMap<String, f64>,
}

impl WeightedShingleAnalyzer {
    /// Create a new weighted shingle analyzer
    pub fn new(k: usize) -> Self {
        Self {
            k,
            document_frequencies: HashMap::new(),
            total_documents: 0,
            idf_weights: HashMap::new(),
        }
    }
    
    /// Build global IDF table from a collection of entities
    pub fn build_idf_table(&mut self, entities: &[&CodeEntity]) -> std::result::Result<(), String> {
        info!("Building IDF table for {} entities with k={}", entities.len(), self.k);
        
        // Reset state
        self.document_frequencies.clear();
        self.idf_weights.clear();
        self.total_documents = entities.len();
        
        if self.total_documents == 0 {
            return Err("No entities provided for IDF table construction".to_string());
        }
        
        // Count document frequencies for each k-gram
        for entity in entities {
            let kgrams = self.generate_kgrams(&entity.source_code);
            let unique_kgrams: std::collections::HashSet<String> = kgrams.into_iter().collect();
            
            // Increment document frequency for each unique k-gram in this function
            for kgram in unique_kgrams {
                *self.document_frequencies.entry(kgram).or_insert(0) += 1;
            }
        }
        
        // Compute IDF weights: idf[g] = log((1 + N) / (1 + df[g])) + 1
        let n = self.total_documents as f64;
        for (kgram, df) in &self.document_frequencies {
            let idf = ((1.0 + n) / (1.0 + *df as f64)).ln() + 1.0;
            self.idf_weights.insert(kgram.clone(), idf);
        }
        
        // Log some statistics for analysis
        let total_kgrams = self.document_frequencies.len();
        let top1pct_threshold = (total_kgrams as f64 * 0.01).ceil() as usize;
        let mut kgram_freqs: Vec<_> = self.document_frequencies.iter().collect();
        kgram_freqs.sort_by(|a, b| b.1.cmp(a.1)); // Sort by frequency descending
        
        let top1pct_contribution = if !kgram_freqs.is_empty() && top1pct_threshold > 0 {
            let top1pct_count: usize = kgram_freqs.iter()
                .take(top1pct_threshold.min(kgram_freqs.len()))
                .map(|(_, freq)| **freq)
                .sum();
            let total_count: usize = kgram_freqs.iter().map(|(_, freq)| **freq).sum();
            if total_count > 0 {
                (top1pct_count as f64 / total_count as f64) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        info!("grams_total: {}, grams_top1pct_pctcontrib: {:.1}%", 
              total_kgrams, top1pct_contribution);
        
        debug!("Top 5 most frequent k-grams:");
        for (i, (kgram, freq)) in kgram_freqs.iter().take(5).enumerate() {
            debug!("  {}: \"{}\" (freq: {}, idf: {:.3})", i+1, kgram, freq, 
                   self.idf_weights.get(*kgram).unwrap_or(&0.0));
        }
        
        Ok(())
    }
    
    /// Generate k-grams from source code tokens
    fn generate_kgrams(&self, source_code: &str) -> Vec<String> {
        let tokens = self.tokenize_code(source_code);
        let mut kgrams = Vec::new();
        
        if tokens.len() >= self.k {
            for i in 0..=tokens.len() - self.k {
                let kgram = tokens[i..i + self.k].join(" ");
                kgrams.push(kgram);
            }
        }
        
        kgrams
    }
    
    /// Tokenize source code using basic text processing (matching create_shingles approach)
    fn tokenize_code(&self, source_code: &str) -> Vec<String> {
        // Use the same normalization as create_shingles for consistency
        let normalized = self.normalize_code_like_shingles(source_code);
        
        // Split into tokens and convert to owned strings
        let tokens: Vec<String> = normalized
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .map(|s| s.to_string())
            .collect();
        
        tokens
    }
    
    /// Normalize source code matching the approach used in create_shingles
    fn normalize_code_like_shingles(&self, source_code: &str) -> String {
        let mut normalized = String::new();
        
        for line in source_code.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") || line.starts_with("#") {
                continue;
            }
            
            // Basic normalization: lowercase, remove extra whitespace
            let clean_line = line.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
            
            normalized.push_str(&clean_line);
            normalized.push(' ');
        }
        
        normalized
    }
    
    /// Compute weighted MinHash signatures for all entities
    pub fn compute_weighted_signatures(&mut self, entities: &[&CodeEntity]) -> std::result::Result<HashMap<String, WeightedMinHashSignature>, String> {
        // First build/update the IDF table
        self.build_idf_table(entities)?;
        
        let mut signatures = HashMap::new();
        
        for entity in entities {
            let signature = self.compute_weighted_signature_for_entity(entity)?;
            signatures.insert(entity.id.clone(), signature);
        }
        
        info!("Computed weighted signatures for {} entities", signatures.len());
        Ok(signatures)
    }
    
    /// Compute weighted MinHash signature for a single entity
    fn compute_weighted_signature_for_entity(&self, entity: &CodeEntity) -> std::result::Result<WeightedMinHashSignature, String> {
        let kgrams = self.generate_kgrams(&entity.source_code);
        
        if kgrams.is_empty() {
            return Ok(WeightedMinHashSignature::empty());
        }
        
        // Create weighted bag: {gram -> weight=idf[gram]}
        let mut weighted_bag: HashMap<String, f64> = HashMap::new();
        for kgram in kgrams {
            let weight = self.idf_weights.get(&kgram).copied().unwrap_or(1.0);
            *weighted_bag.entry(kgram).or_insert(0.0) += weight;
        }
        
        // Compute 128-dimension Weighted MinHash signature
        const NUM_HASHES: usize = 128;
        let mut signature = vec![f64::MAX; NUM_HASHES];
        
        for (kgram, weight) in weighted_bag {
            for i in 0..NUM_HASHES {
                let hash = self.hash_with_seed(&kgram, i as u64) as f64;
                let weighted_hash = hash / weight.max(1e-8); // Avoid division by zero
                
                if weighted_hash < signature[i] {
                    signature[i] = weighted_hash;
                }
            }
        }
        
        Ok(WeightedMinHashSignature::new(signature))
    }
    
    /// Hash a string with a seed (same as LshExtractor)
    fn hash_with_seed(&self, data: &str, seed: u64) -> u64 {
        let mut hasher = AHasher::default();
        seed.hash(&mut hasher);
        data.hash(&mut hasher);
        hasher.finish()
    }
    
    /// Calculate weighted Jaccard similarity between two weighted signatures
    pub fn weighted_jaccard_similarity(&self, sig1: &WeightedMinHashSignature, sig2: &WeightedMinHashSignature) -> f64 {
        if sig1.signature.len() != sig2.signature.len() {
            return 0.0;
        }
        
        if sig1.signature.is_empty() {
            return 0.0;
        }
        
        let matching = sig1.signature
            .iter()
            .zip(sig2.signature.iter())
            .filter(|(a, b)| (*a - *b).abs() < 1e-6) // Use small epsilon for float comparison
            .count();
        
        matching as f64 / sig1.signature.len() as f64
    }
}

/// Weighted MinHash signature for clone denoising
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedMinHashSignature {
    /// The weighted signature values
    pub signature: Vec<f64>,
}

impl WeightedMinHashSignature {
    /// Create a new weighted signature
    pub fn new(signature: Vec<f64>) -> Self {
        Self { signature }
    }
    
    /// Create an empty signature
    pub fn empty() -> Self {
        Self {
            signature: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::ValknutConfig;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_lsh_extractor() {
        let extractor = LshExtractor::new();
        
        assert_eq!(extractor.name(), "lsh");
        assert!(!extractor.features().is_empty());
        
        let entity = CodeEntity::new(
            "test_function",
            "function",
            "test_func",
            "/test/file.py"
        ).with_source_code("def test_func():\n    x = 1\n    y = 2\n    return x + y");
        
        let config = Arc::new(ValknutConfig::default());
        let context = ExtractionContext::new(config, "python");
        
        let features = extractor.extract(&entity, &context).await.unwrap();
        
        assert!(features.contains_key("clone_mass"));
        assert!(features.contains_key("max_similarity"));
        assert!(features.contains_key("avg_similarity"));
        assert!(features.contains_key("duplicate_count"));
    }
    
    #[test]
    fn test_shingle_creation() {
        let extractor = LshExtractor::with_params(64, 2);
        let code = "def func():\n    return 1";
        let shingles = extractor.create_shingles(code);
        
        assert!(!shingles.is_empty());
    }
    
    #[test]
    fn test_minhash_signature() {
        let extractor = LshExtractor::with_params(16, 2);
        let code = "def test(): return 1";
        let signature = extractor.generate_minhash_signature(code);
        
        assert_eq!(signature.len(), 16);
        assert!(signature.iter().any(|&x| x != u64::MAX));
    }
    
    #[test]
    fn test_jaccard_similarity() {
        let sig1 = vec![1, 2, 3, 4];
        let sig2 = vec![1, 2, 5, 6];
        let sig3 = vec![1, 2, 3, 4];
        
        let extractor = LshExtractor::new();
        
        let sim12 = extractor.jaccard_similarity(&sig1, &sig2);
        let sim13 = extractor.jaccard_similarity(&sig1, &sig3);
        
        assert_eq!(sim12, 0.5); // 2 out of 4 match
        assert_eq!(sim13, 1.0); // Perfect match
    }
    
    #[test]
    fn test_lsh_index() {
        let mut index = LshIndex::new(4);
        
        let sig1 = MinHashSignature::new(vec![1, 2, 3, 4, 5, 6, 7, 8], 8, 2);
        let sig2 = MinHashSignature::new(vec![1, 2, 3, 4, 9, 10, 11, 12], 8, 2);
        
        index.add_entity("entity1".to_string(), sig1);
        index.add_entity("entity2".to_string(), sig2);
        
        let candidates = index.find_candidates("entity1");
        assert!(!candidates.is_empty());
    }
    
    #[test]
    fn test_weighted_shingle_analyzer() {
        let mut analyzer = WeightedShingleAnalyzer::new(3);
        
        // Create test entities
        let entity1 = CodeEntity::new(
            "test1",
            "function",
            "func1",
            "/test/file1.py"
        ).with_source_code("def func1():\n    x = 1\n    return x");
        
        let entity2 = CodeEntity::new(
            "test2",
            "function",
            "func2",
            "/test/file2.py"
        ).with_source_code("def func2():\n    y = 2\n    return y");
        
        let entities = vec![&entity1, &entity2];
        
        // Test IDF table construction
        let result = analyzer.build_idf_table(&entities);
        assert!(result.is_ok());
        
        // Test signature computation
        let signatures_result = analyzer.compute_weighted_signatures(&entities);
        assert!(signatures_result.is_ok());
        
        let signatures = signatures_result.unwrap();
        assert_eq!(signatures.len(), 2);
        assert!(signatures.contains_key("test1"));
        assert!(signatures.contains_key("test2"));
    }
    
    #[test]
    fn test_weighted_jaccard_similarity() {
        let analyzer = WeightedShingleAnalyzer::new(2);
        
        let sig1 = WeightedMinHashSignature::new(vec![1.0, 2.0, 3.0, 4.0]);
        let sig2 = WeightedMinHashSignature::new(vec![1.0, 2.0, 5.0, 6.0]);
        let sig3 = WeightedMinHashSignature::new(vec![1.0, 2.0, 3.0, 4.0]);
        
        let sim12 = analyzer.weighted_jaccard_similarity(&sig1, &sig2);
        let sim13 = analyzer.weighted_jaccard_similarity(&sig1, &sig3);
        
        assert_eq!(sim12, 0.5); // 2 out of 4 match
        assert_eq!(sim13, 1.0); // Perfect match
    }
    
    #[test]
    fn test_kgram_generation() {
        let analyzer = WeightedShingleAnalyzer::new(2);
        let code = "def func():\n    return 1";
        let kgrams = analyzer.generate_kgrams(code);
        
        assert!(!kgrams.is_empty());
        // Should contain k-grams like "def func", "func (", etc.
    }
    
    #[test]
    fn test_lsh_extractor_with_denoise() {
        let extractor = LshExtractor::new().with_denoise_enabled(true);
        
        // Should have weighted analyzer enabled
        assert!(extractor.weighted_analyzer.is_some());
        
        let extractor_disabled = LshExtractor::new().with_denoise_enabled(false);
        assert!(extractor_disabled.weighted_analyzer.is_none());
    }
}