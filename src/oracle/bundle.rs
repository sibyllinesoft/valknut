//! Bundle creation logic for the refactoring oracle.
//!
//! This module handles the creation of codebase bundles for AI analysis,
//! including token-budget-aware condensation of analysis results.

use crate::core::errors::{Result, ValknutResultExt};
use crate::core::partitioning::CodeSlice;
use crate::core::pipeline::AnalysisResults;
use crate::core::scoring::Priority;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::helpers::{
    build_refactor_hints, calculate_file_priority, html_escape, is_test_file, normalize_path_for_key,
    truncate_hint, FileCandidate,
};
use super::types::OracleConfig;

/// Token budget for valknut analysis output (70k tokens)
pub const VALKNUT_OUTPUT_TOKEN_BUDGET: usize = 70_000;

/// Directories to skip when walking the project tree
pub const SKIP_DIRS: &[&str] = &[
    "target", "node_modules", "__pycache__", "dist", "build", "coverage", "tmp", "temp",
];

/// Source file extensions to include in codebase bundles
pub const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "cpp", "c", "h", "hpp", "cs", "php",
];

/// Codebook for oracle request/response - reduces token usage by using short codes
/// Include this in prompts so the model knows the mapping
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

/// Bundle builder for creating codebase bundles for AI analysis.
pub struct BundleBuilder<'a> {
    config: &'a OracleConfig,
}

impl<'a> BundleBuilder<'a> {
    /// Create a new bundle builder with the given configuration.
    pub fn new(config: &'a OracleConfig) -> Self {
        Self { config }
    }

