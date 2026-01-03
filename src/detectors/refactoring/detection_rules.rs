//! Refactoring detection rules for identifying code improvement opportunities.
//!
//! This module contains the heuristics for detecting various refactoring opportunities
//! such as long methods, complex conditionals, duplicate code, and large types.

use std::collections::HashMap;

use crate::core::featureset::CodeEntity;
use crate::detectors::complexity::ComplexityMetrics as AnalyzerComplexityMetrics;

use super::{RefactoringRecommendation, RefactoringType};

/// Threshold for marking a function as long
pub const LONG_METHOD_LINE_THRESHOLD: usize = 50;
/// Threshold for marking a class as too large
pub const LARGE_CLASS_LINE_THRESHOLD: usize = 200;
/// Threshold for number of member entities in a class before recommending extraction
pub const LARGE_CLASS_MEMBER_THRESHOLD: usize = 12;
/// Logical operator count that suggests a complex conditional
pub const COMPLEX_CONDITIONAL_THRESHOLD: usize = 4;
/// Minimum lines required to consider a block large enough for duplication checks
pub const DUPLICATE_MIN_LINE_COUNT: usize = 4;
/// Minimum tokens required before we consider a block a meaningful duplication target
pub const DUPLICATE_MIN_TOKEN_COUNT: usize = 10;

/// Detect long methods that should be split into smaller functions.
pub fn detect_long_methods(
    functions: &[CodeEntity],
    entity_complexity_fn: impl Fn(&CodeEntity) -> Option<AnalyzerComplexityMetrics>,
    entity_location_fn: impl Fn(&CodeEntity) -> (usize, usize),
) -> Vec<RefactoringRecommendation> {
    let mut recommendations = Vec::new();

    for function in functions {
        let complexity = entity_complexity_fn(function);
        let loc = complexity
            .as_ref()
            .map(|metrics| metrics.lines_of_code.max(function.line_count() as f64))
            .unwrap_or(function.line_count() as f64);

        if loc < LONG_METHOD_LINE_THRESHOLD as f64 {
            continue;
        }

        let cyclomatic = complexity
            .as_ref()
            .map(|metrics| metrics.cyclomatic_complexity)
            .unwrap_or(0.0);

        let impact = ((loc / 8.0) + (cyclomatic / 2.0)).min(10.0);
        let effort = 4.0 + (loc / 70.0).min(4.0);
        let priority = (impact / effort).max(0.1);
        let loc_display = loc.round() as usize;
        let complexity_note = if cyclomatic > 0.0 {
            format!(" with cyclomatic {:.1}", cyclomatic)
        } else {
            String::new()
        };

        recommendations.push(RefactoringRecommendation {
            refactoring_type: RefactoringType::ExtractMethod,
            description: format!(
                "Function `{}` spans {} lines{}. Extract helper functions to improve cohesion.",
                function.name, loc_display, complexity_note
            ),
            estimated_impact: impact,
            estimated_effort: effort,
            priority_score: priority,
            location: entity_location_fn(function),
        });
    }

    recommendations
}

/// Detect functions with complex conditional logic.
pub fn detect_complex_conditionals(
    functions: &[CodeEntity],
    entity_complexity_fn: impl Fn(&CodeEntity) -> Option<AnalyzerComplexityMetrics>,
    entity_location_fn: impl Fn(&CodeEntity) -> (usize, usize),
) -> Vec<RefactoringRecommendation> {
    let mut recommendations = Vec::new();

    for function in functions {
        let operator_complexity = estimate_logical_operator_complexity(&function.source_code);
        let complexity = entity_complexity_fn(function);
        let (logical_complexity, cognitive_complexity) = match &complexity {
            Some(metrics) => {
                let combined = metrics.decision_points.len().max(operator_complexity);
                let cognitive = if metrics.cognitive_complexity > 0.0 {
                    metrics.cognitive_complexity
                } else {
                    combined as f64
                };
                (combined, cognitive)
            }
            None => (operator_complexity, operator_complexity as f64),
        };

        if logical_complexity < COMPLEX_CONDITIONAL_THRESHOLD {
            continue;
        }

        let impact = (cognitive_complexity * 1.5).min(10.0).max(5.0);
        let effort = 3.5;
        let priority = (impact / effort).max(0.1);

        recommendations.push(RefactoringRecommendation {
            refactoring_type: RefactoringType::SimplifyConditionals,
            description: format!(
                "Function `{}` contains {} decision points (cognitive {:.1}). Consider guard clauses or breaking the logic into smaller helpers.",
                function.name, logical_complexity, cognitive_complexity
            ),
            estimated_impact: impact,
            estimated_effort: effort,
            priority_score: priority,
            location: entity_location_fn(function),
        });
    }

    recommendations
}

