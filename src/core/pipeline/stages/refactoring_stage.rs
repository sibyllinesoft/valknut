//! Refactoring analysis stage for the pipeline.
//!
//! This module handles refactoring opportunity detection and recommendations.

use std::path::PathBuf;

use futures::future;
use tracing::{debug, warn};

use crate::core::pipeline::results::pipeline_results::RefactoringAnalysisResults;
use crate::core::errors::Result;
use crate::detectors::refactoring::RefactoringAnalyzer;

/// Refactoring analysis stage implementation.
pub struct RefactoringStage<'a> {
    refactoring_analyzer: &'a RefactoringAnalyzer,
}

/// Factory and analysis methods for [`RefactoringStage`].
impl<'a> RefactoringStage<'a> {
    /// Create a new refactoring stage with the given analyzer.
    pub fn new(refactoring_analyzer: &'a RefactoringAnalyzer) -> Self {
        Self { refactoring_analyzer }
    }

    /// Run refactoring analysis on the given files.
    pub async fn run_refactoring_analysis(
        &self,
        files: &[PathBuf],
    ) -> Result<RefactoringAnalysisResults> {
        debug!("Running refactoring analysis on {} files", files.len());

        // Parallelize file analysis using tokio::spawn
        let analysis_futures = files.iter().map(|file_path| {
            // Clone the analyzer (it implements Clone)
            let analyzer = self.refactoring_analyzer.clone();
            let path = file_path.clone();

            tokio::spawn(async move { analyzer.analyze_files(&[path]).await })
        });

        // Wait for all concurrent analyses to complete
        let results_of_results = future::join_all(analysis_futures).await;

        // Collect and flatten the results
        let mut detailed_results = Vec::new();
        for result in results_of_results {
            match result {
                Ok(Ok(file_results)) => detailed_results.extend(file_results),
                Ok(Err(e)) => warn!("Refactoring analysis task failed: {}", e),
                Err(e) => warn!("Tokio spawn failed for refactoring analysis: {}", e),
            }
        }
        let opportunities_count = detailed_results
            .iter()
            .map(|r| r.recommendations.len())
            .sum();

        Ok(RefactoringAnalysisResults {
            enabled: true,
            detailed_results,
            opportunities_count,
        })
    }
}
