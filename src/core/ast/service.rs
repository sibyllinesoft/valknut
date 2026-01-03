//! Central AST service for unified parsing across all detectors
//!
//! This module provides a centralized interface for AST parsing and caching,
//! ensuring all detectors use proper tree-sitter analysis instead of text matching.

use crate::core::errors::{Result, ValknutError};
use crate::lang::common::{ParsedEntity, SourceLocation};
use crate::lang::registry::{detect_language_from_path, get_tree_sitter_language};
use dashmap::DashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use tree_sitter::{Language, Node, Parser, Tree};

/// Central AST service for unified parsing and caching
#[derive(Debug)]
pub struct AstService {
    /// Cached parsed trees by content hash for efficient cache hits
    tree_cache: DashMap<String, Arc<CachedTree>>,
}

/// Cached AST tree with metadata
#[derive(Debug)]
pub struct CachedTree {
    pub tree: Tree,
    pub source: String,
    pub language: String,
    pub last_modified: std::time::SystemTime,
    pub content_hash: u64,
}

/// AST analysis context for detectors
#[derive(Debug)]
pub struct AstContext<'a> {
    pub tree: &'a Tree,
    pub source: &'a str,
    pub language: &'a str,
    pub file_path: &'a str,
}

/// Result of AST-based complexity analysis
#[derive(Debug, Clone)]
pub struct ComplexityMetrics {
    pub cyclomatic_complexity: u32,
    pub cognitive_complexity: u32,
    pub nesting_depth: u32,
    pub decision_points: Vec<DecisionPoint>,
}

/// Decision point in control flow for complexity calculation
#[derive(Debug, Clone)]
pub struct DecisionPoint {
    pub kind: DecisionKind,
    pub location: SourceLocation,
    pub nesting_level: u32,
}

/// Types of decision points that contribute to complexity
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionKind {
    If,
    ElseIf,
    While,
    For,
    Match,
    Try,
    Catch,
    LogicalAnd,
    LogicalOr,
    ConditionalExpression,
}

/// Factory, caching, and analysis methods for [`AstService`].
impl AstService {
    /// Create a new AST service
    pub fn new() -> Self {
        Self {
            tree_cache: DashMap::new(),
        }
    }

    /// Calculate fast content hash for cache key
    fn calculate_content_hash(content: &str, language: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        language.hash(&mut hasher);
        hasher.finish()
    }

    /// Generate cache key from file path, content hash, and language
    fn generate_cache_key(file_path: &str, content_hash: u64, language: &str) -> String {
        format!("{}:{}:{}", file_path, content_hash, language)
    }

    /// Get or parse AST for a file using content-based caching
    pub async fn get_ast(&self, file_path: &str, source: &str) -> Result<Arc<CachedTree>> {
        let language = self.detect_language(file_path);
        let content_hash = Self::calculate_content_hash(source, &language);
        let cache_key = Self::generate_cache_key(file_path, content_hash, &language);

        // Check cache first using content-based key
        if let Some(cached) = self.tree_cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        // Parse new tree using spawn_blocking for CPU-bound work
        let language_clone = language.clone();
        let source_clone = source.to_string();
        let file_path_clone = file_path.to_string();

        let tree = tokio::task::spawn_blocking(move || -> Result<Tree> {
            let mut parser = Parser::new();
            let tree_sitter_language = get_tree_sitter_language(&language_clone)?;
            parser.set_language(&tree_sitter_language).map_err(|e| {
                ValknutError::parse(
                    &language_clone,
                    format!("Failed to set parser language: {}", e),
                )
            })?;

            parser
                .parse(&source_clone, None)
                .ok_or_else(|| ValknutError::parse(&language_clone, "Failed to parse source code"))
        })
        .await
        .map_err(|e| ValknutError::parse(&language, &format!("Task join error: {}", e)))??;

        let cached = Arc::new(CachedTree {
            tree,
            source: source.to_string(),
            language,
            last_modified: std::time::SystemTime::now(),
            content_hash,
        });

        self.tree_cache.insert(cache_key, cached.clone());

        // Clean up old cache entries if cache is getting large
        if self.tree_cache.len() > 1000 {
            self.cleanup_cache().await;
        }

        Ok(cached)
    }

    /// Clean up old cache entries to prevent unbounded growth
    async fn cleanup_cache(&self) {
        let cache_size = self.tree_cache.len();
        if cache_size > 800 {
            // Remove random entries to get back to reasonable size
            let keys_to_remove: Vec<_> = self
                .tree_cache
                .iter()
                .take(cache_size - 800)
                .map(|entry| entry.key().clone())
                .collect();

            for key in keys_to_remove {
                self.tree_cache.remove(&key);
            }
        }
    }

    /// Detect language from file path
    fn detect_language(&self, file_path: &str) -> String {
        detect_language_from_path(file_path)
    }

