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
