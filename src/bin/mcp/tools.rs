//! MCP tool implementations for valknut analysis functionality.

use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::mcp::formatters::{
    create_file_quality_report, extract_suggested_actions, filter_refactoring_suggestions,
    format_analysis_results, parse_entity_id,
};

/// Session-level analysis cache for avoiding redundant work
#[derive(Debug, Clone)]
pub struct AnalysisCache {
    pub path: PathBuf,
    pub results: Arc<AnalysisResults>,
    pub timestamp: std::time::Instant,
}

/// Type alias for the analysis cache
pub type AnalysisCacheRef = Arc<Mutex<HashMap<PathBuf, AnalysisCache>>>;

use valknut_rs::api::{
    config_types::AnalysisConfig, engine::ValknutEngine, results::AnalysisResults,
};
use valknut_rs::core::errors::ValknutError;

use crate::mcp::protocol::{error_codes, ContentItem, ToolResult};

/// Parameters for analyze_code tool
#[derive(serde::Deserialize)]
pub struct AnalyzeCodeParams {
    pub path: String,
    #[serde(default = "default_format")]
    pub format: String,
}

/// Parameters for get_refactoring_suggestions tool
#[derive(serde::Deserialize)]
pub struct RefactoringSuggestionsParams {
    pub entity_id: String,
    #[serde(default = "default_max_suggestions")]
    pub max_suggestions: usize,
}

/// Parameters for validate_quality_gates tool
#[derive(serde::Deserialize)]
pub struct ValidateQualityGatesParams {
    pub path: String,
    #[serde(default)]
    pub max_complexity: Option<f64>,
    #[serde(default)]
    pub min_health: Option<f64>,
    #[serde(default)]
    pub max_debt: Option<f64>,
    #[serde(default)]
    pub max_issues: Option<usize>,
}

/// Parameters for analyze_file_quality tool
#[derive(serde::Deserialize)]
pub struct AnalyzeFileQualityParams {
    pub file_path: String,
    #[serde(default = "default_include_suggestions")]
    pub include_suggestions: bool,
}

/// Default value for including suggestions in file quality analysis.
fn default_include_suggestions() -> bool {
    true
}

/// Default output format for analysis results.
fn default_format() -> String {
    "json".to_string()
}

/// Default maximum number of refactoring suggestions.
fn default_max_suggestions() -> usize {
    10
}

/// Execute the analyze_code tool
pub async fn execute_analyze_code(params: AnalyzeCodeParams) -> Result<ToolResult, (i32, String)> {
    info!("Executing analyze_code tool for path: {}", params.path);

    // Validate path exists
    let path = Path::new(&params.path);
    if !path.exists() {
        return Err((
            error_codes::INVALID_PARAMS,
            format!("Path does not exist: {}", params.path),
        ));
    }

    // Create analysis configuration
    let analysis_config = AnalysisConfig::default()
        .with_confidence_threshold(0.75)
        .with_max_files(5000)
        .with_languages(vec![
            "python".to_string(),
            "typescript".to_string(),
            "javascript".to_string(),
            "rust".to_string(),
        ]);

    // Initialize the analysis engine
    let results = match analyze_with_cache(&analysis_config, path).await {
        Ok(results) => results,
        Err(e) => {
            error!("Analysis failed: {}", e);
            return Err((
                error_codes::ANALYSIS_ERROR,
                format!("Analysis failed: {}", e),
            ));
        }
    };

    // Format results according to requested format
    let formatted_output = match format_analysis_results(&results, &params.format) {
        Ok(output) => output,
        Err(e) => {
            error!("Failed to format results: {}", e);
            return Err((
                error_codes::INTERNAL_ERROR,
                format!("Failed to format results: {}", e),
            ));
        }
    };

    Ok(ToolResult {
        content: vec![ContentItem {
            content_type: "text".to_string(),
            text: formatted_output,
        }],
    })
}

/// Execute the get_refactoring_suggestions tool
pub async fn execute_refactoring_suggestions(
    params: RefactoringSuggestionsParams,
) -> Result<ToolResult, (i32, String)> {
    info!(
        "Executing get_refactoring_suggestions tool for entity: {}",
        params.entity_id
    );

    // For this implementation, we'll need to run a targeted analysis
    // Since we don't have a pre-existing analysis, we'll need to infer the path
    // from the entity_id and run a focused analysis

    // Extract path from entity_id (assuming format like "file_path:function_name")
    let (file_path, _entity_name) = parse_entity_id(&params.entity_id)?;

    // Create focused analysis configuration
    let analysis_config = AnalysisConfig::default()
        .with_confidence_threshold(0.5) // Lower threshold for suggestions
        .with_max_files(100); // Focus on relevant files only

    let path = Path::new(&file_path);
    let analysis_target = path.parent().unwrap_or(path);

    let results = match analyze_with_cache(&analysis_config, analysis_target).await {
        Ok(results) => results,
        Err(e) => {
            error!("Analysis failed: {}", e);
            return Err((
                error_codes::ANALYSIS_ERROR,
                format!("Analysis failed: {}", e),
            ));
        }
    };

    // Filter and format refactoring suggestions for the specific entity
    let suggestions =
        filter_refactoring_suggestions(&results, &params.entity_id, params.max_suggestions);

    let formatted_suggestions = match serde_json::to_string_pretty(&suggestions) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize suggestions: {}", e);
            return Err((
                error_codes::INTERNAL_ERROR,
                format!("Failed to serialize suggestions: {}", e),
            ));
        }
    };

    Ok(ToolResult {
        content: vec![ContentItem {
            content_type: "text".to_string(),
            text: formatted_suggestions,
        }],
    })
}

