//! LSH index for efficient similarity search.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use ahash::AHasher;

use super::signature::MinHashSignature;

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
            bands: vec![HashMap::with_capacity(32); num_bands], // Estimate 32 entities per band
            signatures: HashMap::with_capacity(256),            // Estimate 256 total entities
        }
    }

    /// Add an entity to the index
    pub fn add_entity(&mut self, entity_id: String, signature: MinHashSignature) {
        let hashes_per_band = signature.signature.len() / self.num_bands;

        // Calculate band hashes first
        let mut band_hashes = Vec::with_capacity(self.num_bands);

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
            self.bands[band_idx]
                .entry(band_hash)
                .or_default()
                .push(entity_id.clone());
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
        let mut results = Vec::with_capacity(candidates.len());
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

    /// Get a reference to a stored signature
    pub fn get_signature(&self, entity_id: &str) -> Option<&MinHashSignature> {
        self.signatures.get(entity_id)
    }

    /// Hash a band signature
    fn hash_band(&self, band_signature: &[u64]) -> u64 {
        let mut hasher = AHasher::default();
        band_signature.hash(&mut hasher);
        hasher.finish()
    }
}