    /// Create AST context for analysis
    pub fn create_context<'a>(
        &self,
        cached_tree: &'a CachedTree,
        file_path: &'a str,
    ) -> AstContext<'a> {
        AstContext {
            tree: &cached_tree.tree,
            source: &cached_tree.source,
            language: &cached_tree.language,
            file_path,
        }
    }

    /// Calculate complexity metrics using AST analysis
    pub fn calculate_complexity(&self, context: &AstContext) -> Result<ComplexityMetrics> {
        let root_node = context.tree.root_node();
        let mut calculator = ComplexityCalculator::new(context);
        calculator.analyze_node(&root_node, 0)
    }

    /// Clear cache for a specific file
    pub fn invalidate_cache(&self, file_path: &str) {
        self.tree_cache.remove(file_path);
    }

    /// Clear entire cache
    pub fn clear_cache(&self) {
        self.tree_cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            cached_files: self.tree_cache.len(),
        }
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cached_files: usize,
}

/// Internal complexity calculator using AST traversal
struct ComplexityCalculator<'a> {
    context: &'a AstContext<'a>,
    decision_points: Vec<DecisionPoint>,
}

/// Traversal and complexity calculation methods for [`ComplexityCalculator`].
impl<'a> ComplexityCalculator<'a> {
    /// Creates a new complexity calculator for the given AST context.
    fn new(context: &'a AstContext<'a>) -> Self {
        Self {
            context,
            decision_points: Vec::new(),
        }
    }

    /// Analyze a node and its children for complexity
    fn analyze_node(&mut self, node: &Node, nesting_level: u32) -> Result<ComplexityMetrics> {
        self.traverse_node(node, nesting_level);

        // Calculate metrics from decision points
        let cyclomatic_complexity = self.calculate_cyclomatic_complexity();
        let cognitive_complexity = self.calculate_cognitive_complexity();
        let nesting_depth = self.calculate_max_nesting_depth();

        Ok(ComplexityMetrics {
            cyclomatic_complexity,
            cognitive_complexity,
            nesting_depth,
            decision_points: self.decision_points.clone(),
        })
    }

    /// Recursively traverse AST nodes
    fn traverse_node(&mut self, node: &Node, nesting_level: u32) {
        // Check if this node contributes to complexity
        if let Some(decision_kind) = self.classify_node(node) {
            let location = SourceLocation {
                file_path: self.context.file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                start_column: node.start_position().column + 1,
                end_column: node.end_position().column + 1,
            };

            self.decision_points.push(DecisionPoint {
                kind: decision_kind,
                location,
                nesting_level,
            });
        }

        // Determine nesting level for children
        let child_nesting = if self.increases_nesting(node) {
            nesting_level + 1
        } else {
            nesting_level
        };

        // Traverse children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.traverse_node(&child, child_nesting);
        }
    }

    /// Classify node as decision point
    fn classify_node(&self, node: &Node) -> Option<DecisionKind> {
        match node.kind() {
            "if_statement" => Some(DecisionKind::If),
            "else_if_clause" => Some(DecisionKind::ElseIf),
            "while_statement" | "while_expression" => Some(DecisionKind::While),
            "for_statement" | "for_expression" => Some(DecisionKind::For),
            "match_statement" | "match_expression" => Some(DecisionKind::Match),
            "try_statement" | "try_expression" => Some(DecisionKind::Try),
            "catch_clause" => Some(DecisionKind::Catch),
            "binary_expression" => {
                // Check for logical operators
                node.child_by_field_name("operator").and_then(|op| match op.kind() {
                    "&&" | "and" => Some(DecisionKind::LogicalAnd),
                    "||" | "or" => Some(DecisionKind::LogicalOr),
                    _ => None,
                })
            }
            "conditional_expression" | "ternary_expression" => {
                Some(DecisionKind::ConditionalExpression)
            }
            _ => None,
        }
    }

    /// Check if node increases nesting level
    fn increases_nesting(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "if_statement"
                | "while_statement"
                | "for_statement"
                | "match_statement"
                | "try_statement"
                | "function_definition"
                | "method_definition"
                | "block"
                | "compound_statement"
        )
    }

    /// Calculate cyclomatic complexity (M = E - N + 2P)
    /// Simplified: 1 + number of decision points
    fn calculate_cyclomatic_complexity(&self) -> u32 {
        1 + self.decision_points.len() as u32
    }

    /// Calculate cognitive complexity (weighted by nesting)
    fn calculate_cognitive_complexity(&self) -> u32 {
        self.decision_points
            .iter()
            .map(|dp| self.cognitive_weight(&dp.kind) + dp.nesting_level)
            .sum()
    }

    /// Get cognitive complexity weight for decision type
    fn cognitive_weight(&self, kind: &DecisionKind) -> u32 {
        match kind {
            DecisionKind::If | DecisionKind::ElseIf => 1,
            DecisionKind::While | DecisionKind::For => 1,
            DecisionKind::Match => 1,
            DecisionKind::Try | DecisionKind::Catch => 1,
            DecisionKind::LogicalAnd | DecisionKind::LogicalOr => 1,
            DecisionKind::ConditionalExpression => 1,
        }
    }

    /// Calculate maximum nesting depth
    fn calculate_max_nesting_depth(&self) -> u32 {
        self.decision_points
            .iter()
            .map(|dp| dp.nesting_level)
            .max()
            .unwrap_or(0)
    }
}

