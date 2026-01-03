//! TypeScript/JavaScript JSDoc scanner for doc audit.

use super::super::{extract_comment_text, is_incomplete_doc, relative_path, DocIssue};
use std::path::Path;

/// Scans TypeScript/JavaScript source code for missing or incomplete JSDoc comments.
///
/// Detects undocumented functions, classes, and arrow function exports.
pub fn scan_typescript(source: &str, path: &Path, root: &Path) -> Vec<DocIssue> {
    let lines: Vec<&str> = source.lines().collect();
    let mut issues = Vec::new();

    for index in 0..lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();

        if let Some(name) = detect_function(trimmed) {
            push_issue_if_needed(
                &lines,
                index,
                path,
                root,
                "undocumented_ts_function",
                &name,
                format!("Function '{}' missing doc comment", name),
                format!("Function '{}' has incomplete doc comment", name),
                &mut issues,
            );
        } else if let Some(name) = detect_class(trimmed) {
            push_issue_if_needed(
                &lines,
                index,
                path,
                root,
                "undocumented_ts_class",
                &name,
                format!("Class '{}' missing doc comment", name),
                format!("Class '{}' has incomplete doc comment", name),
                &mut issues,
            );
        } else if let Some(name) = detect_arrow_function(trimmed) {
            push_issue_if_needed(
                &lines,
                index,
                path,
                root,
                "undocumented_ts_arrow",
                &name,
                format!("Function '{}' missing doc comment", name),
                format!("Function '{}' has incomplete doc comment", name),
                &mut issues,
            );
        }
    }

    issues
}

/// Checks for documentation and pushes an issue if missing or incomplete.
fn push_issue_if_needed(
    lines: &[&str],
    index: usize,
    path: &Path,
    root: &Path,
    category: &'static str,
    symbol: &str,
    missing_detail: String,
    incomplete_detail: String,
    issues: &mut Vec<DocIssue>,
) {
    match extract_comment_text(lines, index) {
        Some(doc) if !is_incomplete_doc(&doc) => {}
        Some(_) => issues.push(build_issue(
            path,
            root,
            index + 1,
            category,
            Some(symbol),
            incomplete_detail,
        )),
        None => issues.push(build_issue(
            path,
            root,
            index + 1,
            category,
            Some(symbol),
            missing_detail,
        )),
    }
}

/// Creates a documentation issue with the given details.
fn build_issue(
    path: &Path,
    root: &Path,
    line: usize,
    category: &'static str,
    symbol: Option<&str>,
    detail: String,
) -> DocIssue {
    DocIssue {
        category: category.to_string(),
        path: relative_path(path, root),
        line: Some(line),
        symbol: symbol.map(|s| s.to_string()),
        detail,
    }
}

/// Detects a function declaration and returns the function name.
fn detect_function(line: &str) -> Option<String> {
    if !line.contains("function") {
        return None;
    }
    let tokens: Vec<&str> = line.split_whitespace().collect();
    for (idx, token) in tokens.iter().enumerate() {
        if *token == "function" {
            return tokens
                .get(idx + 1)
                .map(|name| name.trim_end_matches(|c| c == '(' || c == '{').to_string());
        }
    }
    None
}

/// Detects a class declaration and returns the class name.
fn detect_class(line: &str) -> Option<String> {
    if !line.contains("class ") {
        return None;
    }
    line.split_whitespace()
        .skip_while(|token| *token != "class")
        .nth(1)
        .map(|name| name.trim_end_matches(|c| c == '{' || c == '(').to_string())
}

/// Detects an arrow function assignment and returns the variable name.
fn detect_arrow_function(line: &str) -> Option<String> {
    if !(line.starts_with("const ") || line.starts_with("let ") || line.starts_with("var ")) {
        return None;
    }
    let lhs = line.split('=').next()?.trim();
    let name_token = lhs.split_whitespace().last()?;
    if line.contains("=>") {
        Some(name_token.to_string())
    } else {
        None
    }
}
