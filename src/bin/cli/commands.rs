//! Command Execution Logic and Analysis Operations
//!
//! This module contains the main command execution logic, analysis operations,
//! and progress tracking functionality.

use crate::cli::args::{
    AIFeaturesArgs, AdvancedCloneArgs, AnalysisControlArgs, AnalyzeArgs, CloneDetectionArgs,
    CohesionArgs, CoverageArgs, DocAuditArgs, DocAuditFormat, InitConfigArgs, McpManifestArgs,
    McpStdioArgs, OutputFormat, PerformanceProfile, QualityGateArgs, SurveyVerbosity,
    ValidateConfigArgs,
};
use crate::cli::config_builder::build_valknut_config;
use crate::cli::config_layer::build_layered_valknut_config;
use crate::cli::quality_gates::{
    evaluate_quality_gates_if_enabled, handle_quality_gate_result, quality_status,
};
use crate::cli::reports::is_quiet;
use anyhow::{self, Context};
use chrono;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde_json;
use serde_yaml;
use std::cmp::Ordering;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use tabled::{settings::Style as TableStyle, Table, Tabled};
use tracing::{info, warn};

// Import comprehensive analysis pipeline
use serde::Deserialize;
use valknut_rs::api::config_types::AnalysisConfig as ApiAnalysisConfig;
use valknut_rs::api::engine::ValknutEngine;
use valknut_rs::api::results::{AnalysisResults, RefactoringCandidate};
use valknut_rs::core::config::ReportFormat;
use valknut_rs::core::config::{CoverageConfig, ValknutConfig};
use valknut_rs::core::file_utils::CoverageDiscovery;
use valknut_rs::core::pipeline::{
    AnalysisConfig as PipelineAnalysisConfig, QualityGateConfig, QualityGateResult,
    QualityGateViolation,
};
use valknut_rs::core::scoring::Priority;
use valknut_rs::detectors::structure::StructureConfig;
use valknut_rs::io::reports::ReportGenerator;
use valknut_rs::lang::{extension_is_supported, registered_languages, LanguageStability};
use valknut_rs::doc_audit;
use valknut_rs::oracle::{OracleConfig, RefactoringOracle};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main analyze command implementation with comprehensive analysis pipeline
pub async fn analyze_command(
    args: AnalyzeArgs,
    _survey: bool,
    _survey_verbosity: SurveyVerbosity,
    verbose: bool,
) -> anyhow::Result<()> {
    let quiet_mode = is_quiet(&args);
    let detail_mode = verbose && !quiet_mode;

    if !quiet_mode {
        print_header();
    }

    let valknut_config = build_valknut_config(&args).await?;
    warn_for_unsupported_languages(&valknut_config, quiet_mode);

    let valid_paths = validate_input_paths(&args.paths)?;
    tokio::fs::create_dir_all(&args.out).await?;

    display_pre_analysis_info(&valid_paths, &args, &valknut_config, quiet_mode, detail_mode).await?;

    let analysis_result =
        run_analysis_phase(&valid_paths, valknut_config, &args, quiet_mode, detail_mode).await?;

    let quality_gate_result = evaluate_quality_gates_if_enabled(&analysis_result, &args, quiet_mode)?;

    if !quiet_mode {
        display_comprehensive_results(&analysis_result, detail_mode);
    }

    let oracle_response = run_oracle_if_enabled(&valid_paths, &analysis_result, &args, quiet_mode).await?;

    generate_reports_with_oracle(&analysis_result, &oracle_response, &args).await?;

    handle_quality_gate_result(quality_gate_result, quiet_mode, detail_mode)?;

    if !quiet_mode {
        println!("Analysis completed.");
    }

    Ok(())
}

/// Validate that all input paths exist and return them.
fn validate_input_paths(paths: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut valid_paths = Vec::new();
    for path in paths {
        if path.exists() {
            valid_paths.push(path.clone());
        } else {
            return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
        }
    }
    if valid_paths.is_empty() {
        return Err(anyhow::anyhow!("No valid paths provided"));
    }
    Ok(valid_paths)
}

/// Display pre-analysis information including run overview and coverage preview.
async fn display_pre_analysis_info(
    valid_paths: &[PathBuf],
    args: &AnalyzeArgs,
    config: &ValknutConfig,
    quiet_mode: bool,
    detail_mode: bool,
) -> anyhow::Result<()> {
    if quiet_mode {
        return Ok(());
    }
    print_run_overview(valid_paths, args, config, detail_mode);
    if config.analysis.enable_coverage_analysis {
        preview_coverage_discovery(valid_paths, &config.coverage, detail_mode).await?;
    }
    Ok(())
}

/// Run the analysis phase with appropriate progress display.
async fn run_analysis_phase(
    valid_paths: &[PathBuf],
    config: ValknutConfig,
    args: &AnalyzeArgs,
    quiet_mode: bool,
    detail_mode: bool,
) -> anyhow::Result<AnalysisResults> {
    if !quiet_mode {
        println!("Running analysis...");
        display_enabled_analyses(&config, detail_mode);
    }

    if quiet_mode {
        run_comprehensive_analysis_without_progress(valid_paths, config, args).await
    } else {
        run_comprehensive_analysis_with_progress(valid_paths, config, args).await
    }
}

/// Run Oracle analysis if enabled.
async fn run_oracle_if_enabled(
    valid_paths: &[PathBuf],
    result: &AnalysisResults,
    args: &AnalyzeArgs,
    quiet_mode: bool,
) -> anyhow::Result<Option<valknut_rs::oracle::RefactoringOracleResponse>> {
    if args.ai_features.oracle_dry_run {
        if !quiet_mode {
            println!("{}", "🔍 Oracle Dry-Run: Showing slicing plan...".bright_blue().bold());
        }
        run_oracle_dry_run(valid_paths, args)?;
        return Ok(None);
    }

    if args.ai_features.oracle {
        if !quiet_mode {
            println!("{}", "🧠 Running AI Refactoring Oracle Analysis...".bright_blue().bold());
        }
        return run_oracle_analysis(valid_paths, result, args).await;
    }

    Ok(None)
}

/// Load doc-audit settings from a YAML file.
fn load_doc_audit_config_file(path: &Path) -> anyhow::Result<DocAuditConfigFile> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read doc audit config at {}", path.display()))?;
    serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse doc audit config {}", path.display()))
}

/// Preview coverage file discovery to show what will be analyzed
async fn preview_coverage_discovery(
    paths: &[PathBuf],
    coverage_config: &CoverageConfig,
    detailed: bool,
) -> anyhow::Result<()> {
    // Use all provided roots (deduped) for discovery to mirror pipeline behavior
    let discovered_files = CoverageDiscovery::discover_coverage_for_roots(paths, coverage_config)
        .map_err(|e| anyhow::anyhow!("Coverage discovery failed: {}", e))?;

    if discovered_files.is_empty() {
        println!("Coverage: none found (analysis will skip coverage)");
        if detailed {
            println!("  tip: provide --coverage-file <path> or enable discovery in config");
        }
    } else {
        let mode = if coverage_config.auto_discover {
            "auto-discover"
        } else {
            "manual"
        };
        println!(
            "Coverage: {} file(s) detected ({mode})",
            discovered_files.len()
        );
        if detailed {
            for file in discovered_files.iter().take(3) {
                println!(
                    "  - {} ({:?}, {} KB)",
                    file.path.display(),
                    file.format,
                    file.size / 1024
                );
            }
            if discovered_files.len() > 3 {
                println!("  - ... {} more", discovered_files.len() - 3);
            }
        }
    }

    Ok(())
}

/// Display which analyses are enabled
fn display_enabled_analyses(config: &ValknutConfig, detailed: bool) {
    let mut enabled = Vec::new();
    if config.analysis.enable_scoring {
        enabled.push("scoring");
    }
    if config.analysis.enable_structure_analysis {
        enabled.push("structure");
    }
    if config.analysis.enable_refactoring_analysis {
        enabled.push("refactoring");
    }
    if config.analysis.enable_graph_analysis {
        enabled.push("impact");
    }
    if config.analysis.enable_lsh_analysis {
        enabled.push("clones");
    }
    if config.analysis.enable_coverage_analysis {
        enabled.push("coverage");
    }

    let enabled_summary = if enabled.is_empty() {
        "none".to_string()
    } else {
        enabled.join(", ")
    };

    println!("Analyses: {enabled_summary}");

    if detailed && config.analysis.enable_lsh_analysis {
        let mut notes = Vec::new();
        if config.denoise.enabled {
            notes.push(format!("denoise {:.0}%", config.denoise.similarity * 100.0));
        }
        if config.lsh.verify_with_apted {
            notes.push("apted verification".to_string());
        }
        if !notes.is_empty() {
            println!("  clones: {}", notes.join(", "));
        }
    }

    if detailed && config.analysis.enable_coverage_analysis {
        let mode = if config.coverage.auto_discover {
            "auto-discover"
        } else {
            "manual"
        };
        println!(
            "  coverage: {} patterns, max age {}d, {}",
            config.coverage.file_patterns.len(),
            config.coverage.max_age_days,
            mode
        );
    }
}

