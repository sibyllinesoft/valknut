//! Weighted shingle analysis for clone denoising
//!
//! This module implements TF-IDF weighted shingling to reduce the contribution
//! of common boilerplate patterns in clone detection.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use xxhash_rust::xxh3::Xxh3;

use crate::core::featureset::CodeEntity;

/// Summary statistics generated while building TF-IDF weighted shingles.
#[derive(Debug, Clone)]
pub struct WeightedShingleStats {
    /// Total number of code fragments analysed
    pub total_documents: usize,
    /// Total number of k-gram occurrences observed across the corpus
    pub total_grams: usize,
    /// Number of unique k-grams observed
    pub unique_grams: usize,
    /// Contribution percentage of the top 1% most frequent k-grams
    pub top1pct_contribution: f64,
}

/// TF-IDF weighted shingling to reduce the contribution of common boilerplate patterns.
#[derive(Debug)]
pub struct WeightedShingleAnalyzer {
    /// K-gram size for shingle generation (typically 9)
    pub(crate) k: usize,

    /// Global document frequency table per k-gram
    document_frequencies: HashMap<String, usize>,

    /// Total number of documents (functions) processed
    total_documents: usize,

    /// Pre-computed IDF weights for efficient lookup
    idf_weights: HashMap<String, f64>,
}

