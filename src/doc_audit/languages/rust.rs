//! Rust rustdoc scanner for doc audit.

use super::super::{extract_comment_text, is_incomplete_doc, relative_path, DocIssue};
use std::path::Path;

/// Scans Rust source code for missing or incomplete rustdoc documentation.
///
/// Detects undocumented public functions, structs, enums, traits, impl blocks, and modules.
/// Test functions and test modules are automatically excluded from the audit.
/// Nested functions (functions defined inside other functions) are also excluded.
pub fn scan_rust(source: &str, path: &Path, root: &Path) -> Vec<DocIssue> {
    let lines: Vec<&str> = source.lines().collect();
    let mut issues = Vec::new();
    let mut pending_attrs: Vec<String> = Vec::new();
    let mut index = 0usize;
    let mut brace_depth = 0isize;
    let mut test_module_depth: Option<isize> = None; // Track depth when entering #[cfg(test)] mod

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();

        // Track brace depth to detect nested functions
        // Use smart counting that ignores braces in strings and comments
        let brace_delta = count_brace_delta(line);

        if trimmed.is_empty() || trimmed.starts_with("///") || trimmed.starts_with("//!") {
            brace_depth += brace_delta;
            index += 1;
            continue;
        }

        // Check if we've exited the test module
        if let Some(test_depth) = test_module_depth {
            if brace_depth <= test_depth {
                test_module_depth = None; // Exited test module
            }
        }

        // Handle attribute lines
        if trimmed.starts_with("#[") {
            let effective_line = handle_attribute_line(trimmed, &mut pending_attrs);
            if let Some(remainder) = effective_line {
                let has_test_attr = has_test_attribute(&pending_attrs);
                let in_test_module = test_module_depth.is_some();

                // Check if this is a test module declaration
                if has_test_attr && remainder.starts_with("mod ") {
                    test_module_depth = Some(brace_depth);
                }

                // Only check items at top level (brace_depth == 0) and not in test module
                if brace_depth == 0 && !in_test_module {
                    if let Some(new_index) = process_item_line(
                        &remainder, &lines, index, has_test_attr, &mut pending_attrs,
                        &mut issues, path, root,
                    ) {
                        brace_depth += brace_delta;
                        index = new_index;
                        continue;
                    }
                }
            }
            brace_depth += brace_delta;
            index += 1;
            continue;
        }

        let has_test_attr = has_test_attribute(&pending_attrs);
        let in_test_module = test_module_depth.is_some();

        // Check if this starts a test module
        if has_test_attr && trimmed.starts_with("mod ") {
            test_module_depth = Some(brace_depth);
        }

        // Only check items at top level (brace_depth == 0) and not in test module
        if brace_depth == 0 && !in_test_module {
            if let Some(new_index) = process_item_line(
                trimmed, &lines, index, has_test_attr, &mut pending_attrs,
                &mut issues, path, root,
            ) {
                brace_depth += brace_delta;
                index = new_index;
                continue;
            }
        }
        pending_attrs.clear();
        brace_depth += brace_delta;
        index += 1;
    }

    issues
}

/// Count brace delta for a line, ignoring braces in strings and comments.
fn count_brace_delta(line: &str) -> isize {
    let mut delta = 0isize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escape_next = false;
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();

    for i in 0..len {
        let c = chars[i];

        // Handle escape sequences
        if escape_next {
            escape_next = false;
            continue;
        }
        if c == '\\' && (in_string || in_char) {
            escape_next = true;
            continue;
        }

        // Skip line comments
        if !in_string && !in_char && c == '/' && i + 1 < len && chars[i + 1] == '/' {
            break; // Rest of line is a comment
        }

        // Handle string boundaries
        if c == '"' && !in_char {
            // Check for raw string (r#"..."#)
            if !in_string && i > 0 && chars[i - 1] == 'r' {
                // Start of raw string, but we just count it as string entry
                in_string = true;
            } else {
                in_string = !in_string;
            }
            continue;
        }

        // Handle char literals
        if c == '\'' && !in_string {
            in_char = !in_char;
            continue;
        }

        // Count braces only outside strings and chars
        if !in_string && !in_char {
            match c {
                '{' => delta += 1,
                '}' => delta -= 1,
                _ => {}
            }
        }
    }

    delta
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

/// Skips over a brace-delimited block and returns the ending line index.
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

/// Creates and pushes a documentation issue to the issues list.
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

/// Extracts the function name from a line containing a function definition.
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

/// Detects struct, enum, or trait definitions and returns the kind and name.
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

/// Detects impl blocks and returns the target type name.
fn detect_impl(line: &str) -> Option<String> {
    let impl_pos = find_keyword(line, "impl")?;
    let prefix = line[..impl_pos].trim();
    if !prefix.is_empty() && prefix != "unsafe" && prefix != "default" {
        return None;
    }
    let mut remainder = line[impl_pos + 4..].trim_start();

    // Skip over generic parameters like <'a, T> or <T: Trait>
    if remainder.starts_with('<') {
        if let Some(close_pos) = find_matching_bracket(remainder) {
            remainder = remainder[close_pos + 1..].trim_start();
        }
    }

    // Now extract the type name (handles "TypeName" or "TypeName<...>")
    let target = remainder
        .split(|c: char| c == '{' || c == ' ' || c == '\t' || c == '\n')
        .filter(|segment| !segment.is_empty())
        .next()?;

    // Extract just the base type name without generic parameters
    let base_name = target.split('<').next().unwrap_or(target);
    if base_name.is_empty() {
        None
    } else {
        Some(base_name.to_string())
    }
}

/// Finds the position of the matching closing bracket for an opening '<'.
fn find_matching_bracket(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extracts an identifier following a keyword (e.g., struct name, mod name).
fn extract_identifier(line: &str, keyword: &str) -> Option<String> {
    // Use find_keyword for proper word boundary checking to avoid matching
    // "structure:" as "struct" + "ure:" or similar false positives
    let keyword_pos = find_keyword(line, keyword)?;

    // Check that prefix is empty or a valid visibility modifier
    let prefix = line[..keyword_pos].trim();
    if !prefix.is_empty() && !prefix.starts_with("pub") {
        return None;
    }

    // Extract remainder after the keyword
    let remainder = line[keyword_pos + keyword.len()..].trim_start();

    // Get the identifier name (split on delimiters including < for generics)
    let name = remainder
        .split(|c: char| c == '{' || c == '(' || c == '<' || c.is_whitespace())
        .next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.trim_end_matches(';').to_string())
    }
}

/// Finds the position of a keyword as a whole word (not part of an identifier).
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

/// Checks if the prefix before `fn` keyword contains only valid modifiers.
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

/// Checks if documentation is missing before the item at the given line index.
fn is_doc_missing(lines: &[&str], index: usize) -> bool {
    extract_comment_text(lines, index).is_none()
}
