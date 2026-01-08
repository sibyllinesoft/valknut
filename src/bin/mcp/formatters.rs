//! Formatting and report generation helpers for MCP tools.
//!
//! This module contains functions for formatting analysis results
//! into various output formats (JSON, HTML, Markdown).

use std::path::Path;

use valknut_rs::api::results::AnalysisResults;
use valknut_rs::core::config::ReportFormat;
use valknut_rs::core::pipeline::{CodeDictionary, RefactoringCandidate};
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
    let mut md = String::new();
    md.push_str("# Code Analysis Report\n\n");
    write_summary_section(&mut md, results);
    write_candidates_section(&mut md, results);
    write_statistics_section(&mut md, results);
    write_warnings_section(&mut md, results);
    Ok(md)
}

fn write_summary_section(md: &mut String, results: &AnalysisResults) {
    let s = &results.summary;
    md.push_str("## Summary\n\n");
    md.push_str(&format!("- **Files Processed**: {}\n", s.files_processed));
    md.push_str(&format!("- **Entities Analyzed**: {}\n", s.entities_analyzed));
    md.push_str(&format!("- **Refactoring Needed**: {}\n", s.refactoring_needed));
    md.push_str(&format!("- **High Priority**: {}\n", s.high_priority));
    md.push_str(&format!("- **Critical**: {}\n", s.critical));
    md.push_str(&format!("- **Average Refactoring Score**: {:.2}\n", s.avg_refactoring_score));
    md.push_str(&format!("- **Code Health Score**: {:.2}\n\n", s.code_health_score));
}

fn write_candidates_section(md: &mut String, results: &AnalysisResults) {
    if results.refactoring_candidates.is_empty() {
        return;
    }
    md.push_str("## Refactoring Candidates\n\n");
    for (i, c) in results.refactoring_candidates.iter().enumerate() {
        write_candidate(md, i + 1, c, &results.code_dictionary);
    }
}

fn write_candidate(
    md: &mut String,
    num: usize,
    c: &RefactoringCandidate,
    dict: &CodeDictionary,
) {
    md.push_str(&format!("### {}. {}\n\n", num, c.name));
    md.push_str(&format!("- **File**: `{}`\n", c.file_path));
    md.push_str(&format!("- **Priority**: {:?}\n", c.priority));
    md.push_str(&format!("- **Score**: {:.2}\n", c.score));
    md.push_str(&format!("- **Confidence**: {:.2}\n", c.confidence));

    if !c.issues.is_empty() {
        md.push_str("- **Issues**:\n");
        for issue in &c.issues {
            let title = dict.issues.get(&issue.code).map(|e| e.title.as_str()).unwrap_or(&issue.category);
            md.push_str(&format!("  - {}: {} (severity {:.2})\n", issue.code, title, issue.severity));
        }
    }

    if !c.suggestions.is_empty() {
        md.push_str("- **Suggestions**:\n");
        for sug in &c.suggestions {
            let title = dict.suggestions.get(&sug.code).map(|e| e.title.as_str()).unwrap_or(&sug.refactoring_type);
            md.push_str(&format!("  - {}: {} (Priority: {:.2}, Effort: {:.2})\n", sug.code, title, sug.priority, sug.effort));
        }
    }
    md.push('\n');
}

fn write_statistics_section(md: &mut String, results: &AnalysisResults) {
    md.push_str("## Statistics\n\n");
    md.push_str(&format!("- **Total Duration**: {:.2} seconds\n", results.statistics.total_duration.as_secs_f64()));
    md.push_str(&format!("- **Average File Processing Time**: {:.3} seconds\n", results.statistics.avg_file_processing_time.as_secs_f64()));
    md.push_str(&format!("- **Average Entity Processing Time**: {:.3} seconds\n", results.statistics.avg_entity_processing_time.as_secs_f64()));
}

fn write_warnings_section(md: &mut String, results: &AnalysisResults) {
    if results.warnings.is_empty() {
        return;
    }
    md.push_str("\n## Warnings\n\n");
    for w in &results.warnings {
        md.push_str(&format!("- {}\n", w));
    }
}
