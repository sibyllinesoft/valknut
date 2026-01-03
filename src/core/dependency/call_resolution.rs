//! Call resolution and scoring for dependency analysis.
//!
//! This module handles parsing and resolving function call identifiers
//! to their target functions in the codebase.

use std::collections::HashMap;

use super::types::{EntityKey, FunctionNode};

/// Parsed call identifier with namespace segments.
#[derive(Debug, Clone)]
pub(crate) struct CallIdentifier {
    segments: Vec<String>,
}

/// Parsing and querying methods for [`CallIdentifier`].
impl CallIdentifier {
    /// Parses a raw call string into a structured identifier.
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        let mut segments = Vec::with_capacity(4);
        let mut buffer = String::new();
        let mut chars = trimmed.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch.is_alphanumeric() || ch == '_' {
                buffer.push(ch);
            } else if ch == '.' || ch == ':' {
                if !buffer.is_empty() {
                    segments.push(buffer.to_lowercase());
                    buffer.clear();
                }
                while matches!(chars.peek(), Some(':')) {
                    chars.next();
                }
            } else if ch == '(' {
                if !buffer.is_empty() {
                    segments.push(buffer.to_lowercase());
                    buffer.clear();
                }
                break;
            } else if ch.is_whitespace() {
                if !buffer.is_empty() {
                    segments.push(buffer.to_lowercase());
                    buffer.clear();
                }
            } else {
                if !buffer.is_empty() {
                    segments.push(buffer.to_lowercase());
                    buffer.clear();
                }
            }
        }

        if !buffer.is_empty() {
            segments.push(buffer.to_lowercase());
        }

        while matches!(segments.first(), Some(segment) if matches!(segment.as_str(), "self" | "this" | "cls" | "super"))
        {
            segments.remove(0);
        }

        if segments.is_empty() {
            return None;
        }

        Some(Self { segments })
    }

    /// Returns the base (final segment) of the call identifier.
    pub fn base(&self) -> &str {
        self.segments.last().map(|s| s.as_str()).unwrap_or("")
    }

    /// Returns the namespace segments (all but the final segment).
    pub fn namespace(&self) -> &[String] {
        if self.segments.len() <= 1 {
            &self.segments[..0]
        } else {
            &self.segments[..self.segments.len() - 1]
        }
    }

    /// Generates candidate lookup keys from progressively shorter segment tails.
    pub fn candidate_keys(&self) -> Vec<String> {
        let mut keys = Vec::with_capacity(self.segments.len());
        for start in 0..self.segments.len() {
            let candidate = self.segments[start..].join("::");
            if !keys.contains(&candidate) {
                keys.push(candidate);
            }
        }
        keys
    }
}

/// Select the best target from candidates for a call.
pub(crate) fn select_target<'a>(
    candidates: &'a [&'a EntityKey],
    source: &FunctionNode,
    nodes: &HashMap<EntityKey, FunctionNode>,
    call: &CallIdentifier,
    candidate_keys: &[String],
) -> Option<&'a EntityKey> {
    candidates
        .iter()
        .filter_map(|&key| {
            let node = nodes.get(key)?;
            let score = score_candidate(source, node, call, candidate_keys)?;
            Some((key, score))
        })
        .max_by_key(|(_, score)| *score)
        .map(|(key, _)| key)
}

/// Calculate match score for a candidate node. Returns None if candidate should be skipped.
fn score_candidate(
    source: &FunctionNode,
    candidate: &FunctionNode,
    call: &CallIdentifier,
    candidate_keys: &[String],
) -> Option<i32> {
    let is_self_call = candidate.unique_id == source.unique_id;

    // Skip self-calls that don't match the call name
    if is_self_call && !call.base().eq_ignore_ascii_case(&candidate.name) {
        return None;
    }

    let mut score = 0;

    // Self-call bonus
    if is_self_call {
        score += 120;
    }

    // Qualified name matching
    score += score_qualified_name_match(candidate, call, candidate_keys);

    // Namespace matching
    if namespace_matches(call.namespace(), &candidate.namespace) {
        score += 50;
    }

    // Same file bonus
    if candidate.file_path == source.file_path {
        score += 20;
    }

    // Namespace proximity
    score += score_namespace_proximity(source, candidate);

    // Line proximity
    score += score_line_proximity(source.start_line, candidate.start_line);

    Some(score)
}

/// Score based on qualified name matching.
fn score_qualified_name_match(
    candidate: &FunctionNode,
    call: &CallIdentifier,
    candidate_keys: &[String],
) -> i32 {
    let qualified_lower = candidate.qualified_name.to_lowercase();

    if !candidate_keys.is_empty() && qualified_lower == candidate_keys[0] {
        100
    } else if candidate_keys.iter().any(|k| k == &qualified_lower) {
        75
    } else if candidate.name.eq_ignore_ascii_case(call.base()) {
        40
    } else {
        0
    }
}

/// Score based on namespace proximity between source and candidate.
fn score_namespace_proximity(source: &FunctionNode, candidate: &FunctionNode) -> i32 {
    if namespace_equals(&source.namespace, &candidate.namespace) {
        15
    } else if namespace_shares_tail(&source.namespace, &candidate.namespace) {
        8
    } else {
        0
    }
}

/// Score based on line distance between source and candidate.
fn score_line_proximity(src_line: Option<usize>, dst_line: Option<usize>) -> i32 {
    match (src_line, dst_line) {
        (Some(src), Some(dst)) => {
            let distance = src.abs_diff(dst).min(400);
            15 - (distance as i32 / 25)
        }
        _ => 0,
    }
}

/// Check if call namespace matches candidate namespace (suffix match).
pub(crate) fn namespace_matches(call_ns: &[String], candidate_ns: &[String]) -> bool {
    if call_ns.is_empty() || call_ns.len() > candidate_ns.len() {
        return false;
    }

    let offset = candidate_ns.len() - call_ns.len();
    for (idx, segment) in call_ns.iter().enumerate() {
        if !candidate_ns[offset + idx].eq_ignore_ascii_case(segment) {
            return false;
        }
    }

    true
}

/// Checks if two namespaces are equal (case-insensitive).
fn namespace_equals(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.iter()
        .zip(b.iter())
        .all(|(lhs, rhs)| lhs.eq_ignore_ascii_case(rhs))
}

/// Checks if two namespaces share the same final segment.
fn namespace_shares_tail(a: &[String], b: &[String]) -> bool {
    match (a.last(), b.last()) {
        (Some(lhs), Some(rhs)) => lhs.eq_ignore_ascii_case(rhs),
        _ => false,
    }
}
