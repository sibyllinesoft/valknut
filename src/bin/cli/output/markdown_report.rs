//! Markdown report generation functions.
//!
//! This module contains functions for generating Markdown analysis reports.

use super::helpers::{format_refactoring_type, refactoring_type_emoji};
use super::report_helpers::{render_issues_markdown, render_recommendations_markdown};

/// Generate a markdown analysis report.
pub async fn generate_markdown_report(result: &serde_json::Value) -> anyhow::Result<String> {
    let mut content = String::new();
    content.push_str("# Valknut Analysis Report\n\n");

    let total_issues = result["summary"]["total_issues"].as_u64().unwrap_or(0);
    let total_files = result["summary"]["total_files"].as_u64().unwrap_or(0);

    render_md_summary(&mut content, total_files, total_issues);

    if total_issues == 0 {
        content.push_str("✅ **Excellent!** No significant issues found in your codebase.\n");
    } else {
        content.push_str("## Issues Requiring Attention\n\n");
        render_md_health_metrics(&mut content, result);
        render_md_complexity_section(&mut content, result);
        render_md_refactoring_section(&mut content, result);
        render_md_recommendations_footer(&mut content);
    }

    Ok(content)
}

/// Render the summary section header.
fn render_md_summary(content: &mut String, total_files: u64, total_issues: u64) {
    content.push_str("## Summary\n\n");
    content.push_str(&format!("- **Files Analyzed**: {}\n", total_files));
    content.push_str(&format!("- **Issues Found**: {}\n", total_issues));
    content.push_str(&format!(
        "- **Analysis Date**: {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
}

/// Render the health metrics section.
fn render_md_health_metrics(content: &mut String, result: &serde_json::Value) {
    let Some(health_metrics) = result.get("health_metrics") else {
        return;
    };

    content.push_str("### Health Metrics\n\n");

    if let Some(overall_health) = health_metrics.get("overall_health_score").and_then(|v| v.as_f64()) {
        let health_emoji = match overall_health {
            h if h >= 80.0 => "🟢",
            h if h >= 60.0 => "🟡",
            _ => "🔴",
        };
        content.push_str(&format!(
            "- **Overall Health Score**: {} {:.1}/100\n",
            health_emoji, overall_health
        ));
    }

    render_md_metric(content, health_metrics, "complexity_score", "Complexity Score", " (lower is better)");
    render_md_metric(content, health_metrics, "technical_debt_ratio", "Technical Debt Ratio", "% (lower is better)");
    render_md_metric(content, health_metrics, "maintainability_score", "Maintainability Score", "/100");
    content.push('\n');
}

/// Render a single metric line if present.
fn render_md_metric(content: &mut String, obj: &serde_json::Value, key: &str, label: &str, suffix: &str) {
    if let Some(value) = obj.get(key).and_then(|v| v.as_f64()) {
        content.push_str(&format!("- **{}**: {:.1}{}\n", label, value, suffix));
    }
}

/// Render the complexity section with high priority files and statistics.
fn render_md_complexity_section(content: &mut String, result: &serde_json::Value) {
    let Some(complexity) = result.get("complexity") else {
        return;
    };

    render_md_high_priority_files(content, complexity);
    render_md_complexity_statistics(content, complexity);
}

/// Render high priority files with issues.
fn render_md_high_priority_files(content: &mut String, complexity: &serde_json::Value) {
    let Some(detailed_results) = complexity.get("detailed_results").and_then(|v| v.as_array()) else {
        return;
    };

    let high_priority_files: Vec<_> = detailed_results
        .iter()
        .filter(|f| {
            f.get("issues")
                .and_then(|i| i.as_array())
                .map(|i| !i.is_empty())
                .unwrap_or(false)
        })
        .collect();

    if high_priority_files.is_empty() {
        return;
    }

    content.push_str("### High Priority Files\n\n");
    for (i, file_result) in high_priority_files.iter().take(5).enumerate() {
        render_md_file_entry(content, file_result, i + 1);
    }
    content.push('\n');
}

/// Render a single file entry in markdown.
fn render_md_file_entry(content: &mut String, file_result: &serde_json::Value, index: usize) {
    let file_path = file_result.get("file_path").and_then(|v| v.as_str()).unwrap_or("unknown");
    content.push_str(&format!("#### {}. `{}`\n\n", index, file_path));

    if let Some(issues) = file_result.get("issues").and_then(|v| v.as_array()) {
        content.push_str(&render_issues_markdown(issues, 3));
    }

    if let Some(recommendations) = file_result.get("recommendations").and_then(|v| v.as_array()) {
        content.push_str(&render_recommendations_markdown(recommendations, 3));
    }

    content.push('\n');
}

/// Render complexity statistics section.
fn render_md_complexity_statistics(content: &mut String, complexity: &serde_json::Value) {
    content.push_str("### Complexity Statistics\n\n");
    render_md_metric(content, complexity, "average_cyclomatic_complexity", "Average Cyclomatic Complexity", "");
    render_md_metric(content, complexity, "average_cognitive_complexity", "Average Cognitive Complexity", "");
    render_md_metric(content, complexity, "average_technical_debt_score", "Average Technical Debt Score", "");
    content.push('\n');
}

/// Render refactoring section.
fn render_md_refactoring_section(content: &mut String, result: &serde_json::Value) {
    let Some(refactoring) = result.get("refactoring") else {
        return;
    };

    let opportunities_count = refactoring.get("opportunities_count").and_then(|v| v.as_u64()).unwrap_or(0);
    if opportunities_count == 0 {
        return;
    }

    content.push_str("### Refactoring Opportunities\n\n");
    content.push_str(&format!("Found **{}** refactoring opportunities:\n\n", opportunities_count));

    if let Some(detailed_results) = refactoring.get("detailed_results").and_then(|v| v.as_array()) {
        for (i, file_result) in detailed_results.iter().take(5).enumerate() {
            let file_path = file_result.get("file_path").and_then(|v| v.as_str()).unwrap_or("unknown");
            content.push_str(&format!("{}. `{}`\n", i + 1, file_path));
        }
    }
    content.push('\n');
}

/// Render recommendations footer.
fn render_md_recommendations_footer(content: &mut String) {
    content.push_str("---\n\n");
    content.push_str("## Recommendations\n\n");
    content.push_str("1. **Start with Critical Issues**: Focus on files with the highest severity scores\n");
    content.push_str("2. **Apply Incremental Changes**: Make small, focused refactoring improvements\n");
    content.push_str("3. **Prioritize by Impact**: Address issues in frequently modified files first\n");
    content.push_str("4. **Add Tests**: Ensure test coverage before major refactoring\n");
}
