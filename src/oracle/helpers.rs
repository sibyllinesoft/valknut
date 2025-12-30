//! Helper functions and types for the oracle module.

use std::collections::HashMap;
use std::path::Path;

use crate::core::pipeline::AnalysisResults;
use crate::core::scoring::Priority;

use super::types::RefactoringTask;

/// High-priority file patterns (boost priority significantly)
pub const HIGH_PRIORITY_PATTERNS: &[&str] = &["main.rs", "lib.rs", "mod.rs"];

/// Medium-priority file patterns (moderate boost)
pub const MEDIUM_PRIORITY_PATTERNS: &[&str] = &["config", "error", "api"];

/// Low-priority file patterns (small boost)
pub const LOW_PRIORITY_PATTERNS: &[&str] = &["core", "engine"];

/// Extension priority boosts (extension, boost amount)
pub const EXTENSION_PRIORITIES: &[(&str, f32)] = &[
    ("rs", 2.0), ("py", 1.5), ("js", 1.5), ("ts", 1.5),
    ("go", 1.0), ("java", 1.0), ("cpp", 1.0),
];

/// Penalty patterns for low-value files
pub const PENALTY_PATTERNS: &[&str] = &["test", "spec", "_test"];

/// Strong penalty patterns for generated/build files
pub const STRONG_PENALTY_PATTERNS: &[&str] = &["generated", "target/", "build/"];

/// Candidate file for inclusion in the codebase bundle
#[derive(Debug)]
pub struct FileCandidate {
    pub path: String,
    pub content: String,
    pub tokens: usize,
    pub priority: f32,
    pub file_type: String,
}

/// Check if a file path indicates it's a test file
pub fn is_test_file(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    let lower = normalized.to_lowercase();

    // Directory-based markers
    const DIR_MARKERS: [&str; 4] = ["/test/", "/tests/", "/__tests__/", "/spec/"];
    if DIR_MARKERS.iter().any(|marker| lower.contains(marker)) {
        return true;
    }

    // Leading path components that typically house tests
    const DIR_PREFIXES: [&str; 3] = ["tests/", "test/", "spec/"];
    if DIR_PREFIXES.iter().any(|prefix| lower.starts_with(prefix)) {
        return true;
    }

    // File-name driven patterns (lowercased for case-insensitive matches)
    const SUFFIXES: [&str; 16] = [
        "_test.rs",
        "_test.py",
        "_test.js",
        "_test.ts",
        ".test.js",
        ".test.ts",
        ".test.tsx",
        ".test.jsx",
        "_spec.js",
        "_spec.ts",
        ".spec.js",
        ".spec.ts",
        "_test.go",
        "_test.java",
        "_test.cpp",
        "_test.c",
    ];
    if SUFFIXES.iter().any(|suffix| lower.ends_with(suffix)) {
        return true;
    }

    // Java naming conventions rely on original casing
    if normalized.ends_with("Test.java")
        || normalized.ends_with("Tests.java")
        || (normalized.ends_with(".java") && normalized.contains("Test"))
    {
        return true;
    }

    // Rust in-module tests (e.g., src/foo/tests.rs), but ignore the top-level tests.rs file
    if lower.contains("tests.rs") && !lower.ends_with("/tests.rs") {
        return true;
    }

    // Python conventions
    if lower.starts_with("test_")
        || lower.contains("/test_")
        || lower.ends_with("/conftest.py")
        || lower == "conftest.py"
    {
        return true;
    }

    false
}

