//! Impact analysis stage for the pipeline.
//!
//! This module handles dependency impact analysis including cycle detection
//! and chokepoint identification.

use std::path::PathBuf;

use tracing::debug;

use crate::core::pipeline::results::pipeline_results::ImpactAnalysisResults;
use crate::core::errors::Result;
use crate::core::dependency::ProjectDependencyAnalysis;

/// Impact analysis stage implementation.
pub struct ImpactStage;

/// Factory and analysis methods for [`ImpactStage`].
impl ImpactStage {
    /// Create a new impact stage.
    pub fn new() -> Self {
        Self
    }

    /// Run impact analysis powered by the dependency graph.
    pub async fn run_impact_analysis(&self, files: &[PathBuf]) -> Result<ImpactAnalysisResults> {
        debug!(
            "Running dependency impact analysis across {} files",
            files.len()
        );

        if files.is_empty() {
            return Ok(ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            });
        }

        let analysis = ProjectDependencyAnalysis::analyze(files)?;

        if analysis.is_empty() {
            return Ok(ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            });
        }

        let dependency_cycles = analysis
            .cycles()
            .iter()
            .map(|cycle| {
                serde_json::json!({
                    "size": cycle.len(),
                    "members": cycle
                        .iter()
                        .map(|node| serde_json::json!({
                            "name": node.name,
                            "file": node.file_path,
                            "start_line": node.start_line,
                        }))
                        .collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();

        let chokepoints = analysis
            .chokepoints()
            .iter()
            .map(|chokepoint| {
                serde_json::json!({
                    "name": chokepoint.node.name,
                    "file": chokepoint.node.file_path,
                    "start_line": chokepoint.node.start_line,
                    "score": chokepoint.score,
                })
            })
            .collect::<Vec<_>>();

        let issues_count = dependency_cycles.len() + chokepoints.len();

        Ok(ImpactAnalysisResults {
            enabled: true,
            dependency_cycles,
            chokepoints,
            clone_groups: Vec::new(),
            issues_count,
        })
    }
}

/// Default implementation for [`ImpactStage`].
impl Default for ImpactStage {
    /// Returns a new impact stage with default settings.
    fn default() -> Self {
        Self::new()
    }
}
