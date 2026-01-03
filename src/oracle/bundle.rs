//! Bundle creation logic for the refactoring oracle.
//!
//! This module handles the creation of codebase bundles for AI analysis.

use crate::core::errors::{Result, ValknutResultExt};
use crate::core::partitioning::CodeSlice;
use crate::core::pipeline::AnalysisResults;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::condense::{condense_analysis_results_with_budget, get_json_schema_instructions};
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

/// Bundle builder for creating codebase bundles for AI analysis.
pub struct BundleBuilder<'a> {
    config: &'a OracleConfig,
}

/// Factory and bundle creation methods for [`BundleBuilder`].
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

            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
                continue;
            };
            if !SOURCE_EXTENSIONS.contains(&ext) {
                continue;
            }

            let relative_path = path
                .strip_prefix(project_path)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            if is_test_file(&relative_path) {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };

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

// Re-export condense functions for backward compatibility
pub use super::condense::{condense_analysis_results, ORACLE_CODEBOOK};
