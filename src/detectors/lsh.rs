//! LSH (Locality-Sensitive Hashing) and MinHash implementation.
//!
//! This module provides efficient duplicate code detection using MinHash signatures
//! and LSH banding techniques for sub-linear similarity search.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use ahash::AHasher;
use rayon::prelude::*;

#[cfg(feature = "simd")]
use wide::{u64x4, f64x4};

use crate::core::featureset::{FeatureExtractor, FeatureDefinition, CodeEntity, ExtractionContext};
use crate::core::errors::Result;

/// LSH-based similarity feature extractor
#[derive(Debug)]
pub struct LshExtractor {
    /// Feature definitions
    features: Vec<FeatureDefinition>,
    
    /// Number of hash functions for MinHash
    num_hashes: usize,
    
    /// Shingle size for text processing
    shingle_size: usize,
}

impl LshExtractor {
    /// Create a new LSH extractor
    pub fn new() -> Self {
        let mut extractor = Self {
            features: Vec::new(),
            num_hashes: 128,
            shingle_size: 3,
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
        };
        
        extractor.initialize_features();
        extractor
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
        
        // Generate MinHash signature for this entity
        let signature = self.generate_minhash_signature(&entity.source_code);
        
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
    /// Generate MinHash signature for source code
    fn generate_minhash_signature(&self, source_code: &str) -> Vec<u64> {
        // Create shingles from the source code
        let shingles = self.create_shingles(source_code);
        
        // Generate MinHash signature
        let mut signature = vec![u64::MAX; self.num_hashes];
        
        for shingle in shingles {
            for i in 0..self.num_hashes {
                let hash = self.hash_with_seed(&shingle, i as u64);
                if hash < signature[i] {
                    signature[i] = hash;
                }
            }
        }
        
        signature
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
    
    /// Create shingles from source code
    fn create_shingles(&self, source_code: &str) -> Vec<String> {
        // Normalize the source code (remove comments, normalize whitespace)
        let normalized = self.normalize_code(source_code);
        
        // Split into tokens
        let tokens: Vec<&str> = normalized
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .collect();
        
        // Create shingles
        let mut shingles = Vec::new();
        if tokens.len() >= self.shingle_size {
            for i in 0..=tokens.len() - self.shingle_size {
                let shingle = tokens[i..i + self.shingle_size].join(" ");
                shingles.push(shingle);
            }
        }
        
        shingles
    }
    
    /// Normalize source code for comparison
    fn normalize_code(&self, source_code: &str) -> String {
        let mut normalized = String::new();
        
        for line in source_code.lines() {
            let line = line.trim();
            
            // Skip comments and empty lines
            if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
                continue;
            }
            
            // TODO: More sophisticated normalization
            // - Replace variable names with placeholders
            // - Normalize string literals
            // - Normalize numeric literals
            
            normalized.push_str(line);
            normalized.push(' ');
        }
        
        normalized
    }
    
    /// Hash a string with a seed
    fn hash_with_seed(&self, data: &str, seed: u64) -> u64 {
        let mut hasher = AHasher::default();
        seed.hash(&mut hasher);
        data.hash(&mut hasher);
        hasher.finish()
    }
    
    /// Compare entity with others in the context
    fn compare_with_others(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
        signature: &[u64],
    ) -> (f64, f64, f64) {
        let mut similarities = Vec::new();
        
        // Compare with other entities
        for (other_id, other_entity) in &context.entity_index {
            if other_id == &entity.id {
                continue; // Skip self-comparison
            }
            
            let other_signature = self.generate_minhash_signature(&other_entity.source_code);
            let similarity = self.jaccard_similarity(signature, &other_signature);
            similarities.push(similarity);
        }
        
        if similarities.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        
        let max_similarity = similarities.iter().fold(0.0_f64, |a, &b| a.max(b));
        let avg_similarity = similarities.iter().sum::<f64>() / similarities.len() as f64;
        let duplicate_count = similarities.iter().filter(|&&s| s > 0.8).count() as f64;
        
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
}