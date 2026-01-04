//! Analysis result condensation for AI consumption.
//!
//! This module provides utilities for condensing valknut analysis results
//! into compact formats suitable for AI model input.

use std::collections::HashSet;

use crate::core::errors::Result;
use crate::core::pipeline::AnalysisResults;
use crate::core::scoring::Priority;

/// Codebook for oracle request/response - reduces token usage by using short codes.
/// Include this in prompts so the model knows the mapping.
pub const ORACLE_CODEBOOK: &str = r#"## Codebook (use these codes in your response)

### Severity/Priority (SEV)
- S1: critical - Must fix immediately, blocking issues
- S2: high - Important, should address soon
- S3: medium - Address when convenient
- S4: low - Nice to have, minor improvement

### Task Categories (CAT)
- C1: readability - Improving code clarity, naming, structure
- C2: maintainability - Reducing coupling, improving modularity
- C3: error-handling - Better error types, propagation, recovery
- C4: logging - Structured logging, observability improvements
- C5: simplification - Removing complexity, using stdlib/libraries
- C6: cleanup - Dead code, deprecated patterns, consolidation
- C7: typing - Type safety, generics, trait bounds

### Risk Level (RISK)
- R1: low - Safe, localized change
- R2: medium - Some coordination needed, moderate scope
- R3: high - Significant change, needs careful review

### Impact (IMP)
- I1: low - Minor improvement
- I2: medium - Noticeable improvement
- I3: high - Significant improvement

### Effort (EFF)
- E1: low - Quick fix, < 1 hour
- E2: medium - Few hours to a day
- E3: high - Multiple days
"#;

/// Condense valknut analysis results for AI consumption.
pub fn condense_analysis_results(results: &AnalysisResults) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "health_score": results.summary.code_health_score,
        "total_issues": results.summary.refactoring_needed,
        "high_priority": results.summary.high_priority,
        "critical": results.summary.critical,
        "files_analyzed": results.summary.files_processed,
        "entities_analyzed": results.summary.entities_analyzed,
        "avg_refactoring_score": results.summary.avg_refactoring_score,
        "code_dictionary": results.code_dictionary.clone(),
        "top_refactoring_candidates": results.refactoring_candidates.iter()
            .take(10)
            .map(|c| serde_json::json!({
                "file": c.file_path,
                "entity": c.name,
                "score": c.score,
                "issue_codes": c.issues.iter().map(|issue| &issue.code).collect::<Vec<_>>(),
                "suggestion_codes": c.suggestions.iter().map(|s| &s.code).collect::<Vec<_>>(),
                "issues": c.issues,
                "suggestions": c.suggestions
            }))
            .collect::<Vec<_>>(),
        "coverage": if !results.coverage_packs.is_empty() {
            Some(serde_json::json!({
                "files_with_coverage": results.coverage_packs.len(),
                "total_gaps": results.coverage_packs.iter()
                    .map(|p| p.gaps.len())
                    .sum::<usize>()
            }))
        } else { None }
    }))
    .unwrap_or_else(|_| "Failed to serialize analysis".to_string())
}

/// Condense analysis results with a specific token budget.
/// Uses a codebook approach: define codes once, then reference them compactly.
pub fn condense_analysis_results_with_budget(
    results: &AnalysisResults,
    token_budget: usize,
) -> Result<String> {
    println!(
        "   🔄 Condensing valknut analysis with {} token budget",
        token_budget
    );

    let top_candidates: Vec<_> = results
        .refactoring_candidates
        .iter()
        .filter(|c| !matches!(c.priority, Priority::None))
        .take(15)
        .collect();

    let (used_issue_codes, used_suggestion_codes) = collect_used_codes(&top_candidates);
    let codebook = build_codebook(results, &used_issue_codes, &used_suggestion_codes);

    let mut condensed = format!(
        "{}\
        ## Metrics\n\
        health={:.2} files={} entities={} issues={} high={} crit={} avg_score={:.2}\n\n",
        codebook,
        results.summary.code_health_score,
        results.summary.files_processed,
        results.summary.entities_analyzed,
        results.summary.refactoring_needed,
        results.summary.high_priority,
        results.summary.critical,
        results.summary.avg_refactoring_score
    );

    let mut current_tokens = condensed.len() / 4;

    if !top_candidates.is_empty() {
        condensed.push_str("## Candidates\n");
        current_tokens += 15;

        for (i, candidate) in top_candidates.iter().enumerate() {
            let candidate_text = format_candidate_compact(i + 1, candidate);
            let candidate_tokens = candidate_text.len() / 4;

            if current_tokens + candidate_tokens > token_budget {
                println!("   ⏭️  Stopping at candidate {} due to token budget", i + 1);
                break;
            }

            condensed.push_str(&candidate_text);
            current_tokens += candidate_tokens;
        }
    }

    let final_tokens = condensed.len() / 4;
    println!(
        "   ✅ Condensed analysis: {} tokens (budget: {})",
        final_tokens, token_budget
    );

    if final_tokens > token_budget {
        println!(
            "   ⚠️  Warning: Exceeded token budget by {} tokens",
            final_tokens - token_budget
        );
    }

    Ok(condensed)
}

