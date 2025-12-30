//! Similarity comparison methods for LSH clone detection.
//!
//! This module provides functionality for comparing entities using MinHash signatures
//! and weighted Jaccard similarity.

use std::collections::{HashMap, HashSet};

use tracing::debug;

#[cfg(feature = "simd")]
use wide::u64x4;

use crate::core::featureset::{CodeEntity, EntityId, ExtractionContext};

use super::config::LshConfig;
use super::similarity_context::LshSimilarityContext;
use super::weighted::{WeightedMinHashSignature, WeightedShingleAnalyzer};

/// Similarity comparator for LSH-based clone detection.
pub struct SimilarityComparator<'a> {
    /// LSH configuration
    lsh_config: &'a LshConfig,
    /// Weighted shingle analyzer (if enabled)
    weighted_analyzer: Option<&'a WeightedShingleAnalyzer>,
}

impl<'a> SimilarityComparator<'a> {
    /// Create a new similarity comparator.
    pub fn new(
        lsh_config: &'a LshConfig,
        weighted_analyzer: Option<&'a WeightedShingleAnalyzer>,
    ) -> Self {
        Self {
            lsh_config,
            weighted_analyzer,
        }
    }

    /// Compare entity with others in the context using efficient LSH-based candidate search.
    pub fn compare_with_others<F>(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
        signature: &[u64],
        similarity_context: Option<&LshSimilarityContext>,
        weighted_signatures: Option<&HashMap<String, WeightedMinHashSignature>>,
        generate_signature_fn: F,
    ) -> (f64, f64, f64)
    where
        F: Fn(&str, &str) -> Vec<u64>,
    {
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
                weighted_signatures,
                generate_signature_fn,
            );
        }

        if partitions_available {
            debug!(
                entity = %entity.id,
                "No clique peers found; skipping similarity comparisons"
            );
            return (0.0, 0.0, 0.0);
        }

        if let Some(sim_context) = similarity_context {
            let max_results = if self.lsh_config.max_candidates == 0 {
                None
            } else {
                Some(self.lsh_config.max_candidates)
            };

            let mut similarities: Vec<f64> = sim_context
                .find_similar_entities(&entity.id, max_results)
                .into_iter()
                .filter_map(|(candidate_id, similarity)| {
                    if let Some(ref lookup) = candidate_lookup {
                        if !lookup.contains(candidate_id.as_str()) {
                            return None;
                        }
                    }

                    if similarity >= self.lsh_config.similarity_threshold {
                        Some(similarity)
                    } else {
                        None
                    }
                })
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

        self.compare_with_others_bruteforce(
            entity,
            context,
            signature,
            candidate_filter,
            weighted_signatures,
            generate_signature_fn,
        )
    }

    /// Get candidate filter for an entity from context partitions.
    fn candidate_filter<'b>(
        &self,
        entity: &CodeEntity,
        context: &'b ExtractionContext,
    ) -> Option<&'b Vec<EntityId>> {
        context
            .candidate_partitions
            .as_ref()
            .and_then(|partitions| partitions.get(&entity.id))
            .filter(|candidates| !candidates.is_empty())
    }

    /// Brute force comparison with all candidates.
    fn compare_with_others_bruteforce<F>(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
        signature: &[u64],
        candidate_filter: Option<&Vec<EntityId>>,
        weighted_signatures: Option<&HashMap<String, WeightedMinHashSignature>>,
        generate_signature_fn: F,
    ) -> (f64, f64, f64)
    where
        F: Fn(&str, &str) -> Vec<u64>,
    {
        let comparison_start = std::time::Instant::now();
        let candidate_count =
            candidate_filter.map_or(context.entity_index.len(), |filter| filter.len());
        let max_candidates = self.effective_max_candidates(candidate_count);

        // Try weighted comparison first
        let similarities = self
            .try_weighted_comparison(
                entity,
                context,
                candidate_filter,
                max_candidates,
                weighted_signatures,
            )
            .unwrap_or_default();

        // Fall back to basic minhash if weighted produced no results
        let similarities = if similarities.is_empty() {
            self.fallback_minhash_comparison(
                entity,
                context,
                signature,
                candidate_filter,
                max_candidates,
                generate_signature_fn,
            )
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
        weighted_signatures: Option<&HashMap<String, WeightedMinHashSignature>>,
    ) -> Option<Vec<f64>> {
        let analyzer = self.weighted_analyzer?;
        let weighted_sigs = weighted_signatures?;
        let entity_sig = weighted_sigs.get(&entity.id)?;

        let similarities = self.collect_weighted_similarities(
            &entity.id,
            entity_sig,
            weighted_sigs,
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
        let threshold = self.lsh_config.similarity_threshold;

        self.iterate_candidates(context, candidate_filter, entity_id, max_candidates)
            .filter_map(|other_id| {
                let other_sig = weighted_signatures.get(other_id)?;
                let similarity = analyzer.weighted_jaccard_similarity(entity_sig, other_sig);
                (similarity >= threshold).then_some(similarity)
            })
            .collect()
    }

    /// Fallback to basic minhash similarity comparison.
    fn fallback_minhash_comparison<F>(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
        signature: &[u64],
        candidate_filter: Option<&Vec<EntityId>>,
        max_candidates: usize,
        generate_signature_fn: F,
    ) -> Vec<f64>
    where
        F: Fn(&str, &str) -> Vec<u64>,
    {
        let threshold = self.lsh_config.similarity_threshold;

        self.iterate_candidates(context, candidate_filter, &entity.id, max_candidates)
            .filter_map(|other_id| {
                let other_entity = context.entity_index.get(other_id)?;
                let other_signature = generate_signature_fn(&other_entity.source_code, other_id);
                let similarity = jaccard_similarity(signature, &other_signature);
                (similarity >= threshold).then_some(similarity)
            })
            .collect()
    }

    /// Iterate over candidate entity IDs, excluding self and respecting max limit.
    fn iterate_candidates<'b>(
        &'b self,
        context: &'b ExtractionContext,
        candidate_filter: Option<&'b Vec<EntityId>>,
        exclude_id: &'b EntityId,
        max_candidates: usize,
    ) -> impl Iterator<Item = &'b EntityId> + 'b {
        let filtered_iter: Box<dyn Iterator<Item = &'b EntityId> + 'b> = match candidate_filter {
            Some(filter) => Box::new(filter.iter()),
            None => Box::new(context.entity_index.keys()),
        };

        filtered_iter
            .filter(move |id| *id != exclude_id)
            .take(max_candidates)
    }
}