/// Factory, IDF table construction, and weighted signature methods for [`WeightedShingleAnalyzer`].
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
        info!(
            "Building IDF table for {} entities with k={}",
            entities.len(),
            self.k
        );

        self.reset_idf_state(entities.len());

        if self.total_documents == 0 {
            return Err("No entities provided for IDF table construction".to_string());
        }

        self.count_document_frequencies(entities);
        self.compute_idf_weights();
        self.log_idf_statistics();

        Ok(())
    }

    /// Reset IDF state for a new build
    fn reset_idf_state(&mut self, document_count: usize) {
        self.document_frequencies.clear();
        self.idf_weights.clear();
        self.total_documents = document_count;
    }

    /// Count document frequencies for all k-grams across entities
    fn count_document_frequencies(&mut self, entities: &[&CodeEntity]) {
        #[cfg(feature = "parallel")]
        self.count_document_frequencies_parallel(entities);

        #[cfg(not(feature = "parallel"))]
        self.count_document_frequencies_sequential(entities);
    }

    /// Parallel document frequency counting using map-reduce
    #[cfg(feature = "parallel")]
    fn count_document_frequencies_parallel(&mut self, entities: &[&CodeEntity]) {
        use rayon::prelude::*;
        use std::collections::HashMap;

        let local_frequency_maps: Vec<HashMap<String, usize>> = entities
            .par_chunks(50)
            .map(|chunk| self.count_chunk_frequencies(chunk))
            .collect();

        self.merge_frequency_maps(local_frequency_maps);
    }

    /// Count frequencies for a chunk of entities
    #[cfg(feature = "parallel")]
    fn count_chunk_frequencies(&self, chunk: &[&CodeEntity]) -> std::collections::HashMap<String, usize> {
        let mut local_frequencies = std::collections::HashMap::new();

        for entity in chunk {
            let unique_kgrams = self.get_unique_kgrams(&entity.source_code);
            for kgram in unique_kgrams {
                *local_frequencies.entry(kgram).or_insert(0) += 1;
            }
        }

        local_frequencies
    }

    /// Merge local frequency maps into global document_frequencies
    #[cfg(feature = "parallel")]
    fn merge_frequency_maps(&mut self, maps: Vec<std::collections::HashMap<String, usize>>) {
        for local_map in maps {
            for (kgram, local_count) in local_map {
                *self.document_frequencies.entry(kgram).or_insert(0) += local_count;
            }
        }
    }

    /// Sequential document frequency counting
    #[cfg(not(feature = "parallel"))]
    fn count_document_frequencies_sequential(&mut self, entities: &[&CodeEntity]) {
        for entity in entities {
            let unique_kgrams = self.get_unique_kgrams(&entity.source_code);
            for kgram in unique_kgrams {
                *self.document_frequencies.entry(kgram).or_insert(0) += 1;
            }
        }
    }

    /// Get unique k-grams from source code
    fn get_unique_kgrams(&self, source_code: &str) -> std::collections::HashSet<String> {
        self.generate_kgrams(source_code).into_iter().collect()
    }

    /// Compute IDF weights from document frequencies
    fn compute_idf_weights(&mut self) {
        let n = self.total_documents as f64;
        for (kgram, df) in &self.document_frequencies {
            let idf = ((1.0 + n) / (1.0 + *df as f64)).ln() + 1.0;
            self.idf_weights.insert(kgram.clone(), idf);
        }
    }

    /// Log IDF statistics for analysis
    fn log_idf_statistics(&self) {
        let stats = self.statistics();
        info!(
            "grams_total: {}, grams_top1pct_pctcontrib: {:.1}%",
            stats.unique_grams, stats.top1pct_contribution
        );

        let mut kgram_freqs: Vec<_> = self.document_frequencies.iter().collect();
        kgram_freqs.sort_by(|a, b| b.1.cmp(a.1));

        debug!("Top 5 most frequent k-grams:");
        for (i, (kgram, freq)) in kgram_freqs.iter().take(5).enumerate() {
            debug!(
                "  {}: \"{}\" (freq: {}, idf: {:.3})",
                i + 1,
                kgram,
                freq,
                self.idf_weights.get(*kgram).unwrap_or(&0.0)
            );
        }
    }

    /// Generate k-grams from source code tokens
    pub(crate) fn generate_kgrams(&self, source_code: &str) -> Vec<String> {
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
        super::generator::normalize_code(source_code)
    }

    /// Compute weighted MinHash signatures for all entities
    pub fn compute_weighted_signatures(
        &mut self,
        entities: &[&CodeEntity],
    ) -> std::result::Result<HashMap<String, WeightedMinHashSignature>, String> {
        // First build/update the IDF table
        self.build_idf_table(entities)?;

        let mut signatures = HashMap::new();

        for entity in entities {
            let signature = self.compute_weighted_signature_for_entity(entity)?;
            signatures.insert(entity.id.clone(), signature);
        }

        info!(
            "Computed weighted signatures for {} entities",
            signatures.len()
        );
        Ok(signatures)
    }

    /// Compute weighted MinHash signature for a single entity
    pub(crate) fn compute_weighted_signature_for_entity(
        &self,
        entity: &CodeEntity,
    ) -> std::result::Result<WeightedMinHashSignature, String> {
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

    /// Hash a string with a seed
    fn hash_with_seed(&self, data: &str, seed: u64) -> u64 {
        super::generator::hash_with_seed(data, seed)
    }

    /// Calculate weighted Jaccard similarity between two weighted signatures
    pub fn weighted_jaccard_similarity(
        &self,
        sig1: &WeightedMinHashSignature,
        sig2: &WeightedMinHashSignature,
    ) -> f64 {
        if sig1.signature.len() != sig2.signature.len() {
            return 0.0;
        }

        if sig1.signature.is_empty() {
            return 0.0;
        }

        // Use SIMD acceleration for large signatures (4+ elements)
        #[cfg(feature = "simd")]
        if sig1.signature.len() >= 4 {
            return self.weighted_jaccard_similarity_simd(&sig1.signature, &sig2.signature);
        }

        let matching = sig1.signature
            .iter()
            .zip(sig2.signature.iter())
            .filter(|(a, b)| (*a - *b).abs() < 1e-6) // Use small epsilon for float comparison
            .count();

        matching as f64 / sig1.signature.len() as f64
    }

    /// SIMD-accelerated weighted Jaccard similarity calculation for f64 signatures
    #[cfg(feature = "simd")]
    fn weighted_jaccard_similarity_simd(&self, sig1: &[f64], sig2: &[f64]) -> f64 {
        use wide::{f64x4, CmpLt};

        let len = sig1.len();
        let chunks = len / 4;
        let remainder = len % 4;
        let mut matching_count = 0usize;

        // Create epsilon vector for floating-point comparison
        let epsilon = f64x4::splat(1e-6);

        // Process in chunks of 4 using SIMD
        for chunk_idx in 0..chunks {
            let base_idx = chunk_idx * 4;

            let vec1 = f64x4::from([
                sig1[base_idx],
                sig1[base_idx + 1],
                sig1[base_idx + 2],
                sig1[base_idx + 3],
            ]);

            let vec2 = f64x4::from([
                sig2[base_idx],
                sig2[base_idx + 1],
                sig2[base_idx + 2],
                sig2[base_idx + 3],
            ]);

            // Calculate absolute difference: |a - b|
            let diff = (vec1 - vec2).abs();

            // Compare with epsilon: |a - b| < 1e-6
            let lt_epsilon = diff.cmp_lt(epsilon);

            // Count matching elements (each lane is either 0 or all 1s)
            let matches = lt_epsilon.to_array();
            for &match_val in &matches {
                if match_val != 0.0 {
                    // Non-zero means match (all bits set)
                    matching_count += 1;
                }
            }
        }

        // Handle remainder elements
        for i in (chunks * 4)..(chunks * 4 + remainder) {
            if (sig1[i] - sig2[i]).abs() < 1e-6 {
                matching_count += 1;
            }
        }

        matching_count as f64 / len as f64
    }

    /// Summarise TF-IDF statistics gathered during IDF table construction
    pub fn statistics(&self) -> WeightedShingleStats {
        let unique_grams = self.document_frequencies.len();
        let total_grams: usize = self.document_frequencies.values().copied().sum();

        let top1pct_threshold = (unique_grams as f64 * 0.01).ceil() as usize;
        let mut kgram_freqs: Vec<_> = self.document_frequencies.iter().collect();
        kgram_freqs.sort_by(|a, b| b.1.cmp(a.1));

        let top1pct_contribution = if !kgram_freqs.is_empty() && top1pct_threshold > 0 {
            let top1pct_count: usize = kgram_freqs
                .iter()
                .take(top1pct_threshold.min(kgram_freqs.len()))
                .map(|(_, freq)| **freq)
                .sum();
            if total_grams > 0 {
                (top1pct_count as f64 / total_grams as f64) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        WeightedShingleStats {
            total_documents: self.total_documents,
            total_grams,
            unique_grams,
            top1pct_contribution,
        }
    }
}

/// Weighted MinHash signature for clone denoising
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedMinHashSignature {
    /// The weighted signature values
    pub signature: Vec<f64>,
}

/// Factory methods for [`WeightedMinHashSignature`].
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
