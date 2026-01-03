//! Oracle (AI refactoring) command implementations.
//!
//! This module provides functions for running AI-powered refactoring analysis
//! using the Refactoring Oracle with Gemini API integration.

use std::path::PathBuf;

use tracing::warn;

use crate::cli::args::AnalyzeArgs;
use crate::cli::reports::is_quiet;
use valknut_rs::api::results::AnalysisResults;
use valknut_rs::oracle::{OracleConfig, RefactoringOracle, RefactoringOracleResponse};

/// Run Oracle dry-run to show slicing plan without calling the API.
///
/// This function displays how a codebase would be partitioned for analysis
/// without actually making any API calls. Useful for understanding the
/// slicing strategy before committing to an analysis run.
pub fn run_oracle_dry_run(paths: &[PathBuf], args: &AnalyzeArgs) -> anyhow::Result<()> {
    // Build config with CLI overrides (no API key needed for dry-run)
    let mut config = OracleConfig {
        api_key: String::new(), // Not needed for dry-run
        max_tokens: 400_000,
        api_endpoint: String::new(),
        model: String::new(),
        enable_slicing: !args.ai_features.no_oracle_slicing,
        slice_token_budget: args.ai_features.oracle_slice_budget.unwrap_or(200_000),
        slice_model: String::new(),
        slicing_threshold: args.ai_features.oracle_slicing_threshold.unwrap_or(300_000),
    };

    if let Some(max_tokens) = args.ai_features.oracle_max_tokens {
        config.max_tokens = max_tokens;
    }

    let oracle = RefactoringOracle::new(config);
    let project_path = paths
        .first()
        .ok_or_else(|| anyhow::anyhow!("No paths provided"))?;

    oracle
        .dry_run(project_path)
        .map_err(|e| anyhow::anyhow!("Oracle dry-run failed: {}", e))
}

/// Run Oracle analysis to get AI refactoring suggestions.
///
/// This function connects to the Gemini API to generate AI-powered
/// refactoring suggestions based on the analysis results.
pub async fn run_oracle_analysis(
    paths: &[PathBuf],
    analysis_result: &AnalysisResults,
    args: &AnalyzeArgs,
) -> anyhow::Result<Option<RefactoringOracleResponse>> {
    let quiet_mode = is_quiet(args);

    // Check if GEMINI_API_KEY is available
    let oracle_config = match OracleConfig::from_env() {
        Ok(mut config) => {
            if let Some(max_tokens) = args.ai_features.oracle_max_tokens {
                config = config.with_max_tokens(max_tokens);
            }
            if let Some(slice_budget) = args.ai_features.oracle_slice_budget {
                config = config.with_slice_budget(slice_budget);
            }
            if args.ai_features.no_oracle_slicing {
                config = config.with_slicing(false);
            }
            if let Some(threshold) = args.ai_features.oracle_slicing_threshold {
                config.slicing_threshold = threshold;
            }
            config
        }
        Err(e) => {
            eprintln!("Oracle configuration failed: {e}");
            eprintln!("Set GEMINI_API_KEY to enable oracle suggestions.");
            return Ok(None);
        }
    };

    let oracle = RefactoringOracle::new(oracle_config);

    // Use the first path as the project root for analysis
    let project_path = paths.first().unwrap();

    if !quiet_mode {
        println!(
            "Oracle: analyzing {} for refactoring suggestions",
            project_path.display()
        );
    }

    match oracle
        .generate_suggestions(project_path, analysis_result)
        .await
    {
        Ok(response) => {
            if !quiet_mode {
                let all_tasks = response.all_tasks();
                let required_tasks = all_tasks
                    .iter()
                    .filter(|t| t.required.unwrap_or(false))
                    .count();
                let optional_tasks = all_tasks.len() - required_tasks;
                println!(
                    "Oracle: {} tasks ({} required, {} optional)",
                    all_tasks.len(),
                    required_tasks,
                    optional_tasks
                );
            }

            // Save oracle response to a separate file for review
            if let Ok(oracle_json) = serde_json::to_string_pretty(&response) {
                let oracle_path = project_path.join(".valknut-oracle-response.json");
                if let Err(e) = tokio::fs::write(&oracle_path, oracle_json).await {
                    warn!(
                        "Failed to write oracle response to {}: {}",
                        oracle_path.display(),
                        e
                    );
                } else if !quiet_mode {
                    println!("Oracle: saved recommendations to {}", oracle_path.display());
                }
            }

            Ok(Some(response))
        }
        Err(e) => {
            if !quiet_mode {
                eprintln!("Oracle analysis failed: {e}");
                eprintln!("Continuing without oracle suggestions.");
            }
            warn!("Oracle analysis failed: {}", e);
            Ok(None)
        }
    }
}
