//! Hash functions and weighted MinHash for similarity detection

use ahash::AHasher;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Weighted MinHash implementation for similarity detection
#[derive(Debug, Clone)]
pub struct WeightedMinHash {
    hash_functions: Vec<HashFunction>,
    num_functions: usize,
}

impl WeightedMinHash {
    /// Create a new WeightedMinHash with the specified number of hash functions
    pub fn new(num_functions: usize) -> Self {
        let mut hash_functions = Vec::with_capacity(num_functions);

        // Generate multiple hash functions with different seeds
        for i in 0..num_functions {
            hash_functions.push(HashFunction::new(i as u64));
        }

        Self {
            hash_functions,
            num_functions,
        }
    }

    /// Compute MinHash signature for a set of weighted tokens
    pub fn compute_signature(&self, tokens: &HashMap<String, f64>) -> WeightedSignature {
        let mut min_hashes = vec![u64::MAX; self.num_functions];
        let mut weights = vec![0.0; self.num_functions];

        // Process each token with its weight
        for (token, weight) in tokens {
            for (i, hash_func) in self.hash_functions.iter().enumerate() {
                let hash = hash_func.hash(token);
                let weighted_hash = self.apply_weight(hash, *weight);

                if weighted_hash < min_hashes[i] {
                    min_hashes[i] = weighted_hash;
                    weights[i] = *weight;
                }
            }
        }

        WeightedSignature::new(min_hashes, weights)
    }

    /// Apply weight to hash value
    fn apply_weight(&self, hash: u64, weight: f64) -> u64 {
        // Scale hash by inverse weight (higher weight = lower hash value = more likely to be minimum)
        if weight > 0.0 {
            ((hash as f64) / weight) as u64
        } else {
            u64::MAX
        }
    }

    /// Compute Jaccard similarity between two weighted signatures
    pub fn weighted_jaccard_similarity(
        &self,
        sig1: &WeightedSignature,
        sig2: &WeightedSignature,
    ) -> f64 {
        if sig1.hashes.len() != sig2.hashes.len() {
            return 0.0;
        }

        let mut matches = 0;
        let total = sig1.hashes.len();

        for i in 0..total {
            if sig1.hashes[i] == sig2.hashes[i] {
                matches += 1;
            }
        }

        matches as f64 / total as f64
    }

    /// Compute cosine similarity between two weighted signatures
    pub fn cosine_similarity(&self, sig1: &WeightedSignature, sig2: &WeightedSignature) -> f64 {
        if sig1.weights.len() != sig2.weights.len() {
            return 0.0;
        }

        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for i in 0..sig1.weights.len() {
            dot_product += sig1.weights[i] * sig2.weights[i];
            norm_a += sig1.weights[i] * sig1.weights[i];
            norm_b += sig2.weights[i] * sig2.weights[i];
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot_product / (norm_a.sqrt() * norm_b.sqrt())
    }

    /// Batch compute similarities for multiple signature pairs
    pub fn batch_similarities(&self, signatures: &[WeightedSignature]) -> Vec<Vec<f64>> {
        let n = signatures.len();
        let mut similarities = vec![vec![0.0; n]; n];

        // Use parallel processing for large signature sets
        if n > 100 {
            similarities
                .par_iter_mut()
                .enumerate()
                .for_each(|(i, row)| {
                    for j in i..n {
                        let sim = self.weighted_jaccard_similarity(&signatures[i], &signatures[j]);
                        row[j] = sim;
                        // Matrix is symmetric
                        if i != j {
                            // Note: We can't mutate similarities[j][i] here due to parallel iteration
                            // This will be handled in a second pass
                        }
                    }
                });

            // Fill in the symmetric part
            for i in 0..n {
                for j in 0..i {
                    similarities[i][j] = similarities[j][i];
                }
            }
        } else {
            // Sequential processing for smaller sets
            for i in 0..n {
                for j in i..n {
                    let sim = self.weighted_jaccard_similarity(&signatures[i], &signatures[j]);
                    similarities[i][j] = sim;
                    similarities[j][i] = sim;
                }
            }
        }

        similarities
    }
}

/// Hash function with a specific seed
#[derive(Debug, Clone)]
pub struct HashFunction {
    seed: u64,
    multiplier: u64,
    increment: u64,
}

impl HashFunction {
    /// Create a new hash function with the given seed
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            // Use different constants for each hash function
            multiplier: 1664525u64.wrapping_mul(seed.wrapping_add(1)),
            increment: 1013904223u64.wrapping_add(seed),
        }
    }

    /// Hash a string using this hash function
    pub fn hash(&self, input: &str) -> u64 {
        let mut hasher = AHasher::default();
        self.seed.hash(&mut hasher);
        self.multiplier.hash(&mut hasher);
        input.hash(&mut hasher);
        hasher.finish().wrapping_add(self.increment)
    }

    /// Hash binary data using this hash function
    pub fn hash_bytes(&self, bytes: &[u8]) -> u64 {
        let mut hasher = AHasher::default();
        self.seed.hash(&mut hasher);
        self.multiplier.hash(&mut hasher);
        bytes.hash(&mut hasher);
        hasher.finish().wrapping_add(self.increment)
    }
}

