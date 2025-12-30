//! Clone detection types and helpers for LSH analysis.

use serde::Serialize;
use std::collections::hash_map::{DefaultHasher, Entry};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tracing::{debug, warn};
use tree_edit_distance::{diff, Node as TedNode, Tree as TedTree};
use tree_sitter::Node as TsNode;

use crate::core::ast_service::CachedTree;
use crate::core::featureset::CodeEntity;

use super::pipeline_results::CloneVerificationResults;

/// Endpoint of a clone pair
#[derive(Debug, Clone, Serialize)]
pub struct CloneEndpoint {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<(usize, usize)>,
}

impl CloneEndpoint {
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

/// Simple AST node for tree-edit-distance computation
#[derive(Debug, Clone)]
pub struct SimpleAstNode {
    pub kind_hash: u64,
    pub kind_label: String,
    pub children: Vec<SimpleAstNode>,
    pub node_count: usize,
}

impl TedNode for SimpleAstNode {
    type Kind = u64;

    fn kind(&self) -> Self::Kind {
        self.kind_hash
    }

    type Weight = u64;

    fn weight(&self) -> Self::Weight {
        1
    }
}

impl TedTree for SimpleAstNode {
    type Children<'c>
        = std::slice::Iter<'c, SimpleAstNode>
    where
        Self: 'c;

    fn children(&self) -> Self::Children<'_> {
        self.children.iter()
    }
}

/// Cached simple AST with metadata
#[derive(Clone)]
pub struct CachedSimpleAst {
    pub ast: Arc<SimpleAstNode>,
    pub node_count: usize,
    pub truncated: bool,
}

/// Results from collecting entities for LSH analysis.
pub struct LshEntityCollection {
    pub entities: Vec<CodeEntity>,
    pub entity_index: HashMap<String, CodeEntity>,
    pub ast_cache: HashMap<String, Arc<CachedTree>>,
}

impl LshEntityCollection {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            entity_index: HashMap::new(),
            ast_cache: HashMap::new(),
        }
    }
}

impl Default for LshEntityCollection {
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

impl CloneDetectionStats {
    pub fn record_similarity(&mut self, similarity: f64) {
        self.max_similarity = self.max_similarity.max(similarity);
        self.similarity_total += similarity;
        self.similarity_count += 1;
    }

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

    pub fn avg_similarity(&self) -> f64 {
        if self.similarity_count > 0 {
            self.similarity_total / self.similarity_count as f64
        } else {
            0.0
        }
    }

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
        match serde_json::to_value(&pair) {
            Ok(value) => {
                // Filter pairs below min_ast_nodes threshold
                if min_ast_nodes > 0 {
                    if let Some(ver) = value.get("verification") {
                        if let Some(counts) = ver.get("node_counts") {
                            if let (Some(a), Some(b)) = (
                                counts.get(0).and_then(|v| v.as_u64()),
                                counts.get(1).and_then(|v| v.as_u64()),
                            ) {
                                if std::cmp::min(a, b) < min_ast_nodes as u64 {
                                    continue;
                                }
                            }
                        }
                    }
                }
                serialized.push(value);
            }
            Err(e) => {
                warn!(
                    "Failed to serialize clone pair {} -> {}: {}",
                    pair.source.id, pair.target.id, e
                );
            }
        }
    }

    serialized
}

/// Hash a node kind string to u64.
pub fn hash_kind(kind: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    kind.hash(&mut hasher);
    hasher.finish()
}

/// Parse byte range from entity properties.
pub fn parse_byte_range(entity: &CodeEntity) -> Option<(usize, usize)> {
    let range = entity.properties.get("byte_range")?.as_array()?;
    if range.len() != 2 {
        return None;
    }
    let start = range[0].as_u64()? as usize;
    let end = range[1].as_u64()? as usize;
    Some((start, end))
}