    /// Create a codebase bundle with XML file tree structure and debugging.
    pub async fn create_codebase_bundle(
        &self,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Result<String> {
        println!("\n🔍 [ORACLE DEBUG] Starting codebase bundle creation");
        println!("   📁 Project path: {}", project_path.display());
        println!("   📊 Token budget: {} tokens", self.config.max_tokens);

        let mut xml_files = Vec::new();
        let mut total_tokens = 0;
        let mut files_included = 0;
        let mut files_skipped = 0;

        let refactor_hints = build_refactor_hints(analysis_results, project_path);

        // First, find README at root level
        self.include_readme(project_path, &mut xml_files, &mut total_tokens, &mut files_included);

        // Collect and prioritize source files
        let candidate_files = self.collect_candidate_files(project_path)?;

        println!(
            "   📋 Found {} candidate source files",
            candidate_files.len()
        );

        // Add files until we hit token budget
        for candidate in candidate_files {
            if total_tokens + candidate.tokens > self.config.max_tokens {
                files_skipped += 1;
                if files_skipped <= 5 {
                    println!(
                        "   ⏭️  Skipped: {} ({} tokens) - would exceed budget",
                        candidate.path, candidate.tokens
                    );
                }
                continue;
            }

            let key = normalize_path_for_key(&candidate.path);
            let hints = refactor_hints
                .get(&key)
                .map(|h| h.join("; "))
                .unwrap_or_else(|| "none".to_string());
            let hints_truncated = truncate_hint(&hints, 80);
            let tuple_label = format!("({}, {})", candidate.path, hints_truncated);

            xml_files.push(format!(
                "    <file path=\"{}\" tuple=\"{}\" hint=\"{}\" type=\"{}\" tokens=\"{}\" priority=\"{:.2}\">\n{}\n    </file>",
                candidate.path,
                html_escape(&tuple_label),
                html_escape(&hints_truncated),
                candidate.file_type,
                candidate.tokens,
                candidate.priority,
                html_escape(&candidate.content)
            ));

            total_tokens += candidate.tokens;
            files_included += 1;

            println!(
                "   ✅ Included: {} ({} tokens, priority: {:.2})",
                candidate.path, candidate.tokens, candidate.priority
            );
        }

        if files_skipped > 5 {
            println!(
                "   ⏭️  ... and {} more files skipped due to token budget",
                files_skipped - 5
            );
        }

        // Create XML structure
        let xml_bundle = format!(
            "<codebase project_path=\"{}\" files_included=\"{}\" total_tokens=\"{}\">\n{}\n</codebase>",
            project_path.display(),
            files_included,
            total_tokens,
            xml_files.join("\n")
        );

        // Create condensed valknut analysis with token budget
        println!("\n🔍 [ORACLE DEBUG] Creating condensed valknut analysis");
        println!(
            "   📊 Analysis token budget: {} tokens",
            VALKNUT_OUTPUT_TOKEN_BUDGET
        );
        let condensed_analysis =
            condense_analysis_results_with_budget(analysis_results, VALKNUT_OUTPUT_TOKEN_BUDGET)?;

        let final_bundle = format!(
            "# Code Quality Improvement Analysis\n\n\
            {}\n\n\
            ## Codebase ({} files, ~{} tokens)\n{}\n\n\
            ## Valknut Analysis\n{}\n\n\
            ## Task\n\
            Analyze this codebase and propose improvements focused on **code quality**: readability, maintainability, \
            clarity, and simplicity. You are reviewing code that works - the goal is to make it easier to understand, \
            modify, and extend.\n\n\
            ## Focus Areas (in priority order)\n\
            1. **Readability**: Clear naming, logical organization, reduced cognitive load, self-documenting code\n\
            2. **Maintainability**: Reduced coupling, clear module boundaries, consistent patterns\n\
            3. **Simplification**: Remove unnecessary complexity, use standard library or well-known crates instead of hand-rolling\n\
            4. **Error Handling**: Robust error types, clear propagation, actionable error messages, proper Result usage\n\
            5. **Logging/Observability**: Structured logging where useful, clear debug output, tracing for complex flows\n\
            6. **Type Safety**: Leverage the type system to prevent bugs, use newtypes, proper trait bounds\n\n\
            ## Out of Scope\n\
            - NO new services, databases, or infrastructure changes\n\
            - NO large architectural rewrites or new frameworks\n\
            - NO performance optimization unless it also improves clarity\n\
            - NO changes that only benefit hypothetical future requirements\n\n\
            ## Response Format\n\
            Respond with valid JSON only. Use codes from the codebook above.\n\n\
            ```json\n\
            {{\n\
              \"assessment\": {{\n\
                \"summary\": \"<2-3 sentences on overall code quality>\",\n\
                \"strengths\": [\"<strength1>\", \"<strength2>\"],\n\
                \"issues\": [\"<issue1>\", \"<issue2>\"]\n\
              }},\n\
              \"tasks\": [\n\
                {{\n\
                  \"id\": \"T1\",\n\
                  \"title\": \"<concise title>\",\n\
                  \"description\": \"<what to change and why it improves quality>\",\n\
                  \"category\": \"<C1-C7>\",\n\
                  \"files\": [\"<path>\"],\n\
                  \"risk\": \"<R1-R3>\",\n\
                  \"impact\": \"<I1-I3>\",\n\
                  \"effort\": \"<E1-E3>\",\n\
                  \"depends_on\": []\n\
                }}\n\
              ]\n\
            }}\n\
            ```\n\n\
            ## Guidelines\n\
            - Provide 8-15 concrete, actionable tasks\n\
            - Order by dependencies, then by impact/effort ratio\n\
            - Be specific about file paths (must exist in codebase)\n\
            - Each task should have clear before/after improvement\n\
            - Prefer quick wins (E1/E2) with good impact (I2/I3)",
            ORACLE_CODEBOOK,
            files_included,
            total_tokens,
            xml_bundle,
            condensed_analysis
        );

        let final_tokens = final_bundle.len() / 4;
        println!("\n🎯 [ORACLE DEBUG] Bundle creation complete");
        println!("   📦 Final bundle: ~{} tokens", final_tokens);
        println!("   📁 Files included: {}", files_included);
        println!("   ⏭️  Files skipped: {}", files_skipped);

        Ok(final_bundle)
    }

    /// Include README file if present at project root.
    fn include_readme(
        &self,
        project_path: &Path,
        xml_files: &mut Vec<String>,
        total_tokens: &mut usize,
        files_included: &mut usize,
    ) {
        let readme_candidates = ["README.md", "readme.md", "README.txt", "README"];
        for readme_name in &readme_candidates {
            let readme_path = project_path.join(readme_name);
            if readme_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&readme_path) {
                    let estimated_tokens = content.len() / 4;
                    if *total_tokens + estimated_tokens < self.config.max_tokens {
                        let tuple_label = format!("({}, {})", readme_name, "overview");
                        xml_files.push(format!(
                            "    <file path=\"{}\" tuple=\"{}\" type=\"documentation\" tokens=\"{}\">\n{}\n    </file>",
                            readme_name,
                            html_escape(&tuple_label),
                            estimated_tokens,
                            html_escape(&content)
                        ));
                        *total_tokens += estimated_tokens;
                        *files_included += 1;
                        println!(
                            "   ✅ Included README: {} ({} tokens)",
                            readme_name, estimated_tokens
                        );
                        break;
                    }
                }
            }
        }
    }

    /// Collect and prioritize candidate source files.
    fn collect_candidate_files(&self, project_path: &Path) -> Result<Vec<FileCandidate>> {
        let walker = WalkDir::new(project_path)
            .max_depth(4)
            .into_iter()
            .filter_entry(|e| {
                let path = e.path();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();

                !name.starts_with('.') && !SKIP_DIRS.iter().any(|d| name == *d)
            });

        let mut candidate_files = Vec::new();

        for entry in walker {
            let entry = entry.map_generic_err("walking project directory")?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if SOURCE_EXTENSIONS.contains(&ext) {
                        let relative_path = path
                            .strip_prefix(project_path)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string();

                        if is_test_file(&relative_path) {
                            continue;
                        }

                        if let Ok(content) = std::fs::read_to_string(path) {
                            let estimated_tokens = content.len() / 4;
                            let priority = calculate_file_priority(&relative_path, ext, content.len());

                            candidate_files.push(FileCandidate {
                                path: relative_path,
                                content,
                                tokens: estimated_tokens,
                                priority,
                                file_type: ext.to_string(),
                            });
                        }
                    }
                }
            }
        }

        // Sort by priority (higher priority first)
        candidate_files.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(candidate_files)
    }
}

