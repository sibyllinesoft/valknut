//! Clone detection types and helpers for LSH analysis.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::core::ast_service::CachedTree;
use crate::core::featureset::CodeEntity;

use crate::core::pipeline::results::pipeline_results::CloneVerificationResults;

// Re-export APTED verification types and functions
pub use super::apted_verification::{
    build_simple_ast_for_entity, build_simple_ast_recursive, compute_apted_verification,
    get_or_build_simple_ast, hash_kind, parse_byte_range, CachedSimpleAst, SimpleAstNode,
};

/// Endpoint of a clone pair
#[derive(Debug, Clone, Serialize)]
pub struct CloneEndpoint {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<(usize, usize)>,
}

/// Factory methods for [`CloneEndpoint`].
impl CloneEndpoint {
    /// Creates a clone endpoint from a code entity.
    pub fn from_entity(entity: &CodeEntity) -> Self {
        Self {
            id: entity.id.clone(),
            name: entity.name.clone(),
            path: entity.file_path.clone(),
            range: entity.line_range,
        }
    }
}

/// Verification details for a clone pair
#[derive(Debug, Clone, Serialize)]
pub struct CloneVerificationDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_cost: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_counts: Option<(usize, usize)>,
    pub truncated: bool,
}

/// Report for a detected clone pair
#[derive(Debug, Clone, Serialize)]
pub struct ClonePairReport {
    pub source: CloneEndpoint,
    pub target: CloneEndpoint,
    pub similarity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<CloneVerificationDetail>,
}

/// Results from collecting entities for LSH analysis.
pub struct LshEntityCollection {
    pub entities: Vec<CodeEntity>,
    pub entity_index: HashMap<String, CodeEntity>,
    pub ast_cache: HashMap<String, Arc<CachedTree>>,
}

/// Factory methods for [`LshEntityCollection`].
impl LshEntityCollection {
    /// Creates a new empty entity collection.
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            entity_index: HashMap::new(),
            ast_cache: HashMap::new(),
        }
    }
}

/// Default implementation for [`LshEntityCollection`].
impl Default for LshEntityCollection {
    /// Returns a new empty entity collection.
    fn default() -> Self {
        Self::new()
    }
}

/// Parameters for LSH clone detection
pub struct LshDetectionParams {
    pub candidate_limit: Option<usize>,
    pub min_ast_nodes: usize,
    pub lsh_threshold: f64,
    pub verify_with_apted: bool,
    pub apted_limit: Option<usize>,
    pub apted_max_nodes: usize,
}

/// Statistics collected during clone detection
#[derive(Default)]
pub struct CloneDetectionStats {
    pub max_similarity: f64,
    pub similarity_total: f64,
    pub similarity_count: usize,
    pub apted_similarity_total: f64,
    pub apted_similarity_count: usize,
    pub apted_pairs_requested: usize,
    pub apted_pairs_scored: usize,
}

/// Tracking and summary methods for [`CloneDetectionStats`].
impl CloneDetectionStats {
    /// Records a similarity score for aggregation.
    pub fn record_similarity(&mut self, similarity: f64) {
        self.max_similarity = self.max_similarity.max(similarity);
        self.similarity_total += similarity;
        self.similarity_count += 1;
    }

    /// Records verification details and updates APTED statistics.
    pub fn record_verification(
        &mut self,
        detail: &Option<CloneVerificationDetail>,
        apted_evaluated: &mut usize,
    ) {
        if let Some(ref d) = detail {
            if d.node_counts.is_some() {
                *apted_evaluated += 1;
            }
            if let Some(sim) = d.similarity {
                self.apted_pairs_scored += 1;
                self.apted_similarity_total += sim;
                self.apted_similarity_count += 1;
            }
        }
    }

    /// Calculates the average similarity across all recorded pairs.
    pub fn avg_similarity(&self) -> f64 {
        if self.similarity_count > 0 {
            self.similarity_total / self.similarity_count as f64
        } else {
            0.0
        }
    }

    /// Generates a verification summary for reporting.
    pub fn verification_summary(&self, enabled: bool) -> Option<CloneVerificationResults> {
        if !enabled {
            return None;
        }
        Some(CloneVerificationResults {
            method: "apted".to_string(),
            pairs_considered: self.similarity_count,
            pairs_evaluated: self.apted_pairs_requested,
            pairs_scored: self.apted_pairs_scored,
            avg_similarity: if self.apted_similarity_count > 0 {
                Some(self.apted_similarity_total / self.apted_similarity_count as f64)
            } else {
                None
            },
        })
    }
}

