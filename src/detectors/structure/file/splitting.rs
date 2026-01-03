//! File split analysis, suggestions, and value/effort calculations.

use petgraph::graph::NodeIndex;
use std::path::Path;

use crate::core::errors::Result;
use crate::detectors::structure::config::{
    CohesionGraph, FileSplitPack, SplitEffort, SplitValue, StructureConfig, SuggestedSplit,
};

use super::cohesion::estimate_clone_factor;
use super::imports::FileDependencyMetrics;

/// Split analyzer for file restructuring recommendations
pub struct SplitAnalyzer<'a> {
    config: &'a StructureConfig,
}

/// File splitting analysis methods for [`SplitAnalyzer`].
impl<'a> SplitAnalyzer<'a> {
    /// Creates a new split analyzer with the given configuration.
    pub fn new(config: &'a StructureConfig) -> Self {
        Self { config }
    }

    /// Check if file exceeds "huge" thresholds
    pub fn is_huge_file(&self, loc: usize, size_bytes: usize) -> bool {
        loc >= self.config.fsfile.huge_loc || size_bytes >= self.config.fsfile.huge_bytes
    }

    /// Collect reasons for why a file is considered huge
    pub fn collect_size_reasons(&self, loc: usize, size_bytes: usize) -> Vec<String> {
        let mut reasons = Vec::new();
        if loc >= self.config.fsfile.huge_loc {
            reasons.push(format!("loc {} > {}", loc, self.config.fsfile.huge_loc));
        }
        if size_bytes >= self.config.fsfile.huge_bytes {
            reasons.push(format!(
                "size {} bytes > {} bytes",
                size_bytes, self.config.fsfile.huge_bytes
            ));
        }
        reasons
    }

    /// Build a FileSplitPack from analysis data
    pub fn build_split_pack(
        &self,
        file_path: &Path,
        loc: usize,
        size_bytes: usize,
        cohesion_graph: &CohesionGraph,
        communities: Vec<Vec<NodeIndex>>,
        dependency_metrics: &FileDependencyMetrics,
    ) -> Result<Option<FileSplitPack>> {
        if !self.is_huge_file(loc, size_bytes) {
            return Ok(None);
        }

        let mut reasons = self.collect_size_reasons(loc, size_bytes);

        if communities.len() < self.config.partitioning.min_clusters {
            return Ok(None);
        }
        reasons.push(format!("{} cohesion communities", communities.len()));

        let suggested_splits = self.generate_split_suggestions(file_path, &communities, cohesion_graph)?;
        let value = self.calculate_split_value(loc, cohesion_graph, dependency_metrics)?;
        let effort = self.calculate_split_effort(dependency_metrics)?;

        Ok(Some(FileSplitPack {
            kind: "file_split".to_string(),
            file: file_path.to_path_buf(),
            reasons,
            suggested_splits,
            value,
            effort,
        }))
    }

    /// Generate split file suggestions
    pub fn generate_split_suggestions(
        &self,
        file_path: &Path,
        communities: &[Vec<NodeIndex>],
        cohesion_graph: &CohesionGraph,
    ) -> Result<Vec<SuggestedSplit>> {
        let base_name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");

        let suffixes = ["_core", "_io", "_api"];
        let mut splits = Vec::new();

        for (community_idx, community) in communities.iter().enumerate().take(3) {
            let suffix = suffixes.get(community_idx).unwrap_or(&"_part");

            let mut entities = Vec::new();
            let mut total_loc = 0;

            for &node_idx in community {
                if let Some(entity) = cohesion_graph.node_weight(node_idx) {
                    entities.push(entity.name.clone());
                    total_loc += entity.loc;
                }
            }

            let split_name = self.generate_split_name(base_name, suffix, &entities, file_path);

            splits.push(SuggestedSplit {
                name: split_name,
                entities,
                loc: total_loc,
            });
        }

        // If no communities found, create default splits
        if splits.is_empty() {
            for (i, suffix) in suffixes.iter().enumerate().take(2) {
                splits.push(SuggestedSplit {
                    name: format!(
                        "{}{}.{}",
                        base_name,
                        suffix,
                        file_path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("py")
                    ),
                    entities: vec![format!("Entity{}", i + 1)],
                    loc: 400,
                });
            }
        }

        Ok(splits)
    }

    /// Generate a meaningful name for a split file based on entity analysis
    pub fn generate_split_name(
        &self,
        base_name: &str,
        suffix: &str,
        entities: &[String],
        file_path: &Path,
    ) -> String {
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("py");

        let entity_analysis = analyze_entity_names(entities);

        let final_suffix = if !entity_analysis.is_empty() {
            entity_analysis
        } else {
            suffix.to_string()
        };

        format!("{}{}.{}", base_name, final_suffix, extension)
    }

    /// Calculate value score for file splitting
    pub fn calculate_split_value(
        &self,
        loc: usize,
        cohesion_graph: &CohesionGraph,
        metrics: &FileDependencyMetrics,
    ) -> Result<SplitValue> {
        let size_factor = (loc as f64 / self.config.fsfile.huge_loc as f64).min(1.0);

        let cycle_factor = if metrics.outgoing_dependencies.is_empty() {
            0.0
        } else {
            let mutual = metrics
                .outgoing_dependencies
                .intersection(&metrics.incoming_importers)
                .count();
            let denominator = metrics
                .outgoing_dependencies
                .union(&metrics.incoming_importers)
                .count()
                .max(1);
            (mutual as f64 / denominator as f64).min(1.0)
        };

        let clone_factor = estimate_clone_factor(cohesion_graph);

        let score = 0.6 * size_factor + 0.3 * cycle_factor + 0.1 * clone_factor;

        Ok(SplitValue { score })
    }

    /// Calculate effort required for file splitting
    pub fn calculate_split_effort(&self, metrics: &FileDependencyMetrics) -> Result<SplitEffort> {
        Ok(SplitEffort {
            exports: metrics.exports.len(),
            external_importers: metrics.incoming_importers.len(),
        })
    }
}

/// Check if a lowercased entity name matches any pattern in the list
fn matches_patterns(name: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| name.contains(p))
}

/// Analyze entity names to suggest appropriate suffixes
pub fn analyze_entity_names(entities: &[String]) -> String {
    const IO_PATTERNS: &[&str] = &["read", "write", "load", "save", "file", "io"];
    const API_PATTERNS: &[&str] = &["api", "endpoint", "route", "handler", "controller"];
    const UTIL_PATTERNS: &[&str] = &["util", "helper", "tool"];

    let (io_count, api_count, util_count, core_count) = entities.iter().fold(
        (0, 0, 0, 0),
        |(io, api, util, core), entity| {
            let lower = entity.to_lowercase();
            if matches_patterns(&lower, IO_PATTERNS) {
                (io + 1, api, util, core)
            } else if matches_patterns(&lower, API_PATTERNS) {
                (io, api + 1, util, core)
            } else if matches_patterns(&lower, UTIL_PATTERNS) {
                (io, api, util + 1, core)
            } else {
                (io, api, util, core + 1)
            }
        },
    );

    let counts = [
        (io_count, "_io"),
        (api_count, "_api"),
        (util_count, "_util"),
        (core_count, "_core"),
    ];
    counts
        .iter()
        .max_by_key(|(count, _)| count)
        .map(|(_, suffix)| *suffix)
        .unwrap_or("_core")
        .to_string()
}