/// Create a bundle for a single slice.
pub fn create_slice_bundle(
    slice: &CodeSlice,
    project_path: &Path,
    analysis_results: &AnalysisResults,
) -> Result<String> {
    let refactor_hints = build_refactor_hints(analysis_results, project_path);
    let mut xml_files = Vec::new();
    let mut total_tokens = 0;

    for (path, content) in &slice.contents {
        let estimated_tokens = content.len() / 4;
        let path_str = path.to_string_lossy();

        let key = normalize_path_for_key(&path_str);
        let hints = refactor_hints
            .get(&key)
            .map(|h| h.join("; "))
            .unwrap_or_else(|| "none".to_string());
        let hints_truncated = truncate_hint(&hints, 80);
        let tuple_label = format!("({}, {})", path_str, hints_truncated);

        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        xml_files.push(format!(
            "    <file path=\"{}\" tuple=\"{}\" hint=\"{}\" type=\"{}\" tokens=\"{}\">\n{}\n    </file>",
            path_str,
            html_escape(&tuple_label),
            html_escape(&hints_truncated),
            ext,
            estimated_tokens,
            html_escape(content)
        ));

        total_tokens += estimated_tokens;
    }

    let slice_name = slice
        .primary_module
        .clone()
        .unwrap_or_else(|| format!("slice_{}", slice.id));

    let xml_bundle = format!(
        "<codebase_slice id=\"{}\" name=\"{}\" files=\"{}\" tokens=\"{}\">\n{}\n</codebase_slice>",
        slice.id,
        slice_name,
        slice.files.len(),
        total_tokens,
        xml_files.join("\n")
    );

    // Create condensed analysis for files in this slice
    let slice_analysis = condense_analysis_for_slice(analysis_results, slice)?;

    let bundle = format!(
        "# Slice Analysis Request\n\n\
        ## Code Slice: {} ({} files, ~{} tokens)\n{}\n\n\
        ## Relevant Analysis\n{}\n\n\
        ## Task Instructions\n\
        Analyze this code slice and identify architectural improvements specific to this module/area.\n\n\
        Focus on:\n\
        1. Internal cohesion and organization within this slice\n\
        2. Patterns that could be introduced or improved\n\
        3. Abstraction opportunities\n\
        4. Code quality issues specific to these files\n\n\
        Note: This is a SLICE of a larger codebase. Focus on improvements within this slice's scope.\n\n\
        {}",
        slice_name,
        slice.files.len(),
        total_tokens,
        xml_bundle,
        slice_analysis,
        get_json_schema_instructions()
    );

    Ok(bundle)
}

