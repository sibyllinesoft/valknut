//! MCP tool implementations for valknut analysis functionality.

use chrono;
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

// Type aliases to reduce complexity
type DynError = Box<dyn std::error::Error>;
type ParseResult = Result<(String, Option<String>), (i32, String)>;

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
use valknut_rs::core::config::ReportFormat;
use valknut_rs::core::errors::ValknutError;
use valknut_rs::core::scoring::Priority;
use valknut_rs::io::reports::ReportGenerator;

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

fn default_include_suggestions() -> bool {
    true
}

fn default_format() -> String {
    "json".to_string()
}

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

fn evict_oldest_cache_entry(cache_guard: &mut HashMap<PathBuf, AnalysisCache>) -> Option<PathBuf> {
    let oldest_key = cache_guard
        .iter()
        .min_by_key(|(_, entry)| entry.timestamp)
        .map(|(path, _)| path.clone())?;
    cache_guard.remove(&oldest_key).map(|_| oldest_key)
}

fn cache_entry_is_fresh(entry: &AnalysisCache) -> bool {
    entry.timestamp.elapsed().as_secs() < 300
}

/// Format analysis results according to requested format
fn format_analysis_results(results: &AnalysisResults, format: &str) -> Result<String, DynError> {
    match format {
        "json" => {
            // Direct JSON serialization for JSON format
            serde_json::to_string_pretty(results).map_err(|e| e.into())
        }
        "html" => {
            // Use the report generator for HTML output
            let generator = ReportGenerator::new();
            let report_format = ReportFormat::Html;

            // Create a temporary directory path for the report generation
            let temp_path = std::env::temp_dir().join("valknut_mcp_report.html");
            match generator.generate_report(results, &temp_path, report_format) {
                Ok(_) => {
                    // Read the generated file and return its contents
                    std::fs::read_to_string(&temp_path).map_err(|e| e.into())
                }
                Err(e) => Err(e.into()),
            }
        }
        "markdown" => {
            // Create a simple markdown report manually since ReportFormat doesn't support markdown
            create_markdown_report(results)
        }
        _ => {
            // Default to JSON if unknown format
            serde_json::to_string_pretty(results).map_err(|e| e.into())
        }
    }
}

#[cfg(test)]
fn format_analysis_results_with_temp_path(
    results: &AnalysisResults,
    format: &str,
    temp_path: &Path,
) -> Result<String, DynError> {
    match format {
        "html" => {
            let generator = ReportGenerator::new();
            generator.generate_report(results, temp_path, ReportFormat::Html)?;
            std::fs::read_to_string(temp_path).map_err(|e| e.into())
        }
        _ => format_analysis_results(results, format),
    }
}

/// Parse entity ID to extract file path and entity name
fn parse_entity_id(entity_id: &str) -> ParseResult {
    if entity_id.is_empty() {
        return Err((
            error_codes::INVALID_PARAMS,
            "Entity ID cannot be empty".to_string(),
        ));
    }

    // Try to split on common delimiters
    if let Some(colon_pos) = entity_id.find(':') {
        let file_path = entity_id[..colon_pos].to_string();
        let entity_name = Some(entity_id[colon_pos + 1..].to_string());
        Ok((file_path, entity_name))
    } else if let Some(hash_pos) = entity_id.find('#') {
        let file_path = entity_id[..hash_pos].to_string();
        let entity_name = Some(entity_id[hash_pos + 1..].to_string());
        Ok((file_path, entity_name))
    } else {
        // Treat the entire entity_id as a file path
        Ok((entity_id.to_string(), None))
    }
}

