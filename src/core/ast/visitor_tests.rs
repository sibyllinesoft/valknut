use super::*;
use crate::core::config::ValknutConfig;
use crate::core::featureset::{CodeEntity, ExtractionContext};
use crate::core::interning::intern;
use crate::{Result, ValknutError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::tempdir;
use tree_sitter::Node;

// Mock detector for testing
struct MockDetector {
    name: String,
    feature_count: usize,
}

impl MockDetector {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            feature_count: 0,
        }
    }
}

#[async_trait]
impl AstVisitable for MockDetector {
    fn detector_name(&self) -> &str {
        &self.name
    }

    async fn visit_node(
        &mut self,
        node: Node<'_>,
        _ast_context: &AstContext<'_>,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();

        // Generate a feature for function definitions
        if node.kind() == "function_definition" {
            self.feature_count += 1;
            features.insert(
                format!("{}_function_count", self.name),
                self.feature_count as f64,
            );
        }

        Ok(features)
    }

    fn feature_names(&self) -> Vec<&str> {
        vec!["function_count"]
    }
}

#[tokio::test]
async fn test_unified_visitor_basic() {
    let mut visitor = UnifiedVisitor::new();

    // Add mock detectors
    visitor.add_detector(Box::new(MockDetector::new("detector1")));
    visitor.add_detector(Box::new(MockDetector::new("detector2")));

    assert_eq!(visitor.detector_count(), 2);

    // Create test entity
    let entity = CodeEntity::new("test", "function", "test_func", "/test/file.py")
        .with_source_code("def test_func():\n    return 1");

    let config = Arc::new(ValknutConfig::default());
    let context = ExtractionContext::new(config, "python");

    let features = visitor.visit_entity(&entity, &context).await.unwrap();

    // Should have visited some nodes
    let stats = visitor.get_statistics();
    assert!(stats.nodes_visited > 0);
    assert_eq!(stats.detectors_count, 2);
    assert_eq!(stats.theoretical_speedup, 2);
}

#[test]
fn test_node_metadata() {
    // This test would need a proper tree-sitter setup
    // For now, just test the metadata structure
    let metadata = NodeMetadata {
        kind: intern("function_definition"),
        depth: 1,
        child_count: 3,
        is_decision_point: false,
        is_function_definition: true,
        is_class_definition: false,
    };

    assert_eq!(metadata.kind_str(), "function_definition");
    assert!(metadata.is_function_definition);
    assert!(!metadata.is_decision_point);
}

#[tokio::test]
async fn visit_entity_returns_empty_without_detectors() {
    let mut visitor = UnifiedVisitor::new();
    let entity = CodeEntity::new("id", "module", "sample", "nonexistent.py")
        .with_source_code("print('hello')");
    let context = ExtractionContext::new(Arc::new(ValknutConfig::default()), "python");

    let features = visitor.visit_entity(&entity, &context).await.unwrap();
    assert!(features.is_empty());
    let stats = visitor.get_statistics();
    assert_eq!(stats.nodes_visited, 0);
    assert_eq!(stats.detectors_count, 0);
}

#[tokio::test]
async fn visit_entity_reads_existing_file() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("existing.py");
    let source = "def foo():\n    return 42\n";
    std::fs::write(&file_path, source).unwrap();

    let mut visitor = UnifiedVisitor::new();
    visitor.add_detector(Box::new(MockDetector::new("detector")));

    let entity = CodeEntity::new("foo", "function", "foo", file_path.to_string_lossy())
        .with_source_code(String::new());
    let context = ExtractionContext::new(Arc::new(ValknutConfig::default()), "python");

    let features = visitor.visit_entity(&entity, &context).await.unwrap();
    assert!(!features.is_empty(), "expected detector to record feature");

    let stats = visitor.get_statistics();
    assert!(stats.nodes_visited > 0);
    assert_eq!(stats.detectors_count, 1);
}

