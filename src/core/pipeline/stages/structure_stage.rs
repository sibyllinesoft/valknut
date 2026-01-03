//! Structure analysis stage for the pipeline.
//!
//! This module handles structure analysis including directory recommendations
//! and file splitting recommendations.

use std::path::PathBuf;

use tracing::{debug, warn};

use crate::core::pipeline::results::pipeline_results::StructureAnalysisResults;
use crate::core::arena_analysis::ArenaAnalysisResult;
use crate::core::errors::Result;
use crate::detectors::structure::{PrecomputedFileMetrics, StructureExtractor};

use crate::detectors::structure::StructureRecommendations;

/// Collect recommendations from StructureRecommendations and convert to JSON values.
fn collect_recommendations(
    recs: StructureRecommendations,
    all_recommendations: &mut Vec<serde_json::Value>,
    file_splitting_recommendations: &mut Vec<serde_json::Value>,
) {
    // Convert branch reorg packs to JSON and add to all_recommendations
    for pack in recs.branch_reorg_packs {
        if let Ok(value) = serde_json::to_value(&pack) {
            all_recommendations.push(value);
        }
    }
    // Convert file split packs to JSON and add to file_splitting_recommendations
    for pack in recs.file_split_packs {
        if let Ok(value) = serde_json::to_value(&pack) {
            file_splitting_recommendations.push(value);
        }
    }
}

/// Structure analysis stage implementation.
pub struct StructureStage<'a> {
    structure_extractor: &'a StructureExtractor,
}

/// Factory and analysis methods for [`StructureStage`].
impl<'a> StructureStage<'a> {
    /// Create a new structure stage with the given extractor.
    pub fn new(structure_extractor: &'a StructureExtractor) -> Self {
        Self { structure_extractor }
    }

    /// Run structure analysis on the given paths.
    pub async fn run_structure_analysis(
        &self,
        paths: &[PathBuf],
    ) -> Result<StructureAnalysisResults> {
        debug!("Running structure analysis");

        let mut all_recommendations = Vec::new();
        let mut file_splitting_recommendations = Vec::new();

        for path in paths {
            match self.structure_extractor.generate_recommendations(path).await {
                Ok(recs) => collect_recommendations(recs, &mut all_recommendations, &mut file_splitting_recommendations),
                Err(e) => warn!("Structure analysis failed for {}: {}", path.display(), e),
            }
        }

        let issues_count = all_recommendations.len() + file_splitting_recommendations.len();

        Ok(StructureAnalysisResults {
            enabled: true,
            directory_recommendations: all_recommendations,
            file_splitting_recommendations,
            issues_count,
        })
    }

    /// Run structure analysis using pre-computed arena results (optimized path - avoids re-reading files)
    pub async fn run_structure_analysis_with_arena_results(
        &self,
        paths: &[PathBuf],
        arena_results: &[ArenaAnalysisResult],
    ) -> Result<StructureAnalysisResults> {
        debug!(
            "Running optimized structure analysis with {} pre-computed file metrics",
            arena_results.len()
        );

        let metrics: Vec<PrecomputedFileMetrics> = arena_results
            .iter()
            .map(PrecomputedFileMetrics::from_arena_result)
            .collect();

        let mut all_recommendations = Vec::new();
        let mut file_splitting_recommendations = Vec::new();

        for path in paths {
            match self.structure_extractor.generate_recommendations_with_metrics(path, &metrics).await {
                Ok(recs) => collect_recommendations(recs, &mut all_recommendations, &mut file_splitting_recommendations),
                Err(e) => warn!("Structure analysis failed for {}: {}", path.display(), e),
            }
        }

        let issues_count = all_recommendations.len() + file_splitting_recommendations.len();

        Ok(StructureAnalysisResults {
            enabled: true,
            directory_recommendations: all_recommendations,
            file_splitting_recommendations,
            issues_count,
        })
    }
}
