//! Graph analysis features - centrality, cycles, fan-in/fan-out.
//!
//! This module provides graph-based feature extraction for analyzing code dependencies,
//! call graphs, and structural relationships between code entities.

use std::collections::HashMap;

use async_trait::async_trait;
use petgraph::{Graph, Directed};
use petgraph::graph::NodeIndex;
use petgraph::algo::kosaraju_scc;
use rayon::prelude::*;
use dashmap::DashMap;
use arc_swap::ArcSwap;

use crate::core::featureset::{FeatureExtractor, FeatureDefinition, CodeEntity, ExtractionContext};
use crate::core::errors::Result;

/// Graph-based feature extractor
#[derive(Debug)]
pub struct GraphExtractor {
    /// Feature definitions for this extractor
    features: Vec<FeatureDefinition>,
}

impl GraphExtractor {
    /// Create a new graph extractor
    pub fn new() -> Self {
        let mut extractor = Self {
            features: Vec::new(),
        };
        
        extractor.initialize_features();
        extractor
    }
    
    /// Initialize graph-based feature definitions
    fn initialize_features(&mut self) {
        self.features = vec![
            FeatureDefinition::new(
                "betweenness_approx",
                "Approximate betweenness centrality"
            )
            .with_range(0.0, 1.0)
            .with_default(0.0),
            
            FeatureDefinition::new(
                "fan_in", 
                "Number of incoming dependencies"
            )
            .with_range(0.0, 100.0)
            .with_default(0.0),
            
            FeatureDefinition::new(
                "fan_out",
                "Number of outgoing dependencies" 
            )
            .with_range(0.0, 100.0)
            .with_default(0.0),
            
            FeatureDefinition::new(
                "in_cycle",
                "Whether entity is part of a dependency cycle"
            )
            .with_range(0.0, 1.0)
            .with_default(0.0),
            
            FeatureDefinition::new(
                "closeness_centrality",
                "Closeness centrality in dependency graph"
            )
            .with_range(0.0, 1.0)
            .with_default(0.0),
        ];
    }
}

impl Default for GraphExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FeatureExtractor for GraphExtractor {
    fn name(&self) -> &str {
        "graph"
    }
    
    fn features(&self) -> &[FeatureDefinition] {
        &self.features
    }
    
    async fn extract(
        &self, 
        entity: &CodeEntity, 
        context: &ExtractionContext
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // TODO: Implement actual graph analysis
        // For now, return placeholder values based on entity properties
        
        // Fan-in: count of entities that depend on this one
        let fan_in = self.calculate_fan_in(entity, context);
        features.insert("fan_in".to_string(), fan_in);
        
        // Fan-out: count of entities this one depends on
        let fan_out = self.calculate_fan_out(entity, context);
        features.insert("fan_out".to_string(), fan_out);
        
        // Betweenness centrality (approximated)
        let betweenness = self.calculate_betweenness_approx(entity, context);
        features.insert("betweenness_approx".to_string(), betweenness);
        
        // Cycle detection
        let in_cycle = self.detect_cycles(entity, context);
        features.insert("in_cycle".to_string(), if in_cycle { 1.0 } else { 0.0 });
        
        // Closeness centrality
        let closeness = self.calculate_closeness_centrality(entity, context);
        features.insert("closeness_centrality".to_string(), closeness);
        
        Ok(features)
    }
    
    fn supports_entity(&self, entity: &CodeEntity) -> bool {
        // Support functions, classes, and modules
        matches!(
            entity.entity_type.as_str(),
            "function" | "method" | "class" | "module" | "interface"
        )
    }
}

impl GraphExtractor {
    /// Calculate fan-in (incoming dependencies)
    fn calculate_fan_in(&self, entity: &CodeEntity, _context: &ExtractionContext) -> f64 {
        // TODO: Implement actual dependency analysis
        // For now, return a placeholder based on entity name length (just for testing)
        (entity.name.len() % 10) as f64
    }
    
    /// Calculate fan-out (outgoing dependencies)
    fn calculate_fan_out(&self, entity: &CodeEntity, _context: &ExtractionContext) -> f64 {
        // TODO: Implement actual dependency analysis
        // Placeholder: count imports or function calls in source code
        let import_count = entity.source_code
            .lines()
            .filter(|line| line.trim().starts_with("import") || line.trim().starts_with("from"))
            .count();
        
        import_count as f64
    }
    
