//! Python docstring scanner for doc audit.

use super::{is_incomplete_doc, relative_path, DocIssue};
use std::path::Path;

pub fn scan_python(source: &str, path: &Path, root: &Path) -> Vec<DocIssue> {
    let lines: Vec<&str> = source.lines().collect();
    let mut issues = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();

        if trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("class ")
        {
            let indent = indentation(line);
            while let Some((current_indent, _)) = stack.last() {
                if *current_indent >= indent {
                    stack.pop();
                } else {
                    break;
                }
            }

            if let Some((symbol, kind)) = parse_symbol(trimmed) {
                let mut full_name = stack
                    .iter()
                    .map(|(_, name)| name.as_str())
                    .collect::<Vec<&str>>();
                full_name.push(&symbol);
                let symbol_name = full_name.join(".");

                match find_docstring(&lines, index + 1, indent) {
                    Some((docstring, end_index)) => {
                        if is_incomplete_doc(&docstring) {
                            issues.push(build_issue(
                                path,
                                root,
                                index + 1,
                                kind,
                                &symbol_name,
                                format!("{} '{}' has incomplete docstring", kind, symbol_name),
                            ));
                        }
                        stack.push((indent, symbol));
                        index = end_index;
                    }
                    None => {
                        issues.push(build_issue(
                            path,
                            root,
                            index + 1,
                            kind,
                            &symbol_name,
                            format!("{} '{}' is missing a docstring", kind, symbol_name),
                        ));
                        stack.push((indent, symbol));
                    }
                }
            }
        }

        index += 1;
    }

    issues
}

fn build_issue(
    path: &Path,
    root: &Path,
    line: usize,
    _kind: &str,
    symbol: &str,
    detail: String,
) -> DocIssue {
    DocIssue {
        category: "undocumented_python".to_string(),
        path: relative_path(path, root),
        line: Some(line),
        symbol: Some(symbol.to_string()),
        detail,
    }
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|ch| ch.is_ascii_whitespace())
        .count()
}

fn parse_symbol(line: &str) -> Option<(String, &'static str)> {
    if line.starts_with("class ") {
        return extract_symbol_name(line, "class ", "Class");
    }
    if line.starts_with("async def ") {
        return extract_symbol_name(line, "async def ", "Function");
    }
    if line.starts_with("def ") {
        return extract_symbol_name(line, "def ", "Function");
    }
    None
}

fn extract_symbol_name(line: &str, prefix: &str, kind: &'static str) -> Option<(String, &'static str)> {
    let name = line[prefix.len()..]
        .split(|c: char| c == '(' || c == ':' || c.is_whitespace())
        .next()?;
    Some((name.to_string(), kind))
}

fn find_docstring(lines: &[&str], mut index: usize, indent: usize) -> Option<(String, usize)> {
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            index += 1;
            continue;
        }

        if indentation(line) <= indent {
            return None;
        }

        if let Some(doc) = extract_docstring(lines, index) {
            return Some(doc);
        }

        break;
    }

    None
}

fn extract_docstring(lines: &[&str], index: usize) -> Option<(String, usize)> {
    let line = lines[index].trim_start();
    let (prefix_len, quote_char) = find_string_prefix(line)?;
    let marker = match quote_char {
        '\'' => "'''",
        '"' => "\"\"\"",
        _ => return None,
    };

    let remainder = &line[prefix_len..];
    if !remainder.starts_with(marker) {
        return None;
    }

    let after_marker = &remainder[marker.len()..];
    if let Some(end_pos) = after_marker.find(marker) {
        let content = after_marker[..end_pos].to_string();
        return Some((content, index));
    }

    let mut collected = vec![after_marker.to_string()];
    let mut current = index + 1;

    while current < lines.len() {
        let current_line = lines[current];
        if let Some(end_pos) = current_line.find(marker) {
            let before = &current_line[..end_pos];
            collected.push(before.to_string());
            let doc = collected.join("\n");
            return Some((doc, current));
        } else {
            collected.push(current_line.to_string());
        }
        current += 1;
    }

    None
}

fn find_string_prefix(line: &str) -> Option<(usize, char)> {
    let mut index = 0;
    let chars: Vec<char> = line.chars().collect();
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\'' || ch == '"' {
            return Some((index, ch));
        }
        if ch.is_ascii_alphabetic() {
            index += 1;
            continue;
        }
        break;
    }
    None
}