/// Display analysis configuration summary (verbose-only)
fn display_analysis_config_summary(config: &ValknutConfig) {
    let max_files = if config.analysis.max_files == 0 {
        "unlimited".to_string()
    } else {
        config.analysis.max_files.to_string()
    };

    println!(
        "Config : confidence {:.0}%, max files {max_files}",
        config.analysis.confidence_threshold * 100.0
    );

    if config.analysis.enable_lsh_analysis && config.denoise.enabled {
        println!(
            "  clones: similarity {:.0}%, blocks {}",
            config.denoise.similarity * 100.0,
            config.denoise.require_blocks
        );
    }
}

/// Print a compact overview of the upcoming run.
fn print_run_overview(
    paths: &[PathBuf],
    args: &AnalyzeArgs,
    config: &ValknutConfig,
    detailed: bool,
) {
    let targets = if paths.is_empty() {
        "-".to_string()
    } else if paths.len() == 1 {
        paths[0].display().to_string()
    } else {
        format!(
            "{} (+{} more)",
            paths[0].display(),
            paths.len().saturating_sub(1)
        )
    };

    let profile = format!("{:?}", args.profile).to_lowercase();
    let coverage = coverage_status(config, args);
    let quality = quality_status(&args.quality_gate);

    println!("Valknut v{VERSION} — analyze");
    println!("Targets : {targets}");
    println!(
        "Output  : {} | Format: {} | Profile: {}",
        args.out.display(),
        format_to_string(&args.format),
        profile
    );
    println!("Coverage: {coverage} | Quality gate: {quality}");

    if detailed {
        display_analysis_config_summary(config);
    }
}

fn coverage_status(config: &ValknutConfig, args: &AnalyzeArgs) -> String {
    if !config.analysis.enable_coverage_analysis {
        return "off".to_string();
    }

    if let Some(file) = &args.coverage.coverage_file {
        return format!("on (file {})", file.display());
    }

    if config.coverage.auto_discover {
        format!(
            "on (auto, max age {}d, {} patterns)",
            config.coverage.max_age_days,
            config.coverage.file_patterns.len()
        )
    } else {
        "on (manual)".to_string()
    }
}

/// Core analysis logic shared by progress and non-progress variants.
async fn run_analysis_core(
    engine: &mut ValknutEngine,
    paths: &[PathBuf],
    mut on_progress: Option<impl FnMut(&str, f64)>,
) -> anyhow::Result<Vec<AnalysisResults>> {
    let mut all_results = Vec::with_capacity(paths.len());

    for (i, path) in paths.iter().enumerate() {
        if let Some(ref mut callback) = on_progress {
            let message = format!("Analyzing {} ({}/{})", path.display(), i + 1, paths.len());
            let percentage = (i as f64) / (paths.len() as f64);
            callback(&message, percentage);
        }

        let result = engine
            .analyze_directory(path)
            .await
            .map_err(|e| anyhow::anyhow!("Analysis failed for {}: {}", path.display(), e))?;

        all_results.push(result);
    }

    Ok(all_results)
}

/// Finalize analysis results by combining if needed.
fn finalize_analysis_results(results: Vec<AnalysisResults>) -> anyhow::Result<AnalysisResults> {
    if results.len() == 1 {
        results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Expected at least one analysis result"))
    } else {
        combine_analysis_results(results)
    }
}

/// Run comprehensive analysis with progress tracking.
async fn run_comprehensive_analysis_with_progress(
    paths: &[PathBuf],
    config: ValknutConfig,
    _args: &AnalyzeArgs,
) -> anyhow::Result<AnalysisResults> {
    let multi_progress = MultiProgress::new();
    let main_progress = multi_progress.add(ProgressBar::new(100));
    if let Ok(style) = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>3}/{len:3} {msg}")
    {
        main_progress.set_style(style.progress_chars("##-"));
    }

    let mut engine = ValknutEngine::new_from_valknut_config(config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create analysis engine: {}", e))?;

    let progress = main_progress.clone();
    let all_results = run_analysis_core(&mut engine, paths, Some(|msg: &str, pct: f64| {
        progress.set_position((pct * 100.0) as u64);
        progress.set_message(msg.to_string());
    }))
    .await?;

    main_progress.finish_with_message("Analysis complete");

    finalize_analysis_results(all_results)
}

/// Run comprehensive analysis without progress tracking.
async fn run_comprehensive_analysis_without_progress(
    paths: &[PathBuf],
    config: ValknutConfig,
    _args: &AnalyzeArgs,
) -> anyhow::Result<AnalysisResults> {
    let mut engine = ValknutEngine::new_from_valknut_config(config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create analysis engine: {}", e))?;

    let all_results = run_analysis_core(&mut engine, paths, None::<fn(&str, f64)>).await?;

    finalize_analysis_results(all_results)
}

/// Combine multiple analysis results into one
fn combine_analysis_results(results: Vec<AnalysisResults>) -> anyhow::Result<AnalysisResults> {
    let mut iter = results.into_iter();
    let mut combined = iter
        .next()
        .ok_or_else(|| anyhow::anyhow!("No analysis results to combine"))?;

    for result in iter {
        combined.merge_in_place(result);
    }

    Ok(combined)
}

/// Build a quality gate violation with common structure.
fn build_violation(
    rule_name: &str,
    description: String,
    current_value: f64,
    threshold: f64,
    severity: &str,
    affected_files: Vec<PathBuf>,
    recommended_actions: Vec<&str>,
) -> QualityGateViolation {
    QualityGateViolation {
        rule_name: rule_name.to_string(),
        description,
        current_value,
        threshold,
        severity: severity.to_string(),
        affected_files,
        recommended_actions: recommended_actions.into_iter().map(String::from).collect(),
    }
}

/// Evaluate quality gates against analysis results
fn evaluate_quality_gates(
    result: &AnalysisResults,
    config: &QualityGateConfig,
    verbose: bool,
) -> anyhow::Result<QualityGateResult> {
    let default_score = (result.summary.code_health_score * 100.0).clamp(0.0, 100.0);

    if !config.enabled {
        let score = result
            .health_metrics
            .as_ref()
            .map(|m| m.overall_health_score)
            .unwrap_or(default_score);

        return Ok(QualityGateResult {
            passed: true,
            violations: Vec::new(),
            overall_score: score,
        });
    }

    let mut violations = Vec::new();
    let high_priority_files = || {
        top_issue_files(
            result,
            |c| matches!(c.priority, Priority::High | Priority::Critical),
            5,
        )
    };

    // Check health metrics if available
    if let Some(metrics) = result.health_metrics.as_ref() {
        check_metric_violations(&mut violations, metrics, config, &high_priority_files);
    } else if verbose {
        println!(
            "{}",
            "⚠️ Quality gate metrics unavailable; skipping maintainability and complexity checks."
                .yellow()
        );
    }

    // Check issue count violations
    check_issue_count_violations(&mut violations, result, config);

    let overall_score = result
        .health_metrics
        .as_ref()
        .map(|m| m.overall_health_score)
        .unwrap_or(default_score)
        .clamp(0.0, 100.0);

    Ok(QualityGateResult {
        passed: violations.is_empty(),
        violations,
        overall_score,
    })
}

/// Check health metric-based violations.
fn check_metric_violations(
    violations: &mut Vec<QualityGateViolation>,
    metrics: &valknut_rs::core::pipeline::HealthMetrics,
    config: &QualityGateConfig,
    high_priority_files: &impl Fn() -> Vec<PathBuf>,
) {
    if metrics.complexity_score > config.max_complexity_score {
        violations.push(build_violation(
            "Complexity Threshold",
            format!(
                "Average complexity score ({:.1}) exceeds configured limit ({:.1})",
                metrics.complexity_score, config.max_complexity_score
            ),
            metrics.complexity_score,
            config.max_complexity_score,
            severity_for_excess(metrics.complexity_score, config.max_complexity_score),
            high_priority_files(),
            vec![
                "Break down the highest complexity functions highlighted above",
                "Introduce guard clauses or helper methods to reduce nesting",
            ],
        ));
    }

    if metrics.technical_debt_ratio > config.max_technical_debt_ratio {
        violations.push(build_violation(
            "Technical Debt Ratio",
            format!(
                "Technical debt ratio ({:.1}%) exceeds maximum allowed ({:.1}%)",
                metrics.technical_debt_ratio, config.max_technical_debt_ratio
            ),
            metrics.technical_debt_ratio,
            config.max_technical_debt_ratio,
            severity_for_excess(metrics.technical_debt_ratio, config.max_technical_debt_ratio),
            high_priority_files(),
            vec![
                "Triage the listed hotspots and schedule debt paydown work",
                "Ensure tests cover recent refactors to prevent regression",
            ],
        ));
    }

    if metrics.maintainability_score < config.min_maintainability_score {
        violations.push(build_violation(
            "Maintainability Score",
            format!(
                "Maintainability score ({:.1}) fell below required minimum ({:.1})",
                metrics.maintainability_score, config.min_maintainability_score
            ),
            metrics.maintainability_score,
            config.min_maintainability_score,
            severity_for_shortfall(metrics.maintainability_score, config.min_maintainability_score),
            high_priority_files(),
            vec![
                "Refactor low-cohesion modules to improve readability",
                "Document intent for complex code paths flagged in the report",
            ],
        ));
    }
}

/// Check issue count violations.
fn check_issue_count_violations(
    violations: &mut Vec<QualityGateViolation>,
    result: &AnalysisResults,
    config: &QualityGateConfig,
) {
    let summary = &result.summary;

    if summary.critical as usize > config.max_critical_issues {
        let affected = top_issue_files(result, |c| matches!(c.priority, Priority::Critical), 5);
        violations.push(build_violation(
            "Critical Issues",
            format!(
                "{} critical issues detected (limit: {})",
                summary.critical, config.max_critical_issues
            ),
            summary.critical as f64,
            config.max_critical_issues as f64,
            severity_for_excess(summary.critical as f64, config.max_critical_issues as f64),
            affected,
            vec![
                "Prioritise fixes for the critical hotspots above",
                "Add regression tests before merging related fixes",
            ],
        ));
    }

    if summary.high_priority as usize > config.max_high_priority_issues {
        let affected = top_issue_files(
            result,
            |c| matches!(c.priority, Priority::High | Priority::Critical),
            5,
        );
        violations.push(build_violation(
            "High Priority Issues",
            format!(
                "{} high-priority issues detected (limit: {})",
                summary.high_priority, config.max_high_priority_issues
            ),
            summary.high_priority as f64,
            config.max_high_priority_issues as f64,
            severity_for_excess(
                summary.high_priority as f64,
                config.max_high_priority_issues as f64,
            ),
            affected,
            vec![
                "Address the highlighted high-priority candidates before release",
                "Break work into smaller refactors to keep velocity high",
            ],
        ));
    }
}

/// Display comprehensive analysis results
fn display_comprehensive_results(result: &AnalysisResults, detailed: bool) {
    println!("Results:");
    display_analysis_summary(result, detailed);
}

/// Display analysis summary
fn display_analysis_summary(result: &AnalysisResults, detailed: bool) {
    let summary = &result.summary;

    println!(
        "  files {} | entities {} | candidates {}",
        summary.files_processed, summary.entities_analyzed, summary.refactoring_needed
    );
    println!(
        "  high priority {} (critical {})",
        summary.high_priority, summary.critical
    );
    println!(
        "  health {:.1}% | avg refactor {:.1}",
        summary.code_health_score * 100.0,
        summary.avg_refactoring_score
    );

    if detailed {
        if let Some(metrics) = result.health_metrics.as_ref() {
            println!(
                "  maintainability {:.1} | debt {:.1}% | complexity {:.1} | structure {:.1}",
                metrics.maintainability_score,
                metrics.technical_debt_ratio,
                metrics.complexity_score,
                metrics.structure_quality_score
            );
        }

        if let Some(clone_analysis) = result.clone_analysis.as_ref() {
            println!(
                "  clones: {} after denoise",
                clone_analysis.candidates_after_denoising
            );
            if let Some(avg_similarity) = clone_analysis.avg_similarity {
                println!("  clone similarity avg {:.2}", avg_similarity);
            }
        }

        let mut hotspots: Vec<&RefactoringCandidate> = result
            .refactoring_candidates
            .iter()
            .filter(|candidate| matches!(candidate.priority, Priority::High | Priority::Critical))
            .collect();

        hotspots.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal))
        });
        hotspots.truncate(3);

        if !hotspots.is_empty() {
            println!("  top hotspots:");
            for candidate in hotspots {
                let file_name = Path::new(&candidate.file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&candidate.file_path);

                println!(
                    "    - {} ({}) score {:.1} @ {}",
                    candidate.name,
                    priority_label(candidate.priority),
                    candidate.score,
                    file_name
                );
            }
        }

        if !result.warnings.is_empty() {
            println!("  warnings:");
            for warning in &result.warnings {
                println!("    - {}", warning);
            }
        }
    }
}