/// Build a simple AST recursively from a tree-sitter node.
pub fn build_simple_ast_recursive(
    node: TsNode,
    max_nodes: usize,
    counter: &mut usize,
) -> (SimpleAstNode, bool) {
    *counter += 1;
    let kind_label = node.kind().to_string();
    let kind_hash = hash_kind(&kind_label);
    let mut simple = SimpleAstNode {
        kind_hash,
        kind_label,
        children: Vec::new(),
        node_count: 1,
    };

    if *counter >= max_nodes {
        return (simple, node.named_child_count() > 0);
    }

    let mut truncated = false;
    let child_count = node.named_child_count();
    for i in 0..child_count {
        if *counter >= max_nodes {
            truncated = true;
            break;
        }
        if let Some(child) = node.named_child(i) {
            let (child_ast, child_truncated) = build_simple_ast_recursive(child, max_nodes, counter);
            simple.node_count += child_ast.node_count;
            simple.children.push(child_ast);
            if child_truncated {
                truncated = true;
            }
        }
    }

    (simple, truncated)
}

/// Build a simple AST for an entity.
pub fn build_simple_ast_for_entity(
    entity: &CodeEntity,
    ast_cache: &HashMap<String, Arc<CachedTree>>,
    max_nodes: usize,
) -> Option<CachedSimpleAst> {
    let (start_byte, end_byte) = parse_byte_range(entity)?;
    let cached_tree = ast_cache.get(&entity.file_path)?;
    let root = cached_tree.tree.root_node();
    let target_node = root
        .descendant_for_byte_range(start_byte, end_byte)
        .or_else(|| root.named_descendant_for_byte_range(start_byte, end_byte))
        .unwrap_or(root);

    let mut counter = 0usize;
    let (simple_ast, truncated) = build_simple_ast_recursive(target_node, max_nodes, &mut counter);

    Some(CachedSimpleAst {
        node_count: simple_ast.node_count,
        truncated,
        ast: Arc::new(simple_ast),
    })
}

/// Get or build a simple AST for an entity, using a cache.
pub fn get_or_build_simple_ast(
    cache: &mut HashMap<String, Option<CachedSimpleAst>>,
    entity: &CodeEntity,
    ast_cache: &HashMap<String, Arc<CachedTree>>,
    max_nodes: usize,
) -> Option<CachedSimpleAst> {
    match cache.entry(entity.id.clone()) {
        Entry::Occupied(entry) => entry.get().clone(),
        Entry::Vacant(entry) => {
            let value = build_simple_ast_for_entity(entity, ast_cache, max_nodes);
            entry.insert(value).clone()
        }
    }
}

/// Compute APTED tree-edit-distance verification for a clone pair.
///
/// Returns `Some(CloneVerificationDetail)` if verification was attempted,
/// `None` if verification was not allowed (limit reached).
pub async fn compute_apted_verification(
    source_entity: &CodeEntity,
    target_entity: &CodeEntity,
    simple_ast_cache: &mut HashMap<String, Option<CachedSimpleAst>>,
    ast_cache: &HashMap<String, Arc<CachedTree>>,
    apted_max_nodes: usize,
) -> Option<CloneVerificationDetail> {
    let source_ast = get_or_build_simple_ast(simple_ast_cache, source_entity, ast_cache, apted_max_nodes);
    let target_ast = get_or_build_simple_ast(simple_ast_cache, target_entity, ast_cache, apted_max_nodes);

    let (source_ast, target_ast) = match (source_ast, target_ast) {
        (Some(s), Some(t)) => (s, t),
        _ => {
            return Some(CloneVerificationDetail {
                similarity: None,
                edit_cost: None,
                node_counts: None,
                truncated: false,
            });
        }
    };

    let nodes_total = (source_ast.node_count + target_ast.node_count).max(1);
    let truncated = source_ast.truncated || target_ast.truncated;
    let tree_a = Arc::clone(&source_ast.ast);
    let tree_b = Arc::clone(&target_ast.ast);
    let node_counts = Some((source_ast.node_count, target_ast.node_count));

    match tokio::task::spawn_blocking(move || {
        let (_, cost) = diff(&*tree_a, &*tree_b);
        cost
    })
    .await
    {
        Ok(cost) => {
            let normalized = (1.0 - (cost as f64 / nodes_total as f64)).clamp(0.0, 1.0);
            Some(CloneVerificationDetail {
                similarity: Some(normalized),
                edit_cost: Some(cost),
                node_counts,
                truncated,
            })
        }
        Err(e) => {
            warn!(
                "APTED computation failed for {} -> {}: {}",
                source_entity.id, target_entity.id, e,
            );
            Some(CloneVerificationDetail {
                similarity: None,
                edit_cost: None,
                node_counts,
                truncated: true,
            })
        }
    }
}