#[tokio::test]
async fn node_metadata_classifies_constructs() {
    fn find_first<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
        if node.kind() == kind {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_first(child, kind) {
                return Some(found);
            }
        }
        None
    }

    let temp = tempdir().unwrap();
    let file_path = temp.path().join("sample.py");
    let source = "class Foo:\n    pass\n\ndef foo():\n    if cond:\n        return 1\n";
    std::fs::write(&file_path, source).unwrap();

    let ast_service = AstService::new();
    let cached = ast_service
        .get_ast(file_path.to_str().unwrap(), source)
        .await
        .expect("parse source");
    let root = cached.tree.root_node();

    let class_node = find_first(root, "class_definition").expect("class node");
    let class_meta = NodeMetadata::new(class_node, 0);
    assert!(class_meta.is_class_definition);
    assert_eq!(class_meta.kind_str(), "class_definition");

    let func_node = find_first(root, "function_definition").expect("function node");
    let func_meta = NodeMetadata::new(func_node, 1);
    assert!(func_meta.is_function_definition);
    assert!(!func_meta.is_decision_point);

    let if_node = find_first(root, "if_statement").expect("if node");
    let if_meta = NodeMetadata::new(if_node, 2);
    assert!(if_meta.is_decision_point);
    assert_eq!(if_meta.depth, 2);
}

#[test]
fn clear_detectors_resets_state() {
    let mut visitor = UnifiedVisitor::new();
    visitor.add_detector(Box::new(MockDetector::new("first")));
    visitor.add_detector(Box::new(MockDetector::new("second")));
    assert_eq!(visitor.detector_count(), 2);

    visitor.clear_detectors();
    assert_eq!(visitor.detector_count(), 0);
    let stats = visitor.get_statistics();
    assert_eq!(stats.detectors_count, 0);
}

struct ConflictDetector {
    name: String,
    value: f64,
    emitted: bool,
}

impl ConflictDetector {
    fn new(name: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            value,
            emitted: false,
        }
    }
}

#[async_trait]
impl AstVisitable for ConflictDetector {
    fn detector_name(&self) -> &str {
        &self.name
    }

    async fn visit_node(
        &mut self,
        _node: Node<'_>,
        _ast_context: &AstContext<'_>,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        if !self.emitted {
            self.emitted = true;
            features.insert("shared_feature".to_string(), self.value);
        }
        Ok(features)
    }

    fn feature_names(&self) -> Vec<&str> {
        vec!["shared_feature"]
    }
}

#[tokio::test]
async fn visit_entity_uses_max_value_for_conflicting_features() {
    let mut visitor = UnifiedVisitor::new();
    visitor.add_detector(Box::new(ConflictDetector::new("low", 1.0)));
    visitor.add_detector(Box::new(ConflictDetector::new("high", 5.0)));

    let entity = CodeEntity::new("conflict", "function", "conflict_fn", "/virtual/file.rs")
        .with_source_code("pub fn conflict_fn() { 42 }");
    let context = ExtractionContext::new(Arc::new(ValknutConfig::default()), "rust");

    let features = visitor
        .visit_entity(&entity, &context)
        .await
        .expect("conflict traversal should succeed");

    assert_eq!(
        features.get("shared_feature"),
        Some(&5.0),
        "expected max value to be retained"
    );
    assert!(visitor.get_statistics().nodes_visited > 0);
}

struct FailingDetector;

#[async_trait]
impl AstVisitable for FailingDetector {
    fn detector_name(&self) -> &str {
        "failing"
    }

