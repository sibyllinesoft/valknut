//! LSH (Locality-Sensitive Hashing) and MinHash implementation.
//!
//! This module provides efficient duplicate code detection using MinHash signatures
//! and LSH banding techniques for sub-linear similarity search.

pub mod ast_analysis;
pub mod comparison;
pub mod config;
pub mod memory_pool;
pub mod signatures;

pub use config::{
    AdaptiveDenoiseConfig, AutoCalibrationConfig, DedupeConfig, DedupeWeights, DenoiseConfig,
    DenoiseWeights, LshConfig, RankingBy, RankingConfig, RankingCriteria, StopMotifsConfig,
};

mod index;
mod lsh_cache;
mod metrics;
mod similarity_context;

// Re-export submodule types
pub use ast_analysis::{
    count_ast_nodes_from_index, count_distinct_blocks_from_index, AstAnalyzer, EntityAstStats,
};
pub use comparison::{
    collect_weighted_similarities, fallback_minhash_comparison, iterate_candidates,
    jaccard_similarity, summarise_similarities, SimilarityComparator,
};
pub use index::LshIndex;
pub use lsh_cache::{CacheStatistics, LshCache};
pub use memory_pool::{LshMemoryPools, PoolStatistics};
pub use metrics::{LshContextStatistics, LshPerformanceMetrics};
pub use similarity_context::LshSimilarityContext;

// Re-export from signatures submodule
pub use signatures::{
    count_tokens, MinHashSignature, ShingleGenerator, SignatureGenerator,
    WeightedMinHashSignature, WeightedShingleAnalyzer, WeightedShingleStats,
};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use rayon::prelude::*;
use tracing::{debug, info};

use crate::core::ast_service::AstService;
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::{
    CodeEntity, EntityId, ExtractionContext, FeatureDefinition, FeatureExtractor,
};
use crate::core::interning::InternedString;

/// LSH-based similarity feature extractor with O(n) candidate search
#[derive(Debug)]
pub struct LshExtractor {
    /// AST analyzer for fragment threshold checks (uses shared AST service)
    ast_analyzer: AstAnalyzer,
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
    cached_weighted_signatures:
        std::sync::RwLock<Option<HashMap<String, WeightedMinHashSignature>>>,

    /// Cache key to detect when weighted signatures need to be invalidated
    weighted_signatures_cache_key: std::sync::RwLock<Option<String>>,

    /// Cached similarity context built from the last extraction pass
    similarity_context_cache: std::sync::RwLock<Option<(String, Arc<LshSimilarityContext>)>>,
}

// EntityAstStats has been moved to ast_analysis module

/// Core LSH extraction and comparison methods for [`LshExtractor`].
impl LshExtractor {
    /// Create with specific parameters and optional dedupe config (internal helper).
    fn create(num_hashes: usize, shingle_size: usize, dedupe_config: Option<DedupeConfig>) -> Self {
        let ast_service = Arc::new(AstService::new());
        let mut extractor = Self {
            ast_analyzer: AstAnalyzer::new(ast_service),
            features: Vec::new(),
            num_hashes,
            shingle_size,
            dedupe_config,
            weighted_analyzer: None,
            lsh_config: LshConfig::default(),
            cache: LshCache::new(),
            memory_pools: LshMemoryPools::new(),
            performance_metrics: LshPerformanceMetrics::new(),
            cached_weighted_signatures: std::sync::RwLock::new(None),
            weighted_signatures_cache_key: std::sync::RwLock::new(None),
            similarity_context_cache: std::sync::RwLock::new(None),
        };
        extractor.initialize_features();
        extractor
    }

    /// Create a new LSH extractor with default parameters.
    pub fn new() -> Self {
        Self::create(128, 3, None)
    }

    /// Create with custom hash and shingle parameters.
    pub fn with_params(num_hashes: usize, shingle_size: usize) -> Self {
        Self::create(num_hashes, shingle_size, None)
    }

    /// Create with enhanced dedupe configuration.
    pub fn with_dedupe_config(dedupe_config: DedupeConfig) -> Self {
        let shingle_size = dedupe_config.shingle_k;
        Self::create(128, shingle_size, Some(dedupe_config))
    }

