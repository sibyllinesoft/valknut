//! Helper utilities for working with AST data across detectors.
//!
//! These functions provide shared logic for mapping `CodeEntity` metadata onto
//! concrete tree-sitter nodes so that detectors can perform structural analysis
//! without reimplementing the same boilerplate.

use std::borrow::ToOwned;

use crate::core::ast_service::AstContext;
use crate::core::featureset::CodeEntity;
use tree_sitter::Node;

/// Extract the byte range associated with an entity.
///
/// The range can be stored in different metadata keys depending on which
/// component created the entity (`start_byte`/`end_byte` or `byte_range`).
/// This helper normalises those representations.
pub fn entity_byte_range(entity: &CodeEntity) -> Option<(usize, usize)> {
    // Preferred explicit start/end byte metadata
    let start = entity
        .properties
        .get("start_byte")
        .and_then(|value| value.as_u64())
        .map(|value| value as usize);
    let end = entity
        .properties
        .get("end_byte")
        .and_then(|value| value.as_u64())
        .map(|value| value as usize);

    match (start, end) {
        (Some(start), Some(end)) => return Some((start, end)),
        _ => {}
    }

    // Fallback to combined byte_range array metadata
    entity
        .properties
        .get("byte_range")
        .and_then(|value| value.as_array())
        .and_then(|range| {
            if range.len() == 2 {
                let start = range[0].as_u64()? as usize;
                let end = range[1].as_u64()? as usize;
                Some((start, end))
            } else {
                None
            }
        })
}

/// Retrieve the recorded AST node kind for an entity, if present.
pub fn entity_ast_kind(entity: &CodeEntity) -> Option<String> {
    entity
        .properties
        .get("ast_kind")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            entity
                .properties
                .get("node_kind")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
}

/// Check if a node is completely outside the target byte range.
fn node_disjoint_from_range(node: &Node, start_byte: usize, end_byte: usize) -> bool {
    node.start_byte() > end_byte || node.end_byte() < start_byte
}

/// Check if a node fully contains the target byte range.
fn node_contains_range(node: &Node, start_byte: usize, end_byte: usize) -> bool {
    start_byte >= node.start_byte() && end_byte <= node.end_byte()
}

/// Check if a node matches the criteria to become a candidate.
fn is_candidate_node(node: &Node, target_kind: Option<&str>, start_byte: usize, end_byte: usize) -> bool {
    let matches_kind = target_kind.map_or(false, |expected| node.kind() == expected);
    let matches_exact_range = node.start_byte() == start_byte && node.end_byte() == end_byte;
    matches_kind || matches_exact_range
}

/// Collect children of a node that overlap with the target range.
fn overlapping_children<'a>(node: Node<'a>, start_byte: usize, end_byte: usize) -> Vec<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|child| child.end_byte() >= start_byte && child.start_byte() <= end_byte)
        .collect()
}

/// Locate the tree-sitter node corresponding to the given entity within the
/// parsed tree provided by the [`AstContext`].
///
/// The search uses the entity's byte range and, when available, the recorded
/// node kind to disambiguate between nested candidates.
pub fn find_entity_node<'a>(context: &'a AstContext<'a>, entity: &CodeEntity) -> Option<Node<'a>> {
    let (start_byte, end_byte) = entity_byte_range(entity)?;
    let target_kind = entity_ast_kind(entity);

    let mut stack = vec![context.tree.root_node()];
    let mut candidate = None;

    while let Some(node) = stack.pop() {
        if node_disjoint_from_range(&node, start_byte, end_byte) {
            continue;
        }
        if !node_contains_range(&node, start_byte, end_byte) {
            continue;
        }

        if is_candidate_node(&node, target_kind.as_deref(), start_byte, end_byte) {
            candidate = Some(node);
        }
        stack.extend(overlapping_children(node, start_byte, end_byte));
    }

    candidate
}

/// Count the number of named AST nodes beneath the supplied node.
pub fn count_named_nodes(node: &Node) -> usize {
    let mut count = 0usize;
    let mut stack = vec![*node];

    while let Some(current) = stack.pop() {
        if current.is_named() {
            count += 1;
        }

        let mut cursor = current.walk();
        for child in current.children(&mut cursor) {
            stack.push(child);
        }
    }

    count
}

/// Count distinct control-flow blocks inside the supplied node.
///
/// This counts constructs that typically delimit logical blocks (functions,
/// classes, and control statements). The heuristic errs on the side of
/// over-counting rather than missing significant structure.
pub fn count_control_blocks(node: &Node) -> usize {
    let mut count = 0usize;
    let mut stack = vec![*node];

    while let Some(current) = stack.pop() {
        let kind = current.kind();
        if matches!(
            kind,
            "function_definition"
                | "function_declaration"
                | "method_definition"
                | "class_definition"
                | "class_declaration"
                | "class_body"
                | "struct_item"
                | "impl_item"
                | "if_statement"
                | "if_expression"
                | "elif_clause"
                | "else_if_clause"
                | "for_statement"
                | "for_expression"
                | "while_statement"
                | "while_expression"
                | "match_statement"
                | "match_expression"
                | "switch_statement"
                | "case_clause"
                | "default_clause"
                | "try_statement"
                | "catch_clause"
                | "block"
        ) {
            count += 1;
        }

        let mut cursor = current.walk();
        for child in current.children(&mut cursor) {
            stack.push(child);
        }
    }

    count.max(1)
}

/// Convenience helper for extracting the UTF-8 source text represented by a
/// node. Returns `None` if the node points outside of the provided source.
pub fn node_text<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    node.utf8_text(source.as_bytes()).ok()
}
