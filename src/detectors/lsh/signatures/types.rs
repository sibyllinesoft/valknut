//! MinHash signature types for LSH similarity computation.

use serde::{Deserialize, Serialize};

use super::super::comparison::jaccard_similarity as compute_jaccard;

/// MinHash signature for efficient similarity computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinHashSignature {
    /// The signature values
    pub signature: Vec<u64>,

    /// Parameters used to generate this signature
    pub num_hashes: usize,
    pub shingle_size: usize,
}

/// Factory and similarity computation methods for [`MinHashSignature`].
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

        Some(compute_jaccard(&self.signature, &other.signature))
    }
}