/// Display quality gate failures and recommended remediation steps.
fn display_quality_failures(result: &QualityGateResult, detailed: bool) {
    for violation in &result.violations {
        println!(
            "  {} - {} (current: {:.1}, threshold: {:.1})",
            violation.rule_name,
            violation.description,
            violation.current_value,
            violation.threshold
        );

        if detailed && !violation.recommended_actions.is_empty() {
            println!("     actions:");
            for action in &violation.recommended_actions {
                println!("       - {}", action);
            }
        }
    }

    if !result.violations.is_empty() {
        println!("  overall quality score: {:.1}/100", result.overall_score);
    }
}

/// Assigns a severity string when a metric exceeds its threshold.
fn severity_for_excess(current: f64, threshold: f64) -> &'static str {
    let delta = current - threshold;
    if threshold == 0.0 {
        if delta >= 5.0 {
            "Critical"
        } else if delta >= 1.0 {
            "High"
        } else {
            "Medium"
        }
    } else if delta >= threshold * 0.5 || delta >= 20.0 {
        "Critical"
    } else if delta >= threshold * 0.25 || delta >= 10.0 {
        "High"
    } else {
        "Medium"
    }
}

/// Assigns a severity string when a metric falls short of its threshold.
fn severity_for_shortfall(current: f64, threshold: f64) -> &'static str {
    let delta = threshold - current;
    if delta >= 20.0 {
        "Critical"
    } else if delta >= 10.0 {
        "High"
    } else {
        "Medium"
    }
}

/// Returns a ranked list of issue file paths filtered by the provided predicate.
fn top_issue_files<F>(result: &AnalysisResults, filter: F, limit: usize) -> Vec<PathBuf>
where
    F: Fn(&RefactoringCandidate) -> bool,
{
    let mut ranked: Vec<_> = result
        .refactoring_candidates
        .iter()
        .filter(|candidate| filter(candidate))
        .map(|candidate| {
            (
                candidate.priority,
                candidate.score,
                PathBuf::from(&candidate.file_path),
            )
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal))
    });

    let mut files = Vec::new();
    for (_, _, path) in ranked {
        if !files.iter().any(|existing| existing == &path) {
            files.push(path);
        }
        if files.len() >= limit {
            break;
        }
    }

    files
}

/// Human-friendly label for a `Priority` value.
fn priority_label(priority: Priority) -> &'static str {
    match priority {
        Priority::None => "none",
        Priority::Low => "low",
        Priority::Medium => "medium",
        Priority::High => "high",
        Priority::Critical => "critical",
    }
}

/// Generate output reports in various formats with optional Oracle results.
/// Helper to write content to a file with consistent error handling.
async fn write_report(path: &std::path::Path, content: &str, format_name: &str) -> anyhow::Result<()> {
    tokio::fs::write(path, content)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to write {} report: {}", format_name, e))
}

/// Generate JSON report content.
fn generate_json_content(result: &AnalysisResults) -> anyhow::Result<String> {
    serde_json::to_string_pretty(result)
        .map_err(|e| anyhow::anyhow!("Failed to serialize JSON: {}", e))
}

/// Generate JSONL report content.
fn generate_jsonl_content(result: &AnalysisResults) -> anyhow::Result<String> {
    serde_json::to_string(result)
        .map_err(|e| anyhow::anyhow!("Failed to serialize JSONL: {}", e))
}

/// Generate YAML report content.
fn generate_yaml_content(result: &AnalysisResults) -> anyhow::Result<String> {
    serde_yaml::to_string(result)
        .map_err(|e| anyhow::anyhow!("Failed to serialize YAML: {}", e))
}

