//! Rust rustdoc scanner for doc audit.

use super::{extract_comment_text, is_incomplete_doc, relative_path, DocIssue};
use std::path::Path;

pub fn scan_rust(source: &str, path: &Path, root: &Path) -> Vec<DocIssue> {
    let lines: Vec<&str> = source.lines().collect();
    let mut issues = Vec::new();
    let mut pending_attrs: Vec<String> = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();

        if trimmed.is_empty() || trimmed.starts_with("///") || trimmed.starts_with("//!") {
            index += 1;
            continue;
        }

        // Handle attribute lines
        if trimmed.starts_with("#[") {
            let effective_line = handle_attribute_line(trimmed, &mut pending_attrs);
            if let Some(remainder) = effective_line {
                let has_test_attr = has_test_attribute(&pending_attrs);
                if let Some(new_index) = process_item_line(
                    &remainder, &lines, index, has_test_attr, &mut pending_attrs,
                    &mut issues, path, root,
                ) {
                    index = new_index;
                    continue;
                }
            }
            index += 1;
            continue;
        }

        let has_test_attr = has_test_attribute(&pending_attrs);
        if let Some(new_index) = process_item_line(
            trimmed, &lines, index, has_test_attr, &mut pending_attrs,
            &mut issues, path, root,
        ) {
            index = new_index;
        } else {
            pending_attrs.clear();
            index += 1;
        }
    }

    issues
}

/// Handle attribute line and return remainder if item follows on same line
fn handle_attribute_line(trimmed: &str, pending_attrs: &mut Vec<String>) -> Option<String> {
    pending_attrs.push(trimmed.to_string());

    if let Some(pos) = trimmed.rfind(']') {
        let remainder = trimmed[pos + 1..].trim_start();
        if !remainder.is_empty() {
            return Some(remainder.to_string());
        }
    }
    None
}

/// Check if pending attributes include test-related markers
fn has_test_attribute(pending_attrs: &[String]) -> bool {
    pending_attrs.iter().any(|attr| attr.contains("cfg(test)") || attr.contains("test"))
}

/// Process an item line and return the new index if handled
fn process_item_line(
    trimmed: &str,
    lines: &[&str],
    index: usize,
    has_test_attr: bool,
    pending_attrs: &mut Vec<String>,
    issues: &mut Vec<DocIssue>,
    path: &Path,
    root: &Path,
) -> Option<usize> {
    if trimmed.starts_with("mod ") {
        pending_attrs.clear();
        return Some(handle_module_item(trimmed, lines, index, has_test_attr, issues, path, root));
    }

    if let Some(name) = detect_function_name(trimmed) {
        pending_attrs.clear();
        if !has_test_attr {
            check_item_docs(lines, index, &name, "undocumented_rust_fn", "Function", issues, path, root);
        }
        return Some(index + 1);
    }

    if let Some((kind, name)) = detect_type(trimmed) {
        pending_attrs.clear();
        if !has_test_attr {
            check_item_docs(lines, index, &name, "undocumented_rust_item", kind, issues, path, root);
        }
        return Some(index + 1);
    }

    if let Some(target) = detect_impl(trimmed) {
        pending_attrs.clear();
        if !has_test_attr {
            check_impl_docs(lines, index, &target, issues, path, root);
        }
        return Some(index + 1);
    }

    None
}

/// Handle module item and return new index
fn handle_module_item(
    trimmed: &str,
    lines: &[&str],
    index: usize,
    has_test_attr: bool,
    issues: &mut Vec<DocIssue>,
    path: &Path,
    root: &Path,
) -> usize {
    if has_test_attr {
        return skip_block(lines, index).map(|end| end + 1).unwrap_or(index + 1);
    }
    if trimmed.ends_with(';') {
        return index + 1;
    }
    if let Some(name) = extract_identifier(trimmed, "mod") {
        if is_doc_missing(lines, index) {
            push_issue(issues, path, root, index + 1, Some(&name),
                "undocumented_rust_module",
                format!("Module '{}' lacks module-level docs", name));
        }
    }
    index + 1
}

/// Check docs for a named item (function, struct, enum, trait)
fn check_item_docs(
    lines: &[&str],
    index: usize,
    name: &str,
    category: &'static str,
    kind: &str,
    issues: &mut Vec<DocIssue>,
    path: &Path,
    root: &Path,
) {
    if let Some(doc) = extract_comment_text(lines, index) {
        if is_incomplete_doc(&doc) {
            push_issue(issues, path, root, index + 1, Some(name), category,
                format!("{} '{}' has incomplete rustdoc", kind, name));
        }
    } else {
        push_issue(issues, path, root, index + 1, Some(name), category,
            format!("{} '{}' lacks rustdoc", kind, name));
    }
}