    /// Replace the internal AST service with a shared instance so multiple
    /// detectors operate on the same parse cache.
    pub fn with_shared_ast_service(mut self, ast_service: Arc<AstService>) -> Self {
        self.ast_analyzer = AstAnalyzer::new(ast_service);
        self
    }

    /// Expose the configured similarity threshold
    pub fn similarity_threshold(&self) -> f64 {
        self.lsh_config.similarity_threshold
    }

    /// Maximum number of candidates to consider per entity
    pub fn max_candidates(&self) -> Option<usize> {
        if self.lsh_config.max_candidates == 0 {
            None
        } else {
            Some(self.lsh_config.max_candidates)
        }
    }

    /// Minimum AST nodes required for a fragment (if dedupe thresholds are enabled)
    pub fn min_ast_nodes_threshold(&self) -> Option<usize> {
        self.dedupe_config.as_ref().map(|cfg| cfg.min_ast_nodes)
    }

    /// Obtain the cached similarity context when available
    pub fn similarity_context(
        &self,
        context: &ExtractionContext,
    ) -> Option<Arc<LshSimilarityContext>> {
        self.get_similarity_context(context)
    }

    /// Returns candidate entities from partition map for similarity comparison.
    fn candidate_filter<'a>(
        &self,
        entity: &CodeEntity,
        context: &'a ExtractionContext,
    ) -> Option<&'a Vec<EntityId>> {
        context
            .candidate_partitions
            .as_ref()
            .and_then(|partitions| partitions.get(&entity.id))
            .filter(|candidates| !candidates.is_empty())
    }

    /// Check whether an entity passes the fragment thresholds configured for dedupe analysis
    pub async fn entity_passes_thresholds(&self, entity: &CodeEntity) -> Result<bool> {
        if let Some(ref config) = self.dedupe_config {
            return self.meets_fragment_thresholds(entity, config).await;
        }
        Ok(true)
    }

    /// Compute AST statistics for an entity (delegates to AstAnalyzer)
    pub async fn compute_entity_ast_stats(
        &self,
        entity: &CodeEntity,
    ) -> Result<Option<EntityAstStats>> {
        self.ast_analyzer.compute_entity_ast_stats(entity).await
    }

    /// Compute weighted shingle signatures and statistics when denoising is enabled
    pub fn weighted_signatures_with_stats(
        &self,
        entities: &[&CodeEntity],
    ) -> std::result::Result<
        (
            HashMap<String, WeightedMinHashSignature>,
            WeightedShingleStats,
        ),
        String,
    > {
        let analyzer_template = self
            .weighted_analyzer
            .as_ref()
            .ok_or_else(|| "Weighted analyzer not enabled".to_string())?;

        let mut analyzer_copy = WeightedShingleAnalyzer::new(analyzer_template.k);
        let signatures = analyzer_copy.compute_weighted_signatures(entities)?;
        let stats = analyzer_copy.statistics();

        Ok((signatures, stats))
    }

    /// Compute TF-IDF statistics for the provided entities when denoising is enabled
    pub fn weighted_statistics(
        &self,
        entities: &[&CodeEntity],
    ) -> std::result::Result<WeightedShingleStats, String> {
        let (_, stats) = self.weighted_signatures_with_stats(entities)?;
        Ok(stats)
    }

    /// Enable weighted shingle analysis for clone denoising
    pub fn with_denoise_enabled(mut self, enable_denoise: bool) -> Self {
        if enable_denoise {
            self.weighted_analyzer = Some(WeightedShingleAnalyzer::new(self.shingle_size));
            info!(
                "WeightedShingleAnalyzer enabled for clone denoising with k={}",
                self.shingle_size
            );
        }
        self
    }

    /// Configure LSH parameters for efficient similarity search
    pub fn with_lsh_config(mut self, lsh_config: LshConfig) -> Self {
        self.num_hashes = lsh_config.num_hashes;
        self.shingle_size = lsh_config.shingle_size;

        // Update memory pools to match signature size
        self.memory_pools = LshMemoryPools::with_capacity(50, self.num_hashes);

        info!(
            "LSH configuration: {} hashes, {} bands, {} shingle size",
            lsh_config.num_hashes, lsh_config.num_bands, lsh_config.shingle_size
        );
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
        info!(
            "LSH Cache Statistics: hits={}, misses={}, hit_rate={:.1}%",
            cache_stats.token_hits + cache_stats.signature_hits,
            cache_stats.token_misses + cache_stats.signature_misses,
            cache_stats.overall_hit_rate() * 100.0
        );

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
        if let Ok(mut similarity_cache) = self.similarity_context_cache.write() {
            *similarity_cache = None;
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

    /// Gets or creates a cached similarity context for fast lookups.
    fn get_similarity_context(
        &self,
        context: &ExtractionContext,
    ) -> Option<Arc<LshSimilarityContext>> {
        if context.entity_index.is_empty() {
            return None;
        }

        let entity_refs: Vec<&CodeEntity> = context.entity_index.values().collect();
        let cache_key = self.generate_cache_key(&entity_refs);

        if let Ok(cache_guard) = self.similarity_context_cache.read() {
            if let Some((ref existing_key, ref cached_context)) = *cache_guard {
                if *existing_key == cache_key {
                    return Some(cached_context.clone());
                }
            }
        }

        let context_instance = Arc::new(self.create_similarity_search_context(&entity_refs));
        if let Ok(mut cache_guard) = self.similarity_context_cache.write() {
            *cache_guard = Some((cache_key, context_instance.clone()));
        }

        Some(context_instance)
    }

    /// Try to get cached weighted signatures if the cache key matches.
    fn try_get_cached_weighted_signatures(
        &self,
        cache_key: &str,
    ) -> Option<HashMap<String, WeightedMinHashSignature>> {
        let cache_key_read = self.weighted_signatures_cache_key.read().ok()?;
        let existing_key = cache_key_read.as_ref()?;
        if existing_key != cache_key {
            return None;
        }

        let cached_sigs = self.cached_weighted_signatures.read().ok()?;
        let signatures = cached_sigs.as_ref()?;
        debug!(
            "Using cached weighted signatures for {} entities",
            signatures.len()
        );
        Some(signatures.clone())
    }

    /// Get cached weighted signatures or compute them if not cached
    fn get_or_compute_weighted_signatures(
        &self,
        entities: &[&crate::core::featureset::CodeEntity],
    ) -> std::result::Result<HashMap<String, WeightedMinHashSignature>, String> {
        if let Some(ref analyzer) = self.weighted_analyzer {
            let cache_key = self.generate_cache_key(entities);

            // Check if signatures are cached
            if let Some(cached) = self.try_get_cached_weighted_signatures(&cache_key) {
                return Ok(cached);
            }

            // Cache miss - compute signatures
            info!(
                "Computing weighted signatures for {} entities (cache miss)",
                entities.len()
            );
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
    fn get_or_compute_weighted_signatures_with_current(
        &self,
        context_entities: &[&crate::core::featureset::CodeEntity],
        current_entity: &crate::core::featureset::CodeEntity,
    ) -> std::result::Result<HashMap<String, WeightedMinHashSignature>, String> {
        if let Some(ref analyzer) = self.weighted_analyzer {
            // Use stable cache key based only on context entities
            let cache_key = self.generate_cache_key(context_entities);

            // Check if signatures are cached
            if let Some(cached) = self.try_get_cached_weighted_signatures(&cache_key) {
                return Ok(cached);
            }

            // Cache miss - compute signatures for ALL entities (context + current)
            let mut all_entities = context_entities.to_vec();
            all_entities.push(current_entity);

            info!(
                "Computing weighted signatures for {} entities (cache miss)",
                all_entities.len()
            );
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
        signatures::generator::create_shingles(self, source_code)
    }

    /// Create interned shingles from source code for zero-allocation performance
    /// This is the high-performance version that uses string interning
    pub fn create_shingles_interned(&self, source_code: &str) -> Vec<InternedString> {
        signatures::generator::create_shingles_interned(self, source_code)
    }

    /// Public access to minhash signature generation for benchmarking
    pub fn generate_minhash_signature(&self, source_code: &str) -> Vec<u64> {
        #[cfg(feature = "simd")]
        {
            signatures::generator::generate_minhash_signature_simd(self, source_code)
        }
        #[cfg(not(feature = "simd"))]
        {
            signatures::generator::generate_minhash_signature(self, source_code)
        }
    }

    /// Generate MinHash signature using interned strings for optimal performance
    /// This version eliminates string allocation overhead in the hot loop
    pub fn generate_minhash_signature_interned(&self, source_code: &str) -> Vec<u64> {
        #[cfg(feature = "simd")]
        {
            signatures::generator::generate_minhash_signature_simd(self, source_code)
        }
        #[cfg(not(feature = "simd"))]
        {
            signatures::generator::generate_minhash_signature_interned(self, source_code)
        }
    }

    /// Initialize LSH feature definitions
    fn initialize_features(&mut self) {
        self.features = vec![
            FeatureDefinition::new("clone_mass", "Fraction of code that appears to be cloned")
                .with_range(0.0, 1.0)
                .with_default(0.0),
            FeatureDefinition::new("max_similarity", "Maximum similarity to any other entity")
                .with_range(0.0, 1.0)
                .with_default(0.0),
            FeatureDefinition::new("avg_similarity", "Average similarity to all other entities")
                .with_range(0.0, 1.0)
                .with_default(0.0),
            FeatureDefinition::new("duplicate_count", "Number of potential duplicates found")
                .with_range(0.0, 100.0)
                .with_default(0.0),
        ];
    }
}

/// Default implementation for [`LshExtractor`].
impl Default for LshExtractor {
    /// Returns a new LSH extractor with default parameters.
    fn default() -> Self {
        Self::new()
    }
}

/// [`SignatureGenerator`] implementation for LSH signature creation.
impl SignatureGenerator for LshExtractor {
    /// Returns the number of hash functions used.
    fn num_hashes(&self) -> usize {
        self.num_hashes
    }

    /// Returns the shingle (k-gram) size.
    fn shingle_size(&self) -> usize {
        self.shingle_size
    }

    /// Returns a reference to the signature cache.
    fn cache(&self) -> &LshCache {
        &self.cache
    }

    /// Returns a reference to the memory pools for allocation reuse.
    fn memory_pools(&self) -> &LshMemoryPools {
        &self.memory_pools
    }
}

/// [`FeatureExtractor`] implementation for LSH-based similarity features.
#[async_trait]
impl FeatureExtractor for LshExtractor {
    /// Returns the extractor name ("lsh").
    fn name(&self) -> &str {
        "lsh"
    }

    /// Returns the feature definitions for this extractor.
    fn features(&self) -> &[FeatureDefinition] {
        &self.features
    }

    /// Extracts LSH similarity features for an entity.
    async fn extract(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::with_capacity(8); // Typical LSH analysis produces 5-10 features

        // Apply enhanced fragment analysis if dedupe config is available
        if let Some(ref config) = self.dedupe_config {
            if !self.meets_fragment_thresholds(entity, config).await? {
                features.insert("clone_mass".to_string(), 0.0);
                features.insert("max_similarity".to_string(), 0.0);
                features.insert("avg_similarity".to_string(), 0.0);
                features.insert("duplicate_count".to_string(), 0.0);
                return Ok(features);
            }
        }

        // Generate MinHash signature for this entity using optimized interned version
        let signature = signatures::generator::generate_minhash_signature_interned(self, &entity.source_code);

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

    /// Checks if this extractor supports the given entity type.
    fn supports_entity(&self, _entity: &CodeEntity) -> bool {
        // LSH can work with any code entity
        true
    }
}

/// Signature generation and comparison helpers for [`LshExtractor`].
impl LshExtractor {
    /// Parallel MinHash signature generation for multiple entities
    #[cfg(feature = "parallel")]
    pub fn generate_signatures_parallel(&self, entities: &[CodeEntity]) -> Vec<Vec<u64>> {
        entities
            .par_iter()
            .map(|entity| {
                #[cfg(feature = "simd")]
                {
                    signatures::generator::generate_minhash_signature_simd(self, &entity.source_code)
                }
                #[cfg(not(feature = "simd"))]
                {
                    signatures::generator::generate_minhash_signature(self, &entity.source_code)
                }
            })
            .collect()
    }

    /// Check if entity meets fragment analysis thresholds using structural data
    async fn meets_fragment_thresholds(
        &self,
        entity: &CodeEntity,
        config: &DedupeConfig,
    ) -> Result<bool> {
        self.ast_analyzer.meets_fragment_thresholds(entity, config).await
    }

    /// Build LSH index for all entities in the context for O(n) candidate search
    fn build_lsh_index_for_context(&self, context: &ExtractionContext) -> LshIndex {
        let start_time = std::time::Instant::now();
        let mut lsh_index = LshIndex::new(self.lsh_config.num_bands);

        debug!(
            "Building LSH index for {} entities",
            context.entity_index.len()
        );

        // Add all entities to the LSH index using optimized interned version
        for (entity_id, entity) in &context.entity_index {
            let signature =
                signatures::generator::generate_minhash_signature_interned(self, &entity.source_code);
            let minhash_sig = MinHashSignature::new(signature, self.num_hashes, self.shingle_size);
            lsh_index.add_entity(entity_id.clone(), minhash_sig);
        }

        let elapsed = start_time.elapsed();
        info!(
            "Built LSH index in {:?} for {} entities with {} bands",
            elapsed,
            context.entity_index.len(),
            self.lsh_config.num_bands
        );

        lsh_index
    }

    /// O(n) similarity search API - builds index once and provides efficient candidate search
    pub fn create_similarity_search_context(
        &self,
        entities: &[&CodeEntity],
    ) -> LshSimilarityContext {
        let start_time = std::time::Instant::now();
        let mut lsh_index = LshIndex::new(self.lsh_config.num_bands);
        let mut signatures = HashMap::with_capacity(entities.len());

        info!(
            "Building LSH similarity context for {} entities",
            entities.len()
        );

        // Build index and store signatures using optimized interned version
        for entity in entities {
            let signature =
                signatures::generator::generate_minhash_signature_interned(self, &entity.source_code);
            let minhash_sig =
                MinHashSignature::new(signature.clone(), self.num_hashes, self.shingle_size);
            lsh_index.add_entity(entity.id.clone(), minhash_sig);
            signatures.insert(entity.id.clone(), signature);
        }

        let elapsed = start_time.elapsed();
        info!("Built LSH similarity context in {:?}", elapsed);

        LshSimilarityContext::new(
            lsh_index,
            signatures,
            self.lsh_config.clone(),
            entities.len(),
        )
    }

    /// Compare entity with others in the context using efficient LSH-based candidate search
    fn compare_with_others(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
        signature: &[u64],
    ) -> (f64, f64, f64) {
        let (candidate_filter, candidate_lookup): (Option<&Vec<EntityId>>, Option<HashSet<&str>>) =
            if let Some(filter) = self.candidate_filter(entity, context) {
                let lookup = filter.iter().map(|s| s.as_str()).collect::<HashSet<&str>>();
                (Some(filter), Some(lookup))
            } else {
                (None, None)
            };

        let partitions_available = context
            .candidate_partitions
            .as_ref()
            .map(|p| !p.is_empty())
            .unwrap_or(false);

        if candidate_filter.is_some() {
            return self.compare_with_others_bruteforce(
                entity,
                context,
                signature,
                candidate_filter,
            );
        }

        if partitions_available {
            debug!(
                entity = %entity.id,
                "No clique peers found; skipping similarity comparisons"
            );
            return (0.0, 0.0, 0.0);
        }

        if let Some(similarity_context) = self.get_similarity_context(context) {
            let max_results = if self.lsh_config.max_candidates == 0 {
                None
            } else {
                Some(self.lsh_config.max_candidates)
            };

            let threshold = self.lsh_config.similarity_threshold;
            let mut similarities: Vec<f64> = similarity_context
                .find_similar_entities(&entity.id, max_results)
                .into_iter()
                .filter(|(candidate_id, _)| {
                    candidate_lookup
                        .as_ref()
                        .map_or(true, |lookup| lookup.contains(candidate_id.as_str()))
                })
                .filter_map(|(_, similarity)| (similarity >= threshold).then_some(similarity))
                .collect();

            if !similarities.is_empty() {
                debug!(
                    "LSH index similarity search found {} candidates for {}",
                    similarities.len(),
                    entity.id
                );
                return summarise_similarities(&similarities);
            }
        }

        self.compare_with_others_bruteforce(entity, context, signature, candidate_filter)
    }

    /// Compares entity against others using brute-force MinHash comparison.
    fn compare_with_others_bruteforce(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
        signature: &[u64],
        candidate_filter: Option<&Vec<EntityId>>,
    ) -> (f64, f64, f64) {
        let comparison_start = std::time::Instant::now();
        let candidate_count =
            candidate_filter.map_or(context.entity_index.len(), |filter| filter.len());
        let max_candidates = self.effective_max_candidates(candidate_count);

        // Try weighted comparison first
        let similarities = self
            .try_weighted_comparison(entity, context, candidate_filter, max_candidates)
            .unwrap_or_default();

        // Fall back to basic minhash if weighted produced no results
        let similarities = if similarities.is_empty() {
            self.fallback_minhash_comparison(entity, context, signature, candidate_filter, max_candidates)
        } else {
            similarities
        };

        debug!(
            "Fallback similarity comparison for {} completed in {:?} with {} matches",
            entity.id,
            comparison_start.elapsed(),
            similarities.len()
        );

        summarise_similarities(&similarities)
    }

    /// Compute effective max candidates based on config and available count.
    fn effective_max_candidates(&self, candidate_count: usize) -> usize {
        if self.lsh_config.max_candidates == 0 {
            candidate_count
        } else {
            self.lsh_config.max_candidates.min(candidate_count)
        }
    }

    /// Try weighted similarity comparison using TF-IDF weighted shingles.
    fn try_weighted_comparison(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
        candidate_filter: Option<&Vec<EntityId>>,
        max_candidates: usize,
    ) -> Option<Vec<f64>> {
        let analyzer = self.weighted_analyzer.as_ref()?;
        let context_entities: Vec<&CodeEntity> = context.entity_index.values().collect();
        let weighted_signatures = self
            .get_or_compute_weighted_signatures_with_current(&context_entities, entity)
            .ok()?;
        let entity_sig = weighted_signatures.get(&entity.id)?;

        let similarities = self.collect_weighted_similarities(
            &entity.id,
            entity_sig,
            &weighted_signatures,
            analyzer,
            context,
            candidate_filter,
            max_candidates,
        );

        Some(similarities)
    }

    /// Collect similarities using weighted Jaccard from candidate iterator.
    fn collect_weighted_similarities(
        &self,
        entity_id: &EntityId,
        entity_sig: &WeightedMinHashSignature,
        weighted_signatures: &HashMap<EntityId, WeightedMinHashSignature>,
        analyzer: &WeightedShingleAnalyzer,
        context: &ExtractionContext,
        candidate_filter: Option<&Vec<EntityId>>,
        max_candidates: usize,
    ) -> Vec<f64> {
        comparison::collect_weighted_similarities(
            entity_id,
            entity_sig,
            weighted_signatures,
            analyzer,
            context,
            candidate_filter,
            max_candidates,
            self.lsh_config.similarity_threshold,
        )
    }

    /// Fallback to basic minhash similarity comparison.
    fn fallback_minhash_comparison(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
        signature: &[u64],
        candidate_filter: Option<&Vec<EntityId>>,
        max_candidates: usize,
    ) -> Vec<f64> {
        comparison::fallback_minhash_comparison(
            entity,
            context,
            signature,
            candidate_filter,
            max_candidates,
            self.lsh_config.similarity_threshold,
            |source_code, entity_id| {
                signatures::generator::generate_minhash_signature_cached(self, source_code, entity_id)
            },
        )
    }

    /// Iterate over candidate entity IDs, excluding self and respecting max limit.
    fn iterate_candidates<'a>(
        &'a self,
        context: &'a ExtractionContext,
        candidate_filter: Option<&'a Vec<EntityId>>,
        exclude_id: &'a EntityId,
        max_candidates: usize,
    ) -> impl Iterator<Item = &'a EntityId> + 'a {
        comparison::iterate_candidates(context, candidate_filter, exclude_id, max_candidates)
    }

    /// Calculate Jaccard similarity between two MinHash signatures
    fn jaccard_similarity(&self, sig1: &[u64], sig2: &[u64]) -> f64 {
        comparison::jaccard_similarity(sig1, sig2)
    }
}

// summarise_similarities has been moved to comparison module

#[cfg(test)]
mod tests;