/// Compute APTED limit from LSH config settings.
pub fn compute_apted_limit(settings: &crate::core::config::LshConfig) -> Option<usize> {
    let limit = if settings.apted_max_pairs_per_entity == 0 {
        settings.max_candidates
    } else if settings.max_candidates == 0 {
        settings.apted_max_pairs_per_entity
    } else {
        settings.apted_max_pairs_per_entity.min(settings.max_candidates)
    };
    if limit == 0 {
        None
    } else {
        Some(limit)
    }
}

/// Log partition statistics for clone detection.
pub fn log_partition_stats(partitions: &crate::detectors::graph::clique::CliquePartitions) {
    use tracing::info;
    let partition_count = partitions.len();
    let total_peers: usize = partitions.values().map(|g| g.len()).sum();
    let max_peers = partitions.values().map(|g| g.len()).max().unwrap_or(0);
    let avg_peers = if partition_count > 0 {
        total_peers as f64 / partition_count as f64
    } else {
        0.0
    };
    info!(
        entities_with_peers = partition_count,
        avg_peers = avg_peers,
        max_peers = max_peers,
        "Similarity clique pre-filter enabled"
    );
}

/// Create ordered pair key for deduplication.
pub fn ordered_pair_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// Check if a clone pair should be skipped due to small AST size.
pub fn should_skip_small_pair(
    verification: &Option<CloneVerificationDetail>,
    min_ast_nodes: usize,
    entity: &CodeEntity,
    candidate: &CodeEntity,
) -> bool {
    if min_ast_nodes == 0 {
        return false;
    }
    if let Some(ref detail) = verification {
        if let Some(ref counts) = detail.node_counts {
            let observed_min = counts.0.min(counts.1);
            if observed_min < min_ast_nodes {
                debug!(
                    "Skipping clone pair below min_ast_nodes (min {}): {} -> {} ({:?})",
                    min_ast_nodes, entity.id, candidate.id, counts
                );
                return true;
            }
        }
    }
    false
}

/// Filter clone pairs below minimum AST node threshold.
pub fn filter_small_pairs(mut pairs: Vec<ClonePairReport>, min_ast_nodes: usize) -> Vec<ClonePairReport> {
    use tracing::info;
    if min_ast_nodes == 0 {
        return pairs;
    }
    let before = pairs.len();
    pairs.retain(|pair| {
        if let Some(ref ver) = pair.verification {
            if let Some(counts) = ver.node_counts {
                return counts.0.min(counts.1) >= min_ast_nodes;
            }
        }
        true
    });
    let filtered = before.saturating_sub(pairs.len());
    if filtered > 0 {
        info!(filtered, min_ast_nodes, "Filtered clone pairs below min_ast_nodes");
    }
    pairs
}

/// Serialize clone pairs to JSON values, filtering by min_ast_nodes threshold.
pub fn serialize_clone_pairs(
    clone_pairs: Vec<ClonePairReport>,
    min_ast_nodes: usize,
) -> Vec<serde_json::Value> {
    let mut serialized = Vec::with_capacity(clone_pairs.len());

    for pair in clone_pairs {
        let Ok(value) = serde_json::to_value(&pair) else {
            warn!(
                "Failed to serialize clone pair {} -> {}",
                pair.source.id, pair.target.id
            );
            continue;
        };

        if !passes_ast_node_threshold(&value, min_ast_nodes) {
            continue;
        }
        serialized.push(value);
    }

    serialized
}

/// Check if a clone pair passes the minimum AST nodes threshold.
fn passes_ast_node_threshold(value: &serde_json::Value, min_ast_nodes: usize) -> bool {
    if min_ast_nodes == 0 {
        return true;
    }

    let Some(node_counts) = value
        .get("verification")
        .and_then(|v| v.get("node_counts"))
    else {
        return true; // No verification data, allow through
    };

    let a = node_counts.get(0).and_then(|v| v.as_u64());
    let b = node_counts.get(1).and_then(|v| v.as_u64());

    match (a, b) {
        (Some(a), Some(b)) => std::cmp::min(a, b) >= min_ast_nodes as u64,
        _ => true, // Incomplete counts, allow through
    }
}