/// Default implementation for [`AstService`].
impl Default for AstService {
    /// Returns a new AST service with empty cache.
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ast_service_creation() {
        let service = AstService::new();
        let stats = service.cache_stats();
        assert_eq!(stats.cached_files, 0);
    }

    #[tokio::test]
    async fn test_python_complexity_calculation() {
        let service = AstService::new();
        let source = r#"
def complex_function(x):
    if x > 0:
        if x < 10:
            return x
        else:
            return 10
    elif x < 0:
        return 0
    else:
        return 1
"#;

        let cached_tree = service.get_ast("test.py", source).await.unwrap();
        let context = service.create_context(&cached_tree, "test.py");
        let metrics = service.calculate_complexity(&context).unwrap();

        // Should have multiple decision points
        assert!(metrics.cyclomatic_complexity > 1);
        assert!(metrics.decision_points.len() > 0);
    }

    #[test]
    fn test_language_detection() {
        let service = AstService::new();
        assert_eq!(service.detect_language("test.py"), "py");
        assert_eq!(service.detect_language("test.rs"), "rs");
        assert_eq!(service.detect_language("test.js"), "js");
        assert_eq!(service.detect_language("test.ts"), "ts");
        assert_eq!(service.detect_language("test.go"), "go");
    }

    #[test]
    fn test_cache_operations() {
        let service = AstService::new();
        service.invalidate_cache("test.py");
        service.clear_cache();

        let stats = service.cache_stats();
        assert_eq!(stats.cached_files, 0);
    }

    #[tokio::test]
    async fn test_javascript_complexity() {
        let service = AstService::new();
        let source = r#"
function complexFunction(x) {
    if (x > 0) {
        for (let i = 0; i < x; i++) {
            if (i % 2 === 0) {
                console.log(i);
            }
        }
        return x;
    } else {
        return 0;
    }
}
"#;

        let cached_tree = service.get_ast("test.js", source).await.unwrap();
        let context = service.create_context(&cached_tree, "test.js");
        let metrics = service.calculate_complexity(&context).unwrap();

        assert!(metrics.cyclomatic_complexity > 1);
        assert!(metrics.cognitive_complexity > 0);
        assert!(metrics.decision_points.len() >= 2); // if and for
    }

    #[tokio::test]
    async fn test_rust_complexity() {
        let service = AstService::new();
        let source = r#"
fn complex_function(x: i32) -> i32 {
    match x {
        0..=10 => {
            if x % 2 == 0 {
                x * 2
            } else {
                x + 1
            }
        }
        11..=20 => x - 5,
        _ => 0,
    }
}
"#;

        let cached_tree = service.get_ast("test.rs", source).await.unwrap();
        let context = service.create_context(&cached_tree, "test.rs");
        let metrics = service.calculate_complexity(&context).unwrap();

        assert!(metrics.cyclomatic_complexity > 1);
        assert!(metrics.decision_points.len() > 0);
    }

    #[tokio::test]
    async fn test_go_complexity() {
        let service = AstService::new();
        let source = r#"
func complexFunction(x int) int {
    if x > 0 {
        switch x {
        case 1, 2:
            return x * 2
        case 3, 4:
            return x + 1
        default:
            return x
        }
    }
    return 0
}
"#;

        let cached_tree = service.get_ast("test.go", source).await.unwrap();
        let context = service.create_context(&cached_tree, "test.go");
        let metrics = service.calculate_complexity(&context).unwrap();

        assert!(metrics.cyclomatic_complexity > 1);
        assert!(metrics.decision_points.len() > 0);
    }

    #[tokio::test]
    async fn test_typescript_complexity() {
        let service = AstService::new();
        let source = r#"
function complexFunction(x: number): number {
    if (x > 0) {
        while (x > 10) {
            x -= 5;
            if (x % 3 === 0) {
                break;
            }
        }
        return x;
    }
    return 0;
}
"#;

        let cached_tree = service.get_ast("test.ts", source).await.unwrap();
        let context = service.create_context(&cached_tree, "test.ts");
        let metrics = service.calculate_complexity(&context).unwrap();

        assert!(metrics.cyclomatic_complexity > 1);
        assert!(metrics.nesting_depth > 0);
    }