/// Detect potential duplicate code blocks.
pub fn detect_duplicate_code(
    functions: &[CodeEntity],
    duplicate_signature_fn: impl Fn(&CodeEntity) -> Option<(u64, usize)>,
    entity_location_fn: impl Fn(&CodeEntity) -> (usize, usize),
) -> Vec<RefactoringRecommendation> {
    let mut buckets: HashMap<u64, Vec<&CodeEntity>> = HashMap::new();

    for function in functions {
        if function.line_count() < DUPLICATE_MIN_LINE_COUNT {
            continue;
        }

        if let Some((fingerprint, complexity)) = duplicate_signature_fn(function) {
            if complexity >= DUPLICATE_MIN_TOKEN_COUNT {
                buckets.entry(fingerprint).or_default().push(function);
            }
        }
    }

    let mut recommendations = Vec::new();

    for duplicates in buckets.values() {
        if duplicates.len() < 2 {
            continue;
        }

        let names: Vec<String> = duplicates.iter().map(|f| f.name.clone()).collect();
        let names_display = names.join(", ");

        for function in duplicates {
            let impact = (function.line_count() as f64 / 8.0).min(10.0).max(6.0);
            let effort = 5.5;
            let priority = (impact / effort).max(0.1);

            recommendations.push(RefactoringRecommendation {
                refactoring_type: RefactoringType::EliminateDuplication,
                description: format!(
                    "Function `{}` shares near-identical implementation with [{}]. Consolidate shared logic into a reusable helper.",
                    function.name, names_display
                ),
                estimated_impact: impact,
                estimated_effort: effort,
                priority_score: priority,
                location: entity_location_fn(function),
            });
        }
    }

    recommendations
}

/// Detect types (classes, structs) that are too large.
pub fn detect_large_types(
    types: &[CodeEntity],
    member_count_fn: impl Fn(&CodeEntity) -> usize,
    entity_location_fn: impl Fn(&CodeEntity) -> (usize, usize),
) -> Vec<RefactoringRecommendation> {
    let mut recommendations = Vec::new();

    for entity in types {
        let line_count = entity.line_count();
        let member_count = member_count_fn(entity);

        if line_count < LARGE_CLASS_LINE_THRESHOLD && member_count < LARGE_CLASS_MEMBER_THRESHOLD {
            continue;
        }

        let impact = ((line_count as f64 / 20.0) + member_count as f64 * 0.5)
            .min(10.0)
            .max(5.0);
        let effort = 7.5;
        let priority = (impact / effort).max(0.1);

        recommendations.push(RefactoringRecommendation {
            refactoring_type: RefactoringType::ExtractClass,
            description: format!(
                "Type `{}` spans {} lines with {} members. Split responsibilities into focused components.",
                entity.name, line_count, member_count
            ),
            estimated_impact: impact,
            estimated_effort: effort,
            priority_score: priority,
            location: entity_location_fn(entity),
        });
    }

    recommendations
}

/// Estimate logical operator complexity from source code.
pub fn estimate_logical_operator_complexity(snippet: &str) -> usize {
    let mut count = 0;

    for line in snippet.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }

        count += trimmed.matches("&&").count();
        count += trimmed.matches("||").count();
    }

    count
        + snippet
            .split(|c: char| !c.is_alphabetic())
            .filter(|token| matches!(token.to_ascii_lowercase().as_str(), "and" | "or"))
            .count()
}