/// Weighted signature containing both hashes and weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedSignature {
    pub hashes: Vec<u64>,
    pub weights: Vec<f64>,
}

impl WeightedSignature {
    /// Create a new weighted signature
    pub fn new(hashes: Vec<u64>, weights: Vec<f64>) -> Self {
        assert_eq!(
            hashes.len(),
            weights.len(),
            "Hashes and weights must have the same length"
        );
        Self { hashes, weights }
    }

    /// Get the size of the signature (number of hash functions)
    pub fn size(&self) -> usize {
        self.hashes.len()
    }

    /// Check if the signature is empty
    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }

    /// Get the average weight of the signature
    pub fn average_weight(&self) -> f64 {
        if self.weights.is_empty() {
            0.0
        } else {
            self.weights.iter().sum::<f64>() / self.weights.len() as f64
        }
    }

    /// Get the maximum weight in the signature
    pub fn max_weight(&self) -> f64 {
        self.weights.iter().cloned().fold(0.0, f64::max)
    }

    /// Get the minimum weight in the signature
    pub fn min_weight(&self) -> f64 {
        self.weights.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    /// Merge two signatures by taking element-wise minimum hashes
    pub fn merge(&self, other: &WeightedSignature) -> Option<WeightedSignature> {
        if self.size() != other.size() {
            return None;
        }

        let mut merged_hashes = Vec::with_capacity(self.size());
        let mut merged_weights = Vec::with_capacity(self.size());

        for i in 0..self.size() {
            if self.hashes[i] <= other.hashes[i] {
                merged_hashes.push(self.hashes[i]);
                merged_weights.push(self.weights[i]);
            } else {
                merged_hashes.push(other.hashes[i]);
                merged_weights.push(other.weights[i]);
            }
        }

        Some(WeightedSignature::new(merged_hashes, merged_weights))
    }

    /// Convert to a compact string representation for caching
    pub fn to_compact_string(&self) -> String {
        let hash_str = self
            .hashes
            .iter()
            .map(|h| format!("{:x}", h))
            .collect::<Vec<_>>()
            .join(",");
        let weight_str = self
            .weights
            .iter()
            .map(|w| format!("{:.3}", w))
            .collect::<Vec<_>>()
            .join(",");
        format!("h:{};w:{}", hash_str, weight_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_function() {
        let hash_func = HashFunction::new(42);
        let hash1 = hash_func.hash("test");
        let hash2 = hash_func.hash("test");
        let hash3 = hash_func.hash("different");

        // Same input should produce same hash
        assert_eq!(hash1, hash2);
        // Different input should produce different hash
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_weighted_minhash() {
        let minhash = WeightedMinHash::new(64);

        let mut tokens1 = HashMap::new();
        tokens1.insert("hello".to_string(), 1.0);
        tokens1.insert("world".to_string(), 2.0);

        let mut tokens2 = HashMap::new();
        tokens2.insert("hello".to_string(), 1.0);
        tokens2.insert("rust".to_string(), 2.0);

        let sig1 = minhash.compute_signature(&tokens1);
        let sig2 = minhash.compute_signature(&tokens2);

        assert_eq!(sig1.size(), 64);
        assert_eq!(sig2.size(), 64);

        let similarity = minhash.weighted_jaccard_similarity(&sig1, &sig2);
        assert!(similarity >= 0.0 && similarity <= 1.0);
    }

    #[test]
    fn test_signature_identical_sets() {
        let minhash = WeightedMinHash::new(32);

        let mut tokens = HashMap::new();
        tokens.insert("test".to_string(), 1.0);
        tokens.insert("data".to_string(), 1.0);

        let sig1 = minhash.compute_signature(&tokens);
        let sig2 = minhash.compute_signature(&tokens);

        let similarity = minhash.weighted_jaccard_similarity(&sig1, &sig2);
        assert_eq!(similarity, 1.0); // Identical sets should have similarity of 1.0
    }

    #[test]
    fn test_signature_disjoint_sets() {
        let minhash = WeightedMinHash::new(32);

        let mut tokens1 = HashMap::new();
        tokens1.insert("a".to_string(), 1.0);
        tokens1.insert("b".to_string(), 1.0);

        let mut tokens2 = HashMap::new();
        tokens2.insert("x".to_string(), 1.0);
        tokens2.insert("y".to_string(), 1.0);

        let sig1 = minhash.compute_signature(&tokens1);
        let sig2 = minhash.compute_signature(&tokens2);

        let similarity = minhash.weighted_jaccard_similarity(&sig1, &sig2);
        // Disjoint sets should have low similarity, but not necessarily 0 due to hash collisions
        assert!(similarity < 0.5);
    }

    #[test]
    fn test_weighted_signature_stats() {
        let hashes = vec![1, 2, 3, 4, 5];
        let weights = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let signature = WeightedSignature::new(hashes, weights);

        assert_eq!(signature.size(), 5);
        assert!(!signature.is_empty());
        assert_eq!(signature.average_weight(), 3.0);
        assert_eq!(signature.max_weight(), 5.0);
        assert_eq!(signature.min_weight(), 1.0);
    }

    #[test]
    fn test_signature_merge() {
        let sig1 = WeightedSignature::new(vec![10, 30, 50], vec![1.0, 3.0, 5.0]);
        let sig2 = WeightedSignature::new(vec![20, 25, 60], vec![2.0, 2.5, 6.0]);

        let merged = sig1.merge(&sig2).unwrap();

        // Should take minimum hash values and corresponding weights
        assert_eq!(merged.hashes, vec![10, 25, 50]);
        assert_eq!(merged.weights, vec![1.0, 2.5, 5.0]);
    }

    #[test]
    fn test_cosine_similarity() {
        let minhash = WeightedMinHash::new(4);

        let sig1 = WeightedSignature::new(vec![1, 2, 3, 4], vec![1.0, 0.0, 1.0, 0.0]);
        let sig2 = WeightedSignature::new(vec![1, 2, 3, 4], vec![1.0, 0.0, 1.0, 0.0]);
        let sig3 = WeightedSignature::new(vec![1, 2, 3, 4], vec![0.0, 1.0, 0.0, 1.0]);

        let sim_identical = minhash.cosine_similarity(&sig1, &sig2);
        let sim_orthogonal = minhash.cosine_similarity(&sig1, &sig3);

        assert!((sim_identical - 1.0).abs() < 1e-10); // Identical vectors
        assert!((sim_orthogonal - 0.0).abs() < 1e-10); // Orthogonal vectors
    }
}