/// Generate markdown report content.
async fn generate_markdown_content(result: &AnalysisResults) -> anyhow::Result<String> {
    let result_json = serde_json::to_value(result)?;
    super::output::generate_markdown_report(&result_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to generate markdown report: {}", e))
}

/// Generate HTML report file.
fn generate_html_file(
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
    file_path: &std::path::Path,
) -> anyhow::Result<()> {
    let default_config = valknut_rs::api::config_types::AnalysisConfig::default();
    let generator = ReportGenerator::new().with_config(default_config);

    match oracle_response {
        Some(oracle) => generator
            .generate_report_with_oracle(result, oracle, file_path, ReportFormat::Html)
            .map_err(|e| anyhow::anyhow!("Failed to generate HTML report with oracle: {}", e)),
        None => generator
            .generate_report(result, file_path, ReportFormat::Html)
            .map_err(|e| anyhow::anyhow!("Failed to generate HTML report: {}", e)),
    }
}

/// Generate SonarQube report content.
async fn generate_sonar_content(result: &AnalysisResults) -> anyhow::Result<String> {
    let result_json = serde_json::to_value(result)?;
    super::output::generate_sonar_report(&result_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to generate SonarQube report: {}", e))
}

/// Generate CSV report content.
async fn generate_csv_content(result: &AnalysisResults) -> anyhow::Result<String> {
    let result_json = serde_json::to_value(result)?;
    super::output::generate_csv_report(&result_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to generate CSV report: {}", e))
}

/// Generate default JSON report with optional oracle data.
fn generate_default_content(
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
) -> anyhow::Result<String> {
    let combined = match oracle_response {
        Some(oracle) => serde_json::json!({
            "oracle_refactoring_plan": oracle,
            "analysis_results": result
        }),
        None => serde_json::to_value(result)
            .map_err(|e| anyhow::anyhow!("Failed to convert analysis to JSON: {}", e))?,
    };
    serde_json::to_string_pretty(&combined)
        .map_err(|e| anyhow::anyhow!("Failed to serialize JSON: {}", e))
}

/// Returns the (filename, format_label) for a given output format.
fn format_file_info(format: &OutputFormat) -> (&'static str, &'static str) {
    match format {
        OutputFormat::Json => ("analysis-results.json", "JSON"),
        OutputFormat::Jsonl => ("analysis-results.jsonl", "JSONL"),
        OutputFormat::Yaml => ("analysis-results.yaml", "YAML"),
        OutputFormat::Markdown => ("team-report.md", "markdown"),
        OutputFormat::Sonar => ("sonarqube-issues.json", "SonarQube"),
        OutputFormat::Csv => ("analysis-data.csv", "CSV"),
        _ => ("analysis-results.json", "JSON"),
    }
}

/// Generates report content for non-HTML formats.
async fn generate_format_content(
    format: &OutputFormat,
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
) -> anyhow::Result<String> {
    match format {
        OutputFormat::Json => generate_json_content(result),
        OutputFormat::Jsonl => generate_jsonl_content(result),
        OutputFormat::Yaml => generate_yaml_content(result),
        OutputFormat::Markdown => generate_markdown_content(result).await,
        OutputFormat::Sonar => generate_sonar_content(result).await,
        OutputFormat::Csv => generate_csv_content(result).await,
        _ => generate_default_content(result, oracle_response),
    }
}

async fn generate_reports_with_oracle(
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
    args: &AnalyzeArgs,
) -> anyhow::Result<()> {
    let quiet_mode = is_quiet(args);
    if !quiet_mode {
        println!("Saving report...");
    }

    let output_file = if args.format == OutputFormat::Html {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let path = args.out.join(format!("report_{}.html", timestamp));
        generate_html_file(result, oracle_response, &path)?;
        path
    } else {
        let (filename, format_label) = format_file_info(&args.format);
        let path = args.out.join(filename);
        let content = generate_format_content(&args.format, result, oracle_response).await?;
        write_report(&path, &content, format_label).await?;
        path
    };

    if !quiet_mode {
        println!("Report: {}", output_file.display());
    }
    Ok(())
}

/// Print default configuration in YAML format
pub async fn print_default_config() -> anyhow::Result<()> {
    println!("{}", "# Default valknut configuration".dimmed());
    println!(
        "{}",
        "# Save this to a file and customize as needed".dimmed()
    );
    println!(
        "{}",
        "# Usage: valknut analyze --config your-config.yml".dimmed()
    );
    println!();

    let config = valknut_rs::core::config::ValknutConfig::default();
    let yaml_output = serde_yaml::to_string(&config)?;
    println!("{}", yaml_output);

    Ok(())
}

/// Initialize a configuration file with defaults
pub async fn init_config(args: InitConfigArgs) -> anyhow::Result<()> {
    // Check if file exists and force not specified
    if args.output.exists() && !args.force {
        return Err(anyhow::anyhow!(
            "Configuration file already exists: {}. Use --force to overwrite or choose a different name with --output",
            args.output.display()
        ));
    }

    let config = valknut_rs::core::config::ValknutConfig::default();
    let yaml_content = serde_yaml::to_string(&config)?;
    tokio::fs::write(&args.output, yaml_content).await?;

    println!(
        "{} {}",
        "✅ Configuration saved to:".bright_green().bold(),
        args.output.display().to_string().cyan()
    );
    println!();
    println!("{}", "📝 Next steps:".bright_blue().bold());
    println!("   1. Edit the configuration file to customize analysis settings");
    println!(
        "   2. Run analysis with: {}",
        format!("valknut analyze --config {} <paths>", args.output.display()).cyan()
    );

    println!();
    println!(
        "{}",
        "🔧 Key settings you can customize:".bright_blue().bold()
    );

    /// Row type for the configuration tips table.
    #[derive(Tabled)]
    struct CustomizationRow {
        setting: String,
        description: String,
    }

    let customization_rows = vec![
        CustomizationRow {
            setting: "denoise.enabled".to_string(),
            description: "Enable intelligent clone detection (default: true)".to_string(),
        },
        CustomizationRow {
            setting: "denoise.auto".to_string(),
            description: "Enable auto-calibration (default: true)".to_string(),
        },
        CustomizationRow {
            setting: "denoise.min_function_tokens".to_string(),
            description: "Minimum function size for analysis (default: 40)".to_string(),
        },
        CustomizationRow {
            setting: "denoise.similarity".to_string(),
            description: "Similarity threshold for clone detection (default: 0.82)".to_string(),
        },
        CustomizationRow {
            setting: "structure.enable_branch_packs".to_string(),
            description: "Enable directory reorganization analysis".to_string(),
        },
        CustomizationRow {
            setting: "structure.enable_file_split_packs".to_string(),
            description: "Enable file splitting recommendations".to_string(),
        },
    ];

    let mut table = Table::new(customization_rows);
    table.with(TableStyle::rounded());
    println!("{}", table);

    Ok(())
}

/// Validate a Valknut configuration file
pub async fn validate_config(args: ValidateConfigArgs) -> anyhow::Result<()> {
    println!(
        "{} {}",
        "🔍 Validating configuration:".bright_blue().bold(),
        args.config.display().to_string().cyan()
    );
    println!();

    let config = match load_configuration(Some(&args.config)).await {
        Ok(config) => {
            println!(
                "{}",
                "✅ Configuration file is valid!".bright_green().bold()
            );
            println!();
            config
        }
        Err(e) => {
            eprintln!("{} {}", "❌ Configuration validation failed:".red(), e);
            println!();
            println!("{}", "🔧 Common issues:".bright_blue().bold());
            println!("   • Check YAML syntax (indentation, colons, quotes)");
            println!("   • Verify all required fields are present");
            println!("   • Ensure numeric values are in valid ranges");
            println!();
            println!(
                "{}",
                "💡 Tip: Use 'valknut print-default-config' to see valid format".dimmed()
            );
            return Err(anyhow::anyhow!("Configuration validation failed: {}", e));
        }
    };

    // Display configuration summary
    display_config_summary(&config);

    if args.verbose {
        println!("{}", "🔧 Detailed Settings".bright_blue().bold());
        println!();

        /// Row used when printing verbose configuration details.
        #[derive(Tabled)]
        struct DetailRow {
            setting: String,
            value: String,
        }

        let detail_rows = vec![
            DetailRow {
                setting: "Branch Packs Enabled".to_string(),
                value: config.enable_branch_packs.to_string(),
            },
            DetailRow {
                setting: "File Split Packs Enabled".to_string(),
                value: config.enable_file_split_packs.to_string(),
            },
            DetailRow {
                setting: "Top Packs Limit".to_string(),
                value: config.top_packs.to_string(),
            },
        ];

        let mut table = Table::new(detail_rows);
        table.with(TableStyle::rounded());
        println!("{}", table);
    }

    println!();
    println!("{}", "💡 Recommendations:".bright_blue().bold());
    println!("   ✅ Configuration looks optimal!");

    Ok(())
}

/// Run MCP server over stdio for IDE integration.
///
/// This command starts a full JSON-RPC 2.0 MCP (Model Context Protocol) server
/// that exposes valknut's code analysis capabilities over stdin/stdout.
///
/// Available MCP tools:
/// - analyze_code: Analyze code for refactoring opportunities and quality metrics
/// - get_refactoring_suggestions: Get specific refactoring suggestions for a code entity
///
/// The server follows the MCP specification and can be used with Claude Code
/// and other MCP-compatible clients.
pub async fn mcp_stdio_command(
    args: McpStdioArgs,
    survey: bool,
    survey_verbosity: SurveyVerbosity,
) -> anyhow::Result<()> {
    use crate::mcp::server::run_mcp_server;

    eprintln!("📡 Starting MCP stdio server for IDE integration...");

    // Load configuration
    let _config = if let Some(config_path) = args.config {
        load_configuration(Some(&config_path)).await?
    } else {
        StructureConfig::default()
    };

    if survey {
        eprintln!("📊 Survey enabled with {:?} verbosity", survey_verbosity);
    } else {
        eprintln!("📊 Survey disabled");
    }

    // Initialize and run MCP server
    eprintln!("🚀 MCP JSON-RPC 2.0 server ready for requests");

    if let Err(e) = run_mcp_server(VERSION).await {
        eprintln!("❌ MCP server error: {}", e);
        return Err(anyhow::anyhow!("MCP server failed: {}", e));
    }

    Ok(())
}

/// Generate MCP manifest JSON.
pub async fn mcp_manifest_command(args: McpManifestArgs) -> anyhow::Result<()> {
    let manifest = serde_json::json!({
        "name": "valknut",
        "version": VERSION,
        "description": "AI-Powered Code Analysis & Refactoring Assistant",
        "author": "Nathan Rice",
        "license": "MIT",
        "homepage": "https://github.com/nathanricedev/valknut",
        "capabilities": {
            "tools": [
                {
                    "name": "analyze_code",
                    "description": "Analyze code for complexity, technical debt, and refactoring opportunities",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string", "description": "Path to code directory or file"},
                            "format": {"type": "string", "enum": ["json", "markdown", "html"], "description": "Output format"}
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "get_refactoring_suggestions",
                    "description": "Get specific refactoring suggestions for code entities",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "entity_id": {"type": "string", "description": "Code entity identifier"},
                            "max_suggestions": {"type": "integer", "description": "Maximum number of suggestions"}
                        },
                        "required": ["entity_id"]
                    }
                },
                {
                    "name": "validate_quality_gates",
                    "description": "Validate code against quality gate thresholds for CI/CD integration",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string", "description": "Path to code directory or file"},
                            "max_complexity": {"type": "number", "description": "Maximum allowed complexity score"},
                            "min_health": {"type": "number", "description": "Minimum required health score"},
                            "max_debt": {"type": "number", "description": "Maximum allowed technical debt ratio"},
                            "max_issues": {"type": "integer", "description": "Maximum allowed number of issues"}
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "analyze_file_quality",
                    "description": "Analyze quality metrics and issues for a specific file",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "file_path": {"type": "string", "description": "Path to the specific file to analyze"},
                            "include_suggestions": {"type": "boolean", "description": "Whether to include refactoring suggestions"}
                        },
                        "required": ["file_path"]
                    }
                }
            ]
        },
        "server": {
            "command": "valknut",
            "args": ["mcp-stdio"]
        }
    });

    let manifest_json = serde_json::to_string_pretty(&manifest)?;

    if let Some(output_path) = args.output {
        tokio::fs::write(&output_path, &manifest_json).await?;
        println!("✅ MCP manifest saved to {}", output_path.display());
    } else {
        println!("{}", manifest_json);
    }

    Ok(())
}

