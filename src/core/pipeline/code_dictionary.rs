use crate::core::pipeline::result_types::CodeDefinition;

/// Ensure codes stay short and alphanumeric while remaining suggestive.
fn sanitize_code(source: &str) -> String {
    let mut code = String::new();
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() {
            code.push(ch.to_ascii_uppercase());
        }
        if code.len() == 8 {
            break;
        }
    }
    if code.is_empty() {
        "GENERIC".to_string()
    } else {
        code
    }
}

fn title_case(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

pub fn issue_definition_for_category(category: &str) -> CodeDefinition {
    let lowered = category.to_ascii_lowercase();

    match lowered.as_str() {
        "complexity" => CodeDefinition {
            code: "CMPLX".to_string(),
            title: "Complexity Hotspot".to_string(),
            summary: "Cyclomatic or cognitive complexity exceeds recommended bounds, increasing maintenance risk.".to_string(),
            category: Some("complexity".to_string()),
        },
        "cognitive" => CodeDefinition {
            code: "COGNIT".to_string(),
            title: "Cognitive Overload".to_string(),
            summary: "Nested control flow and branching overload short-term memory, making the code hard to follow.".to_string(),
            category: Some("cognitive".to_string()),
        },
        "structure" => CodeDefinition {
            code: "STRUCTR".to_string(),
            title: "Structural Imbalance".to_string(),
            summary: "Responsibilities bleed across modules or classes, pointing to poor separation of concerns.".to_string(),
            category: Some("structure".to_string()),
        },
        "graph" => CodeDefinition {
            code: "COUPLNG".to_string(),
            title: "Coupling Risk".to_string(),
            summary: "Dependency graph metrics show chokepoints or excessive fan-in/out that hinder change.".to_string(),
            category: Some("graph".to_string()),
        },
        "style" => CodeDefinition {
            code: "STYLE".to_string(),
            title: "Style Deviation".to_string(),
            summary: "Formatting or naming drift reduces readability and consistency.".to_string(),
            category: Some("style".to_string()),
        },
        "coverage" => CodeDefinition {
            code: "COVGAP".to_string(),
            title: "Coverage Gap".to_string(),
            summary: "Key paths lack automated tests, widening the safety net.".to_string(),
            category: Some("coverage".to_string()),
        },
        "debt" => CodeDefinition {
            code: "TECHDEBT".to_string(),
            title: "Technical Debt".to_string(),
            summary: "Indicators show accruing debt that will require refactoring to sustain velocity.".to_string(),
            category: Some("debt".to_string()),
        },
        "maintainability" => CodeDefinition {
            code: "MAINTAIN".to_string(),
            title: "Maintainability Drift".to_string(),
            summary: "Signals reveal code that resists change or lacks clear ownership.".to_string(),
            category: Some("maintainability".to_string()),
        },
        "readability" => CodeDefinition {
            code: "READABL".to_string(),
            title: "Readability Friction".to_string(),
            summary: "Naming, structure, or style issues slow down comprehension.".to_string(),
            category: Some("readability".to_string()),
        },
        "refactoring" => CodeDefinition {
            code: "REFACTR".to_string(),
            title: "Refactoring Opportunity".to_string(),
            summary: "General refactoring signals indicate room for improvement.".to_string(),
            category: Some("refactoring".to_string()),
        },
        known => {
            let code = sanitize_code(known);
            CodeDefinition {
                code: code.clone(),
                title: format!("{} Issue", title_case(known)),
                summary: format!(
                    "Analysis detected elevated signals in the '{}' category.",
                    known
                ),
                category: Some(known.to_string()),
            }
        }
    }
}

/// Pattern matching mode for suggestion lookups.
enum PatternMatch<'a> {
    StartsWith(&'a str),
    Contains(&'a str),
    StartsAndContains(&'a str, &'a str),
    Exact(&'a str),
}

/// Entry in the suggestion definition lookup table.
struct SuggestionEntry {
    pattern: PatternMatch<'static>,
    code: &'static str,
    title: &'static str,
    summary: &'static str,
}

impl SuggestionEntry {
    fn matches(&self, lowered: &str) -> bool {
        match &self.pattern {
            PatternMatch::StartsWith(prefix) => lowered.starts_with(prefix),
            PatternMatch::Contains(substr) => lowered.contains(substr),
            PatternMatch::StartsAndContains(prefix, substr) => {
                lowered.starts_with(prefix) && lowered.contains(substr)
            }
            PatternMatch::Exact(exact) => lowered == *exact,
        }
    }
}

/// Static lookup table for suggestion definitions.
const SUGGESTION_ENTRIES: &[SuggestionEntry] = &[
    SuggestionEntry { pattern: PatternMatch::StartsWith("eliminate_duplication"), code: "DEDUP", title: "Eliminate Duplication", summary: "Consolidate repeated logic to a shared helper before it diverges." },
    SuggestionEntry { pattern: PatternMatch::StartsWith("extract_method"), code: "XTRMTH", title: "Extract Method", summary: "Pull focused helpers from large routines to shrink cognitive load." },
    SuggestionEntry { pattern: PatternMatch::StartsWith("extract_class"), code: "XTRCLS", title: "Extract Class", summary: "Split multi-purpose modules into cohesive components." },
    SuggestionEntry { pattern: PatternMatch::StartsAndContains("simplify", "conditional"), code: "SIMPCND", title: "Simplify Conditionals", summary: "Flatten or reorganize complex branching to clarify intent." },
    SuggestionEntry { pattern: PatternMatch::StartsWith("reduce_cyclomatic_complexity"), code: "RDCYCLEX", title: "Reduce Cyclomatic", summary: "Break apart dense branching to keep cyclomatic complexity in check." },
    SuggestionEntry { pattern: PatternMatch::StartsWith("reduce_cognitive_complexity"), code: "RDCOGN", title: "Reduce Cognitive", summary: "Streamline control flow to ease human comprehension." },
    SuggestionEntry { pattern: PatternMatch::StartsWith("reduce_fan_in"), code: "RDFANIN", title: "Reduce Fan-In", summary: "Distribute responsibilities so fewer callers funnel through one hotspot." },
    SuggestionEntry { pattern: PatternMatch::StartsWith("reduce_fan_out"), code: "RDFANOUT", title: "Reduce Fan-Out", summary: "Contain dependencies so modules rely on fewer collaborators." },
    SuggestionEntry { pattern: PatternMatch::StartsWith("reduce_centrality"), code: "RDCNTRL", title: "Reduce Centrality", summary: "Limit chokepoint responsibilities in the dependency graph." },
    SuggestionEntry { pattern: PatternMatch::StartsWith("reduce_chokepoint"), code: "RDCHOKE", title: "Fix Chokepoint", summary: "Split chokepoint modules so change risk is shared." },
    SuggestionEntry { pattern: PatternMatch::Contains("nested_branching"), code: "RDNEST", title: "Reduce Nesting", summary: "Flatten nested branching to keep logic approachable." },
    SuggestionEntry { pattern: PatternMatch::Exact("simplify_logic"), code: "SMPLOGIC", title: "Simplify Logic", summary: "Clarify logic with smaller expressions or well-named helpers." },
    SuggestionEntry { pattern: PatternMatch::Contains("split_responsibilities"), code: "SPLRESP", title: "Split Responsibilities", summary: "Separate distinct concerns into dedicated units." },
    SuggestionEntry { pattern: PatternMatch::Contains("move_method"), code: "MOVEMTH", title: "Move Method", summary: "Relocate behavior to the module with the right knowledge." },
    SuggestionEntry { pattern: PatternMatch::Contains("organize_imports"), code: "ORGIMPT", title: "Organize Imports", summary: "Tidy imports to highlight the dependencies that matter." },
    SuggestionEntry { pattern: PatternMatch::Contains("introduce_facade"), code: "FACAD", title: "Introduce Facade", summary: "Wrap complex subsystems behind a focused interface." },
    SuggestionEntry { pattern: PatternMatch::Contains("extract_interface"), code: "XTRIFCE", title: "Extract Interface", summary: "Define interfaces to decouple callers from implementations." },
    SuggestionEntry { pattern: PatternMatch::Contains("inline_temp"), code: "INLTEMP", title: "Inline Temporary", summary: "Replace temporary variables with direct expressions to reduce clutter." },
    SuggestionEntry { pattern: PatternMatch::Contains("rename_class"), code: "RENCLSS", title: "Rename Class", summary: "Choose a name that conveys the module's real role." },
    SuggestionEntry { pattern: PatternMatch::Contains("rename_method"), code: "RENMTHD", title: "Rename Method", summary: "Make intentions clear with well-chosen method names." },
    SuggestionEntry { pattern: PatternMatch::Contains("extract_variable"), code: "XTRVAR", title: "Extract Variable", summary: "Introduce named variables to document intent and reuse values." },
    SuggestionEntry { pattern: PatternMatch::Contains("add_comments"), code: "ADDCMNT", title: "Add Comments", summary: "Capture the why behind tricky logic paths." },
    SuggestionEntry { pattern: PatternMatch::Contains("rename_variable"), code: "RENVAR", title: "Rename Variable", summary: "Rename identifiers so they read like documentation." },
    SuggestionEntry { pattern: PatternMatch::Contains("replace_magic_number"), code: "REPMAG", title: "Replace Magic Number", summary: "Bind constants to descriptive names to reveal intent." },
    SuggestionEntry { pattern: PatternMatch::Contains("format_code"), code: "FMTSTYLE", title: "Format Code", summary: "Apply consistent formatting to reduce visual noise." },
    SuggestionEntry { pattern: PatternMatch::Contains("refactor_code_quality"), code: "REFQLTY", title: "Refactor Code Quality", summary: "Invest in broad cleanups to stabilize quality drift." },
];

pub fn suggestion_definition_for_kind(kind: &str) -> CodeDefinition {
    let lowered = kind.to_ascii_lowercase();

    // Look up in static table
    if let Some(entry) = SUGGESTION_ENTRIES.iter().find(|e| e.matches(&lowered)) {
        return CodeDefinition {
            code: entry.code.to_string(),
            title: entry.title.to_string(),
            summary: entry.summary.to_string(),
            category: Some("refactoring".to_string()),
        };
    }

    // Fallback for unknown kinds
    let code = sanitize_code(&lowered);
    let title = format!("Refactor {}", title_case(&lowered.replace('_', " ")));
    CodeDefinition {
        code,
        title,
        summary: "General refactoring action suggested by the analyzer.".to_string(),
        category: Some("refactoring".to_string()),
    }
}

pub fn suggestion_code_for_kind(kind: &str) -> String {
    suggestion_definition_for_kind(kind).code
}

pub fn issue_code_for_category(category: &str) -> String {
    issue_definition_for_category(category).code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_definition_falls_back_for_unknown_category() {
        let def = issue_definition_for_category("Custom-Signal");
        assert_eq!(def.code, "CUSTOMSI");
        assert_eq!(def.title, "Custom-signal Issue");
        assert!(
            def.summary.contains("custom-signal"),
            "summary should reference the original category"
        );
        assert_eq!(def.category.as_deref(), Some("custom-signal"));

        let generic = issue_definition_for_category("!!!");
        assert_eq!(generic.code, "GENERIC");
        assert!(
            generic.summary.contains("!!!"),
            "original category should appear in summary even when generic"
        );
    }

    #[test]
    fn issue_definition_covers_known_categories() {
        let expectations = [
            ("complexity", "CMPLX"),
            ("cognitive", "COGNIT"),
            ("structure", "STRUCTR"),
            ("graph", "COUPLNG"),
            ("style", "STYLE"),
            ("coverage", "COVGAP"),
            ("debt", "TECHDEBT"),
            ("maintainability", "MAINTAIN"),
            ("readability", "READABL"),
            ("refactoring", "REFACTR"),
        ];

        for (category, code) in expectations {
            let definition = issue_definition_for_category(category);
            assert_eq!(definition.code, code, "unexpected code for {category}");
            assert_eq!(
                definition.category.as_deref(),
                Some(category),
                "category field should echo input"
            );
            assert!(
                !definition.summary.is_empty(),
                "summary should not be empty for {category}"
            );
        }
    }

    #[test]
    fn sanitize_and_title_case_helpers() {
        assert_eq!(sanitize_code("refactor"), "REFACTOR");
        assert_eq!(sanitize_code("???"), "GENERIC");
        assert_eq!(sanitize_code("snake-case"), "SNAKECAS");
        assert_eq!(title_case("refine"), "Refine");
        assert_eq!(title_case(""), "");
    }

    #[test]
    fn suggestion_definition_maps_known_kinds() {
        let extract = suggestion_definition_for_kind("extract_method_for_cleanup");
        assert_eq!(extract.code, "XTRMTH");
        assert_eq!(extract.title, "Extract Method");
        assert_eq!(extract.category.as_deref(), Some("refactoring"));

        let rename = suggestion_definition_for_kind("rename_variable");
        assert_eq!(rename.code, "RENVAR");
        assert!(rename.summary.contains("Rename identifiers"));
    }

    #[test]
    fn suggestion_definition_covers_specialised_actions() {
        let cases = [
            ("eliminate_duplication_block", "DEDUP"),
            ("extract_class_controller", "XTRCLS"),
            ("simplify_nested_conditional_paths", "SIMPCND"),
            ("reduce_cyclomatic_complexity_in_loop", "RDCYCLEX"),
            ("reduce_fan_in_hotspot", "RDFANIN"),
            ("reduce_fan_out_calls", "RDFANOUT"),
            ("reduce_centrality_module", "RDCNTRL"),
            ("reduce_chokepoint_service", "RDCHOKE"),
            ("address_nested_branching_issue", "RDNEST"),
            ("simplify_logic", "SMPLOGIC"),
            ("split_responsibilities_module", "SPLRESP"),
            ("move_method_to_helper", "MOVEMTH"),
            ("organize_imports_cleanup", "ORGIMPT"),
            ("introduce_facade_layer", "FACAD"),
            ("extract_interface_adapter", "XTRIFCE"),
            ("inline_temp_variable", "INLTEMP"),
            ("rename_class_handler", "RENCLSS"),
            ("rename_method_handler", "RENMTHD"),
            ("extract_variable_threshold", "XTRVAR"),
            ("add_comments_for_complex_flow", "ADDCMNT"),
            ("replace_magic_number_pi", "REPMAG"),
            ("format_code_style_update", "FMTSTYLE"),
            ("refactor_code_quality_sweep", "REFQLTY"),
        ];

        for (kind, expected_code) in cases {
            let definition = suggestion_definition_for_kind(kind);
            assert_eq!(
                definition.code, expected_code,
                "unexpected code for suggestion kind {kind}"
            );
            assert_eq!(
                definition.category.as_deref(),
                Some("refactoring"),
                "category should remain 'refactoring'"
            );
            assert!(
                !definition.summary.is_empty(),
                "summary should not be empty for {kind}"
            );
        }
    }

    #[test]
    fn suggestion_definition_handles_unknown_kind() {
        let fallback = suggestion_definition_for_kind("rename@something");
        assert_eq!(fallback.code, "RENAMESO");
        assert_eq!(fallback.title, "Refactor Rename@something");
        assert_eq!(fallback.category.as_deref(), Some("refactoring"));
    }

    #[test]
    fn helpers_return_codes_directly() {
        assert_eq!(issue_code_for_category("complexity"), "CMPLX");
        assert_eq!(
            suggestion_code_for_kind("reduce_cognitive_complexity"),
            "RDCOGN"
        );
    }
}
