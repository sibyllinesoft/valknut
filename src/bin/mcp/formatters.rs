//! Formatting and report generation helpers for MCP tools.
//!
//! This module contains functions for formatting analysis results
//! into various output formats (JSON, HTML, Markdown).

use std::path::Path;

use valknut_rs::api::results::AnalysisResults;
use valknut_rs::core::config::ReportFormat;
use valknut_rs::core::scoring::Priority;
use valknut_rs::io::reports::ReportGenerator;

use crate::mcp::protocol::error_codes;

// Type aliases
type DynError = Box<dyn std::error::Error>;
pub(crate) type ParseResult = Result<(String, Option<String>), (i32, String)>;

/// Format analysis results according to requested format
pub(crate) fn format_analysis_results(
    results: &AnalysisResults,
    format: &str,
) -> Result<String, DynError> {
    match format {
        "json" => serde_json::to_string_pretty(results).map_err(|e| e.into()),
        "html" => {
            let generator = ReportGenerator::new();
            let report_format = ReportFormat::Html;
            let temp_path = std::env::temp_dir().join("valknut_mcp_report.html");
            match generator.generate_report(results, &temp_path, report_format) {
                Ok(_) => std::fs::read_to_string(&temp_path).map_err(|e| e.into()),
                Err(e) => Err(e.into()),
            }
        }
        "markdown" => create_markdown_report(results),
        _ => serde_json::to_string_pretty(results).map_err(|e| e.into()),
    }
}

#[cfg(test)]
pub fn format_analysis_results_with_temp_path(
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
pub(crate) fn parse_entity_id(entity_id: &str) -> ParseResult {
    if entity_id.is_empty() {
        return Err((
            error_codes::INVALID_PARAMS,
            "Entity ID cannot be empty".to_string(),
        ));
    }

    if let Some(colon_pos) = entity_id.find(':') {
        let file_path = entity_id[..colon_pos].to_string();
        let entity_name = Some(entity_id[colon_pos + 1..].to_string());
        Ok((file_path, entity_name))
    } else if let Some(hash_pos) = entity_id.find('#') {
        let file_path = entity_id[..hash_pos].to_string();
        let entity_name = Some(entity_id[hash_pos + 1..].to_string());
        Ok((file_path, entity_name))
    } else {
        Ok((entity_id.to_string(), None))
    }
}

/// Filter refactoring suggestions for a specific entity
pub(crate) fn filter_refactoring_suggestions(
    results: &AnalysisResults,
    entity_id: &str,
    max_suggestions: usize,
) -> serde_json::Value {
    let matching_candidates: Vec<_> = results
        .refactoring_candidates
        .iter()
        .filter(|candidate| {
            candidate.entity_id.contains(entity_id) || entity_id.contains(&candidate.entity_id)
        })
        .take(max_suggestions)
        .collect();

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
pub(crate) fn extract_suggested_actions(
    candidate: &valknut_rs::api::results::RefactoringCandidate,
) -> Vec<String> {
    let mut actions = Vec::new();

    match candidate.priority {
        Priority::Critical => {
            actions.push("Immediate refactoring required".to_string());
        }
        Priority::High => {
            actions.push("Schedule refactoring in next sprint".to_string());
        }
        Priority::Medium => {
            actions.push("Consider refactoring when modifying this code".to_string());
        }
        Priority::Low => {
            actions.push("Refactoring optional, monitor for changes".to_string());
        }
        Priority::None => {
            actions.push("No immediate action required".to_string());
        }
    }

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

/// Create a file quality report for a specific file
pub(crate) fn create_file_quality_report(
    results: &AnalysisResults,
    file_path: &str,
    include_suggestions: bool,
) -> serde_json::Value {
    let file_candidates: Vec<_> = results
        .refactoring_candidates
        .iter()
        .filter(|c| c.file_path.contains(file_path) || file_path.contains(&c.file_path))
        .collect();

    let avg_score = if file_candidates.is_empty() {
        0.0
    } else {
        file_candidates.iter().map(|c| c.score).sum::<f64>() / file_candidates.len() as f64
    };

    let avg_confidence = if file_candidates.is_empty() {
        1.0
    } else {
        file_candidates.iter().map(|c| c.confidence).sum::<f64>() / file_candidates.len() as f64
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

/// Create a simple markdown report
pub fn create_markdown_report(results: &AnalysisResults) -> Result<String, DynError> {
    let mut markdown = String::new();

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
