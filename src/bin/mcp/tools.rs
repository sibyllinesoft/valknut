//! MCP tool implementations for valknut analysis functionality.

use std::path::Path;
use serde_json;
use tracing::{info, error};

use valknut_rs::api::{engine::ValknutEngine, config_types::AnalysisConfig, results::AnalysisResults};
use valknut_rs::io::reports::ReportGenerator;
use valknut_rs::core::config::ReportFormat;

use crate::mcp::protocol::{ToolResult, ContentItem, error_codes};
// use crate::cli::config::StructureConfig;

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
        return Err((error_codes::INVALID_PARAMS, 
                   format!("Path does not exist: {}", params.path)));
    }

    // Create analysis configuration
    let analysis_config = AnalysisConfig::default()
        .with_confidence_threshold(0.75)
        .with_max_files(5000)
        .with_languages(vec!["python".to_string(), "typescript".to_string(), "javascript".to_string(), "rust".to_string()]);

    // Initialize the analysis engine
    let mut engine = match ValknutEngine::new(analysis_config).await {
        Ok(engine) => engine,
        Err(e) => {
            error!("Failed to initialize analysis engine: {}", e);
            return Err((error_codes::ANALYSIS_ERROR, 
                       format!("Failed to initialize analysis engine: {}", e)));
        }
    };

    // Run analysis
    let results = match engine.analyze_directory(&path).await {
        Ok(results) => results,
        Err(e) => {
            error!("Analysis failed: {}", e);
            return Err((error_codes::ANALYSIS_ERROR, 
                       format!("Analysis failed: {}", e)));
        }
    };

    // Format results according to requested format
    let formatted_output = match format_analysis_results(&results, &params.format) {
        Ok(output) => output,
        Err(e) => {
            error!("Failed to format results: {}", e);
            return Err((error_codes::INTERNAL_ERROR,
                       format!("Failed to format results: {}", e)));
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
pub async fn execute_refactoring_suggestions(params: RefactoringSuggestionsParams) -> Result<ToolResult, (i32, String)> {
    info!("Executing get_refactoring_suggestions tool for entity: {}", params.entity_id);
    
    // For this implementation, we'll need to run a targeted analysis
    // Since we don't have a pre-existing analysis, we'll need to infer the path
    // from the entity_id and run a focused analysis
    
    // Extract path from entity_id (assuming format like "file_path:function_name")
    let (file_path, _entity_name) = parse_entity_id(&params.entity_id)?;
    
    // Create focused analysis configuration
    let analysis_config = AnalysisConfig::default()
        .with_confidence_threshold(0.5) // Lower threshold for suggestions
        .with_max_files(100); // Focus on relevant files only

    // Initialize the analysis engine
    let mut engine = match ValknutEngine::new(analysis_config).await {
        Ok(engine) => engine,
        Err(e) => {
            error!("Failed to initialize analysis engine: {}", e);
            return Err((error_codes::ANALYSIS_ERROR, 
                       format!("Failed to initialize analysis engine: {}", e)));
        }
    };

    // Run analysis on the specific file or directory containing the entity
    let path = Path::new(&file_path);
    let results = match engine.analyze_directory(path.parent().unwrap_or(path)).await {
        Ok(results) => results,
        Err(e) => {
            error!("Analysis failed: {}", e);
            return Err((error_codes::ANALYSIS_ERROR, 
                       format!("Analysis failed: {}", e)));
        }
    };

    // Filter and format refactoring suggestions for the specific entity
    let suggestions = filter_refactoring_suggestions(&results, &params.entity_id, params.max_suggestions);
    
    let formatted_suggestions = match serde_json::to_string_pretty(&suggestions) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize suggestions: {}", e);
            return Err((error_codes::INTERNAL_ERROR,
                       format!("Failed to serialize suggestions: {}", e)));
        }
    };

    Ok(ToolResult {
        content: vec![ContentItem {
            content_type: "text".to_string(),
            text: formatted_suggestions,
        }],
    })
}

/// Format analysis results according to requested format
fn format_analysis_results(results: &AnalysisResults, format: &str) -> Result<String, Box<dyn std::error::Error>> {
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
            let temp_path = std::env::temp_dir().join("valknut_mcp_report");
            match generator.generate_report(results, &temp_path, report_format) {
                Ok(_) => {
                    // Read the generated file and return its contents
                    let report_file = temp_path.with_extension("html");
                    std::fs::read_to_string(report_file).map_err(|e| e.into())
                }
                Err(e) => Err(e.into())
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

/// Parse entity ID to extract file path and entity name
fn parse_entity_id(entity_id: &str) -> Result<(String, Option<String>), (i32, String)> {
    if entity_id.is_empty() {
        return Err((error_codes::INVALID_PARAMS, "Entity ID cannot be empty".to_string()));
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
    max_suggestions: usize
) -> serde_json::Value {
    // Find candidates that match the entity ID
    let matching_candidates: Vec<_> = results.refactoring_candidates
        .iter()
        .filter(|candidate| {
            candidate.entity_id.contains(entity_id) || 
            entity_id.contains(&candidate.entity_id)
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
fn extract_suggested_actions(candidate: &valknut_rs::api::results::RefactoringCandidate) -> Vec<String> {
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

/// Create a simple markdown report manually
fn create_markdown_report(results: &AnalysisResults) -> Result<String, Box<dyn std::error::Error>> {
    let mut markdown = String::new();
    
    // Title
    markdown.push_str("# Code Analysis Report\n\n");
    
    // Summary section
    markdown.push_str("## Summary\n\n");
    markdown.push_str(&format!("- **Files Processed**: {}\n", results.summary.files_processed));
    markdown.push_str(&format!("- **Entities Analyzed**: {}\n", results.summary.entities_analyzed));
    markdown.push_str(&format!("- **Refactoring Needed**: {}\n", results.summary.refactoring_needed));
    markdown.push_str(&format!("- **High Priority**: {}\n", results.summary.high_priority));
    markdown.push_str(&format!("- **Critical**: {}\n", results.summary.critical));
    markdown.push_str(&format!("- **Average Refactoring Score**: {:.2}\n", results.summary.avg_refactoring_score));
    markdown.push_str(&format!("- **Code Health Score**: {:.2}\n\n", results.summary.code_health_score));
    
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
                    markdown.push_str(&format!("  - {}: {}\n", issue.category, issue.description));
                }
            }
            
            if !candidate.suggestions.is_empty() {
                markdown.push_str("- **Suggestions**:\n");
                for suggestion in &candidate.suggestions {
                    markdown.push_str(&format!("  - {}: {} (Priority: {:.2}, Effort: {:.2})\n", 
                                              suggestion.refactoring_type, 
                                              suggestion.description, 
                                              suggestion.priority,
                                              suggestion.effort));
                }
            }
            
            markdown.push_str("\n");
        }
    }
    
    // Statistics
    markdown.push_str("## Statistics\n\n");
    markdown.push_str(&format!("- **Total Duration**: {:.2} seconds\n", results.statistics.total_duration.as_secs_f64()));
    markdown.push_str(&format!("- **Average File Processing Time**: {:.3} seconds\n", results.statistics.avg_file_processing_time.as_secs_f64()));
    markdown.push_str(&format!("- **Average Entity Processing Time**: {:.3} seconds\n", results.statistics.avg_entity_processing_time.as_secs_f64()));
    
    // Warnings
    if !results.warnings.is_empty() {
        markdown.push_str("\n## Warnings\n\n");
        for warning in &results.warnings {
            markdown.push_str(&format!("- {}\n", warning));
        }
    }
    
    Ok(markdown)
}