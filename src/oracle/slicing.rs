//! Slicing logic for the refactoring oracle.
//!
//! This module handles codebase partitioning and sliced analysis for large codebases.

use crate::core::errors::{Result, ValknutError, ValknutResultExt};
use crate::core::partitioning::{CodeSlice, ImportGraphPartitioner, PartitionConfig, PartitionResult};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::bundle::{SKIP_DIRS, SOURCE_EXTENSIONS};
use super::helpers::{is_test_file, task_priority_score};
use super::types::{CodebaseAssessment, OracleConfig, RefactoringOracleResponse, RefactoringTask};
use super::gemini::SliceAnalysisResult;

/// Dry-run mode: show slicing plan without calling the API.
pub fn dry_run(config: &OracleConfig, project_path: &Path) -> Result<()> {
    let files = collect_source_files(project_path)?;
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
        config.slicing_threshold
    );
    println!(
        "   💰 Slice token budget: {}",
        config.slice_token_budget
    );

    if !config.enable_slicing {
        println!("\n⚠️  Slicing is disabled. Would analyze as single bundle.");
        return Ok(());
    }

    if total_tokens <= config.slicing_threshold {
        println!("\n📦 Codebase is under threshold. Would analyze as single bundle.");
        return Ok(());
    }

    println!("\n✂️  Would use sliced analysis. Partitioning codebase...\n");

    // Partition the codebase
    let partition_config = PartitionConfig::default()
        .with_token_budget(config.slice_token_budget);
    let partitioner = ImportGraphPartitioner::new(partition_config);
    let partition_result = partitioner.partition(project_path, &files)?;

    // Print partition statistics
    print_partition_stats(&partition_result);

    // Print each slice
    print_slice_details(&partition_result);

    println!(
        "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    );
    println!(
        "✅ Dry-run complete. {} slices would be sent to the API.",
        partition_result.slices.len()
    );
    println!("   Estimated API calls: {}", partition_result.slices.len());
    println!("   Run with --oracle (without --oracle-dry-run) to execute.");

    Ok(())
}

/// Print partition statistics.
fn print_partition_stats(partition_result: &PartitionResult) {
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
}

/// Print details about each slice.
fn print_slice_details(partition_result: &PartitionResult) {
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
}

/// Collect source files from project.
pub fn collect_source_files(project_path: &Path) -> Result<Vec<PathBuf>> {
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

/// Partition the codebase into slices.
pub fn partition_codebase(
    config: &OracleConfig,
    project_path: &Path,
    files: &[PathBuf],
) -> Result<PartitionResult> {
    let partition_config =
        PartitionConfig::default().with_token_budget(config.slice_token_budget);
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

/// Print information about a slice being analyzed.
pub fn print_slice_info(slice: &CodeSlice, current: usize, total: usize) {
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

/// Get the module prefix for a slice result.
fn get_module_prefix(result: &SliceAnalysisResult) -> String {
    result.primary_module.clone().unwrap_or_else(|| format!("slice_{}", result.slice_id))
}

/// Aggregate assessment data from all slice results.
fn aggregate_assessments(results: &[SliceAnalysisResult]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut summaries = Vec::new();
    let mut strengths = Vec::new();
    let mut issues = Vec::new();

    for result in results {
        let prefix = get_module_prefix(result);
        summaries.push(format!("[{}] {}", prefix, result.response.assessment.get_summary()));

        for s in &result.response.assessment.strengths {
            strengths.push(format!("[{}] {}", prefix, s));
        }
        for i in &result.response.assessment.issues {
            issues.push(format!("[{}] {}", prefix, i));
        }
    }

    (summaries, strengths, issues)
}

/// Aggregate and sort tasks from all slice results.
fn aggregate_tasks(results: &[SliceAnalysisResult]) -> Vec<RefactoringTask> {
    let mut tasks = Vec::new();
    let mut id_counter = 1;

    for result in results {
        let prefix = get_module_prefix(result);
        for task in result.response.all_tasks() {
            tasks.push(RefactoringTask {
                id: format!("T{}", id_counter),
                title: format!("[{}] {}", prefix, task.title),
                depends_on: vec![],
                ..task.clone()
            });
            id_counter += 1;
        }
    }

    tasks.sort_by(|a, b| {
        task_priority_score(b)
            .partial_cmp(&task_priority_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    tasks.truncate(20);
    tasks
}

/// Aggregate results from multiple slices into a single response.
pub fn aggregate_slice_results(
    slice_results: Vec<SliceAnalysisResult>,
    _project_path: &Path,
) -> Result<RefactoringOracleResponse> {
    if slice_results.is_empty() {
        return Err(ValknutError::internal("No slice results to aggregate".to_string()));
    }

    if slice_results.len() == 1 {
        return Ok(slice_results.into_iter().next().unwrap().response);
    }

    let (summaries, strengths, issues) = aggregate_assessments(&slice_results);
    let tasks = aggregate_tasks(&slice_results);

    Ok(RefactoringOracleResponse {
        assessment: CodebaseAssessment {
            summary: Some(format!("Aggregated from {} slices. {}", slice_results.len(), summaries.join(" "))),
            architectural_narrative: None,
            architectural_style: None,
            strengths: strengths.into_iter().take(5).collect(),
            issues: issues.into_iter().take(10).collect(),
        },
        tasks,
        refactoring_roadmap: None,
    })
}