/// Filter refactoring suggestions for a specific entity
fn filter_refactoring_suggestions(
    results: &AnalysisResults,
    entity_id: &str,
    max_suggestions: usize,
) -> serde_json::Value {
    // Find candidates that match the entity ID
    let matching_candidates: Vec<_> = results
        .refactoring_candidates
        .iter()
        .filter(|candidate| {
            candidate.entity_id.contains(entity_id) || entity_id.contains(&candidate.entity_id)
        })
        .take(max_suggestions)
        .collect();

    // Create structured response
    serde_json::json!({
        "entity_id": entity_id,
        "suggestions_count": matching_candidates.len(),
        "suggestions": matching_candidates.iter().map(|candidate| {
            serde_json::json!({
                "entity_id": candidate.entity_id,
                "name": candidate.name,
                "file_path": candidate.file_path,
                "line_range": candidate.line_range,
                "priority": candidate.priority,
                "refactoring_score": candidate.score,
                "confidence": candidate.confidence,
                "issues": candidate.issues,
                "suggested_actions": extract_suggested_actions(candidate)
            })
        }).collect::<Vec<_>>(),
        "summary": {
            "total_files_analyzed": results.summary.files_processed,
            "total_entities_analyzed": results.summary.entities_analyzed,
            "code_health_score": results.summary.code_health_score
        }
    })
}

