//! AI Refactoring Oracle - Gemini integration for intelligent refactoring suggestions
//!
//! This module provides intelligent refactoring suggestions by bundling codebase contents
//! and sending them to Gemini along with valknut analysis results. For large codebases,
//! the oracle partitions the code into coherent slices based on import graphs.
//!
//! Key features:
//! - Import graph-based codebase partitioning for scalability
//! - Token-budget-aware slice generation
//! - Per-slice analysis with result aggregation
//! - Configurable models for different slice sizes

pub mod bundle;
pub mod gemini;
pub mod helpers;
pub mod slicing;
pub mod types;

use crate::core::errors::{Result, ValknutError, ValknutResultExt};
use crate::core::partitioning::CodeSlice;
use crate::core::pipeline::AnalysisResults;
use std::path::Path;

// Re-export public types
pub use types::{
    CodebaseAssessment, OracleConfig, RefactoringOracleResponse, RefactoringRoadmap,
    RefactoringTask,
};

// Re-export Gemini types for external use
pub use gemini::{
    GeminiCandidate, GeminiContent, GeminiGenerationConfig, GeminiPart, GeminiRequest,
    GeminiResponse, GeminiResponseContent, GeminiResponsePart, SliceAnalysisResult,
};

// Re-export helper functions and types
pub use helpers::{
    abbreviate_label, build_refactor_hints, calculate_file_priority, html_escape, is_test_file,
    normalize_path_for_key, task_priority_score, truncate_hint, FileCandidate,
};

// Re-export bundle functions and constants
pub use bundle::{
    condense_analysis_results, condense_analysis_results_with_budget, create_slice_bundle,
    get_json_schema_instructions, BundleBuilder, ORACLE_CODEBOOK, SKIP_DIRS, SOURCE_EXTENSIONS,
    VALKNUT_OUTPUT_TOKEN_BUDGET,
};

// Re-export slicing functions
pub use slicing::{aggregate_slice_results, collect_source_files, partition_codebase, print_slice_info};

/// AI refactoring oracle that provides intelligent suggestions using Gemini 2.5 Pro
pub struct RefactoringOracle {
    config: OracleConfig,
    client: reqwest::Client,
}

impl RefactoringOracle {
    /// Create a new refactoring oracle with the given configuration
    pub fn new(config: OracleConfig) -> Self {
        let client = reqwest::Client::new();
        Self { config, client }
    }

    /// Dry-run mode: show slicing plan without calling the API
    pub fn dry_run(&self, project_path: &Path) -> Result<()> {
        slicing::dry_run(&self.config, project_path)
    }

    /// Generate refactoring suggestions for the given codebase
    pub async fn generate_suggestions(
        &self,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Result<RefactoringOracleResponse> {
        // First, estimate total codebase size to decide on slicing strategy
        let files = collect_source_files(project_path)?;
        let total_tokens: usize = files
            .iter()
            .filter_map(|f| std::fs::read_to_string(f).ok())
            .map(|content| content.len() / 4)
            .sum();

        println!("\n🔍 [ORACLE] Codebase analysis");
        println!("   📁 Total files: {}", files.len());
        println!("   📊 Estimated tokens: {}", total_tokens);
        println!(
            "   🎯 Slicing threshold: {}",
            self.config.slicing_threshold
        );

        // Decide whether to use sliced analysis
        if self.config.enable_slicing && total_tokens > self.config.slicing_threshold {
            println!("   ✂️  Using sliced analysis (codebase exceeds threshold)");
            self.generate_suggestions_sliced(project_path, analysis_results, &files)
                .await
        } else {
            println!("   📦 Using single-bundle analysis");
            self.generate_suggestions_single(project_path, analysis_results)
                .await
        }
    }

    /// Generate suggestions using single-bundle approach (for smaller codebases)
    async fn generate_suggestions_single(
        &self,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Result<RefactoringOracleResponse> {
        let builder = BundleBuilder::new(&self.config);
        let bundle = builder
            .create_codebase_bundle(project_path, analysis_results)
            .await?;

        self.query_gemini(&bundle, &self.config.model).await
    }

    /// Generate suggestions using sliced analysis (for larger codebases)
    async fn generate_suggestions_sliced(
        &self,
        project_path: &Path,
        analysis_results: &AnalysisResults,
        files: &[std::path::PathBuf],
    ) -> Result<RefactoringOracleResponse> {
        let partition_result = partition_codebase(&self.config, project_path, files)?;

        if partition_result.slices.is_empty() {
            return Err(ValknutError::internal(
                "Failed to create any slices from codebase".to_string(),
            ));
        }

        let slice_results = self
            .analyze_all_slices(&partition_result, project_path, analysis_results)
            .await;

        if slice_results.is_empty() {
            return Err(ValknutError::internal(
                "All slice analyses failed".to_string(),
            ));
        }

        println!(
            "\n🔗 [ORACLE] Aggregating {} slice results...",
            slice_results.len()
        );
        aggregate_slice_results(slice_results, project_path)
    }

    /// Analyze all slices and collect results.
    async fn analyze_all_slices(
        &self,
        partition_result: &crate::core::partitioning::PartitionResult,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Vec<SliceAnalysisResult> {
        let total_slices = partition_result.slices.len();
        let mut results = Vec::new();

        for (i, slice) in partition_result.slices.iter().enumerate() {
            print_slice_info(slice, i + 1, total_slices);

            match self
                .analyze_slice(slice, project_path, analysis_results)
                .await
            {
                Ok(response) => {
                    results.push(SliceAnalysisResult {
                        slice_id: slice.id,
                        primary_module: slice.primary_module.clone(),
                        response,
                    });
                    println!("   ✅ Slice {} complete", i + 1);
                }
                Err(e) => {
                    println!("   ⚠️  Slice {} failed: {}", i + 1, e);
                }
            }
        }

        results
    }

    /// Analyze a single slice
    async fn analyze_slice(
        &self,
        slice: &CodeSlice,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Result<RefactoringOracleResponse> {
        let bundle = create_slice_bundle(slice, project_path, analysis_results)?;
        self.query_gemini(&bundle, &self.config.slice_model).await
    }

    /// Query Gemini API with the bundled content
    async fn query_gemini(&self, content: &str, model: &str) -> Result<RefactoringOracleResponse> {
        let url = format!(
            "{}/{}:generateContent?key={}",
            self.config.api_endpoint, model, self.config.api_key
        );

        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: content.to_string(),
                }],
            }],
            generation_config: GeminiGenerationConfig {
                temperature: 0.2,
                top_k: 40,
                top_p: 0.95,
                max_output_tokens: 32000,
                response_mime_type: "application/json".to_string(),
            },
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_generic_err("sending request to Gemini API")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ValknutError::internal(format!(
                "Gemini API error: {}",
                error_text
            )));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .map_generic_err("parsing Gemini API response")?;

        let response_text = gemini_response
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| ValknutError::internal("No candidates in Gemini response".to_string()))?
            .content
            .parts
            .into_iter()
            .next()
            .ok_or_else(|| ValknutError::internal("No parts in Gemini response".to_string()))?
            .text;

        let oracle_response: RefactoringOracleResponse =
            serde_json::from_str(&response_text).map_json_err("Oracle response")?;

        Ok(oracle_response)
    }

    /// Condense valknut analysis results for AI consumption (delegation method for backward compatibility)
    pub fn condense_analysis_results(&self, results: &AnalysisResults) -> String {
        condense_analysis_results(results)
    }
}

#[cfg(test)]
mod tests;