/// Calculate Jaccard similarity between two MinHash signatures.
pub fn jaccard_similarity(sig1: &[u64], sig2: &[u64]) -> f64 {
    if sig1.len() != sig2.len() {
        return 0.0;
    }

    // Use SIMD acceleration for large signatures
    #[cfg(feature = "simd")]
    if sig1.len() >= 16 {
        return jaccard_similarity_simd(sig1, sig2);
    }

    let matching = sig1.iter().zip(sig2.iter()).filter(|(a, b)| a == b).count();
    matching as f64 / sig1.len() as f64
}

/// SIMD-accelerated Jaccard similarity calculation for large signatures.
#[cfg(feature = "simd")]
pub fn jaccard_similarity_simd(sig1: &[u64], sig2: &[u64]) -> f64 {
    let len = sig1.len();
    let chunks = len / 4;
    let remainder = len % 4;
    let mut matching_count = 0usize;

    // Process in chunks of 4 using SIMD
    for chunk_idx in 0..chunks {
        let base_idx = chunk_idx * 4;

        let vec1 = u64x4::from([
            sig1[base_idx],
            sig1[base_idx + 1],
            sig1[base_idx + 2],
            sig1[base_idx + 3],
        ]);

        let vec2 = u64x4::from([
            sig2[base_idx],
            sig2[base_idx + 1],
            sig2[base_idx + 2],
            sig2[base_idx + 3],
        ]);

        // Element-wise comparison
        let eq_mask = vec1.cmp_eq(vec2);

        // Count matching elements (each lane is either 0 or all 1s)
        let matches = eq_mask.to_array();
        for &match_val in &matches {
            if match_val == u64::MAX {
                matching_count += 1;
            }
        }
    }

    // Handle remainder elements
    for i in (chunks * 4)..(chunks * 4 + remainder) {
        if sig1[i] == sig2[i] {
            matching_count += 1;
        }
    }

    matching_count as f64 / len as f64
}

/// Summarize similarity results into (max, avg, duplicate_count).
pub fn summarise_similarities(similarities: &[f64]) -> (f64, f64, f64) {
    if similarities.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let max_similarity = similarities
        .iter()
        .fold(0.0_f64, |acc, &value| acc.max(value));
    let avg_similarity = similarities.iter().copied().sum::<f64>() / similarities.len() as f64;
    let duplicate_count = similarities.iter().filter(|&&s| s > 0.8).count() as f64;

    (max_similarity, avg_similarity, duplicate_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaccard_similarity_identical() {
        let sig1 = vec![1, 2, 3, 4, 5];
        let sig2 = vec![1, 2, 3, 4, 5];
        assert!((jaccard_similarity(&sig1, &sig2) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaccard_similarity_different() {
        let sig1 = vec![1, 2, 3, 4, 5];
        let sig2 = vec![6, 7, 8, 9, 10];
        assert!((jaccard_similarity(&sig1, &sig2) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaccard_similarity_partial() {
        let sig1 = vec![1, 2, 3, 4, 5];
        let sig2 = vec![1, 2, 3, 9, 10];
        assert!((jaccard_similarity(&sig1, &sig2) - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_summarise_similarities_empty() {
        let (max, avg, count) = summarise_similarities(&[]);
        assert_eq!(max, 0.0);
        assert_eq!(avg, 0.0);
        assert_eq!(count, 0.0);
    }

    #[test]
    fn test_summarise_similarities() {
        let similarities = vec![0.5, 0.7, 0.85, 0.9];
        let (max, avg, count) = summarise_similarities(&similarities);
        assert!((max - 0.9).abs() < f64::EPSILON);
        assert!((avg - 0.7375).abs() < 0.001);
        assert!((count - 2.0).abs() < f64::EPSILON); // 0.85 and 0.9 are > 0.8
    }
}
