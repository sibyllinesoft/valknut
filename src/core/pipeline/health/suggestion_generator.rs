//! Suggestion generation for refactoring candidates.
//!
//! This module provides functions for generating refactoring suggestions
//! based on detected issues and feature contributions.

use std::collections::HashSet;

use crate::core::pipeline::discovery::code_dictionary::suggestion_code_for_kind;
use crate::core::pipeline::results::result_types::{FeatureContribution, RefactoringIssue, RefactoringSuggestion};

/// Helper struct for suggestion base values computed from issue severity
pub struct SuggestionBase {
    pub priority: f64,
    pub impact: f64,
}

/// Factory methods for [`SuggestionBase`].
impl SuggestionBase {
    /// Creates a suggestion base from a severity score.
    pub fn from_severity(severity: f64) -> Self {
        let severity_factor = (severity / 2.0).clamp(0.1, 1.0);
        Self {
            priority: (0.45 + severity_factor * 0.5).clamp(0.1, 1.0),
            impact: (0.55 + severity_factor * 0.35).min(1.0),
        }
    }
}

/// Generate refactoring suggestions based on issues and entity context
pub fn generate_suggestions(
    issues: &[RefactoringIssue],
    _entity_name: &str,
    _line_range: Option<(usize, usize)>,
) -> Vec<RefactoringSuggestion> {
    if issues.is_empty() {
        return Vec::new();
    }

    let mut suggestions = Vec::new();
    let mut emitted_codes: HashSet<String> = HashSet::new();

    for issue in issues {
        let base = SuggestionBase::from_severity(issue.severity);
        let mut category_emitted = false;

        for feature in &issue.contributing_features {
            if let Some(suggestion) = suggestion_for_feature(feature, &base) {
                if emitted_codes.insert(suggestion.code.clone()) {
                    suggestions.push(suggestion);
                }
                category_emitted = true;
            }
        }

        if !category_emitted {
            let kind = fallback_suggestion_kind(&issue.category, issue.severity);
            let code = suggestion_code_for_kind(kind);
            if emitted_codes.insert(code.clone()) {
                suggestions.push(RefactoringSuggestion {
                    refactoring_type: kind.to_string(),
                    code,
                    priority: base.priority,
                    effort: 0.4,
                    impact: base.impact,
                });
            }
        }
    }

    suggestions
}

/// Generate a suggestion for a specific feature contribution
pub fn suggestion_for_feature(
    feature: &FeatureContribution,
    base: &SuggestionBase,
) -> Option<RefactoringSuggestion> {
    let name = feature.feature_name.to_lowercase();
    let value = feature.value;

    if value <= 0.0 {
        return None;
    }

    let count = value.round().max(1.0) as usize;

    // Feature pattern matching with associated suggestion params
    let (kind_fmt, effort, priority_boost, impact_boost): (&str, f64, f64, f64) =
        if name.contains("duplicate_code_count") {
            ("eliminate_duplication_{}_blocks", 0.65, 0.0, count as f64 * 0.05)
        } else if name.contains("extract_method_count") {
            ("extract_method_{}_helpers", 0.55, 0.0, 0.0)
        } else if name.contains("extract_class_count") {
            ("extract_class_{}_areas", 0.7, 0.0, 0.1)
        } else if name.contains("simplify_conditionals_count") {
            ("simplify_{}_conditionals", 0.45, 0.0, 0.0)
        } else if name.contains("cyclomatic") {
            ("reduce_cyclomatic_complexity_{}", 0.5, 0.1, 0.0)
        } else if name.contains("cognitive") {
            ("reduce_cognitive_complexity_{}", 0.5, 0.1, 0.0)
        } else if name.contains("fan_in") {
            ("reduce_fan_in_{}", 0.6, 0.0, 0.1)
        } else if name.contains("fan_out") {
            ("reduce_fan_out_{}", 0.6, 0.0, 0.1)
        } else if name.contains("centrality") {
            ("reduce_centrality_{}", 0.65, 0.0, 0.15)
        } else if name.contains("choke") {
            ("reduce_chokepoint_{}", 0.65, 0.0, 0.15)
        } else {
            return None;
        };

    let kind = kind_fmt.replace("{}", &count.to_string());
    let code = suggestion_code_for_kind(&kind);

    Some(RefactoringSuggestion {
        refactoring_type: kind,
        code,
        priority: (base.priority + priority_boost).min(1.0),
        effort: effort.clamp(0.1, 1.0),
        impact: (base.impact + impact_boost).min(1.0),
    })
}

/// Get a fallback suggestion kind based on category and severity
pub fn fallback_suggestion_kind(category: &str, severity: f64) -> &'static str {
    let severity_level = if severity >= 2.0 {
        3 // very high / critical
    } else if severity >= 1.5 {
        2 // high
    } else if severity >= 1.0 {
        1 // moderate
    } else {
        0 // low
    };

    match (category, severity_level) {
        ("complexity", 3 | 2) => "extract_method_high_complexity",
        ("complexity", 1) => "reduce_nested_branching",
        ("complexity", _) => "simplify_logic",

        ("structure", 3 | 2) => "extract_class_large_module",
        ("structure", 1) => "move_method_better_cohesion",
        ("structure", _) => "organize_imports",

        ("graph", 3 | 2) => "introduce_facade_decouple_deps",
        ("graph", 1) => "move_method_reduce_coupling",
        ("graph", _) => "inline_temp_simplify_deps",

        ("maintainability", 3 | 2) => "rename_class_improve_clarity",
        ("maintainability", 1) => "extract_variable_clarify_logic",
        ("maintainability", _) => "add_comments_explain_purpose",

        ("readability", 3 | 2) => "extract_method_clarify_intent",
        ("readability", 1) => "replace_magic_number_constant",
        ("readability", _) => "format_code_consistent_style",

        _ => "refactor_code_quality",
    }
}