/// Runs analysis for the given path (no caching at this level).
async fn analyze_with_cache(
    config: &AnalysisConfig,
    path: &Path,
) -> Result<AnalysisResults, ValknutError> {
    // For now, create a new engine each time since we don't have cache access here
    // The actual caching will be handled at the server level
    let mut engine = ValknutEngine::new(config.clone()).await?;
    engine.analyze_directory(path).await
}

/// Analyze with session-level cache support
pub async fn analyze_with_session_cache(
    config: &AnalysisConfig,
    path: &Path,
    cache: &AnalysisCacheRef,
) -> Result<Arc<AnalysisResults>, ValknutError> {
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    // Check cache first
    {
        let cache_guard = cache.lock().await;
        if let Some(cached) = cache_guard.get(&canonical_path) {
            if cache_entry_is_fresh(cached) {
                info!("Using cached analysis results for: {}", path.display());
                return Ok(cached.results.clone());
            } else {
                info!("Cache expired for: {}", path.display());
            }
        }
    }

    // Cache miss - run analysis
    info!("Running fresh analysis for: {}", path.display());
    let mut engine = ValknutEngine::new(config.clone()).await?;
    let results = engine.analyze_directory(path).await?;
    let results_arc = Arc::new(results);

    // Cache the results
    {
        let mut cache_guard = cache.lock().await;
        insert_analysis_into_cache(
            &mut cache_guard,
            canonical_path.clone(),
            results_arc.clone(),
        );
        info!("Cached analysis results for: {}", path.display());
    }

    Ok(results_arc)
}

/// Inserts analysis results into the cache, evicting if necessary.
fn insert_analysis_into_cache(
    cache_guard: &mut HashMap<PathBuf, AnalysisCache>,
    canonical_path: PathBuf,
    results_arc: Arc<AnalysisResults>,
) {
    if cache_guard.len() >= 10 {
        if let Some(evicted) = evict_oldest_cache_entry(cache_guard) {
            info!("Evicted oldest cache entry: {}", evicted.display());
        }
    }

    cache_guard.insert(
        canonical_path.clone(),
        AnalysisCache {
            path: canonical_path,
            results: results_arc,
            timestamp: std::time::Instant::now(),
        },
    );
}

/// Removes the oldest entry from the cache and returns its path.
fn evict_oldest_cache_entry(cache_guard: &mut HashMap<PathBuf, AnalysisCache>) -> Option<PathBuf> {
    let oldest_key = cache_guard
        .iter()
        .min_by_key(|(_, entry)| entry.timestamp)
        .map(|(path, _)| path.clone())?;
    cache_guard.remove(&oldest_key).map(|_| oldest_key)
}

/// Checks if a cache entry is still valid (under 5 minutes old).
fn cache_entry_is_fresh(entry: &AnalysisCache) -> bool {
    entry.timestamp.elapsed().as_secs() < 300
}

/// Execute the validate_quality_gates tool
pub async fn execute_validate_quality_gates(
    params: ValidateQualityGatesParams,
) -> Result<ToolResult, (i32, String)> {
    info!(
        "Executing validate_quality_gates tool for path: {}",
        params.path
    );

    // Validate path exists
    let path = Path::new(&params.path);
    if !path.exists() {
        return Err((
            error_codes::INVALID_PARAMS,
            format!("Path does not exist: {}", params.path),
        ));
    }

    // Create analysis configuration
    let analysis_config = AnalysisConfig::default()
        .with_confidence_threshold(0.75)
        .with_max_files(5000);

    // Initialize the analysis engine
    let mut engine = match ValknutEngine::new(analysis_config).await {
        Ok(engine) => engine,
        Err(e) => {
            error!("Failed to initialize analysis engine: {}", e);
            return Err((
                error_codes::ANALYSIS_ERROR,
                format!("Failed to initialize analysis engine: {}", e),
            ));
        }
    };

    // Run analysis
    let results = match engine.analyze_directory(&path).await {
        Ok(results) => results,
        Err(e) => {
            error!("Analysis failed: {}", e);
            return Err((
                error_codes::ANALYSIS_ERROR,
                format!("Analysis failed: {}", e),
            ));
        }
    };

    // Evaluate quality gates
    let quality_result = evaluate_quality_gates(&results, &params);
    let formatted_result = match serde_json::to_string_pretty(&quality_result) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize quality gate results: {}", e);
            return Err((
                error_codes::INTERNAL_ERROR,
                format!("Failed to serialize quality gate results: {}", e),
            ));
        }
    };

    Ok(ToolResult {
        content: vec![ContentItem {
            content_type: "text".to_string(),
            text: formatted_result,
        }],
    })
}

