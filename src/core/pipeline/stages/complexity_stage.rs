//! Complexity analysis stage for the pipeline.
//!
//! This module handles complexity metrics calculation including cyclomatic
//! complexity, cognitive complexity, technical debt, and maintainability index.

use std::path::PathBuf;

use futures::future;
use tracing::{debug, warn};

use crate::core::pipeline::results::pipeline_results::ComplexityAnalysisResults;
use crate::core::arena_analysis::ArenaAnalysisResult;
use crate::core::errors::Result;
use crate::detectors::complexity::{AstComplexityAnalyzer, ComplexityAnalysisResult};

/// Complexity analysis stage implementation.
pub struct ComplexityStage {
    ast_complexity_analyzer: AstComplexityAnalyzer,
}

/// Factory and analysis methods for [`ComplexityStage`].
impl ComplexityStage {
    /// Create a new complexity stage with the given analyzer.
    pub fn new(ast_complexity_analyzer: AstComplexityAnalyzer) -> Self {
        Self {
            ast_complexity_analyzer,
        }
    }

    /// Run complexity analysis from pre-extracted arena results (optimized path).
    pub async fn run_from_arena_results(
        &self,
        arena_results: &[ArenaAnalysisResult],
    ) -> Result<ComplexityAnalysisResults> {
        debug!(
            "Running complexity analysis from {} arena results",
            arena_results.len()
        );

        // Use the configured analyzer instance and run analyses in parallel.
        let analysis_futures = arena_results.iter().map(|arena_result| {
            let analyzer = self.ast_complexity_analyzer.clone();
            let file_path_str = arena_result.file_path_str().to_string();
            let file_path = PathBuf::from(&file_path_str);

            tokio::spawn(async move {
                match tokio::fs::read_to_string(&file_path).await {
                    Ok(source) => analyzer
                        .analyze_file_with_results(&file_path_str, &source)
                        .await,
                    Err(e) => {
                        warn!(
                            "Could not read file for complexity analysis {}: {}",
                            file_path.display(),
                            e
                        );
                        Ok(Vec::new())
                    }
                }
            })
        });

        let results_of_results = future::join_all(analysis_futures).await;

        // Collect and flatten the results
        let mut detailed_results = Vec::new();
        for result in results_of_results {
            match result {
                Ok(Ok(file_results)) => detailed_results.extend(file_results),
                Ok(Err(e)) => warn!("Complexity analysis task failed: {}", e),
                Err(e) => warn!("Tokio spawn failed for complexity analysis: {}", e),
            }
        }

        Self::build_results(detailed_results)
    }

    /// Run complexity analysis (legacy path - re-parses files).
    pub async fn run_from_files(&self, files: &[PathBuf]) -> Result<ComplexityAnalysisResults> {
        debug!("Running complexity analysis on {} files", files.len());

        // Parallelize file analysis using tokio::spawn
        let analysis_futures = files.iter().map(|file_path| {
            let analyzer = self.ast_complexity_analyzer.clone();
            let path = file_path.clone();

            tokio::spawn(async move {
                let file_refs = vec![path.as_path()];
                analyzer.analyze_files(&file_refs).await
            })
        });

        // Wait for all concurrent analyses to complete
        let results_of_results = future::join_all(analysis_futures).await;

        // Collect and flatten the results
        let mut detailed_results = Vec::new();
        for result in results_of_results {
            match result {
                Ok(Ok(file_results)) => detailed_results.extend(file_results),
                Ok(Err(e)) => warn!("Complexity analysis task failed: {}", e),
                Err(e) => warn!("Tokio spawn failed for complexity analysis: {}", e),
            }
        }

        Self::build_results(detailed_results)
    }

    /// Build complexity analysis results from detailed results.
    fn build_results(
        detailed_results: Vec<ComplexityAnalysisResult>,
    ) -> Result<ComplexityAnalysisResults> {
        let count = detailed_results.len() as f64;

        let (total_cyclomatic, total_cognitive, total_debt, total_maintainability) = if count > 0.0
        {
            let total_cyclomatic: f64 = detailed_results
                .iter()
                .map(|r| r.metrics.cyclomatic())
                .sum();
            let total_cognitive: f64 = detailed_results.iter().map(|r| r.metrics.cognitive()).sum();
            let total_debt: f64 = detailed_results
                .iter()
                .map(|r| r.metrics.technical_debt_score)
                .sum();
            let total_maintainability: f64 = detailed_results
                .iter()
                .map(|r| r.metrics.maintainability_index)
                .sum();
            (
                total_cyclomatic,
                total_cognitive,
                total_debt,
                total_maintainability,
            )
        } else {
            (0.0, 0.0, 0.0, 100.0)
        };

        let issues_count = detailed_results.iter().map(|r| r.issues.len()).sum();

        debug!(
            "Complexity analysis completed: {} entities, avg cyclomatic: {:.2}, avg cognitive: {:.2}",
            detailed_results.len(),
            if count > 0.0 { total_cyclomatic / count } else { 0.0 },
            if count > 0.0 { total_cognitive / count } else { 0.0 }
        );

        Ok(ComplexityAnalysisResults {
            enabled: true,
            detailed_results,
            average_cyclomatic_complexity: if count > 0.0 {
                total_cyclomatic / count
            } else {
                0.0
            },
            average_cognitive_complexity: if count > 0.0 {
                total_cognitive / count
            } else {
                0.0
            },
            average_technical_debt_score: if count > 0.0 { total_debt / count } else { 0.0 },
            average_maintainability_index: if count > 0.0 {
                total_maintainability / count
            } else {
                100.0
            },
            issues_count,
        })
    }
}