    async fn begin_entity(
        &mut self,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<()> {
        Err(ValknutError::validation(
            "detector initialization failed".to_string(),
        ))
    }

    async fn visit_node(
        &mut self,
        _node: Node<'_>,
        _ast_context: &AstContext<'_>,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        Ok(HashMap::new())
    }

    fn feature_names(&self) -> Vec<&str> {
        vec![]
    }
}

#[tokio::test]
async fn visit_entity_propagates_begin_entity_errors() {
    let mut visitor = UnifiedVisitor::new();
    visitor.add_detector(Box::new(FailingDetector));

    let entity = CodeEntity::new("fail", "function", "fail_fn", "/missing.rs")
        .with_source_code("pub fn fail_fn() {}");
    let context = ExtractionContext::new(Arc::new(ValknutConfig::default()), "rust");

    let err = visitor
        .visit_entity(&entity, &context)
        .await
        .expect_err("begin_entity error should bubble up");
    assert!(
        err.to_string().contains("detector initialization failed"),
        "unexpected error: {err}"
    );
    assert_eq!(visitor.get_statistics().nodes_visited, 0);
}

#[tokio::test]
async fn visit_entity_uses_entity_source_when_file_missing() {
    let mut visitor = UnifiedVisitor::new();
    visitor.add_detector(Box::new(MockDetector::new("fallback")));

    let entity = CodeEntity::new("fallback", "function", "fallback_fn", "/no/file.py")
        .with_source_code("def fallback_fn():\n    return 1\n");
    let context = ExtractionContext::new(Arc::new(ValknutConfig::default()), "python");

    let features = visitor
        .visit_entity(&entity, &context)
        .await
        .expect("source fallback should succeed");
    assert!(
        features
            .keys()
            .any(|key| key.contains("fallback_function_count")),
        "expected detector output using in-memory source"
    );
    assert!(visitor.get_statistics().nodes_visited > 0);
}

struct EndFeatureDetector {
    name: String,
    finalize_value: f64,
}

impl EndFeatureDetector {
    fn new(name: &str, finalize_value: f64) -> Self {
        Self {
            name: name.to_string(),
            finalize_value,
        }
    }
}

#[async_trait]
impl AstVisitable for EndFeatureDetector {
    fn detector_name(&self) -> &str {
        &self.name
    }

    async fn visit_node(
        &mut self,
        _node: Node<'_>,
        _ast_context: &AstContext<'_>,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        Ok(HashMap::new())
    }

    async fn end_entity(
        &mut self,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        features.insert(format!("{}_final_score", self.name), self.finalize_value);
        Ok(features)
    }

    fn feature_names(&self) -> Vec<&str> {
        vec!["final_score"]
    }
}

#[tokio::test]
async fn visit_entity_merges_end_entity_features() {
    let mut visitor = UnifiedVisitor::new();
    visitor.add_detector(Box::new(EndFeatureDetector::new("end", 7.5)));

    let entity = CodeEntity::new("end", "function", "end_fn", "/virtual/end.rs")
        .with_source_code("pub fn end_fn() -> i32 { 1 }");
    let context = ExtractionContext::new(Arc::new(ValknutConfig::default()), "rust");

    let features = visitor
        .visit_entity(&entity, &context)
        .await
        .expect("end-entity features should merge");

    assert_eq!(
        features.get("end_final_score"),
        Some(&7.5),
        "finalization features should be included in combined map"
    );
}

struct EndFailDetector;

#[async_trait]
impl AstVisitable for EndFailDetector {
    fn detector_name(&self) -> &str {
        "end_fail"
    }

    async fn visit_node(
        &mut self,
        _node: Node<'_>,
        _ast_context: &AstContext<'_>,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        Ok(HashMap::new())
    }

    async fn end_entity(
        &mut self,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        Err(ValknutError::internal(
            "detector finalization failed".to_string(),
        ))
    }

    fn feature_names(&self) -> Vec<&str> {
        vec![]
    }
}

#[tokio::test]
async fn visit_entity_propagates_end_entity_errors() {
    let mut visitor = UnifiedVisitor::new();
    visitor.add_detector(Box::new(EndFailDetector));

    let entity = CodeEntity::new("fail_end", "function", "fail_end_fn", "/virtual/f.rs")
        .with_source_code("pub fn fail_end_fn() {}");
    let context = ExtractionContext::new(Arc::new(ValknutConfig::default()), "rust");

    let err = visitor
        .visit_entity(&entity, &context)
        .await
        .expect_err("end_entity failure should bubble up");
    assert!(
        err.to_string().contains("detector finalization failed"),
        "unexpected error: {err}"
    );
}