    /// Calculate approximate betweenness centrality
    fn calculate_betweenness_approx(&self, entity: &CodeEntity, context: &ExtractionContext) -> f64 {
        // TODO: Implement actual betweenness centrality calculation
        // This would require building a full dependency graph and computing shortest paths
        
        // Placeholder: simple heuristic based on fan-in and fan-out
        let fan_in = self.calculate_fan_in(entity, context);
        let fan_out = self.calculate_fan_out(entity, context);
        
        let centrality_score = (fan_in * fan_out) / (fan_in + fan_out + 1.0);
        centrality_score / 10.0  // Normalize to [0, 1] range approximately
    }
    
    /// Detect if entity is part of a dependency cycle
    fn detect_cycles(&self, _entity: &CodeEntity, _context: &ExtractionContext) -> bool {
        // TODO: Implement actual cycle detection using graph algorithms
        // This would require building the full dependency graph and checking for cycles
        
        // Placeholder: return false for now
        false
    }
    
    /// Calculate closeness centrality
    fn calculate_closeness_centrality(&self, entity: &CodeEntity, context: &ExtractionContext) -> f64 {
        // TODO: Implement actual closeness centrality calculation
        // This requires computing shortest paths from this node to all other nodes
        
        // Placeholder: simple inverse of average distance heuristic
        let fan_in = self.calculate_fan_in(entity, context);
        let fan_out = self.calculate_fan_out(entity, context);
        
        if fan_in + fan_out == 0.0 {
            0.0
        } else {
            1.0 / (1.0 + (fan_in + fan_out) / 2.0)
        }
    }
}

/// Dependency graph representation for analysis
#[derive(Debug)]
pub struct DependencyGraph {
    /// The underlying petgraph structure
    graph: Graph<String, f64, Directed>,
    
    /// Mapping from entity IDs to node indices
    entity_to_node: HashMap<String, NodeIndex>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            entity_to_node: HashMap::new(),
        }
    }
    
    /// Add an entity to the graph
    pub fn add_entity(&mut self, entity_id: String) -> NodeIndex {
        if let Some(&node_index) = self.entity_to_node.get(&entity_id) {
            return node_index;
        }
        
        let node_index = self.graph.add_node(entity_id.clone());
        self.entity_to_node.insert(entity_id, node_index);
        node_index
    }
    
    /// Add a dependency edge between two entities
    pub fn add_dependency(&mut self, from_entity: &str, to_entity: &str, weight: f64) {
        let from_node = self.add_entity(from_entity.to_string());
        let to_node = self.add_entity(to_entity.to_string());
        
        self.graph.add_edge(from_node, to_node, weight);
    }
    
    /// Get the node index for an entity
    pub fn get_node(&self, entity_id: &str) -> Option<NodeIndex> {
        self.entity_to_node.get(entity_id).copied()
    }
    
    /// Calculate betweenness centrality for all nodes
    pub fn calculate_betweenness_centrality(&self) -> HashMap<String, f64> {
        // TODO: Implement Brandes' algorithm for betweenness centrality
        // For now, return empty map
        HashMap::new()
    }
    
    /// Detect strongly connected components (cycles)
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        // TODO: Implement cycle detection using Tarjan's or Kosaraju's algorithm
        // For now, return empty vector
        Vec::new()
    }
}

/// High-performance concurrent dependency graph using lock-free data structures
#[derive(Debug)]
pub struct ConcurrentDependencyGraph {
    /// Thread-safe graph representation
    graph: ArcSwap<Graph<String, f64, Directed>>,
    
    /// Lock-free mapping from entity IDs to node indices
    entity_to_node: DashMap<String, NodeIndex>,
}

impl ConcurrentDependencyGraph {
    /// Create a new concurrent dependency graph
    pub fn new() -> Self {
        Self {
            graph: ArcSwap::new(std::sync::Arc::new(Graph::new())),
            entity_to_node: DashMap::new(),
        }
    }

    /// Add entities in parallel
    #[cfg(feature = "parallel")]
    pub fn add_entities_parallel(&self, entity_ids: &[String]) {
        entity_ids
            .par_iter()
            .for_each(|entity_id| {
                self.add_entity_atomic(entity_id.clone());
            });
    }