/// Collect issue and suggestion codes used by the given candidates.
fn collect_used_codes(
    candidates: &[&crate::core::pipeline::RefactoringCandidate],
) -> (HashSet<String>, HashSet<String>) {
    let mut issue_codes = HashSet::new();
    let mut suggestion_codes = HashSet::new();

    for candidate in candidates {
        for issue in &candidate.issues {
            issue_codes.insert(issue.code.clone());
        }
        for suggestion in candidate.suggestions.iter().take(2) {
            suggestion_codes.insert(suggestion.code.clone());
        }
    }

    (issue_codes, suggestion_codes)
}

/// Build the codebook section with definitions for used codes.
fn build_codebook(
    results: &AnalysisResults,
    issue_codes: &HashSet<String>,
    suggestion_codes: &HashSet<String>,
) -> String {
    let mut codebook = String::from("## Codebook\n");

    if !issue_codes.is_empty() {
        codebook.push_str("ISS:\n");
        for code in issue_codes {
            if let Some(def) = results.code_dictionary.issues.get(code) {
                codebook.push_str(&format!("  {}: {}\n", code, def.title));
            }
        }
    }

    if !suggestion_codes.is_empty() {
        codebook.push_str("SUG:\n");
        for code in suggestion_codes {
            if let Some(def) = results.code_dictionary.suggestions.get(code) {
                codebook.push_str(&format!("  {}: {}\n", code, def.title));
            }
        }
    }

    codebook.push('\n');
    codebook
}

/// Format a candidate in compact form: index. name|file|score|priority|[issues]|[suggestions]
fn format_candidate_compact(
    index: usize,
    candidate: &crate::core::pipeline::RefactoringCandidate,
) -> String {
    let issues_compact: String = candidate
        .issues
        .iter()
        .map(|issue| format!("{}@{:.0}", issue.code, issue.severity * 100.0))
        .collect::<Vec<_>>()
        .join(",");

    let suggestions_compact: String = candidate
        .suggestions
        .iter()
        .take(2)
        .map(|s| s.code.clone())
        .collect::<Vec<_>>()
        .join(",");

    let priority_code = priority_to_code(&candidate.priority);

    format!(
        "{}. {}|{}|{:.0}|{}|[{}]|[{}]\n",
        index,
        candidate.name.split(':').last().unwrap_or(&candidate.name),
        candidate.file_path,
        candidate.score,
        priority_code,
        issues_compact,
        suggestions_compact
    )
}

/// Convert priority to compact code string.
fn priority_to_code(priority: &Priority) -> &'static str {
    match priority {
        Priority::Critical => "CRIT",
        Priority::High => "HIGH",
        Priority::Medium => "MED",
        Priority::Low => "LOW",
        Priority::None => "NONE",
    }
}

/// Get the JSON schema instructions (shared between bundle types).
pub fn get_json_schema_instructions() -> String {
    format!(
        "{}\n\n\
        ## Focus Areas\n\
        1. Readability: Clear naming, logical organization, self-documenting code\n\
        2. Maintainability: Reduced coupling, clear module boundaries\n\
        3. Simplification: Use stdlib/crates instead of hand-rolling\n\
        4. Error Handling: Robust error types, clear propagation\n\
        5. Logging: Structured logging where useful\n\n\
        ## Out of Scope\n\
        - NO new services or infrastructure\n\
        - NO large architectural rewrites\n\n\
        ## Response Format (JSON only, use codes from codebook)\n\
        ```json\n\
        {{\n\
          \"assessment\": {{\n\
            \"summary\": \"<2-3 sentences on code quality>\",\n\
            \"strengths\": [\"<strength>\"],\n\
            \"issues\": [\"<issue>\"]\n\
          }},\n\
          \"tasks\": [\n\
            {{\n\
              \"id\": \"T1\",\n\
              \"title\": \"<title>\",\n\
              \"description\": \"<what and why>\",\n\
              \"category\": \"<C1-C7>\",\n\
              \"files\": [\"<path>\"],\n\
              \"risk\": \"<R1-R3>\",\n\
              \"impact\": \"<I1-I3>\",\n\
              \"effort\": \"<E1-E3>\",\n\
              \"depends_on\": []\n\
            }}\n\
          ]\n\
        }}\n\
        ```",
        ORACLE_CODEBOOK
    )
}
