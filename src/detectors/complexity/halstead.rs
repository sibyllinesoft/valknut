//! Halstead software science metrics calculation.
//!
//! This module implements Halstead's software science metrics for measuring
//! program complexity based on operators and operands in the source code.

use std::collections::HashSet;

use tracing::debug;

use super::types::HalsteadMetrics;

/// Calculate Halstead metrics for an AST node.
///
/// Walks the AST rooted at `root_node` and counts distinct and total
/// operators and operands to compute the Halstead metrics.
pub fn calculate_halstead_for_node(
    root_node: tree_sitter::Node<'_>,
    source: &str,
) -> HalsteadMetrics {
    let mut operator_set: HashSet<String> = HashSet::new();
    let mut operand_set: HashSet<String> = HashSet::new();
    let mut operator_total = 0.0;
    let mut operand_total = 0.0;

    let source_len = source.len();
    let mut stack = vec![root_node];

    while let Some(node) = stack.pop() {
        // Skip invalid nodes with malformed byte ranges
        let start = node.start_byte();
        let end = node.end_byte();
        if (start as usize) > source_len || (end as usize) > source_len || start > end {
            debug!(
                "Skipping invalid node {} with range {}-{}",
                node.kind(),
                start,
                end
            );
            continue;
        }

        if node.is_named() {
            let kind = node.kind();

            if is_halstead_operator_node(kind) {
                operator_set.insert(kind.to_string());
                operator_total += 1.0;
            } else if is_halstead_operand_node(kind) {
                let operand = operand_representation(&node, source);
                operand_set.insert(operand);
                operand_total += 1.0;
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // Also skip invalid children before pushing to stack
            let child_start = child.start_byte();
            let child_end = child.end_byte();
            if (child_start as usize) <= source_len
                && (child_end as usize) <= source_len
                && child_start <= child_end
            {
                stack.push(child);
            } else {
                debug!(
                    "Skipping invalid child node {} with range {}-{}",
                    child.kind(),
                    child_start,
                    child_end
                );
            }
        }
    }

    compute_halstead_from_counts(
        operator_set.len() as f64,
        operand_set.len() as f64,
        operator_total,
        operand_total,
    )
}

/// Compute Halstead metrics from operator/operand counts.
pub fn compute_halstead_from_counts(n1: f64, n2: f64, n_1: f64, n_2: f64) -> HalsteadMetrics {
    let mut metrics = HalsteadMetrics::default();
    metrics.n1 = n1;
    metrics.n2 = n2;
    metrics.n_1 = n_1;
    metrics.n_2 = n_2;
    metrics.vocabulary = metrics.n1 + metrics.n2;
    metrics.length = metrics.n_1 + metrics.n_2;
    metrics.calculated_length = calculate_halstead_length(metrics.n1, metrics.n2);

    if metrics.vocabulary > 0.0 && metrics.length > 0.0 {
        metrics.volume = metrics.length * metrics.vocabulary.log2();
    }
    if metrics.n2 > 0.0 {
        metrics.difficulty = (metrics.n1 / 2.0) * (metrics.n_2 / metrics.n2.max(1.0));
    }
    metrics.effort = metrics.difficulty * metrics.volume;
    metrics.time = metrics.effort / 18.0;
    metrics.bugs = metrics.volume / 3000.0;

    metrics
}

/// Calculate the theoretical Halstead program length.
pub fn calculate_halstead_length(n1: f64, n2: f64) -> f64 {
    let part1 = if n1 > 0.0 { n1 * n1.log2() } else { 0.0 };
    let part2 = if n2 > 0.0 { n2 * n2.log2() } else { 0.0 };
    part1 + part2
}

/// Check if an AST node kind represents a Halstead operator.
pub fn is_halstead_operator_node(kind: &str) -> bool {
    kind.contains("operator")
        || kind.contains("assignment")
        || kind.ends_with("_expression")
        || kind.ends_with("_statement")
        || kind.ends_with("_clause")
        || matches!(
            kind,
            "if_statement"
                | "else_clause"
                | "elif_clause"
                | "for_statement"
                | "while_statement"
                | "loop_expression"
                | "match_expression"
                | "switch_statement"
                | "case_clause"
                | "default_clause"
                | "return_statement"
                | "break_statement"
                | "continue_statement"
                | "yield_statement"
                | "await_expression"
                | "call_expression"
                | "lambda_expression"
        )
}

/// Check if an AST node kind represents a Halstead operand.
pub fn is_halstead_operand_node(kind: &str) -> bool {
    kind.contains("identifier")
        || kind.ends_with("_name")
        || kind.contains("literal")
        || matches!(
            kind,
            "identifier"
                | "field_identifier"
                | "property_identifier"
                | "type_identifier"
                | "string"
                | "string_literal"
                | "number"
                | "integer"
                | "float"
                | "boolean"
                | "true"
                | "false"
                | "null"
                | "nil"
                | "char_literal"
        )
}

/// Get the string representation of an operand node.
pub fn operand_representation(node: &tree_sitter::Node, source: &str) -> String {
    let start = node.start_byte();
    let end = node.end_byte();

    // Validate bounds before utf8_text to prevent panic
    let source_len = source.len();
    if (start as usize) > source_len || (end as usize) > source_len || start > end {
        debug!(
            "Invalid operand node range: start={}, end={}, source_len={}",
            start, end, source_len
        );
        return node.kind().to_string();
    }

    if let Ok(text) = node.utf8_text(source.as_bytes()) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return format!("{}:{}", node.kind(), trimmed);
        }
    }
    node.kind().to_string()
}
