//! AI Refactoring Oracle - Gemini integration for intelligent refactoring suggestions
//!
//! This module provides intelligent refactoring suggestions by bundling codebase contents
//! and sending them to Gemini along with valknut analysis results. For large codebases,
//! the oracle partitions the code into coherent slices based on import graphs.
//!
//! Key features:
//! - Import graph-based codebase partitioning for scalability
//! - Token-budget-aware slice generation
//! - Per-slice analysis with result aggregation
//! - Configurable models for different slice sizes

pub mod gemini;
pub mod helpers;
pub mod types;

use crate::core::errors::{Result, ValknutError, ValknutResultExt};
use crate::core::partitioning::{CodeSlice, ImportGraphPartitioner, PartitionConfig, PartitionResult};
use crate::core::pipeline::{AnalysisResults, StageResultsBundle};
use crate::core::scoring::Priority;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// Re-export public types
pub use types::{
    CodebaseAssessment, OracleConfig, RefactoringOracleResponse, RefactoringRoadmap,
    RefactoringTask,
};

// Re-export Gemini types for external use
pub use gemini::{
    GeminiCandidate, GeminiContent, GeminiGenerationConfig, GeminiPart, GeminiRequest,
    GeminiResponse, GeminiResponseContent, GeminiResponsePart, SliceAnalysisResult,
};

// Re-export helper functions and types
pub use helpers::{
    abbreviate_label, build_refactor_hints, calculate_file_priority, html_escape, is_test_file,
    normalize_path_for_key, task_priority_score, truncate_hint, FileCandidate,
};

/// Token budget for valknut analysis output (70k tokens)
const VALKNUT_OUTPUT_TOKEN_BUDGET: usize = 70_000;

/// Directories to skip when walking the project tree
const SKIP_DIRS: &[&str] = &[
    "target", "node_modules", "__pycache__", "dist", "build", "coverage", "tmp", "temp",
];

/// Source file extensions to include in codebase bundles
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "cpp", "c", "h", "hpp", "cs", "php",
];

/// Codebook for oracle request/response - reduces token usage by using short codes
/// Include this in prompts so the model knows the mapping
const ORACLE_CODEBOOK: &str = r#"## Codebook (use these codes in your response)

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

/// AI refactoring oracle that provides intelligent suggestions using Gemini 2.5 Pro
pub struct RefactoringOracle {
    config: OracleConfig,
    client: reqwest::Client,
}

impl RefactoringOracle {
    /// Create a new refactoring oracle with the given configuration
    pub fn new(config: OracleConfig) -> Self {
        let client = reqwest::Client::new();
        Self { config, client }
    }