/// List supported programming languages and their status.
pub async fn list_languages() -> anyhow::Result<()> {
    println!(
        "{}",
        "🔤 Supported Programming Languages".bright_blue().bold()
    );
    let languages = registered_languages();
    println!("   Found {} supported languages", languages.len());
    println!();

    /// Table row for language listing output.
    #[derive(Tabled)]
    struct LanguageRow {
        language: String,
        extension: String,
        status: String,
        features: String,
    }

    let rows: Vec<LanguageRow> = languages
        .iter()
        .map(|info| {
            let extensions = info
                .extensions
                .iter()
                .map(|ext| format!(".{}", ext))
                .collect::<Vec<_>>()
                .join(", ");
            let status = match info.status {
                LanguageStability::Stable => "✅ Full Support",
                LanguageStability::Beta => "🚧 Beta",
            };

            LanguageRow {
                language: info.name.to_string(),
                extension: extensions,
                status: status.to_string(),
                features: info.notes.to_string(),
            }
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(TableStyle::rounded());
    println!("{}", table);

    println!();
    println!("{}", "📝 Usage Notes:".bright_blue().bold());
    println!("   • Full Support: Complete feature set with refactoring suggestions");
    println!("   • Beta: Parsing + structure/complexity insights while features mature");
    println!("   • Configure languages in your config file with language-specific settings");
    println!();
    println!(
        "{}",
        "💡 Tip: Use 'valknut init-config' to create a configuration file".dimmed()
    );

    Ok(())
}

#[derive(Debug, Deserialize)]
/// Optional YAML configuration for the standalone doc-audit command.
struct DocAuditConfigFile {
    root: Option<PathBuf>,
    complexity_threshold: Option<usize>,
    max_readme_commits: Option<usize>,
    #[serde(default)]
    ignore_dir: Vec<String>,
    #[serde(default)]
    ignore_suffix: Vec<String>,
    #[serde(default)]
    ignore: Vec<String>,
}

/// Run the standalone documentation audit command.
pub fn doc_audit_command(args: DocAuditArgs) -> anyhow::Result<()> {
    let file_config = find_doc_audit_config_file(&args.config)?;
    let root_path = resolve_doc_audit_root(&args.root, file_config.as_ref())?;

    let mut config = doc_audit::DocAuditConfig::new(root_path);

    if let Some(file_cfg) = file_config {
        apply_file_config_to_doc_audit(&mut config, file_cfg);
    }

    config.complexity_threshold = args.complexity_threshold;
    config.max_readme_commits = args.max_readme_commits;
    apply_cli_ignores_to_doc_audit(&mut config, &args.ignore_dir, &args.ignore_suffix, &args.ignore);

    let result = doc_audit::run_audit(&config)?;
    render_doc_audit_output(&result, &args.format)?;

    if args.strict && result.has_issues() {
        anyhow::bail!("Documentation audit found issues");
    }

    Ok(())
}

/// Find and load doc audit config file from explicit path or implicit locations.
fn find_doc_audit_config_file(
    explicit_path: &Option<PathBuf>,
) -> anyhow::Result<Option<DocAuditConfigFile>> {
    let implicit_config = [".valknut.docaudit.yml", ".valknut.docaudit.yaml"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists());

    match explicit_path.clone().or(implicit_config) {
        Some(path) => Ok(Some(load_doc_audit_config_file(&path)?)),
        None => Ok(None),
    }
}

/// Resolve and validate the doc audit root path.
fn resolve_doc_audit_root(
    cli_root: &Path,
    file_config: Option<&DocAuditConfigFile>,
) -> anyhow::Result<PathBuf> {
    let root_override = if cli_root != Path::new(".") {
        cli_root.to_path_buf()
    } else {
        file_config
            .and_then(|c| c.root.clone())
            .unwrap_or_else(|| cli_root.to_path_buf())
    };

    if !root_override.exists() {
        return Err(anyhow::anyhow!(
            "Audit root does not exist: {}",
            root_override.display()
        ));
    }

    let root_path = std::fs::canonicalize(&root_override).map_err(|err| {
        anyhow::anyhow!(
            "Failed to resolve audit root {}: {}",
            root_override.display(),
            err
        )
    })?;

    if !root_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Audit root must be a directory: {}",
            root_path.display()
        ));
    }

    Ok(root_path)
}

/// Apply file config settings to doc audit config.
fn apply_file_config_to_doc_audit(config: &mut doc_audit::DocAuditConfig, file_cfg: DocAuditConfigFile) {
    if let Some(threshold) = file_cfg.complexity_threshold {
        config.complexity_threshold = threshold;
    }
    if let Some(commits) = file_cfg.max_readme_commits {
        config.max_readme_commits = commits;
    }
    extend_ignore_set(&mut config.ignore_dirs, file_cfg.ignore_dir);
    extend_ignore_set(&mut config.ignore_suffixes, file_cfg.ignore_suffix);
    extend_ignore_vec(&mut config.ignore_globs, file_cfg.ignore);
}

/// Apply CLI ignore arguments to doc audit config.
fn apply_cli_ignores_to_doc_audit(
    config: &mut doc_audit::DocAuditConfig,
    ignore_dir: &[String],
    ignore_suffix: &[String],
    ignore: &[String],
) {
    extend_ignore_set(&mut config.ignore_dirs, ignore_dir.to_vec());
    extend_ignore_set(&mut config.ignore_suffixes, ignore_suffix.to_vec());
    extend_ignore_vec(&mut config.ignore_globs, ignore.to_vec());
}

/// Extend a HashSet with non-empty trimmed strings.
fn extend_ignore_set(set: &mut std::collections::HashSet<String>, items: Vec<String>) {
    for item in items {
        if !item.trim().is_empty() {
            set.insert(item);
        }
    }
}

/// Extend a Vec with non-empty trimmed strings.
fn extend_ignore_vec(vec: &mut Vec<String>, items: Vec<String>) {
    for item in items {
        if !item.trim().is_empty() {
            vec.push(item);
        }
    }
}

/// Render doc audit output in the requested format.
fn render_doc_audit_output(result: &doc_audit::AuditResult, format: &DocAuditFormat) -> anyhow::Result<()> {
    match format {
        DocAuditFormat::Text => println!("{}", doc_audit::render_text(result)),
        DocAuditFormat::Json => println!("{}", doc_audit::render_json(result)?),
    }
    Ok(())
}

/// Print Valknut header with version info
pub fn print_header() {
    println!("Valknut v{VERSION}");
}

/// Build the stylized header lines to fit the given terminal width.
fn header_lines_for_width(width: u16) -> Vec<String> {
    let _ = width; // width retained for test call signature
    vec![format!("Valknut v{VERSION}")]
}

/// Display configuration summary in a formatted table
pub fn display_config_summary(config: &StructureConfig) {
    #[derive(Tabled)]
    /// Single row shown in the configuration summary table.
    struct ConfigRow {
        setting: String,
        value: String,
    }

    let config_rows = vec![
        ConfigRow {
            setting: "Languages".to_string(),
            value: "Auto-detected".to_string(), // TODO: Add language detection
        },
        ConfigRow {
            setting: "Top-K Results".to_string(),
            value: config.top_packs.to_string(),
        },
        ConfigRow {
            setting: "Granularity".to_string(),
            value: "File and Directory".to_string(),
        },
        ConfigRow {
            setting: "Analysis Mode".to_string(),
            value: if config.enable_branch_packs && config.enable_file_split_packs {
                "Full Analysis".to_string()
            } else if config.enable_branch_packs {
                "Directory Analysis".to_string()
            } else if config.enable_file_split_packs {
                "File Split Analysis".to_string()
            } else {
                "Custom".to_string()
            },
        },
    ];

    let mut table = Table::new(config_rows);
    table.with(TableStyle::rounded());
    println!("{}", table);
    println!();
}