/// Calculate priority score for file inclusion
pub fn calculate_file_priority(path: &str, extension: &str, size: usize) -> f32 {
    let mut priority = 1.0;

    // Boost priority for important files using const arrays
    if HIGH_PRIORITY_PATTERNS.iter().any(|p| path.contains(p)) {
        priority += 3.0;
    }
    if MEDIUM_PRIORITY_PATTERNS.iter().any(|p| path.contains(p)) {
        priority += 2.0;
    }
    if LOW_PRIORITY_PATTERNS.iter().any(|p| path.contains(p)) {
        priority += 1.5;
    }

    // Language-specific priority adjustments using const array
    if let Some((_, boost)) = EXTENSION_PRIORITIES.iter().find(|(ext, _)| *ext == extension) {
        priority += boost;
    }

    // Penalize very large files (they consume too many tokens)
    if size > 50_000 {
        priority *= 0.5;
    } else if size > 20_000 {
        priority *= 0.7;
    }

    // Boost smaller, focused files
    if size < 1_000 {
        priority *= 1.2;
    }

    // Penalize test files and generated files using const arrays
    if PENALTY_PATTERNS.iter().any(|p| path.contains(p)) {
        priority *= 0.3;
    }
    if STRONG_PENALTY_PATTERNS.iter().any(|p| path.contains(p)) {
        priority *= 0.1;
    }

    priority
}

/// Calculate a priority score for a task based on impact and effort
/// Supports both new codes (I1-I3, E1-E3) and legacy strings (low/medium/high)
pub fn task_priority_score(task: &RefactoringTask) -> f64 {
    let impact_score = match task.impact.as_deref() {
        Some("I3") | Some("high") => 3.0,
        Some("I2") | Some("medium") => 2.0,
        Some("I1") | Some("low") => 1.0,
        _ => 1.5,
    };

    let effort_penalty = match task.effort.as_deref() {
        Some("E3") | Some("high") => 0.5,
        Some("E2") | Some("medium") => 0.75,
        Some("E1") | Some("low") => 1.0,
        _ => 0.75,
    };

    let required_bonus = if task.required.unwrap_or(false) { 1.5 } else { 1.0 };

    impact_score * effort_penalty * required_bonus
}

pub fn build_refactor_hints(
    results: &AnalysisResults,
    project_root: &Path,
) -> HashMap<String, Vec<String>> {
    let mut hints: HashMap<String, Vec<String>> = HashMap::new();

    for candidate in &results.refactoring_candidates {
        if !matches!(candidate.priority, Priority::Critical | Priority::High) {
            continue;
        }

        let issue = match candidate.issues.iter().max_by(|a, b| {
            a.severity
                .partial_cmp(&b.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            Some(issue) => issue,
            None => continue,
        };

        let mut severity_pct = (issue.severity * 100.0).round() as i32;
        severity_pct = severity_pct.clamp(0, 999);

        let category = abbreviate_label(&issue.category);
        let suggestion_label = candidate
            .suggestions
            .iter()
            .max_by(|a, b| {
                a.priority
                    .partial_cmp(&b.priority)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|s| abbreviate_label(&s.refactoring_type));

        let mut hint = if let Some(suggestion) = suggestion_label {
            format!("{} {}% {}", category, severity_pct, suggestion)
        } else {
            format!("{} {}%", category, severity_pct)
        };

        hint = truncate_hint(&hint, 60);

        let normalized_path = normalize_path_for_key(
            Path::new(&candidate.file_path)
                .strip_prefix(project_root)
                .unwrap_or_else(|_| Path::new(&candidate.file_path))
                .to_string_lossy()
                .as_ref(),
        );

        hints.entry(normalized_path).or_default().push(hint);
    }

    hints
}

pub fn abbreviate_label(label: &str) -> String {
    let words = label
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>();

    if words.is_empty() {
        let trimmed = label.trim();
        return trimmed.chars().take(8).collect();
    }

    if words.len() == 1 {
        let word = words[0];
        let mut chars = word.chars();
        let first = chars
            .next()
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or_default();
        let rest: String = chars.take(6).collect();
        return format!("{}{}", first, rest);
    }

    let mut abbr = String::new();
    for word in words.iter().take(3) {
        if let Some(ch) = word.chars().next() {
            abbr.push(ch.to_ascii_uppercase());
        }
    }

    if abbr.is_empty() {
        label.chars().take(3).collect()
    } else {
        abbr
    }
}

pub fn truncate_hint(hint: &str, max_len: usize) -> String {
    if hint.len() <= max_len {
        return hint.to_string();
    }
    let mut truncated = hint
        .chars()
        .take(max_len.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

pub fn normalize_path_for_key(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    path.replace('\\', "/")
}

/// HTML escape utility function
pub fn html_escape(content: &str) -> String {
    content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
