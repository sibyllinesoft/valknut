//! Graph-based dependency analysis using AST-derived call graphs.
//!
//! This module exposes two primary abstractions:
//! - [`GraphExtractor`], a feature extractor that surfaces dependency metrics for
//!   individual code entities.
//! - [`DependencyGraph`], a lightweight helper that can be used in tests and tools to
//!   construct and inspect dependency structures programmatically.

pub mod clique;
pub mod config;
pub use clique::{CliquePartitions, SimilarityCliquePartitioner};
pub use config::GraphConfig;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use tracing::debug;

use crate::core::dependency::{
    canonicalize_path, DependencyMetrics as DepMetrics, EntityKey, ProjectDependencyAnalysis,
};
use crate::core::errors::Result;
use crate::core::featureset::{CodeEntity, ExtractionContext, FeatureDefinition, FeatureExtractor};

/// Cache of file-level dependency analyses keyed by canonical file paths.
static FILE_ANALYSIS_CACHE: Lazy<DashMap<PathBuf, Arc<ProjectDependencyAnalysis>>> =
    Lazy::new(DashMap::new);

/// Graph-based feature extractor deriving metrics from AST-backed dependency graphs.
#[derive(Debug)]
pub struct GraphExtractor {
    features: Vec<FeatureDefinition>,
}

impl GraphExtractor {
    /// Create a new graph extractor instance.
    pub fn new() -> Self {
        let mut extractor = Self {
            features: Vec::new(),
        };
        extractor.initialize_features();
        extractor
    }

    fn initialize_features(&mut self) {
        self.features = vec![
            FeatureDefinition::new("betweenness_approx", "Approximate betweenness centrality")
                .with_range(0.0, 100.0)
                .with_default(0.0),
            FeatureDefinition::new("fan_in", "Number of incoming dependencies")
                .with_range(0.0, 100.0)
                .with_default(0.0),
            FeatureDefinition::new("fan_out", "Number of outgoing dependencies")
                .with_range(0.0, 100.0)
                .with_default(0.0),
            FeatureDefinition::new(
                "in_cycle",
                "Whether entity participates in a dependency cycle",
            )
            .with_range(0.0, 1.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "closeness_centrality",
                "Closeness centrality within the call graph",
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
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();

        if let Some(metrics) = lookup_metrics(entity)? {
            features.insert("fan_in".into(), metrics.fan_in);
            features.insert("fan_out".into(), metrics.fan_out);
            features.insert("betweenness_approx".into(), metrics.choke_score);
            features.insert("closeness_centrality".into(), metrics.closeness);
            features.insert("in_cycle".into(), if metrics.in_cycle { 1.0 } else { 0.0 });
        } else {
            for feature in &self.features {
                features.insert(feature.name.clone(), feature.default_value);
            }
        }

        Ok(features)
    }

    fn supports_entity(&self, entity: &CodeEntity) -> bool {
        matches!(
            entity.entity_type.as_str(),
            "function" | "method" | "class" | "module" | "interface"
        )
    }
}

/// Retrieve cached dependency metrics for the file containing `entity`.
fn lookup_metrics(entity: &CodeEntity) -> Result<Option<DepMetrics>> {
    let file_path = Path::new(&entity.file_path);
    if !file_path.exists() {
        debug!(
            "Skipping dependency metrics for {} - file not found",
            entity.file_path
        );
        return Ok(None);
    }

    let canonical = canonicalize_path(file_path);
    let analysis = get_or_build_analysis(&canonical)?;

    let qualified_name = entity
        .properties
        .get("qualified_name")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .unwrap_or_else(|| entity.name.clone());

    let key = EntityKey::new(
        canonical.clone(),
        entity.name.clone(),
        qualified_name,
        entity.line_range.map(|(start, _)| start),
    );

    Ok(analysis.metrics_for(&key).cloned())
}

fn get_or_build_analysis(path: &Path) -> Result<Arc<ProjectDependencyAnalysis>> {
    if let Some(entry) = FILE_ANALYSIS_CACHE.get(path) {
        return Ok(entry.value().clone());
    }

    let analysis = ProjectDependencyAnalysis::analyze(&[path.to_path_buf()])?;
    let arc = Arc::new(analysis);
    FILE_ANALYSIS_CACHE.insert(path.to_path_buf(), arc.clone());
    Ok(arc)
}

/// Small helper structure for constructing dependency graphs programmatically.
#[derive(Debug)]
pub struct DependencyGraph {
    graph: petgraph::Graph<String, (), petgraph::Directed>,
    node_indices: HashMap<String, NodeIndex>,
}

use petgraph::graph::NodeIndex;

impl DependencyGraph {
    /// Create a new, empty dependency graph.
    pub fn new() -> Self {
        Self {
            graph: petgraph::Graph::new(),
            node_indices: HashMap::new(),
        }
    }

    /// Add a dependency edge (`from` -> `to`).
    pub fn add_dependency(&mut self, from: &str, to: &str, _weight: f64) {
        let from_index = self.get_or_add_node(from);
        let to_index = self.get_or_add_node(to);
        self.graph.add_edge(from_index, to_index, ());
    }

    fn get_or_add_node(&mut self, id: &str) -> NodeIndex {
        if let Some(index) = self.node_indices.get(id) {
            *index
        } else {
            let index = self.graph.add_node(id.to_string());
            self.node_indices.insert(id.to_string(), index);
            index
        }
    }

    /// Retrieve the node index for a given identifier.
    pub fn get_node(&self, id: &str) -> Option<NodeIndex> {
        self.node_indices.get(id).copied()
    }

    /// Calculate betweenness-like scores using simple fan-in/out heuristics.
    pub fn calculate_betweenness_centrality(&self) -> HashMap<String, f64> {
        let mut scores = HashMap::new();

        for (id, index) in &self.node_indices {
            let fan_in = self
                .graph
                .neighbors_directed(*index, petgraph::Direction::Incoming)
                .count() as f64;
            let fan_out = self
                .graph
                .neighbors_directed(*index, petgraph::Direction::Outgoing)
                .count() as f64;
            scores.insert(id.clone(), fan_in * fan_out);
        }

        scores
    }

    /// Detect dependency cycles using strongly connected components.
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        kosaraju_scc(&self.graph)
            .into_iter()
            .filter_map(|component| {
                if component.len() > 1 {
                    Some(
                        component
                            .into_iter()
                            .filter_map(|index| self.graph.node_weight(index))
                            .cloned()
                            .collect::<Vec<String>>(),
                    )
                } else {
                    let index = component[0];
                    if self.graph.find_edge(index, index).is_some() {
                        self.graph.node_weight(index).map(|id| vec![id.clone()])
                    } else {
                        None
                    }
                }
            })
            .collect()
    }
}

use petgraph::algo::kosaraju_scc;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::ValknutConfig;
    use crate::core::featureset::ExtractionContext;
    use tempfile::TempDir;