/// Build ValknutConfig from CLI args for analysis.
/// Shared by both progress and non-progress analysis functions.
async fn build_analysis_config(args: &AnalyzeArgs) -> anyhow::Result<ValknutConfig> {
    use valknut_rs::core::config::DenoiseConfig;

    let mut config = ValknutConfig::default();
    config.analysis.enable_lsh_analysis = true;
    config.analysis.enable_coverage_analysis = true;
    if config.analysis.max_files == 0 {
        config.analysis.max_files = 5000;
    }

    // Configure APTED verification
    apply_apted_config(&mut config, &args.advanced_clone);

    // Configure denoise settings
    let denoise_enabled = true;
    let auto_enabled = !args.advanced_clone.no_auto;

    log_denoise_status(denoise_enabled);

    config.denoise = build_denoise_config(args, denoise_enabled, auto_enabled);

    // Apply denoise-specific settings
    if denoise_enabled {
        apply_denoise_settings(&mut config, args, auto_enabled).await?;
    }

    // Apply analysis control flags
    apply_analysis_control_flags(&mut config, args);

    // Configure coverage
    config.coverage = build_coverage_config(&args.coverage);

    Ok(config)
}

/// Apply APTED verification settings to config.
fn apply_apted_config(config: &mut ValknutConfig, args: &AdvancedCloneArgs) {
    if args.no_apted_verify {
        config.lsh.verify_with_apted = false;
    } else if args.apted_verify {
        config.lsh.verify_with_apted = true;
    }
    if let Some(max_nodes) = args.apted_max_nodes {
        config.lsh.apted_max_nodes = max_nodes;
    }
    if let Some(max_pairs) = args.apted_max_pairs {
        config.lsh.apted_max_pairs_per_entity = max_pairs;
    }
}

/// Log denoise status.
fn log_denoise_status(enabled: bool) {
    if enabled {
        info!("Clone denoising enabled (advanced analysis mode)");
    } else {
        info!("Clone denoising disabled via --no-denoise flag");
    }
}

/// Build denoise configuration from CLI args.
fn build_denoise_config(
    args: &AnalyzeArgs,
    denoise_enabled: bool,
    auto_enabled: bool,
) -> valknut_rs::core::config::DenoiseConfig {
    use valknut_rs::core::config::{
        AutoCalibrationConfig, DenoiseConfig, DenoiseWeights, RankingConfig, StopMotifsConfig,
    };

    let min_function_tokens = args.clone_detection.min_function_tokens.unwrap_or(40);
    let min_match_tokens = args.clone_detection.min_match_tokens.unwrap_or(24);
    let require_blocks = args.clone_detection.require_blocks.unwrap_or(2);
    let similarity = args.clone_detection.similarity.unwrap_or(0.82);

    let weights = build_denoise_weights(&args.advanced_clone);
    let auto_calibration = build_auto_calibration_config(&args.advanced_clone, auto_enabled);
    let ranking = build_ranking_config(&args.advanced_clone);

    DenoiseConfig {
        enabled: denoise_enabled,
        auto: auto_enabled,
        min_function_tokens,
        min_match_tokens,
        require_blocks,
        similarity,
        weights,
        io_mismatch_penalty: args.advanced_clone.io_mismatch_penalty.unwrap_or(0.25),
        threshold_s: similarity,
        stop_motifs: StopMotifsConfig::default(),
        auto_calibration,
        ranking,
        dry_run: args.clone_detection.denoise_dry_run,
    }
}

/// Build denoise weights from CLI args.
fn build_denoise_weights(args: &AdvancedCloneArgs) -> valknut_rs::core::config::DenoiseWeights {
    let mut weights = valknut_rs::core::config::DenoiseWeights::default();
    if let Some(ast_weight) = args.ast_weight {
        weights.ast = ast_weight;
    }
    if let Some(pdg_weight) = args.pdg_weight {
        weights.pdg = pdg_weight;
    }
    if let Some(emb_weight) = args.emb_weight {
        weights.emb = emb_weight;
    }
    weights
}

/// Build auto-calibration config from CLI args.
fn build_auto_calibration_config(
    args: &AdvancedCloneArgs,
    auto_enabled: bool,
) -> valknut_rs::core::config::AutoCalibrationConfig {
    let mut config = valknut_rs::core::config::AutoCalibrationConfig {
        enabled: auto_enabled,
        ..Default::default()
    };
    if let Some(quality_target) = args.quality_target {
        config.quality_target = quality_target;
    }
    if let Some(sample_size) = args.sample_size {
        config.sample_size = sample_size;
    }
    config
}

/// Build ranking config from CLI args.
fn build_ranking_config(args: &AdvancedCloneArgs) -> valknut_rs::core::config::RankingConfig {
    let mut ranking = valknut_rs::core::config::RankingConfig::default();
    if let Some(min_saved_tokens) = args.min_saved_tokens {
        ranking.min_saved_tokens = min_saved_tokens;
    }
    if let Some(min_rarity_gain) = args.min_rarity_gain {
        ranking.min_rarity_gain = min_rarity_gain;
    }
    ranking
}

/// Apply denoise-specific settings when denoise is enabled.
async fn apply_denoise_settings(
    config: &mut ValknutConfig,
    args: &AnalyzeArgs,
    auto_enabled: bool,
) -> anyhow::Result<()> {
    config.dedupe.adaptive.rarity_weighting = true;
    config.lsh.shingle_size = 9;

    info!(
        "Denoise config - min_function_tokens: {}, min_match_tokens: {}, require_blocks: {}, similarity: {:.2}",
        config.denoise.min_function_tokens,
        config.denoise.min_match_tokens,
        config.denoise.require_blocks,
        config.denoise.similarity
    );

    create_denoise_cache_directories().await?;

    if auto_enabled {
        info!("Auto-calibration enabled (default)");
    } else {
        info!("Auto-calibration disabled via --no-auto flag");
    }

    if args.clone_detection.denoise_dry_run {
        info!("DRY-RUN mode enabled");
        println!("{}", "denoise: DRY-RUN (no changes).".yellow());
    }

    Ok(())
}

/// Apply analysis control flags from CLI args.
fn apply_analysis_control_flags(config: &mut ValknutConfig, args: &AnalyzeArgs) {
    if args.coverage.no_coverage {
        config.analysis.enable_coverage_analysis = false;
    }
    if args.analysis_control.no_complexity {
        config.analysis.enable_scoring = false;
    }
    if args.analysis_control.no_structure {
        config.analysis.enable_structure_analysis = false;
    }
    if args.analysis_control.no_refactoring {
        config.analysis.enable_refactoring_analysis = false;
    }
    if args.analysis_control.no_impact {
        config.analysis.enable_graph_analysis = false;
    }
    if args.analysis_control.no_lsh {
        config.analysis.enable_lsh_analysis = false;
    }
    if args.analysis_control.cohesion {
        config.analysis.enable_cohesion_analysis = true;
        config.cohesion.enabled = true;
        apply_cohesion_args(config, &args.cohesion);
    }
}

/// Apply cohesion-specific CLI args.
fn apply_cohesion_args(config: &mut ValknutConfig, args: &CohesionArgs) {
    if let Some(min_score) = args.cohesion_min_score {
        config.cohesion.thresholds.min_cohesion = min_score;
    }
    if let Some(min_doc_alignment) = args.cohesion_min_doc_alignment {
        config.cohesion.thresholds.min_doc_alignment = min_doc_alignment;
    }
    if let Some(outlier_percentile) = args.cohesion_outlier_percentile {
        config.cohesion.thresholds.outlier_percentile = outlier_percentile;
    }
}

/// Build coverage config from CLI args.
fn build_coverage_config(args: &CoverageArgs) -> valknut_rs::core::config::CoverageConfig {
    let mut config = valknut_rs::core::config::CoverageConfig::default();
    if let Some(coverage_file) = &args.coverage_file {
        config.coverage_file = Some(coverage_file.clone());
        config.auto_discover = false;
    }
    if args.no_coverage_auto_discover {
        config.auto_discover = false;
    }
    if let Some(max_age_days) = args.coverage_max_age_days {
        config.max_age_days = max_age_days;
    }
    config
}

/// Display analysis configuration summary.
fn display_analysis_config(pipeline_config: &PipelineAnalysisConfig, cohesion_enabled: bool) {
    let enabled_analyses = [
        ("Complexity", pipeline_config.enable_complexity_analysis),
        ("Structure", pipeline_config.enable_structure_analysis),
        ("Refactoring", pipeline_config.enable_refactoring_analysis),
        ("Impact", pipeline_config.enable_impact_analysis),
        ("Clone Detection (LSH)", pipeline_config.enable_lsh_analysis),
        ("Coverage", pipeline_config.enable_coverage_analysis),
        ("Semantic Cohesion", cohesion_enabled),
    ];

    println!("{}", "📊 Analysis Configuration:".bright_blue().bold());
    for (name, enabled) in enabled_analyses {
        let status = if enabled {
            "✅ Enabled".green().to_string()
        } else {
            "❌ Disabled".red().to_string()
        };
        println!("  {}: {}", name, status);
    }
    println!();
}

