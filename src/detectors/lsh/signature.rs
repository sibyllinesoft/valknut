//! MinHash signature types for LSH similarity computation.

use serde::{Deserialize, Serialize};

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

        let matching = self
            .signature
            .iter()
            .zip(other.signature.iter())
            .filter(|(a, b)| a == b)
            .count();

        Some(matching as f64 / self.signature.len() as f64)
    }
}