/// Extract suggested actions from a refactoring candidate
fn extract_suggested_actions(
    candidate: &valknut_rs::api::results::RefactoringCandidate,
) -> Vec<String> {
    let mut actions = Vec::new();

    // Add actions based on the priority and reasons
    match candidate.priority {
        valknut_rs::core::scoring::Priority::Critical => {
            actions.push("Immediate refactoring required".to_string());
        }
        valknut_rs::core::scoring::Priority::High => {
            actions.push("Schedule refactoring in next sprint".to_string());
        }
        valknut_rs::core::scoring::Priority::Medium => {
            actions.push("Consider refactoring when modifying this code".to_string());
        }
        valknut_rs::core::scoring::Priority::Low => {
            actions.push("Refactoring optional, monitor for changes".to_string());
        }
        valknut_rs::core::scoring::Priority::None => {
            actions.push("No immediate action required".to_string());
        }
    }

    // Add specific actions based on issues
    for issue in &candidate.issues {
        if issue.category.contains("complexity") {
            actions.push("Break down complex functions into smaller units".to_string());
        }
        if issue.category.contains("coupling") {
            actions.push("Reduce dependencies between modules".to_string());
        }
        if issue.category.contains("duplication") {
            actions.push("Extract common code into shared utilities".to_string());
        }
    }

    actions
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

/// Create file-specific quality report
fn create_file_quality_report(
    results: &AnalysisResults,
    file_path: &str,
    include_suggestions: bool,
) -> serde_json::Value {
    // Find refactoring candidates for this file
    let file_candidates: Vec<_> = results
        .refactoring_candidates
        .iter()
        .filter(|candidate| candidate.file_path.contains(file_path))
        .collect();

    // Calculate average scores for this file
    let avg_score = if !file_candidates.is_empty() {
        file_candidates.iter().map(|c| c.score).sum::<f64>() / file_candidates.len() as f64
    } else {
        0.0
    };

    let avg_confidence = if !file_candidates.is_empty() {
        file_candidates.iter().map(|c| c.confidence).sum::<f64>() / file_candidates.len() as f64
    } else {
        1.0
    };

    let mut report = serde_json::json!({
        "file_path": file_path,
        "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
        "file_exists": Path::new(file_path).exists(),
        "quality_metrics": {
            "refactoring_score": avg_score,
            "confidence": avg_confidence,
            "priority_issues": file_candidates.iter().filter(|c| matches!(c.priority, Priority::High | Priority::Critical)).count(),
            "total_issues": file_candidates.iter().map(|c| c.issues.len()).sum::<usize>()
        },
        "refactoring_opportunities_count": file_candidates.len()
    });

    if include_suggestions && !file_candidates.is_empty() {
        let suggestions: Vec<serde_json::Value> = file_candidates
            .iter()
            .map(|candidate| {
                serde_json::json!({
                    "entity_name": candidate.name,
                    "entity_id": candidate.entity_id,
                    "priority": candidate.priority,
                    "confidence": candidate.confidence,
                    "refactoring_score": candidate.score,
                    "suggested_actions": extract_suggested_actions(candidate),
                    "line_range": candidate.line_range,
                    "issues": candidate.issues
                })
            })
            .collect();

        report["refactoring_suggestions"] = serde_json::Value::Array(suggestions);
    }

    report
}

/// Create a simple markdown report manually
fn create_markdown_report(results: &AnalysisResults) -> Result<String, DynError> {
    let mut markdown = String::new();

    // Title
    markdown.push_str("# Code Analysis Report\n\n");

    // Summary section
    markdown.push_str("## Summary\n\n");
    markdown.push_str(&format!(
        "- **Files Processed**: {}\n",
        results.summary.files_processed
    ));
    markdown.push_str(&format!(
        "- **Entities Analyzed**: {}\n",
        results.summary.entities_analyzed
    ));
    markdown.push_str(&format!(
        "- **Refactoring Needed**: {}\n",
        results.summary.refactoring_needed
    ));
    markdown.push_str(&format!(
        "- **High Priority**: {}\n",
        results.summary.high_priority
    ));
    markdown.push_str(&format!("- **Critical**: {}\n", results.summary.critical));
    markdown.push_str(&format!(
        "- **Average Refactoring Score**: {:.2}\n",
        results.summary.avg_refactoring_score
    ));
    markdown.push_str(&format!(
        "- **Code Health Score**: {:.2}\n\n",
        results.summary.code_health_score
    ));

    // Refactoring candidates
    if !results.refactoring_candidates.is_empty() {
        markdown.push_str("## Refactoring Candidates\n\n");

        for (i, candidate) in results.refactoring_candidates.iter().enumerate() {
            markdown.push_str(&format!("### {}. {}\n\n", i + 1, candidate.name));
            markdown.push_str(&format!("- **File**: `{}`\n", candidate.file_path));
            markdown.push_str(&format!("- **Priority**: {:?}\n", candidate.priority));
            markdown.push_str(&format!("- **Score**: {:.2}\n", candidate.score));
            markdown.push_str(&format!("- **Confidence**: {:.2}\n", candidate.confidence));

            if !candidate.issues.is_empty() {
                markdown.push_str("- **Issues**:\n");
                for issue in &candidate.issues {
                    let issue_title = results
                        .code_dictionary
                        .issues
                        .get(&issue.code)
                        .map(|entry| entry.title.as_str())
                        .unwrap_or(issue.category.as_str());
                    markdown.push_str(&format!(
                        "  - {}: {} (severity {:.2})\n",
                        issue.code, issue_title, issue.severity
                    ));
                }
            }

            if !candidate.suggestions.is_empty() {
                markdown.push_str("- **Suggestions**:\n");
                for suggestion in &candidate.suggestions {
                    let suggestion_title = results
                        .code_dictionary
                        .suggestions
                        .get(&suggestion.code)
                        .map(|entry| entry.title.as_str())
                        .unwrap_or(suggestion.refactoring_type.as_str());
                    markdown.push_str(&format!(
                        "  - {}: {} (Priority: {:.2}, Effort: {:.2})\n",
                        suggestion.code, suggestion_title, suggestion.priority, suggestion.effort
                    ));
                }
            }

            markdown.push('\n');
        }
    }

    // Statistics
    markdown.push_str("## Statistics\n\n");
    markdown.push_str(&format!(
        "- **Total Duration**: {:.2} seconds\n",
        results.statistics.total_duration.as_secs_f64()
    ));
    markdown.push_str(&format!(
        "- **Average File Processing Time**: {:.3} seconds\n",
        results.statistics.avg_file_processing_time.as_secs_f64()
    ));
    markdown.push_str(&format!(
        "- **Average Entity Processing Time**: {:.3} seconds\n",
        results.statistics.avg_entity_processing_time.as_secs_f64()
    ));

    // Warnings
    if !results.warnings.is_empty() {
        markdown.push_str("\n## Warnings\n\n");
        for warning in &results.warnings {
            markdown.push_str(&format!("- {}\n", warning));
        }
    }

    Ok(markdown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tempfile::{tempdir, TempDir};
    use valknut_rs::core::pipeline::{CodeDefinition, CodeDictionary};

    fn sample_results() -> AnalysisResults {
        let summary = valknut_rs::api::results::AnalysisSummary {
            files_processed: 2,
            entities_analyzed: 3,
            refactoring_needed: 2,
            high_priority: 1,
            critical: 1,
            avg_refactoring_score: 0.72,
            code_health_score: 0.58,
            total_files: 2,
            total_entities: 3,
            total_lines_of_code: 420,
            languages: vec!["Rust".to_string()],
            total_issues: 3,
            high_priority_issues: 2,
            critical_issues: 1,
        };

        let candidate = valknut_rs::api::results::RefactoringCandidate {
            entity_id: "src/lib.rs::sample_fn".to_string(),
            name: "sample_fn".to_string(),
            file_path: "src/lib.rs".to_string(),
            line_range: Some((10, 40)),
            priority: Priority::Critical,
            score: 0.82,
            confidence: 0.93,
            issues: vec![
                valknut_rs::api::results::RefactoringIssue {
                    code: "CMPLX".to_string(),
                    category: "complexity".to_string(),
                    severity: 2.1,
                    contributing_features: vec![valknut_rs::api::results::FeatureContribution {
                        feature_name: "cyclomatic_complexity".to_string(),
                        value: 18.0,
                        normalized_value: 0.7,
                        contribution: 1.2,
                    }],
                },
                valknut_rs::api::results::RefactoringIssue {
                    code: "COUPL".to_string(),
                    category: "coupling".to_string(),
                    severity: 1.4,
                    contributing_features: vec![valknut_rs::api::results::FeatureContribution {
                        feature_name: "fan_in".to_string(),
                        value: 12.0,
                        normalized_value: 0.6,
                        contribution: 0.8,
                    }],
                },
            ],
            suggestions: vec![valknut_rs::api::results::RefactoringSuggestion {
                refactoring_type: "extract_method".to_string(),
                code: "XTRMTH".to_string(),
                priority: 0.9,
                effort: 0.4,
                impact: 0.85,
            }],
            issue_count: 2,
            suggestion_count: 1,
        };

        let mut code_dictionary = CodeDictionary::default();
        code_dictionary.issues.insert(
            "CMPLX".to_string(),
            CodeDefinition {
                code: "CMPLX".to_string(),
                title: "Complexity Too High".to_string(),
                summary: "Function exceeds complexity thresholds".to_string(),
                category: Some("complexity".to_string()),
            },
        );
        code_dictionary.issues.insert(
            "COUPL".to_string(),
            CodeDefinition {
                code: "COUPL".to_string(),
                title: "High Coupling".to_string(),
                summary: "Module has excessive dependencies".to_string(),
                category: Some("architecture".to_string()),
            },
        );

        AnalysisResults {
            summary,
            refactoring_candidates: vec![candidate],
            refactoring_candidates_by_file: Vec::new(),
            statistics: valknut_rs::api::results::AnalysisStatistics {
                total_duration: std::time::Duration::from_secs(2),
                avg_file_processing_time: std::time::Duration::from_millis(150),
                avg_entity_processing_time: std::time::Duration::from_millis(20),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: valknut_rs::api::results::MemoryStats {
                    peak_memory_bytes: 1_000_000,
                    final_memory_bytes: 500_000,
                    efficiency_score: 0.7,
                },
            },
            health_metrics: None,
            directory_health_tree: None,
            clone_analysis: None,
            coverage_packs: Vec::new(),
            unified_hierarchy: vec![serde_json::json!({"id": "root"})],
            warnings: Vec::new(),
            code_dictionary,
        }
    }

    #[test]
    fn default_parameter_helpers_match_expected_values() {
        assert!(default_include_suggestions());
        assert_eq!(default_format(), "json");
        assert_eq!(default_max_suggestions(), 10);
    }

    #[test]
    fn parse_entity_id_handles_delimiters_and_errors() {
        assert_eq!(
            parse_entity_id("src/lib.rs:sample_fn").unwrap(),
            ("src/lib.rs".to_string(), Some("sample_fn".to_string()))
        );
        assert_eq!(
            parse_entity_id("src/lib.rs#sample_fn").unwrap(),
            ("src/lib.rs".to_string(), Some("sample_fn".to_string()))
        );
        assert_eq!(
            parse_entity_id("src/lib.rs").unwrap(),
            ("src/lib.rs".to_string(), None)
        );
        let error = parse_entity_id("");
        assert!(error.is_err());
    }

    #[test]
    fn filter_refactoring_suggestions_limits_results() {
        let results = sample_results();
        let response = filter_refactoring_suggestions(&results, "src/lib.rs", 5);
        assert_eq!(response["suggestions_count"], 1);
        assert_eq!(response["entity_id"], "src/lib.rs");
        assert!(response["suggestions"][0]["suggested_actions"][0]
            .as_str()
            .unwrap()
            .contains("Immediate"));
    }

    #[test]
    fn extract_suggested_actions_reflects_priority_and_issue_categories() {
        let mut candidate = sample_results().refactoring_candidates[0].clone();
        candidate.priority = Priority::Medium;
        candidate
            .issues
            .push(valknut_rs::api::results::RefactoringIssue {
                code: "DUP".to_string(),
                category: "duplication".to_string(),
                severity: 1.0,
                contributing_features: Vec::new(),
            });

        let actions = extract_suggested_actions(&candidate);
        assert!(
            actions
                .iter()
                .any(|action| action.contains("Consider refactoring")),
            "expected medium priority guidance in actions: {actions:?}"
        );
        assert!(
            actions
                .iter()
                .any(|action| action.contains("Extract common code")),
            "expected duplication hint in actions: {actions:?}"
        );
    }

    #[test]
    fn create_file_quality_report_respects_suggestion_flag() {
        let results = sample_results();
        let with_suggestions = create_file_quality_report(&results, "src/lib.rs", true);
        assert!(
            with_suggestions["refactoring_suggestions"].is_array(),
            "expected suggestions array when include_suggestions=true"
        );

        let without_suggestions = create_file_quality_report(&results, "src/lib.rs", false);
        assert!(
            without_suggestions.get("refactoring_suggestions").is_none(),
            "suggestions key should be absent when include_suggestions=false"
        );
    }

    #[test]
    fn evaluate_quality_gates_detects_threshold_violations() {
        let mut results = sample_results();
        results.summary.code_health_score = 0.4;
        results.summary.avg_refactoring_score = 0.9;
        results.summary.high_priority = 2;
        results.summary.critical = 1;

        let params = ValidateQualityGatesParams {
            path: ".".to_string(),
            max_complexity: Some(60.0),
            min_health: Some(0.6),
            max_debt: Some(50.0),
            max_issues: Some(1),
        };

        let report = evaluate_quality_gates(&results, &params);
        assert!(!report["quality_gates_passed"].as_bool().unwrap());
        let violations = report["violations"].as_array().unwrap();
        assert!(violations.iter().any(|v| v["rule"] == "Min Health Score"));
        assert!(violations.iter().any(|v| v["rule"] == "Max Complexity"));
        assert!(violations.iter().any(|v| v["rule"] == "Max Issues"));
        assert!(violations.iter().any(|v| v["rule"] == "Max Technical Debt"));
    }

    #[test]
    fn evaluate_quality_gates_allows_passing_when_within_limits() {
        let results = sample_results();
        let params = ValidateQualityGatesParams {
            path: ".".to_string(),
            max_complexity: Some(90.0),
            min_health: Some(0.5),
            max_debt: Some(90.0),
            max_issues: Some(5),
        };

        let report = evaluate_quality_gates(&results, &params);
        assert!(report["quality_gates_passed"].as_bool().unwrap());
        assert!(report["violations"].as_array().unwrap().is_empty());
    }

    #[test]
    fn format_analysis_results_defaults_to_json_for_unknown_formats() {
        let results = sample_results();
        let serialized = format_analysis_results(&results, "yaml").expect("fallback should work");
        let parsed: serde_json::Value =
            serde_json::from_str(&serialized).expect("result should be valid JSON");
        assert_eq!(parsed["summary"]["files_processed"], 2);
    }

    #[test]
    fn filter_refactoring_suggestions_handles_non_matches() {
        let results = sample_results();
        let response = filter_refactoring_suggestions(&results, "other/file.rs", 3);
        assert_eq!(response["suggestions_count"], 0);
        assert_eq!(response["suggestions"], serde_json::json!([]));
        assert_eq!(response["summary"]["total_files_analyzed"], 2);
    }

    #[test]
    fn extract_suggested_actions_reflects_priority_and_issues() {
        let results = sample_results();
        let candidate = &results.refactoring_candidates[0];
        let actions = extract_suggested_actions(candidate);
        assert!(actions.iter().any(|a| a.contains("Immediate")));
        assert!(actions.iter().any(|a| a.contains("Break down")));
        assert!(actions.iter().any(|a| a.contains("Reduce dependencies")));
    }

    #[test]
    fn extract_suggested_actions_handles_low_priority_duplication() {
        let mut results = sample_results();
        let mut candidate = results.refactoring_candidates[0].clone();
        candidate.priority = Priority::Low;
        candidate
            .issues
            .push(valknut_rs::api::results::RefactoringIssue {
                code: "DUPL".to_string(),
                category: "duplication".to_string(),
                severity: 1.1,
                contributing_features: vec![],
            });
        let actions = extract_suggested_actions(&candidate);
        assert!(actions.iter().any(|a| a.contains("optional")));
        assert!(actions.iter().any(|a| a.contains("Extract common code")));
    }

    #[test]
    fn evaluate_quality_gates_reports_violations() {
        let results = sample_results();
        let params = ValidateQualityGatesParams {
            path: ".".to_string(),
            max_complexity: Some(50.0),
            min_health: Some(0.75),
            max_debt: Some(60.0),
            max_issues: Some(1),
        };

        let evaluation = evaluate_quality_gates(&results, &params);
        assert!(!evaluation["quality_gates_passed"].as_bool().unwrap());
        assert!(evaluation["violations"].as_array().unwrap().len() >= 3);
    }

    #[test]
    fn evaluate_quality_gates_passes_within_thresholds() {
        let results = sample_results();
        let params = ValidateQualityGatesParams {
            path: ".".to_string(),
            max_complexity: Some(99.0),
            min_health: Some(0.4),
            max_debt: Some(95.0),
            max_issues: Some(5),
        };

        let evaluation = evaluate_quality_gates(&results, &params);
        assert!(evaluation["quality_gates_passed"].as_bool().unwrap());
        assert!(evaluation["violations"].as_array().unwrap().is_empty());
    }

    #[test]
    fn create_file_quality_report_includes_optional_suggestions() {
        let results = sample_results();
        let report = create_file_quality_report(&results, "src/lib.rs", true);
        assert_eq!(report["file_path"], "src/lib.rs");
        assert!(
            report["quality_metrics"]["refactoring_score"]
                .as_f64()
                .unwrap()
                > 0.0
        );
        assert!(
            report
                .get("refactoring_suggestions")
                .expect("expected suggestions")
                .as_array()
                .unwrap()
                .len()
                > 0
        );

        let minimal = create_file_quality_report(&results, "src/lib.rs", false);
        assert!(minimal.get("refactoring_suggestions").is_none());
    }

    #[test]
    fn create_file_quality_report_handles_missing_file() {
        let results = sample_results();
        let report = create_file_quality_report(&results, "does/not/exist.rs", true);
        assert_eq!(report["file_path"], "does/not/exist.rs");
        assert!(!report["file_exists"].as_bool().unwrap());
        assert_eq!(report["refactoring_opportunities_count"], 0);
        assert_eq!(
            report["quality_metrics"]["refactoring_score"]
                .as_f64()
                .unwrap(),
            0.0
        );
        assert!(report.get("refactoring_suggestions").is_none());
    }

    #[test]
    fn format_analysis_results_supports_json_and_markdown() {
        let results = sample_results();
        let json_output = format_analysis_results(&results, "json").unwrap();
        assert!(json_output.contains("\"files_processed\": 2"));

        let markdown_output = format_analysis_results(&results, "markdown").unwrap();
        assert!(markdown_output.contains("# Code Analysis Report"));
        assert!(markdown_output.contains("Refactoring Candidates"));

        let fallback_output = format_analysis_results(&results, "unknown").unwrap();
        assert!(fallback_output.contains("\"entities_analyzed\": 3"));
    }

    #[test]
    fn format_analysis_results_supports_html() {
        let results = sample_results();
        let temp_file = tempfile::NamedTempFile::new().expect("temp file");
        let html_output =
            format_analysis_results_with_temp_path(&results, "html", temp_file.path())
                .expect("html generation should succeed");
        assert!(
            html_output.to_lowercase().contains("<html"),
            "html output should include root tag"
        );
        assert!(
            temp_file.path().exists(),
            "html report should be written to disk"
        );
    }

    #[tokio::test]
    async fn analyze_with_session_cache_uses_warm_entry() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let path = temp_dir.path();
        let canonical_path = path.canonicalize().expect("canonicalize temp dir");

        let cached_results = Arc::new(sample_results());
        let cache: AnalysisCacheRef = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        {
            let mut guard = cache.lock().await;
            guard.insert(
                canonical_path.clone(),
                AnalysisCache {
                    path: canonical_path.clone(),
                    results: cached_results.clone(),
                    timestamp: std::time::Instant::now(),
                },
            );
        }

        let config = AnalysisConfig::default();
        let returned = analyze_with_session_cache(&config, path, &cache)
            .await
            .expect("cache hit should succeed");

        assert!(
            Arc::ptr_eq(&returned, &cached_results),
            "should return the cached Arc"
        );
    }

    #[tokio::test]
    async fn analyze_with_session_cache_recomputes_expired_entry() {
        let project = tempdir().expect("temp dir");
        let project_path = project.path();
        let file_path = project_path.join("lib.rs");
        fs::write(
            &file_path,
            r#"
pub fn coverage_demo() -> i32 {
    41 + 1
}
"#,
        )
        .expect("should write sample source");

        let cache: AnalysisCacheRef = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        let canonical_path = project_path
            .canonicalize()
            .expect("canonicalize project path");
        let expired_results = Arc::new(sample_results());
        {
            let mut guard = cache.lock().await;
            guard.insert(
                canonical_path.clone(),
                AnalysisCache {
                    path: canonical_path.clone(),
                    results: expired_results.clone(),
                    timestamp: Instant::now() - Duration::from_secs(600),
                },
            );
        }

        let config = AnalysisConfig::default()
            .with_languages(vec!["rust".to_string()])
            .with_max_files(1);

        let refreshed = analyze_with_session_cache(&config, project_path, &cache)
            .await
            .expect("expired cache entry should trigger fresh analysis");

        assert!(
            !Arc::ptr_eq(&refreshed, &expired_results),
            "fresh analysis should replace the expired Arc"
        );

        let cache_guard = cache.lock().await;
        let cached_entry = cache_guard
            .get(&canonical_path)
            .expect("cache should contain refreshed entry");
        assert!(
            Arc::ptr_eq(&cached_entry.results, &refreshed),
            "cache should store the refreshed analysis results"
        );
    }

    #[test]
    fn insert_analysis_into_cache_enforces_capacity() {
        let mut cache = HashMap::new();
        let base = Instant::now();
        for idx in 0..10 {
            let path = PathBuf::from(format!("cache_entry_{idx}.json"));
            cache.insert(
                path.clone(),
                AnalysisCache {
                    path,
                    results: Arc::new(sample_results()),
                    timestamp: base - Duration::from_secs((idx + 1) as u64),
                },
            );
        }

        assert!(cache.contains_key(&PathBuf::from("cache_entry_9.json")));

        let new_path = PathBuf::from("cache_entry_new.json");
        let result_arc = Arc::new(sample_results());
        insert_analysis_into_cache(&mut cache, new_path.clone(), result_arc.clone());

        assert_eq!(cache.len(), 10, "capacity should remain capped");
        assert!(
            cache.contains_key(&new_path),
            "new entry should be present after insertion"
        );
        assert!(
            !cache.contains_key(&PathBuf::from("cache_entry_9.json")),
            "oldest entry should be evicted"
        );
        let stored = cache.get(&new_path).expect("new entry should exist");
        assert!(
            Arc::ptr_eq(&stored.results, &result_arc),
            "stored results should reuse the supplied Arc"
        );
    }

    #[test]
    fn evict_oldest_cache_entry_handles_empty_cache() {
        let mut cache = HashMap::new();
        assert!(
            evict_oldest_cache_entry(&mut cache).is_none(),
            "evict helper should return None when cache is empty"
        );
    }

    #[test]
    fn cache_entry_is_fresh_detects_recent_entries() {
        let entry = AnalysisCache {
            path: PathBuf::from("recent"),
            results: Arc::new(sample_results()),
            timestamp: Instant::now() - Duration::from_secs(10),
        };

        assert!(
            cache_entry_is_fresh(&entry),
            "entries newer than 5 minutes should be considered fresh"
        );
    }

    #[test]
    fn cache_entry_is_fresh_detects_expired_entries() {
        let entry = AnalysisCache {
            path: PathBuf::from("expired"),
            results: Arc::new(sample_results()),
            timestamp: Instant::now() - Duration::from_secs(600),
        };

        assert!(
            !cache_entry_is_fresh(&entry),
            "entries older than 5 minutes should expire"
        );
    }

    #[test]
    fn create_markdown_report_includes_warnings_section() {
        let mut results = sample_results();
        results.warnings.push("First warning".to_string());
        results.warnings.push("Second warning".to_string());

        let markdown = create_markdown_report(&results).unwrap();
        assert!(markdown.contains("## Warnings"));
        assert!(markdown.contains("First warning"));
        assert!(markdown.contains("Second warning"));
    }

    #[tokio::test]
    async fn execute_analyze_code_returns_invalid_params_for_missing_path() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let missing_path = std::env::temp_dir().join(format!("valknut_missing_{unique}"));

        let params = AnalyzeCodeParams {
            path: missing_path.to_string_lossy().into_owned(),
            format: "json".to_string(),
        };

        let err = execute_analyze_code(params)
            .await
            .expect_err("non-existent paths should be rejected early");

        assert_eq!(err.0, error_codes::INVALID_PARAMS);
        assert!(
            err.1.contains("does not exist"),
            "unexpected error message: {}",
            err.1
        );
    }

    #[tokio::test]
    async fn execute_refactoring_suggestions_rejects_empty_entity_id() {
        let params = RefactoringSuggestionsParams {
            entity_id: String::new(),
            max_suggestions: 5,
        };

        let err = execute_refactoring_suggestions(params)
            .await
            .expect_err("empty entity ids should fail validation");

        assert_eq!(err.0, error_codes::INVALID_PARAMS);
        assert!(
            err.1.to_lowercase().contains("entity id"),
            "unexpected error message: {}",
            err.1
        );
    }

    #[tokio::test]
    async fn execute_validate_quality_gates_requires_existing_path() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let missing_dir = std::env::temp_dir().join(format!("valknut_missing_dir_{unique}"));

        let params = ValidateQualityGatesParams {
            path: missing_dir.to_string_lossy().into_owned(),
            max_complexity: Some(50.0),
            min_health: Some(0.7),
            max_debt: None,
            max_issues: None,
        };

        let err = execute_validate_quality_gates(params)
            .await
            .expect_err("missing directories should yield validation errors");

        assert_eq!(err.0, error_codes::INVALID_PARAMS);
        assert!(
            err.1.contains("does not exist"),
            "unexpected error message: {}",
            err.1
        );
    }

    #[tokio::test]
    async fn execute_analyze_file_quality_requires_real_files() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();
        let missing_file = std::env::temp_dir().join(format!("valknut_missing_file_{unique}.rs"));

        let params = AnalyzeFileQualityParams {
            file_path: missing_file.to_string_lossy().into_owned(),
            include_suggestions: true,
        };

        let err = execute_analyze_file_quality(params)
            .await
            .expect_err("missing files should be rejected");

        assert_eq!(err.0, error_codes::INVALID_PARAMS);
        assert!(
            err.1.contains("does not exist"),
            "unexpected error message: {}",
            err.1
        );
    }

    #[tokio::test]
    async fn execute_analyze_file_quality_rejects_directory_paths() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let params = AnalyzeFileQualityParams {
            file_path: temp_dir.path().to_string_lossy().into_owned(),
            include_suggestions: false,
        };

        let err = execute_analyze_file_quality(params)
            .await
            .expect_err("directories should not be accepted as file inputs");

        assert_eq!(err.0, error_codes::INVALID_PARAMS);
        assert!(
            err.1.contains("not a file"),
            "unexpected error message: {}",
            err.1
        );
    }
}