    /// Thread-safe entity addition
    fn add_entity_atomic(&self, entity_id: String) -> NodeIndex {
        if let Some(node_index) = self.entity_to_node.get(&entity_id) {
            return *node_index;
        }

        // Create new graph with the added node
        let current_graph = self.graph.load();
        let mut new_graph = (**current_graph).clone();
        let node_index = new_graph.add_node(entity_id.clone());
        
        // Update the atomic graph
        self.graph.store(std::sync::Arc::new(new_graph));
        self.entity_to_node.insert(entity_id, node_index);
        
        node_index
    }

    /// Parallel dependency analysis
    #[cfg(feature = "parallel")]
    pub fn analyze_dependencies_parallel(&self, entities: &[CodeEntity]) -> Vec<(String, f64)> {
        entities
            .par_iter()
            .map(|entity| {
                let centrality = self.calculate_node_centrality(&entity.id);
                (entity.id.clone(), centrality)
            })
            .collect()
    }

    /// Fast cycle detection using Kosaraju's algorithm
    pub fn detect_cycles_fast(&self) -> Vec<Vec<String>> {
        let graph = self.graph.load();
        let sccs = kosaraju_scc(&**graph);
        
        sccs.into_iter()
            .filter(|scc| scc.len() > 1) // Only cycles with more than one node
            .map(|scc| {
                scc.into_iter()
                    .map(|node_idx| graph[node_idx].clone())
                    .collect()
            })
            .collect()
    }

    /// Calculate centrality for a single node (optimized)
    fn calculate_node_centrality(&self, entity_id: &str) -> f64 {
        let graph = self.graph.load();
        if let Some(node_idx) = self.entity_to_node.get(entity_id) {
            let in_degree = graph.neighbors_directed(*node_idx, petgraph::Direction::Incoming).count();
            let out_degree = graph.neighbors_directed(*node_idx, petgraph::Direction::Outgoing).count();
            
            // Simple centrality measure: (in_degree + out_degree) / total_nodes
            (in_degree + out_degree) as f64 / graph.node_count() as f64
        } else {
            0.0
        }
    }

    /// Memory-efficient batch processing
    #[cfg(feature = "parallel")]
    pub fn process_batch<T, F, R>(&self, items: &[T], processor: F) -> Vec<R>
    where
        T: Sync,
        F: Fn(&T) -> R + Sync + Send,
        R: Send,
    {
        items
            .par_iter()
            .map(processor)
            .collect()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::ValknutConfig;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_graph_extractor() {
        let extractor = GraphExtractor::new();
        
        assert_eq!(extractor.name(), "graph");
        assert!(!extractor.features().is_empty());
        
        // Create test entity and context
        let entity = CodeEntity::new(
            "test_function",
            "function",
            "test_func",
            "/test/file.py"
        ).with_source_code("import os\nimport sys\ndef test_func():\n    pass");
        
        let config = Arc::new(ValknutConfig::default());
        let context = ExtractionContext::new(config, "python");
        
        let features = extractor.extract(&entity, &context).await.unwrap();
        
        // Check that all expected features are present
        assert!(features.contains_key("fan_in"));
        assert!(features.contains_key("fan_out"));
        assert!(features.contains_key("betweenness_approx"));
        assert!(features.contains_key("in_cycle"));
        assert!(features.contains_key("closeness_centrality"));
        
        // Check that fan_out is positive (should detect imports)
        assert!(features["fan_out"] >= 2.0); // Should detect 2 imports
    }
    
    #[test]
    fn test_dependency_graph() {
        let mut graph = DependencyGraph::new();
        
        // Add entities and dependencies
        graph.add_dependency("A", "B", 1.0);
        graph.add_dependency("B", "C", 1.0);
        graph.add_dependency("A", "C", 1.0);
        
        // Check that nodes were created
        assert!(graph.get_node("A").is_some());
        assert!(graph.get_node("B").is_some());
        assert!(graph.get_node("C").is_some());
        assert!(graph.get_node("D").is_none());
    }
    
    #[tokio::test]
    async fn test_entity_support() {
        let extractor = GraphExtractor::new();
        
        let function_entity = CodeEntity::new("test", "function", "test", "/test.py");
        let class_entity = CodeEntity::new("test", "class", "Test", "/test.py");
        let variable_entity = CodeEntity::new("test", "variable", "x", "/test.py");
        
        assert!(extractor.supports_entity(&function_entity));
        assert!(extractor.supports_entity(&class_entity));
        assert!(!extractor.supports_entity(&variable_entity));
    }
}