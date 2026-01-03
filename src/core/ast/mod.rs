//! AST parsing, caching, and traversal services.
//!
//! This module provides:
//! - AST service for parsing and caching syntax trees
//! - AST utility functions for tree navigation
//! - Unified visitor for language-agnostic AST traversal

pub mod service;
pub mod utils;
pub mod visitor;

#[cfg(test)]
#[path = "visitor_tests.rs"]
mod visitor_tests;

// Re-export main types from service
pub use service::{AstContext, AstService, CachedTree, CacheStats, DecisionKind};

// Re-export utility functions
pub use utils::{count_control_blocks, count_named_nodes, find_entity_node, node_text};

// Re-export visitor types
pub use visitor::{AstVisitable, NodeMetadata, UnifiedVisitor, UnifiedVisitorStatistics};