    /// Dry-run mode: show slicing plan without calling the API
    pub fn dry_run(&self, project_path: &Path) -> Result<()> {
        let files = self.collect_source_files(project_path)?;
        let total_tokens: usize = files
            .iter()
            .filter_map(|f| {
                let full_path = project_path.join(f);
                std::fs::read_to_string(&full_path).ok()
            })
            .map(|content| content.len() / 4)
            .sum();

        println!("\n🔍 [ORACLE DRY-RUN] Codebase Analysis");
        println!("   📁 Total source files: {}", files.len());
        println!("   📊 Estimated tokens: {}", total_tokens);
        println!(
            "   🎯 Slicing threshold: {}",
            self.config.slicing_threshold
        );
        println!(
            "   💰 Slice token budget: {}",
            self.config.slice_token_budget
        );

        if !self.config.enable_slicing {
            println!("\n⚠️  Slicing is disabled. Would analyze as single bundle.");
            return Ok(());
        }

        if total_tokens <= self.config.slicing_threshold {
            println!("\n📦 Codebase is under threshold. Would analyze as single bundle.");
            return Ok(());
        }

        println!("\n✂️  Would use sliced analysis. Partitioning codebase...\n");

        // Partition the codebase
        let partition_config = PartitionConfig::default()
            .with_token_budget(self.config.slice_token_budget);
        let partitioner = ImportGraphPartitioner::new(partition_config);
        let partition_result = partitioner.partition(project_path, &files)?;

        // Print partition statistics
        println!("📊 Partition Statistics:");
        println!("   - Total files: {}", partition_result.stats.total_files);
        println!("   - Total tokens: {}", partition_result.stats.total_tokens);
        println!("   - Slices created: {}", partition_result.stats.slice_count);
        println!("   - SCCs found: {}", partition_result.stats.scc_count);
        println!("   - Largest SCC: {} files", partition_result.stats.largest_scc);
        println!(
            "   - Cross-slice imports: {}",
            partition_result.stats.cross_slice_imports
        );

        if !partition_result.unassigned.is_empty() {
            println!(
                "   - Unassigned files: {}",
                partition_result.unassigned.len()
            );
        }

        // Print each slice
        println!("\n🗂️  Slice Details:\n");
        for slice in &partition_result.slices {
            let module_name = slice
                .primary_module
                .clone()
                .unwrap_or_else(|| format!("slice_{}", slice.id));
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            println!(
                "📦 Slice {} - {} ({} files, ~{} tokens)",
                slice.id,
                module_name,
                slice.files.len(),
                slice.token_count
            );
            println!("   Files:");
            for (i, file) in slice.files.iter().enumerate() {
                let tokens = slice
                    .contents
                    .get(file)
                    .map(|c| c.len() / 4)
                    .unwrap_or(0);
                if i < 20 {
                    println!("     - {} (~{} tokens)", file.display(), tokens);
                } else if i == 20 {
                    println!("     ... and {} more files", slice.files.len() - 20);
                    break;
                }
            }
            if !slice.bridge_dependencies.is_empty() {
                println!("   Bridge dependencies:");
                for dep in slice.bridge_dependencies.iter().take(5) {
                    println!("     - {}", dep.display());
                }
                if slice.bridge_dependencies.len() > 5 {
                    println!(
                        "     ... and {} more",
                        slice.bridge_dependencies.len() - 5
                    );
                }
            }
        }

        println!(
            "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        );
        println!("✅ Dry-run complete. {} slices would be sent to the API.", partition_result.slices.len());
        println!("   Estimated API calls: {}", partition_result.slices.len());
        println!("   Run with --oracle (without --oracle-dry-run) to execute.");

        Ok(())
    }

    /// Generate refactoring suggestions for the given codebase
    pub async fn generate_suggestions(
        &self,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Result<RefactoringOracleResponse> {
        // First, estimate total codebase size to decide on slicing strategy
        let files = self.collect_source_files(project_path)?;
        let total_tokens: usize = files
            .iter()
            .filter_map(|f| std::fs::read_to_string(f).ok())
            .map(|content| content.len() / 4)
            .sum();

        println!("\n🔍 [ORACLE] Codebase analysis");
        println!("   📁 Total files: {}", files.len());
        println!("   📊 Estimated tokens: {}", total_tokens);
        println!(
            "   🎯 Slicing threshold: {}",
            self.config.slicing_threshold
        );

        // Decide whether to use sliced analysis
        if self.config.enable_slicing && total_tokens > self.config.slicing_threshold {
            println!("   ✂️  Using sliced analysis (codebase exceeds threshold)");
            self.generate_suggestions_sliced(project_path, analysis_results, &files)
                .await
        } else {
            println!("   📦 Using single-bundle analysis");
            self.generate_suggestions_single(project_path, analysis_results)
                .await
        }
    }

    /// Generate suggestions using single-bundle approach (for smaller codebases)
    async fn generate_suggestions_single(
        &self,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Result<RefactoringOracleResponse> {
        let bundle = self
            .create_codebase_bundle(project_path, analysis_results)
            .await?;

        self.query_gemini(&bundle, &self.config.model).await
    }

    /// Generate suggestions using sliced analysis (for larger codebases)
    async fn generate_suggestions_sliced(
        &self,
        project_path: &Path,
        analysis_results: &AnalysisResults,
        files: &[PathBuf],
    ) -> Result<RefactoringOracleResponse> {
        let partition_result = self.partition_codebase(project_path, files)?;

        if partition_result.slices.is_empty() {
            return Err(ValknutError::internal(
                "Failed to create any slices from codebase".to_string(),
            ));
        }

        let slice_results = self
            .analyze_all_slices(&partition_result, project_path, analysis_results)
            .await;

        if slice_results.is_empty() {
            return Err(ValknutError::internal(
                "All slice analyses failed".to_string(),
            ));
        }

        println!(
            "\n🔗 [ORACLE] Aggregating {} slice results...",
            slice_results.len()
        );
        self.aggregate_slice_results(slice_results, project_path)
    }

    fn partition_codebase(
        &self,
        project_path: &Path,
        files: &[PathBuf],
    ) -> Result<PartitionResult> {
        let partition_config =
            PartitionConfig::default().with_token_budget(self.config.slice_token_budget);
        let partitioner = ImportGraphPartitioner::new(partition_config);

        println!("\n🔪 [ORACLE] Partitioning codebase...");
        let result = partitioner.partition(project_path, files)?;

        println!("   📊 Partition stats:");
        println!("      - Slices created: {}", result.stats.slice_count);
        println!("      - SCCs found: {}", result.stats.scc_count);
        println!(
            "      - Largest SCC: {} files",
            result.stats.largest_scc
        );

        Ok(result)
    }

    async fn analyze_all_slices(
        &self,
        partition_result: &PartitionResult,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Vec<SliceAnalysisResult> {
        let total_slices = partition_result.slices.len();
        let mut results = Vec::new();

        for (i, slice) in partition_result.slices.iter().enumerate() {
            self.print_slice_info(slice, i + 1, total_slices);

            match self
                .analyze_slice(slice, project_path, analysis_results)
                .await
            {
                Ok(response) => {
                    results.push(SliceAnalysisResult {
                        slice_id: slice.id,
                        primary_module: slice.primary_module.clone(),
                        response,
                    });
                    println!("   ✅ Slice {} complete", i + 1);
                }
                Err(e) => {
                    println!("   ⚠️  Slice {} failed: {}", i + 1, e);
                }
            }
        }

        results
    }

    fn print_slice_info(&self, slice: &CodeSlice, current: usize, total: usize) {
        println!(
            "\n📦 [ORACLE] Analyzing slice {}/{} ({} files, ~{} tokens)",
            current,
            total,
            slice.files.len(),
            slice.token_count
        );
        if let Some(ref module) = slice.primary_module {
            println!("   📂 Primary module: {}", module);
        }
    }

    /// Analyze a single slice
    async fn analyze_slice(
        &self,
        slice: &CodeSlice,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Result<RefactoringOracleResponse> {
        let bundle = self.create_slice_bundle(slice, project_path, analysis_results)?;
        self.query_gemini(&bundle, &self.config.slice_model).await
    }

    /// Create a bundle for a single slice
    fn create_slice_bundle(
        &self,
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
        let slice_analysis = self.condense_analysis_for_slice(analysis_results, slice)?;

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
            self.get_json_schema_instructions()
        );

        Ok(bundle)
    }

    /// Condense analysis results for files in a specific slice
    fn condense_analysis_for_slice(
        &self,
        results: &AnalysisResults,
        slice: &CodeSlice,
    ) -> Result<String> {
        let slice_files: std::collections::HashSet<_> = slice
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

    /// Aggregate results from multiple slices into a single response
    fn aggregate_slice_results(
        &self,
        slice_results: Vec<SliceAnalysisResult>,
        _project_path: &Path,
    ) -> Result<RefactoringOracleResponse> {
        if slice_results.is_empty() {
            return Err(ValknutError::internal("No slice results to aggregate".to_string()));
        }

        // If only one slice, return it directly
        if slice_results.len() == 1 {
            return Ok(slice_results.into_iter().next().unwrap().response);
        }

        // Combine assessments
        let mut all_issues = Vec::new();
        let mut all_strengths = Vec::new();
        let mut summaries = Vec::new();

        for result in &slice_results {
            let module_prefix = result
                .primary_module
                .clone()
                .unwrap_or_else(|| format!("slice_{}", result.slice_id));

            let summary = result.response.assessment.get_summary();
            summaries.push(format!("[{}] {}", module_prefix, summary));

            for strength in &result.response.assessment.strengths {
                all_strengths.push(format!("[{}] {}", module_prefix, strength));
            }

            for issue in &result.response.assessment.issues {
                all_issues.push(format!("[{}] {}", module_prefix, issue));
            }
        }

        // Combine tasks, adding slice context
        let mut all_tasks = Vec::new();
        let mut task_id_counter = 1;

        for result in &slice_results {
            let module_prefix = result
                .primary_module
                .clone()
                .unwrap_or_else(|| format!("slice_{}", result.slice_id));

            for task in result.response.all_tasks() {
                let mut new_task = task.clone();
                new_task.id = format!("T{}", task_id_counter);
                new_task.title = format!("[{}] {}", module_prefix, task.title);
                // Clear depends_on since cross-slice dependencies are complex
                new_task.depends_on = vec![];
                all_tasks.push(new_task);
                task_id_counter += 1;
            }
        }

        // Deduplicate and sort tasks by impact/effort
        all_tasks.sort_by(|a, b| {
            let a_score = task_priority_score(a);
            let b_score = task_priority_score(b);
            b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Limit to top 20 tasks
        all_tasks.truncate(20);

        Ok(RefactoringOracleResponse {
            assessment: CodebaseAssessment {
                summary: Some(format!(
                    "Aggregated from {} slices. {}",
                    slice_results.len(),
                    summaries.join(" ")
                )),
                architectural_narrative: None,
                architectural_style: None,
                strengths: all_strengths.into_iter().take(5).collect(),
                issues: all_issues.into_iter().take(10).collect(),
            },
            tasks: all_tasks,
            refactoring_roadmap: None,
        })
    }

    /// Collect source files from project
    fn collect_source_files(&self, project_path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        let walker = WalkDir::new(project_path)
            .max_depth(6)
            .into_iter()
            .filter_entry(|e| {
                let path = e.path();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();

                !name.starts_with('.') && !SKIP_DIRS.iter().any(|d| name == *d)
            });

        for entry in walker {
            let entry = entry.map_generic_err("walking project directory")?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if SOURCE_EXTENSIONS.contains(&ext) {
                        let relative = path
                            .strip_prefix(project_path)
                            .unwrap_or(path)
                            .to_path_buf();

                        if !is_test_file(&relative.to_string_lossy()) {
                            files.push(relative);
                        }
                    }
                }
            }
        }

        Ok(files)
    }

    /// Get the JSON schema instructions (shared between bundle types)
    fn get_json_schema_instructions(&self) -> String {
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

    /// Create a codebase bundle with XML file tree structure and debugging
    async fn create_codebase_bundle(
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
        let readme_candidates = ["README.md", "readme.md", "README.txt", "README"];
        for readme_name in &readme_candidates {
            let readme_path = project_path.join(readme_name);
            if readme_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&readme_path) {
                    let estimated_tokens = content.len() / 4; // Rough token estimate
                    if total_tokens + estimated_tokens < self.config.max_tokens {
                        let tuple_label = format!("({}, {})", readme_name, "overview");
                        xml_files.push(format!(
                "    <file path=\"{}\" tuple=\"{}\" type=\"documentation\" tokens=\"{}\">\n{}\n    </file>",
                readme_name,
                html_escape(&tuple_label),
                estimated_tokens,
                html_escape(&content)
            ));
                        total_tokens += estimated_tokens;
                        files_included += 1;
                        println!(
                            "   ✅ Included README: {} ({} tokens)",
                            readme_name, estimated_tokens
                        );
                        break;
                    }
                }
            }
        }

        // Walk through project files and collect source files
        let walker = WalkDir::new(project_path)
            .max_depth(4)
            .into_iter()
            .filter_entry(|e| {
                let path = e.path();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();

                // Skip common directories and files we don't want
                !name.starts_with('.') && !SKIP_DIRS.iter().any(|d| name == *d)
            });

        let mut candidate_files = Vec::new();

        // Collect all candidate source files with metadata
        for entry in walker {
            let entry = entry.map_generic_err("walking project directory")?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    // Include main source files
                    if SOURCE_EXTENSIONS.contains(&ext) {
                        let relative_path = path
                            .strip_prefix(project_path)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string();

                        // Skip test files
                        if is_test_file(&relative_path) {
                            continue;
                        }

                        if let Ok(content) = std::fs::read_to_string(path) {
                            let estimated_tokens = content.len() / 4;
                            let priority =
                                calculate_file_priority(&relative_path, ext, content.len());

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

        println!(
            "   📋 Found {} candidate source files",
            candidate_files.len()
        );

        // Sort by priority (higher priority first)
        candidate_files.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Add files until we hit token budget
        for candidate in candidate_files {
            if total_tokens + candidate.tokens > self.config.max_tokens {
                files_skipped += 1;
                if files_skipped <= 5 {
                    // Only log first few skipped files
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
        let condensed_analysis = self
            .condense_analysis_results_with_budget(analysis_results, VALKNUT_OUTPUT_TOKEN_BUDGET)?;

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

    /// Query Gemini API with the bundled content
    async fn query_gemini(&self, content: &str, model: &str) -> Result<RefactoringOracleResponse> {
        let url = format!(
            "{}/{}:generateContent?key={}",
            self.config.api_endpoint, model, self.config.api_key
        );

        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: content.to_string(),
                }],
            }],
            generation_config: GeminiGenerationConfig {
                temperature: 0.2,
                top_k: 40,
                top_p: 0.95,
                max_output_tokens: 32000,
                response_mime_type: "application/json".to_string(),
            },
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_generic_err("sending request to Gemini API")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ValknutError::internal(format!(
                "Gemini API error: {}",
                error_text
            )));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .map_generic_err("parsing Gemini API response")?;

        let response_text = gemini_response
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| ValknutError::internal("No candidates in Gemini response".to_string()))?
            .content
            .parts
            .into_iter()
            .next()
            .ok_or_else(|| ValknutError::internal("No parts in Gemini response".to_string()))?
            .text;

        let oracle_response: RefactoringOracleResponse =
            serde_json::from_str(&response_text).map_json_err("Oracle response")?;

        Ok(oracle_response)
    }

    /// Condense valknut analysis results for AI consumption
    pub fn condense_analysis_results(&self, results: &AnalysisResults) -> String {
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

    /// Condense analysis results with a specific token budget
    /// Uses a codebook approach: define codes once, then reference them compactly
    fn condense_analysis_results_with_budget(
        &self,
        results: &AnalysisResults,
        token_budget: usize,
    ) -> Result<String> {
        println!(
            "   🔄 Condensing valknut analysis with {} token budget",
            token_budget
        );

        // Collect which issue/suggestion codes are actually used
        let mut used_issue_codes: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut used_suggestion_codes: std::collections::HashSet<String> = std::collections::HashSet::new();

        for candidate in results.refactoring_candidates.iter()
            .filter(|c| !matches!(c.priority, crate::core::scoring::Priority::None))
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

            for (i, candidate) in results.refactoring_candidates.iter()
                .filter(|c| !matches!(c.priority, crate::core::scoring::Priority::None))
                .take(15)
                .enumerate()
            {
                // Compact format: entity|file|score|priority|issues|suggestions
                let issues_compact: String = candidate.issues.iter()
                    .map(|issue| format!("{}@{:.0}", issue.code, issue.severity * 100.0))
                    .collect::<Vec<_>>()
                    .join(",");

                let suggestions_compact: String = candidate.suggestions.iter()
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
}


#[cfg(test)]
mod tests;