/// Check docs for impl blocks
fn check_impl_docs(
    lines: &[&str],
    index: usize,
    target: &str,
    issues: &mut Vec<DocIssue>,
    path: &Path,
    root: &Path,
) {
    if let Some(doc) = extract_comment_text(lines, index) {
        if is_incomplete_doc(&doc) {
            push_issue(issues, path, root, index + 1, Some(target),
                "undocumented_rust_impl",
                format!("impl block for '{}' has incomplete docs", target));
        }
    } else {
        push_issue(issues, path, root, index + 1, Some(target),
            "undocumented_rust_impl",
            format!("impl block for '{}' lacks overview docs", target));
    }
}

fn skip_block(lines: &[&str], start: usize) -> Option<usize> {
    let mut depth: isize = 0;
    for (idx, line) in lines.iter().enumerate().skip(start) {
        depth += line.chars().filter(|c| *c == '{').count() as isize;
        depth -= line.chars().filter(|c| *c == '}').count() as isize;
        if depth == 0 && idx >= start {
            return Some(idx);
        }
    }
    None
}

fn push_issue(
    issues: &mut Vec<DocIssue>,
    path: &Path,
    root: &Path,
    line: usize,
    symbol: Option<&str>,
    category: &'static str,
    detail: String,
) {
    issues.push(DocIssue {
        category: category.to_string(),
        path: relative_path(path, root),
        line: Some(line),
        symbol: symbol.map(|s| s.to_string()),
        detail,
    });
}

fn detect_function_name(line: &str) -> Option<String> {
    let fn_pos = find_keyword(line, "fn")?;
    let prefix = line[..fn_pos].trim();
    if !is_valid_fn_prefix(prefix) {
        return None;
    }
    let remainder = line[fn_pos + 2..].trim_start();
    let name = remainder
        .split(|c: char| c == '(' || c == '<' || c.is_whitespace())
        .next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn detect_type(line: &str) -> Option<(&'static str, String)> {
    for keyword in ["struct", "enum", "trait"] {
        if let Some(name) = extract_identifier(line, keyword) {
            let kind = match keyword {
                "struct" => "Struct",
                "enum" => "Enum",
                "trait" => "Trait",
                _ => unreachable!(),
            };
            return Some((kind, name));
        }
    }
    None
}

fn detect_impl(line: &str) -> Option<String> {
    let impl_pos = find_keyword(line, "impl")?;
    let prefix = line[..impl_pos].trim();
    if !prefix.is_empty() && prefix != "unsafe" && prefix != "default" {
        return None;
    }
    let remainder = line[impl_pos + 4..].trim_start();
    let target = remainder
        .split(|c: char| c == '{' || c == ' ' || c == '\t' || c == '\n')
        .filter(|segment| !segment.is_empty())
        .next()?;
    Some(target.trim_matches(|c| c == '<' || c == '>').to_string())
}

fn extract_identifier(line: &str, keyword: &str) -> Option<String> {
    if !line.starts_with(keyword) && !line.starts_with(&format!("pub {}", keyword)) {
        return None;
    }
    let remainder = line[keyword.len()..].trim_start();
    let name = remainder
        .split(|c: char| c == '{' || c == '(' || c.is_whitespace())
        .next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.trim_end_matches(';').to_string())
    }
}

fn find_keyword(line: &str, keyword: &str) -> Option<usize> {
    let mut offset = 0usize;
    while let Some(found) = line[offset..].find(keyword) {
        let idx = offset + found;
        let start_ok = idx == 0
            || !line
                .chars()
                .nth(idx - 1)
                .map(|ch| ch.is_alphanumeric() || ch == '_')
                .unwrap_or(false);
        let end_ok = line
            .chars()
            .nth(idx + keyword.len())
            .map(|ch| !ch.is_alphanumeric() && ch != '_')
            .unwrap_or(true);
        if start_ok && end_ok {
            return Some(idx);
        }
        offset = idx + keyword.len();
    }
    None
}

fn is_valid_fn_prefix(prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }
    let tokens = prefix.split_whitespace();
    for token in tokens {
        let token = token.trim();
        if token.starts_with("pub") || matches!(token, "async" | "unsafe" | "const") {
            continue;
        }
        if token.starts_with("extern") {
            continue;
        }
        return false;
    }
    true
}

fn is_doc_missing(lines: &[&str], index: usize) -> bool {
    extract_comment_text(lines, index).is_none()
}
