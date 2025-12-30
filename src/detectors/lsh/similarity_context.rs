//! LSH similarity context for efficient similarity search.

use std::collections::HashMap;

use tracing::debug;

use super::config::LshConfig;
use super::index::LshIndex;
use super::metrics::LshContextStatistics;

/// O(n) similarity search context with prebuilt LSH index
#[derive(Debug)]
pub struct LshSimilarityContext {
    /// LSH index for efficient candidate search
    pub(crate) lsh_index: LshIndex,
    /// Signature storage for similarity computation
    pub(crate) signatures: HashMap<String, Vec<u64>>,
    /// LSH configuration used
    pub(crate) lsh_config: LshConfig,
    /// Number of entities in the context
    pub(crate) entities_count: usize,
}

impl LshSimilarityContext {
    /// Create a new similarity context
    pub fn new(
        lsh_index: LshIndex,
        signatures: HashMap<String, Vec<u64>>,
        lsh_config: LshConfig,
        entities_count: usize,
    ) -> Self {
        Self {
            lsh_index,
            signatures,
            lsh_config,
            entities_count,
        }
    }

    /// Find similar entities to the given entity using O(log n) LSH candidate search
    pub fn find_similar_entities(
        &self,
        entity_id: &str,
        max_results: Option<usize>,
    ) -> Vec<(String, f64)> {
        let start_time = std::time::Instant::now();

        // Use LSH index to find candidates efficiently
        let mut candidates = self.lsh_index.find_candidates(entity_id);

        // Limit results if requested
        if let Some(max) = max_results {
            candidates.truncate(max);
        }

        let elapsed = start_time.elapsed();
        debug!(
            "LSH candidate search for {} found {} candidates in {:?}",
            entity_id,
            candidates.len(),
            elapsed
        );

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
