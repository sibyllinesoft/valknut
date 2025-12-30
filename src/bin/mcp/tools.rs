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
#[path = "tools_tests.rs"]
mod tests;