/// Condense analysis results for files in a specific slice.
pub fn condense_analysis_for_slice(
    results: &AnalysisResults,
    slice: &CodeSlice,
) -> Result<String> {
    let slice_files: HashSet<_> = slice
        .files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // Filter refactoring candidates to those in this slice
    let relevant_candidates: Vec<_> = results
        .refactoring_candidates
        .iter()
        .filter(|c| {
            let file_path = PathBuf::from(&c.file_path);
            let relative = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            slice_files.contains(&c.file_path) || slice_files.iter().any(|sf| sf.ends_with(&relative))
        })
        .take(10)
        .collect();

    let mut condensed = format!(
        "Files in slice: {}\n\
        Relevant refactoring candidates: {}\n\n",
        slice.files.len(),
        relevant_candidates.len()
    );

    for (i, candidate) in relevant_candidates.iter().enumerate() {
        condensed.push_str(&format!(
            "{}. {} ({:?})\n   File: {}\n   Score: {:.1}\n\n",
            i + 1,
            candidate.name.split(':').last().unwrap_or(&candidate.name),
            candidate.priority,
            candidate.file_path,
            candidate.score
        ));
    }

    Ok(condensed)
}

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

    // Collect which issue/suggestion codes are actually used
    let mut used_issue_codes: HashSet<String> = HashSet::new();
    let mut used_suggestion_codes: HashSet<String> = HashSet::new();

    for candidate in results
        .refactoring_candidates
        .iter()
        .filter(|c| !matches!(c.priority, Priority::None))
        .take(15)
    {
        for issue in &candidate.issues {
            used_issue_codes.insert(issue.code.clone());
        }
        for suggestion in candidate.suggestions.iter().take(2) {
            used_suggestion_codes.insert(suggestion.code.clone());
        }
    }

    // Build codebook section with only used codes
    let mut codebook = String::from("## Codebook\n");

    if !used_issue_codes.is_empty() {
        codebook.push_str("ISS:\n");
        for code in &used_issue_codes {
            if let Some(def) = results.code_dictionary.issues.get(code) {
                codebook.push_str(&format!("  {}: {}\n", code, def.title));
            }
        }
    }

    if !used_suggestion_codes.is_empty() {
        codebook.push_str("SUG:\n");
        for code in &used_suggestion_codes {
            if let Some(def) = results.code_dictionary.suggestions.get(code) {
                codebook.push_str(&format!("  {}: {}\n", code, def.title));
            }
        }
    }
    codebook.push('\n');

    // Start with codebook + essential summary (compact format)
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

    // Add top refactoring candidates in compact format
    if !results.refactoring_candidates.is_empty() {
        condensed.push_str("## Candidates\n");
        current_tokens += 15;

        for (i, candidate) in results
            .refactoring_candidates
            .iter()
            .filter(|c| !matches!(c.priority, Priority::None))
            .take(15)
            .enumerate()
        {
            // Compact format: entity|file|score|priority|issues|suggestions
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

            let priority_code = match candidate.priority {
                Priority::Critical => "CRIT",
                Priority::High => "HIGH",
                Priority::Medium => "MED",
                Priority::Low => "LOW",
                Priority::None => "NONE",
            };

            let candidate_text = format!(
                "{}. {}|{}|{:.0}|{}|[{}]|[{}]\n",
                i + 1,
                candidate.name.split(':').last().unwrap_or(&candidate.name),
                candidate.file_path,
                candidate.score,
                priority_code,
                issues_compact,
                suggestions_compact
            );

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