    fn create_context() -> ExtractionContext {
        ExtractionContext::new(Arc::new(ValknutConfig::default()), "python")
    }

    #[tokio::test]
    async fn graph_extractor_reports_dependency_metrics() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("module.py");
        std::fs::write(
            &file_path,
            r#"def helper():
    return 42

def caller():
    return helper()
"#,
        )
        .unwrap();

        let mut entity = CodeEntity::new(
            "module::caller",
            "function",
            "caller",
            file_path.to_string_lossy(),
        )
        .with_line_range(4, 6);
        entity.source_code = std::fs::read_to_string(&file_path).unwrap();

        let extractor = GraphExtractor::new();
        let features = extractor.extract(&entity, &create_context()).await.unwrap();

        assert_eq!(features.get("fan_out").copied().unwrap_or_default(), 1.0);
        assert!(features.get("fan_in").copied().unwrap_or_default() >= 0.0);
        assert_eq!(features.get("in_cycle").copied().unwrap_or_default(), 0.0);
    }

    #[tokio::test]
    async fn graph_extractor_detects_self_cycle() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("recursive.py");
        std::fs::write(
            &file_path,
            r#"def recurse(n):
    if n <= 0:
        return 0
    return recurse(n - 1)
"#,
        )
        .unwrap();

        let mut entity = CodeEntity::new(
            "recursive::recurse",
            "function",
            "recurse",
            file_path.to_string_lossy(),
        )
        .with_line_range(1, 4);
        entity.source_code = std::fs::read_to_string(&file_path).unwrap();

        let extractor = GraphExtractor::new();
        let features = extractor.extract(&entity, &create_context()).await.unwrap();

        assert_eq!(features.get("in_cycle").copied().unwrap_or_default(), 1.0);
    }

    #[test]
    fn dependency_graph_cycle_detection() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("A", "B", 1.0);
        graph.add_dependency("B", "C", 1.0);
        graph.add_dependency("C", "A", 1.0);

        let cycles = graph.detect_cycles();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), 3);
    }
}