/// Execute the analyze_file_quality tool
pub async fn execute_analyze_file_quality(
    params: AnalyzeFileQualityParams,
) -> Result<ToolResult, (i32, String)> {
    info!(
        "Executing analyze_file_quality tool for file: {}",
        params.file_path
    );

    // Validate file exists
    let file_path = Path::new(&params.file_path);
    if !file_path.exists() {
        return Err((
            error_codes::INVALID_PARAMS,
            format!("File does not exist: {}", params.file_path),
        ));
    }

    if !file_path.is_file() {
        return Err((
            error_codes::INVALID_PARAMS,
            format!("Path is not a file: {}", params.file_path),
        ));
    }

    // Create targeted analysis configuration
    let analysis_config = AnalysisConfig::default()
        .with_confidence_threshold(0.5)
        .with_max_files(1); // Only analyze this one file

    // Initialize the analysis engine
    let mut engine = match ValknutEngine::new(analysis_config).await {
        Ok(engine) => engine,
        Err(e) => {
            error!("Failed to initialize analysis engine: {}", e);
            return Err((
                error_codes::ANALYSIS_ERROR,
                format!("Failed to initialize analysis engine: {}", e),
            ));
        }
    };

    // Run analysis on the file's parent directory but focus on this file
    let parent_dir = file_path.parent().unwrap_or(file_path);
    let results = match engine.analyze_directory(parent_dir).await {
        Ok(results) => results,
        Err(e) => {
            error!("Analysis failed: {}", e);
            return Err((
                error_codes::ANALYSIS_ERROR,
                format!("Analysis failed: {}", e),
            ));
        }
    };

    // Filter results for just this file
    let file_quality_report =
        create_file_quality_report(&results, &params.file_path, params.include_suggestions);
    let formatted_report = match serde_json::to_string_pretty(&file_quality_report) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize file quality report: {}", e);
            return Err((
                error_codes::INTERNAL_ERROR,
                format!("Failed to serialize file quality report: {}", e),
            ));
        }
    };

    Ok(ToolResult {
        content: vec![ContentItem {
            content_type: "text".to_string(),
            text: formatted_report,
        }],
    })
}

/// Evaluate quality gates against analysis results
fn evaluate_quality_gates(
    results: &AnalysisResults,
    params: &ValidateQualityGatesParams,
) -> serde_json::Value {
    let mut violations = Vec::new();
    let mut passed = true;

    // Check health score threshold
    if let Some(min_health) = params.min_health {
        if results.summary.code_health_score < min_health {
            violations.push(serde_json::json!({
                "rule": "Min Health Score",
                "current": results.summary.code_health_score,
                "threshold": min_health,
                "status": "FAILED",
                "message": format!("Health score ({:.1}) is below minimum required ({:.1})",
                                 results.summary.code_health_score, min_health)
            }));
            passed = false;
        }
    }

    // Check refactoring score as complexity proxy
    if let Some(max_complexity) = params.max_complexity {
        if results.summary.avg_refactoring_score > max_complexity / 100.0 {
            violations.push(serde_json::json!({
                "rule": "Max Complexity",
                "current": results.summary.avg_refactoring_score * 100.0,
                "threshold": max_complexity,
                "status": "FAILED",
                "message": format!("Complexity score ({:.1}) exceeds maximum allowed ({:.1})",
                                 results.summary.avg_refactoring_score * 100.0, max_complexity)
            }));
            passed = false;
        }
    }

    // Check issues count threshold (use refactoring_needed + critical + high_priority as proxy)
    if let Some(max_issues) = params.max_issues {
        let total_issues = results.summary.critical + results.summary.high_priority;
        if total_issues > max_issues {
            violations.push(serde_json::json!({
                "rule": "Max Issues",
                "current": total_issues,
                "threshold": max_issues,
                "status": "FAILED",
                "message": format!("Total issues ({}) exceeds maximum allowed ({})",
                                 total_issues, max_issues)
            }));
            passed = false;
        }
    }

    // Use refactoring score as tech debt proxy
    if let Some(max_debt) = params.max_debt {
        let debt_score = results.summary.avg_refactoring_score * 100.0;
        if debt_score > max_debt {
            violations.push(serde_json::json!({
                "rule": "Max Technical Debt",
                "current": debt_score,
                "threshold": max_debt,
                "status": "FAILED",
                "message": format!("Technical debt ratio ({:.1}%) exceeds maximum allowed ({:.1}%)",
                                 debt_score, max_debt)
            }));
            passed = false;
        }
    }

    let total_issues = results.summary.critical + results.summary.high_priority;

    serde_json::json!({
        "quality_gates_passed": passed,
        "overall_health_score": results.summary.code_health_score,
        "complexity_score": results.summary.avg_refactoring_score * 100.0,
        "technical_debt_ratio": results.summary.avg_refactoring_score * 100.0,
        "total_issues": total_issues,
        "violations": violations,
        "summary": {
            "total_files": results.summary.files_processed,
            "files_with_issues": total_issues,
            "refactoring_needed": results.summary.refactoring_needed
        }
    })
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
