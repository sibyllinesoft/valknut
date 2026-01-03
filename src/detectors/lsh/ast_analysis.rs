//! AST analysis and stop motif detection for LSH clone filtering.
//!
//! This module provides functionality for analyzing AST structure and detecting
//! common code patterns (stop motifs) that should be excluded from clone detection.

use std::sync::Arc;

use tokio::fs;
use tracing::debug;
use tree_sitter::Node;

use crate::core::ast_service::AstService;
use crate::core::ast_utils::{count_control_blocks, count_named_nodes, find_entity_node, node_text};
use crate::core::errors::Result;
use crate::core::featureset::CodeEntity;
use crate::lang::common::{EntityKind, ParseIndex};

use super::config::DedupeConfig;
use super::signatures::shingles::count_tokens;

/// AST statistics for an entity used in fragment threshold checks.
#[derive(Debug, Clone)]
pub struct EntityAstStats {
    /// Number of named AST nodes
    pub node_count: usize,
    /// Number of control flow blocks
    pub block_count: usize,
    /// Whether the entity contains a stop motif
    pub has_stop_motif: bool,
}

/// AST analyzer for computing entity statistics and detecting stop motifs.
#[derive(Debug)]
pub struct AstAnalyzer {
    /// Shared AST service for structural analysis
    ast_service: Arc<AstService>,
}

/// Factory and AST analysis methods for [`AstAnalyzer`].
impl AstAnalyzer {
    /// Create a new AST analyzer with a shared AST service.
    pub fn new(ast_service: Arc<AstService>) -> Self {
        Self { ast_service }
    }

    /// Compute AST statistics for an entity.
    pub async fn compute_entity_ast_stats(
        &self,
        entity: &CodeEntity,
    ) -> Result<Option<EntityAstStats>> {
        let mut cache_key = entity.file_path.clone();
        let source = match fs::read_to_string(&entity.file_path).await {
            Ok(content) => content,
            Err(err) => {
                debug!(
                    "Falling back to entity source for AST metrics ({}): {}",
                    entity.file_path, err
                );
                if entity.source_code.is_empty() {
                    return Ok(None);
                }
                cache_key = format!("{}::fragment:{}", entity.file_path, entity.id);
                entity.source_code.clone()
            }
        };

        let cached_tree = self.ast_service.get_ast(&cache_key, &source).await?;
        let context = self
            .ast_service
            .create_context(&cached_tree, &entity.file_path);

        let Some(entity_node) = find_entity_node(&context, entity) else {
            return Ok(None);
        };

        let node_count = count_named_nodes(&entity_node);
        let block_count = count_control_blocks(&entity_node);
        let has_stop_motif = self.detect_ast_stop_motifs(&context, entity_node);

        Ok(Some(EntityAstStats {
            node_count,
            block_count,
            has_stop_motif,
        }))
    }

    /// Detect if an AST subtree contains any stop motifs.
    pub fn detect_ast_stop_motifs(
        &self,
        context: &crate::core::ast_service::AstContext<'_>,
        root: Node<'_>,
    ) -> bool {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if self.node_matches_stop_motif(context, node) {
                return true;
            }

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }

        false
    }

    /// Check if a node matches any known stop motif pattern.
    pub fn node_matches_stop_motif(
        &self,
        context: &crate::core::ast_service::AstContext<'_>,
        node: Node<'_>,
    ) -> bool {
        let text = node_text(node, context.source)
            .unwrap_or_default()
            .to_lowercase();
        let kind = node.kind();

        match context.language {
            "py" | "pyw" => match kind {
                "import_statement" | "import_from_statement" => {
                    matches_any(&text, &["import os", "import sys", "from typing"])
                }
                "if_statement" => matches_all(&text, &["__name__", "__main__"]),
                "function_definition" => text.contains("__init__"),
                _ => false,
            },
            "js" | "jsx" => match kind {
                "call_expression" => matches_any(&text, &["console.log", "require("]),
                "assignment_expression" => text.contains("module.exports"),
                _ => false,
            },
            "ts" | "tsx" => match kind {
                "call_expression" => text.contains("console.log"),
                "import_statement" => text.contains("from \"@angular/core\""),
                _ => false,
            },
            "rs" => match kind {
                "macro_invocation" | "macro_invocation_body" => {
                    matches_any(&text, &["println!", "dbg!", "todo!"])
                }
                _ => false,
            },
            "go" => kind == "call_expression" && text.contains("fmt.println"),
            _ => false,
        }
    }

    /// Check if entity meets fragment analysis thresholds using structural data.
    pub async fn meets_fragment_thresholds(
        &self,
        entity: &CodeEntity,
        config: &DedupeConfig,
    ) -> Result<bool> {
        let source_code = &entity.source_code;

        let token_count = count_tokens(source_code);
        if token_count < config.min_function_tokens {
            return Ok(false);
        }

        let Some(stats) = self.compute_entity_ast_stats(entity).await? else {
            return Ok(false);
        };

        if stats.node_count < config.min_ast_nodes {
            return Ok(false);
        }

        if stats.block_count < config.require_distinct_blocks {
            return Ok(false);
        }

        if stats.has_stop_motif {
            return Ok(false);
        }

        Ok(true)
    }
}

/// Check if text matches any pattern in the list.
fn matches_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}

/// Check if text matches all patterns in the list.
fn matches_all(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().all(|p| text.contains(p))
}

/// Count AST nodes from language adapter index (heuristic).
pub fn count_ast_nodes_from_index(index: &ParseIndex) -> usize {
    index.entities.len() * 10 // Simple heuristic - each entity has ~10 nodes
}

/// Count distinct code blocks from language adapter index.
pub fn count_distinct_blocks_from_index(index: &ParseIndex) -> usize {
    let mut block_count = 0;

    for (_id, entity) in &index.entities {
        match entity.kind {
            EntityKind::Function | EntityKind::Method => block_count += 1,
            EntityKind::Class | EntityKind::Struct | EntityKind::Enum => block_count += 1,
            EntityKind::Interface => block_count += 1,
            EntityKind::Module => block_count += 1,
            // Control structures are typically not stored as entities in the index
            // They would be counted by examining the AST structure more deeply
            _ => {}
        }
    }

    // Add heuristic for control structures based on function count
    // Functions typically contain control structures, so estimate based on that
    let function_count = index
        .entities
        .iter()
        .filter(|(_id, entity)| matches!(entity.kind, EntityKind::Function | EntityKind::Method))
        .count();

    block_count += function_count * 2; // Heuristic: each function has ~2 control structures

    block_count.max(1) // At least 1 block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_any() {
        assert!(matches_any("import os", &["import os", "import sys"]));
        assert!(!matches_any("import json", &["import os", "import sys"]));
    }

    #[test]
    fn test_matches_all() {
        assert!(matches_all("if __name__ == '__main__':", &["__name__", "__main__"]));
        assert!(!matches_all("if __name__:", &["__name__", "__main__"]));
    }
}