/// Log analysis completion summary.
fn log_analysis_completion(result: &valknut_rs::core::pipeline::ComprehensiveAnalysisResult) {
    info!("Analysis completed successfully");
    info!("Total files: {}", result.summary.total_files);
    info!("Total issues: {}", result.summary.total_issues);
    info!(
        "Overall health score: {:.1}",
        result.health_metrics.overall_health_score
    );
}

#[allow(dead_code)]
/// Run comprehensive analysis with detailed progress tracking.
pub async fn run_analysis_with_progress(
    paths: &[PathBuf],
    _config: StructureConfig,
    args: &AnalyzeArgs,
) -> anyhow::Result<serde_json::Value> {
    use valknut_rs::core::pipeline::{AnalysisPipeline, ProgressCallback};

    let quiet_mode = is_quiet(args);
    let multi_progress = MultiProgress::new();

    let main_pb = multi_progress.add(ProgressBar::new(100));
    main_pb.set_style(ProgressStyle::with_template(
        "🚀 {msg} [{bar:40.bright_blue/blue}] {pos:>3}% {elapsed_precise}",
    )?);
    main_pb.set_message("Comprehensive Analysis");

    let valknut_config = build_analysis_config(args).await?;
    let pipeline_config = PipelineAnalysisConfig::from(valknut_config.clone());

    if !quiet_mode {
        display_analysis_config(&pipeline_config, valknut_config.cohesion.enabled);
    }

    let pipeline = AnalysisPipeline::new_with_config(pipeline_config, valknut_config);

    let progress_callback: ProgressCallback = Box::new({
        let pb = main_pb.clone();
        move |stage: &str, progress: f64| {
            pb.set_message(stage.to_string());
            pb.set_position(progress as u64);
        }
    });

    info!("Starting comprehensive analysis for {} paths", paths.len());
    let analysis_result = pipeline
        .analyze_paths(paths, Some(progress_callback))
        .await
        .map_err(|e| anyhow::anyhow!("Analysis failed: {}", e))?;

    main_pb.finish_with_message("Analysis Complete");
    log_analysis_completion(&analysis_result);

    Ok(serde_json::to_value(&analysis_result)?)
}

#[allow(dead_code)]
/// Run analysis without progress bars for quiet mode.
pub async fn run_analysis_without_progress(
    paths: &[PathBuf],
    _config: StructureConfig,
    args: &AnalyzeArgs,
) -> anyhow::Result<serde_json::Value> {
    use valknut_rs::core::pipeline::AnalysisPipeline;

    let valknut_config = build_analysis_config(args).await?;
    let pipeline_config = PipelineAnalysisConfig::from(valknut_config.clone());
    let pipeline = AnalysisPipeline::new_with_config(pipeline_config, valknut_config);

    info!("Starting comprehensive analysis for {} paths", paths.len());
    let analysis_result = pipeline
        .analyze_paths(paths, None)
        .await
        .map_err(|e| anyhow::anyhow!("Analysis failed: {}", e))?;

    log_analysis_completion(&analysis_result);

    Ok(serde_json::to_value(&analysis_result)?)
}

#[allow(dead_code)]
/// Create denoise cache directories if they don't exist.
async fn create_denoise_cache_directories() -> anyhow::Result<()> {
    let cache_base = std::path::Path::new(".valknut/cache/denoise");

    // Create the denoise cache directory
    tokio::fs::create_dir_all(&cache_base).await?;

    // Create cache files if they don't exist
    let stop_motifs_path = cache_base.join("stop_motifs.v1.json");
    let auto_calibration_path = cache_base.join("auto_calibration.v1.json");

    if !stop_motifs_path.exists() {
        let empty_motifs = serde_json::json!({
            "version": 1,
            "created": chrono::Utc::now().to_rfc3339(),
            "stop_motifs": []
        });
        tokio::fs::write(
            &stop_motifs_path,
            serde_json::to_string_pretty(&empty_motifs)?,
        )
        .await?;
        info!("Created denoise cache file: {}", stop_motifs_path.display());
    }

    if !auto_calibration_path.exists() {
        let empty_calibration = serde_json::json!({
            "version": 1,
            "created": chrono::Utc::now().to_rfc3339(),
            "calibration_data": {}
        });
        tokio::fs::write(
            &auto_calibration_path,
            serde_json::to_string_pretty(&empty_calibration)?,
        )
        .await?;
        info!(
            "Created denoise cache file: {}",
            auto_calibration_path.display()
        );
    }

    Ok(())
}

/// Load configuration from file or use defaults
pub async fn load_configuration(config_path: Option<&Path>) -> anyhow::Result<StructureConfig> {
    let config = match config_path {
        Some(path) => {
            let content = tokio::fs::read_to_string(path).await?;
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("yaml" | "yml") => serde_yaml::from_str(&content)?,
                Some("json") => serde_json::from_str(&content)?,
                _ => serde_yaml::from_str(&content)?,
            }
        }
        None => StructureConfig::default(),
    };

    Ok(config)
}

// Helper functions
/// Convert an `OutputFormat` to the lowercase string used in reports.
pub fn format_to_string(format: &OutputFormat) -> &str {
    match format {
        OutputFormat::Jsonl => "jsonl",
        OutputFormat::Json => "json",
        OutputFormat::Yaml => "yaml",
        OutputFormat::Markdown => "markdown",
        OutputFormat::Html => "html",
        OutputFormat::Sonar => "sonar",
        OutputFormat::Csv => "csv",
        OutputFormat::CiSummary => "ci-summary",
        OutputFormat::Pretty => "pretty",
    }
}

/// Emit console warnings when disabled/unsupported languages are detected.
fn warn_for_unsupported_languages(config: &ValknutConfig, quiet_mode: bool) {
    use owo_colors::OwoColorize;

    let unsupported: Vec<String> = config
        .languages
        .iter()
        .filter(|(_, cfg)| cfg.enabled)
        .filter(|(_, cfg)| {
            !cfg.file_extensions
                .iter()
                .any(|ext| extension_is_supported(ext.trim_start_matches('.')))
        })
        .map(|(name, _)| name.clone())
        .collect();

    if unsupported.is_empty() {
        return;
    }

    let message = format!(
        "Languages configured but not yet supported: {}. They will be skipped during analysis.",
        unsupported.join(", ")
    );

    if quiet_mode {
        warn!("{}", message);
    } else {
        println!("warn: {}", message);
    }
}

#[allow(dead_code)]
/// Handle quality gate evaluation for JSON results emitted by tests.
async fn handle_quality_gates(
    args: &AnalyzeArgs,
    result: &serde_json::Value,
) -> anyhow::Result<QualityGateResult> {
    use valknut_rs::core::pipeline::QualityGateViolation;

    // Build quality gate configuration from CLI args
    let quality_gate_config = build_quality_gate_config(args);

    let mut violations = Vec::new();

    // Extract summary data (this should always be present)
    let summary = result
        .get("summary")
        .ok_or_else(|| anyhow::anyhow!("Summary not found in analysis result"))?;

    let total_issues = summary
        .get("total_issues")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    // Check available metrics against thresholds
    if quality_gate_config.max_critical_issues > 0
        && total_issues > quality_gate_config.max_critical_issues
    {
        violations.push(QualityGateViolation {
            rule_name: "Total Issues Count".to_string(),
            current_value: total_issues as f64,
            threshold: quality_gate_config.max_critical_issues as f64,
            description: format!(
                "Total issues ({}) exceeds maximum allowed ({})",
                total_issues, quality_gate_config.max_critical_issues
            ),
            severity: if total_issues > quality_gate_config.max_critical_issues * 2 {
                "Critical".to_string()
            } else {
                "High".to_string()
            },
            affected_files: Vec::new(),
            recommended_actions: vec!["Review and address high-priority issues".to_string()],
        });
    }

    // Try to extract health metrics if available (for more comprehensive analysis)
    if let Some(health_metrics) = result.get("health_metrics") {
        if let Some(overall_health) = health_metrics
            .get("overall_health_score")
            .and_then(|v| v.as_f64())
        {
            if overall_health < quality_gate_config.min_maintainability_score {
                violations.push(QualityGateViolation {
                    rule_name: "Overall Health Score".to_string(),
                    current_value: overall_health,
                    threshold: quality_gate_config.min_maintainability_score,
                    description: format!(
                        "Health score ({:.1}) is below minimum required ({:.1})",
                        overall_health, quality_gate_config.min_maintainability_score
                    ),
                    severity: if overall_health
                        < quality_gate_config.min_maintainability_score - 20.0
                    {
                        "Blocker".to_string()
                    } else {
                        "Critical".to_string()
                    },
                    affected_files: Vec::new(),
                    recommended_actions: vec![
                        "Improve code structure and reduce technical debt".to_string()
                    ],
                });
            }
        }

        if let Some(complexity_score) = health_metrics
            .get("complexity_score")
            .and_then(|v| v.as_f64())
        {
            if complexity_score > quality_gate_config.max_complexity_score {
                violations.push(QualityGateViolation {
                    rule_name: "Complexity Score".to_string(),
                    current_value: complexity_score,
                    threshold: quality_gate_config.max_complexity_score,
                    description: format!(
                        "Complexity score ({:.1}) exceeds maximum allowed ({:.1})",
                        complexity_score, quality_gate_config.max_complexity_score
                    ),
                    severity: if complexity_score > quality_gate_config.max_complexity_score + 10.0
                    {
                        "Critical".to_string()
                    } else {
                        "High".to_string()
                    },
                    affected_files: Vec::new(),
                    recommended_actions: vec![
                        "Simplify complex functions and reduce nesting".to_string()
                    ],
                });
            }
        }

        if let Some(debt_ratio) = health_metrics
            .get("technical_debt_ratio")
            .and_then(|v| v.as_f64())
        {
            if debt_ratio > quality_gate_config.max_technical_debt_ratio {
                violations.push(QualityGateViolation {
                    rule_name: "Technical Debt Ratio".to_string(),
                    current_value: debt_ratio,
                    threshold: quality_gate_config.max_technical_debt_ratio,
                    description: format!(
                        "Technical debt ratio ({:.1}%) exceeds maximum allowed ({:.1}%)",
                        debt_ratio, quality_gate_config.max_technical_debt_ratio
                    ),
                    severity: if debt_ratio > quality_gate_config.max_technical_debt_ratio + 20.0 {
                        "Critical".to_string()
                    } else {
                        "High".to_string()
                    },
                    affected_files: Vec::new(),
                    recommended_actions: vec!["Refactor code to reduce technical debt".to_string()],
                });
            }
        }
    }

    let passed = violations.is_empty();
    let overall_score = result
        .get("health_metrics")
        .and_then(|hm| hm.get("overall_health_score"))
        .and_then(|v| v.as_f64())
        .unwrap_or(50.0); // Default score if not available

    Ok(QualityGateResult {
        passed,
        violations,
        overall_score,
    })
}

