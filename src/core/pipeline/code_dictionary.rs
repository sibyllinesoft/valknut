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

pub fn suggestion_definition_for_kind(kind: &str) -> CodeDefinition {
    let lowered = kind.to_ascii_lowercase();

    let (code, title, summary) = if lowered.starts_with("eliminate_duplication") {
        (
            "DEDUP",
            "Eliminate Duplication",
            "Consolidate repeated logic to a shared helper before it diverges.",
        )
    } else if lowered.starts_with("extract_method") {
        (
            "XTRMTH",
            "Extract Method",
            "Pull focused helpers from large routines to shrink cognitive load.",
        )
    } else if lowered.starts_with("extract_class") {
        (
            "XTRCLS",
            "Extract Class",
            "Split multi-purpose modules into cohesive components.",
        )
    } else if lowered.starts_with("simplify") && lowered.contains("conditional") {
        (
            "SIMPCND",
            "Simplify Conditionals",
            "Flatten or reorganize complex branching to clarify intent.",
        )
    } else if lowered.starts_with("reduce_cyclomatic_complexity") {
        (
            "RDCYCLEX",
            "Reduce Cyclomatic",
            "Break apart dense branching to keep cyclomatic complexity in check.",
        )
    } else if lowered.starts_with("reduce_cognitive_complexity") {
        (
            "RDCOGN",
            "Reduce Cognitive",
            "Streamline control flow to ease human comprehension.",
        )
    } else if lowered.starts_with("reduce_fan_in") {
        (
            "RDFANIN",
            "Reduce Fan-In",
            "Distribute responsibilities so fewer callers funnel through one hotspot.",
        )
    } else if lowered.starts_with("reduce_fan_out") {
        (
            "RDFANOUT",
            "Reduce Fan-Out",
            "Contain dependencies so modules rely on fewer collaborators.",
        )
    } else if lowered.starts_with("reduce_centrality") {
        (
            "RDCNTRL",
            "Reduce Centrality",
            "Limit chokepoint responsibilities in the dependency graph.",
        )
    } else if lowered.starts_with("reduce_chokepoint") {
        (
            "RDCHOKE",
            "Fix Chokepoint",
            "Split chokepoint modules so change risk is shared.",
        )
    } else if lowered.contains("nested_branching") {
        (
            "RDNEST",
            "Reduce Nesting",
            "Flatten nested branching to keep logic approachable.",
        )
    } else if lowered == "simplify_logic" {
        (
            "SMPLOGIC",
            "Simplify Logic",
            "Clarify logic with smaller expressions or well-named helpers.",
        )
    } else if lowered.contains("split_responsibilities") {
        (
            "SPLRESP",
            "Split Responsibilities",
            "Separate distinct concerns into dedicated units.",
        )
    } else if lowered.contains("move_method") {
        (
            "MOVEMTH",
            "Move Method",
            "Relocate behavior to the module with the right knowledge.",
        )
    } else if lowered.contains("organize_imports") {
        (
            "ORGIMPT",
            "Organize Imports",
            "Tidy imports to highlight the dependencies that matter.",
        )
    } else if lowered.contains("introduce_facade") {
        (
            "FACAD",
            "Introduce Facade",
            "Wrap complex subsystems behind a focused interface.",
        )
    } else if lowered.contains("extract_interface") {
        (
            "XTRIFCE",
            "Extract Interface",
            "Define interfaces to decouple callers from implementations.",
        )
    } else if lowered.contains("inline_temp") {
        (
            "INLTEMP",
            "Inline Temporary",
            "Replace temporary variables with direct expressions to reduce clutter.",
        )
    } else if lowered.contains("rename_class") {
        (
            "RENCLSS",
            "Rename Class",
            "Choose a name that conveys the module's real role.",
        )
    } else if lowered.contains("rename_method") {
        (
            "RENMTHD",
            "Rename Method",
            "Make intentions clear with well-chosen method names.",
        )
    } else if lowered.contains("extract_variable") {
        (
            "XTRVAR",
            "Extract Variable",
            "Introduce named variables to document intent and reuse values.",
        )
    } else if lowered.contains("add_comments") {
        (
            "ADDCMNT",
            "Add Comments",
            "Capture the why behind tricky logic paths.",
        )
    } else if lowered.contains("rename_variable") {
        (
            "RENVAR",
            "Rename Variable",
            "Rename identifiers so they read like documentation.",
        )
    } else if lowered.contains("replace_magic_number") {
        (
            "REPMAG",
            "Replace Magic Number",
            "Bind constants to descriptive names to reveal intent.",
        )
    } else if lowered.contains("format_code") {
        (
            "FMTSTYLE",
            "Format Code",
            "Apply consistent formatting to reduce visual noise.",
        )
    } else if lowered.contains("refactor_code_quality") {
        (
            "REFQLTY",
            "Refactor Code Quality",
            "Invest in broad cleanups to stabilize quality drift.",
        )
    } else {
        let code = sanitize_code(&lowered);
        let title = format!("Refactor {}", title_case(&lowered.replace('_', " ")));
        let summary = "General refactoring action suggested by the analyzer.".to_string();
        return CodeDefinition {
            code,
            title,
            summary,
            category: Some("refactoring".to_string()),
        };
    };

    CodeDefinition {
        code: code.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
        category: Some("refactoring".to_string()),
    }
}

pub fn suggestion_code_for_kind(kind: &str) -> String {
    suggestion_definition_for_kind(kind).code
}

pub fn issue_code_for_category(category: &str) -> String {
    issue_definition_for_category(category).code
}