    #[tokio::test]
    async fn test_cache_reuse() {
        let service = AstService::new();
        let source = r#"
def simple_function():
    return True
"#;

        // First parse
        let cached_tree1 = service.get_ast("test.py", source).await.unwrap();
        let stats1 = service.cache_stats();
        assert_eq!(stats1.cached_files, 1);

        // Second parse should use cache
        let cached_tree2 = service.get_ast("test.py", source).await.unwrap();
        let stats2 = service.cache_stats();
        assert_eq!(stats2.cached_files, 1);

        // Both should be the same Arc
        assert!(Arc::ptr_eq(&cached_tree1, &cached_tree2));
    }

    #[test]
    fn test_unsupported_language() {
        use crate::lang::registry::get_tree_sitter_language;
        let result = get_tree_sitter_language("xyz");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_error_handling() {
        let service = AstService::new();
        let invalid_source = "invalid syntax !!!";

        // This should still parse (tree-sitter is very forgiving)
        // but we test that it doesn't panic
        let result = service.get_ast("test.py", invalid_source).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_complexity_with_deep_nesting() {
        let service = AstService::new();
        let source = r#"
def deeply_nested(x):
    if x > 0:
        if x < 100:
            for i in range(x):
                if i % 2 == 0:
                    if i % 4 == 0:
                        return i
    return 0
"#;

        let cached_tree = service.get_ast("test.py", source).await.unwrap();
        let context = service.create_context(&cached_tree, "test.py");
        let metrics = service.calculate_complexity(&context).unwrap();

        assert!(metrics.nesting_depth >= 4);
        assert!(metrics.cognitive_complexity > metrics.cyclomatic_complexity);
    }

    #[tokio::test]
    async fn test_empty_source() {
        let service = AstService::new();
        let empty_source = "";

        let cached_tree = service.get_ast("empty.py", empty_source).await.unwrap();
        let context = service.create_context(&cached_tree, "empty.py");
        let metrics = service.calculate_complexity(&context).unwrap();

        assert_eq!(metrics.cyclomatic_complexity, 1); // Base complexity
        assert_eq!(metrics.cognitive_complexity, 0);
        assert_eq!(metrics.nesting_depth, 0);
        assert_eq!(metrics.decision_points.len(), 0);
    }

    #[test]
    fn test_decision_kind_variants() {
        use super::DecisionKind;

        // Test all variants exist
        let kinds = vec![
            DecisionKind::If,
            DecisionKind::ElseIf,
            DecisionKind::While,
            DecisionKind::For,
            DecisionKind::Match,
            DecisionKind::Try,
            DecisionKind::Catch,
            DecisionKind::LogicalAnd,
            DecisionKind::LogicalOr,
            DecisionKind::ConditionalExpression,
        ];

        assert_eq!(kinds.len(), 10);

        // Test PartialEq
        assert_eq!(DecisionKind::If, DecisionKind::If);
        assert_ne!(DecisionKind::If, DecisionKind::While);
    }

    #[test]
    fn test_decision_point_creation() {
        use super::{DecisionKind, DecisionPoint, SourceLocation};

        let location = SourceLocation {
            file_path: "test.py".to_string(),
            start_line: 1,
            end_line: 1,
            start_column: 1,
            end_column: 5,
        };

        let decision_point = DecisionPoint {
            kind: DecisionKind::If,
            location: location.clone(),
            nesting_level: 2,
        };

        assert_eq!(decision_point.kind, DecisionKind::If);
        assert_eq!(decision_point.nesting_level, 2);
        assert_eq!(decision_point.location.file_path, "test.py");
    }

    #[test]
    fn test_complexity_metrics_creation() {
        use super::{ComplexityMetrics, DecisionKind, DecisionPoint};
        use crate::lang::common::SourceLocation;

        let location = SourceLocation {
            file_path: "test.py".to_string(),
            start_line: 1,
            end_line: 1,
            start_column: 1,
            end_column: 5,
        };

        let decision_point = DecisionPoint {
            kind: DecisionKind::If,
            location,
            nesting_level: 1,
        };

        let metrics = ComplexityMetrics {
            cyclomatic_complexity: 3,
            cognitive_complexity: 5,
            nesting_depth: 2,
            decision_points: vec![decision_point],
        };

        assert_eq!(metrics.cyclomatic_complexity, 3);
        assert_eq!(metrics.cognitive_complexity, 5);
        assert_eq!(metrics.nesting_depth, 2);
        assert_eq!(metrics.decision_points.len(), 1);
    }

    #[test]
    fn test_cache_stats() {
        use super::CacheStats;

        let stats = CacheStats { cached_files: 5 };

        assert_eq!(stats.cached_files, 5);
    }
}