/// Build quality gate configuration from CLI arguments
fn build_quality_gate_config(args: &AnalyzeArgs) -> QualityGateConfig {
    let mut config = QualityGateConfig {
        enabled: args.quality_gate.quality_gate || args.quality_gate.fail_on_issues,
        min_health_score: QualityGateConfig::default().min_health_score,
        min_doc_health_score: QualityGateConfig::default().min_doc_health_score,
        ..Default::default()
    };

    // Override defaults with CLI values if provided
    if let Some(max_complexity) = args.quality_gate.max_complexity {
        config.max_complexity_score = max_complexity;
    }
    if let Some(min_health) = args.quality_gate.min_health {
        config.min_maintainability_score = min_health;
        config.min_health_score = min_health;
    }
    if let Some(min_doc_health) = args.quality_gate.min_doc_health {
        config.min_doc_health_score = min_doc_health;
    }
    if let Some(max_debt) = args.quality_gate.max_debt {
        config.max_technical_debt_ratio = max_debt;
    }
    if let Some(min_maintainability) = args.quality_gate.min_maintainability {
        config.min_maintainability_score = min_maintainability;
    }
    if let Some(max_issues) = args.quality_gate.max_issues {
        config.max_critical_issues = max_issues;
    }
    if let Some(max_critical) = args.quality_gate.max_critical {
        config.max_critical_issues = max_critical;
    }
    if let Some(max_high_priority) = args.quality_gate.max_high_priority {
        config.max_high_priority_issues = max_high_priority;
    }

    // Handle fail_on_issues flag (sets max_issues to 0)
    if args.quality_gate.fail_on_issues {
        config.max_critical_issues = 0;
        config.max_high_priority_issues = 0;
    }

    config
}

/// Print a group of violations with a header.
fn print_violation_group(header: &str, violations: &[&QualityGateViolation]) {
    if violations.is_empty() {
        return;
    }
    println!("{}", header);
    for v in violations {
        println!(
            "  • {}: {:.1} (threshold: {:.1})",
            v.rule_name.yellow(),
            v.current_value,
            v.threshold
        );
        println!("    {}", v.description.dimmed());
    }
    println!();
}

#[allow(dead_code)]
/// Display quality gate violations in a user-friendly format.
fn display_quality_gate_violations(result: &QualityGateResult) {
    println!();
    println!("{}", "❌ Quality Gate Failed".red().bold());
    println!(
        "{} {:.1}",
        "Quality Score:".dimmed(),
        result.overall_score.to_string().yellow()
    );
    println!();

    // Group and display violations by severity
    let (blockers, criticals, warnings): (Vec<_>, Vec<_>, Vec<_>) = result
        .violations
        .iter()
        .fold((vec![], vec![], vec![]), |(mut b, mut c, mut w), v| {
            match v.severity.as_str() {
                "Blocker" => b.push(v),
                "Critical" => c.push(v),
                "Warning" | "High" => w.push(v),
                _ => {}
            }
            (b, c, w)
        });

    print_violation_group(&"🚫 BLOCKER Issues:".red().bold().to_string(), &blockers);
    print_violation_group(&"🔴 CRITICAL Issues:".red().bold().to_string(), &criticals);
    print_violation_group(&"⚠️  WARNING Issues:".yellow().bold().to_string(), &warnings);

    println!("{}", "To fix these issues:".bold());
    println!("  1. Reduce code complexity by refactoring large functions");
    println!("  2. Address critical and high-priority issues first");
    println!("  3. Improve code maintainability through better structure");
    println!("  4. Reduce technical debt by following best practices");
    println!();
}

/// Run Oracle dry-run to show slicing plan without calling the API
fn run_oracle_dry_run(paths: &[PathBuf], args: &AnalyzeArgs) -> anyhow::Result<()> {
    // Build config with CLI overrides (no API key needed for dry-run)
    let mut config = OracleConfig {
        api_key: String::new(), // Not needed for dry-run
        max_tokens: 400_000,
        api_endpoint: String::new(),
        model: String::new(),
        enable_slicing: !args.ai_features.no_oracle_slicing,
        slice_token_budget: args.ai_features.oracle_slice_budget.unwrap_or(200_000),
        slice_model: String::new(),
        slicing_threshold: args.ai_features.oracle_slicing_threshold.unwrap_or(300_000),
    };

    if let Some(max_tokens) = args.ai_features.oracle_max_tokens {
        config.max_tokens = max_tokens;
    }

    let oracle = RefactoringOracle::new(config);
    let project_path = paths.first().ok_or_else(|| anyhow::anyhow!("No paths provided"))?;

    oracle.dry_run(project_path).map_err(|e| anyhow::anyhow!("Oracle dry-run failed: {}", e))
}

/// Run Oracle analysis to get AI refactoring suggestions
async fn run_oracle_analysis(
    paths: &[PathBuf],
    analysis_result: &AnalysisResults,
    args: &AnalyzeArgs,
) -> anyhow::Result<Option<valknut_rs::oracle::RefactoringOracleResponse>> {
    let quiet_mode = is_quiet(args);

    // Check if GEMINI_API_KEY is available
    let oracle_config = match OracleConfig::from_env() {
        Ok(mut config) => {
            if let Some(max_tokens) = args.ai_features.oracle_max_tokens {
                config = config.with_max_tokens(max_tokens);
            }
            if let Some(slice_budget) = args.ai_features.oracle_slice_budget {
                config = config.with_slice_budget(slice_budget);
            }
            if args.ai_features.no_oracle_slicing {
                config = config.with_slicing(false);
            }
            if let Some(threshold) = args.ai_features.oracle_slicing_threshold {
                config.slicing_threshold = threshold;
            }
            config
        }
        Err(e) => {
            eprintln!("Oracle configuration failed: {e}");
            eprintln!("Set GEMINI_API_KEY to enable oracle suggestions.");
            return Ok(None);
        }
    };

    let oracle = RefactoringOracle::new(oracle_config);

    // Use the first path as the project root for analysis
    let project_path = paths.first().unwrap();

    if !quiet_mode {
        println!(
            "Oracle: analyzing {} for refactoring suggestions",
            project_path.display()
        );
    }

    match oracle
        .generate_suggestions(project_path, analysis_result)
        .await
    {
        Ok(response) => {
            if !quiet_mode {
                let all_tasks = response.all_tasks();
                let required_tasks = all_tasks
                    .iter()
                    .filter(|t| t.required.unwrap_or(false))
                    .count();
                let optional_tasks = all_tasks.len() - required_tasks;
                println!(
                    "Oracle: {} tasks ({} required, {} optional)",
                    all_tasks.len(),
                    required_tasks,
                    optional_tasks
                );
            }

            // Save oracle response to a separate file for review
            if let Ok(oracle_json) = serde_json::to_string_pretty(&response) {
                let oracle_path = project_path.join(".valknut-oracle-response.json");
                if let Err(e) = tokio::fs::write(&oracle_path, oracle_json).await {
                    warn!(
                        "Failed to write oracle response to {}: {}",
                        oracle_path.display(),
                        e
                    );
                } else if !quiet_mode {
                    println!("Oracle: saved recommendations to {}", oracle_path.display());
                }
            }

            Ok(Some(response))
        }
        Err(e) => {
            if !quiet_mode {
                eprintln!("Oracle analysis failed: {e}");
                eprintln!("Continuing without oracle suggestions.");
            }
            warn!("Oracle analysis failed: {}", e);
            Ok(None)
        }
    }
}

#[allow(dead_code)]
/// Generate output reports in various formats (legacy version for compatibility).
async fn generate_reports(result: &AnalysisResults, args: &AnalyzeArgs) -> anyhow::Result<()> {
    generate_reports_with_oracle(result, &None, args).await
}


#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
