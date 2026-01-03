//! APTED tree-edit-distance verification for clone detection.
//!
//! This module provides AST-based verification of clone pairs using
//! tree-edit-distance algorithms to compute structural similarity.

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use tracing::warn;
use tree_edit_distance::{diff, Node as TedNode, Tree as TedTree};
use tree_sitter::Node as TsNode;

use crate::core::ast_service::CachedTree;
use crate::core::featureset::CodeEntity;

use super::clone_detection::CloneVerificationDetail;

/// Simple AST node for tree-edit-distance computation.
#[derive(Debug, Clone)]
pub struct SimpleAstNode {
    pub kind_hash: u64,
    pub kind_label: String,
    pub children: Vec<SimpleAstNode>,
    pub node_count: usize,
}

/// [`TedNode`] implementation for [`SimpleAstNode`].
impl TedNode for SimpleAstNode {
    type Kind = u64;

    /// Returns the node kind hash for tree edit distance comparison.
    fn kind(&self) -> Self::Kind {
        self.kind_hash
    }

    type Weight = u64;

    /// Returns the weight of this node (always 1).
    fn weight(&self) -> Self::Weight {
        1
    }
}

/// [`TedTree`] implementation for [`SimpleAstNode`].
impl TedTree for SimpleAstNode {
    type Children<'c>
        = std::slice::Iter<'c, SimpleAstNode>
    where
        Self: 'c;

    /// Returns an iterator over this node's children.
    fn children(&self) -> Self::Children<'_> {
        self.children.iter()
    }
}

/// Cached simple AST with metadata.
#[derive(Clone)]
pub struct CachedSimpleAst {
    pub ast: Arc<SimpleAstNode>,
    pub node_count: usize,
    pub truncated: bool,
}

/// Hash a node kind string to u64.
pub fn hash_kind(kind: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
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
    let source_ast =
        get_or_build_simple_ast(simple_ast_cache, source_entity, ast_cache, apted_max_nodes);
    let target_ast =
        get_or_build_simple_ast(simple_ast_cache, target_entity, ast_cache, apted_max_nodes);

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
