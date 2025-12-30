//! Helper functions for output formatting and display

use crate::cli::args::OutputFormat;

/// Get emoji for a refactoring type.
pub fn refactoring_type_emoji(refactoring_type: &str) -> &'static str {
    match refactoring_type {
        "ExtractMethod" => "⚡",
        "ExtractClass" => "📦",
        "ReduceComplexity" => "🎯",
        "EliminateDuplication" => "🔄",
        "ImproveNaming" => "📝",
        "SimplifyConditionals" => "🔀",
        "RemoveDeadCode" => "🧹",
        _ => "🔧",
    }
}

/// Format a refactoring type for display (e.g., "ExtractMethod" -> "Extract Method").
pub fn format_refactoring_type(refactoring_type: &str) -> String {
    refactoring_type
        .replace("Extract", "Extract ")
        .replace("Reduce", "Reduce ")
        .replace("Eliminate", "Eliminate ")
        .replace("Improve", "Improve ")
        .replace("Simplify", "Simplify ")
        .replace("Remove", "Remove ")
}

/// Format location from a recommendation's location array.
pub fn format_location(rec: &serde_json::Value) -> String {
    let Some(location) = rec.get("location").and_then(|v| v.as_array()) else {
        return String::new();
    };
    if location.len() < 2 {
        return String::new();
    }
    match (location[0].as_u64(), location[1].as_u64()) {
        (Some(start), Some(end)) if start == end => format!(" (line {})", start),
        (Some(start), Some(end)) => format!(" (lines {}-{})", start, end),
        _ => String::new(),
    }
}

/// Get CSS class based on metric value and thresholds.
pub fn metric_class(
    value: f64,
    good_threshold: f64,
    warning_threshold: f64,
    higher_is_better: bool,
) -> &'static str {
    if higher_is_better {
        if value >= good_threshold {
            "good"
        } else if value >= warning_threshold {
            "warning"
        } else {
            "danger"
        }
    } else if value <= good_threshold {
        "good"
    } else if value <= warning_threshold {
        "warning"
    } else {
        "danger"
    }
}

/// Render a metric card HTML snippet.
pub fn render_metric_card(
    title: &str,
    value: f64,
    class: &str,
    suffix: &str,
    note: Option<&str>,
) -> String {
    let note_html = note.map_or(String::new(), |n| format!("<small>{}</small>", n));
    format!(
        r#"<div class="metric-card {}"><h3>{}</h3><div class="value">{:.1}{}</div>{}</div>"#,
        class, title, value, suffix, note_html
    )
}

/// Get severity indicator (emoji and CSS class).
pub fn severity_indicator(severity: &str) -> (&'static str, &'static str) {
    match severity.to_lowercase().as_str() {
        "critical" | "blocker" => ("🔴", "critical"),
        "high" | "major" => ("🟠", "high"),
        "medium" | "minor" => ("🟡", "medium"),
        "low" | "info" => ("🟢", "low"),
        _ => ("⚪", "unknown"),
    }
}

/// Get effort level CSS class.
pub fn effort_class(effort: u64) -> &'static str {
    match effort {
        1..=3 => "low-effort",
        4..=6 => "medium-effort",
        _ => "high-effort",
    }
}

/// Escape a string for CSV output.
pub fn escape_csv(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Map an `OutputFormat` to its CLI/output string representation.
#[allow(dead_code)]
pub fn format_to_string(format: &OutputFormat) -> &str {
    match format {
        OutputFormat::Jsonl => "jsonl",
        OutputFormat::Json => "json",
        OutputFormat::Yaml => "yaml",
        OutputFormat::Markdown => "markdown",
        OutputFormat::Html => "html",
        OutputFormat::Sonar => "sonar",
        OutputFormat::Csv => "csv",
        OutputFormat::CiSummary => "ci-summary",
        OutputFormat::Pretty => "pretty",
    }
}

/// Map severity string to SonarQube severity level.
pub fn map_severity_to_sonar(severity: Option<&str>) -> &'static str {
    match severity {
        Some("critical") | Some("blocker") => "BLOCKER",
        Some("high") | Some("major") => "MAJOR",
        Some("medium") | Some("minor") => "MINOR",
        Some("low") | Some("info") => "INFO",
        _ => "MINOR",
    }
}
