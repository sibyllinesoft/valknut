//! Command Execution Logic and Analysis Operations
//!
//! This module contains the main command execution logic, analysis operations,
//! and progress tracking functionality.

use crate::cli::analysis_display::{
    display_analysis_config, display_analysis_summary, display_comprehensive_results,
    log_analysis_completion, priority_label,
};
use crate::cli::args::{
    AIFeaturesArgs, AdvancedCloneArgs, AnalysisControlArgs, AnalyzeArgs, CloneDetectionArgs,
    CohesionArgs, CoverageArgs, InitConfigArgs, OutputFormat, PerformanceProfile, QualityGateArgs,
    SurveyVerbosity, ValidateConfigArgs,
};
use crate::cli::config_builder::{
    build_analysis_config, build_coverage_config, build_denoise_config, build_valknut_config,
    create_denoise_cache_directories,
};
use crate::cli::config_layer::build_layered_valknut_config;
use crate::cli::quality_gates::{
    evaluate_quality_gates_if_enabled, handle_quality_gate_result, quality_status,
};
// Re-export quality gate functions for tests (they use `super::*`)
pub use crate::cli::quality_gates::{
    build_quality_gate_config, build_violation, check_issue_count_violations,
    check_metric_violations, display_quality_failures, display_quality_gate_violations,
    evaluate_quality_gates, print_violation_group, severity_for_excess, severity_for_shortfall,
    top_issue_files,
};
use crate::cli::reports::is_quiet;
// Re-export report generation functions for tests (they use `super::*`)
pub use crate::cli::reports::{
    format_file_info, format_to_string, generate_default_content, generate_html_file,
    generate_json_content, generate_jsonl_content, generate_reports_with_oracle,
    generate_yaml_content,
};
// Re-export display functions for tests
pub use crate::cli::analysis_display::{
    display_config_summary, header_lines_for_width, print_header,
};
use anyhow;
use chrono;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde_json;
use serde_yaml;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use tabled::{settings::Style as TableStyle, Table, Tabled};
use tracing::{info, warn};

// Import comprehensive analysis pipeline
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

    run_comprehensive_analysis(valid_paths, config, !quiet_mode).await
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
    let enabled = collect_enabled_analyses(config);
    let enabled_summary = if enabled.is_empty() {
        "none".to_string()
    } else {
        enabled.join(", ")
    };

    println!("Analyses: {enabled_summary}");

    if detailed {
        display_clone_details(config);
        display_coverage_details(config);
    }
}

/// Collect names of enabled analyses.
fn collect_enabled_analyses(config: &ValknutConfig) -> Vec<&'static str> {
    let checks: &[(&'static str, bool)] = &[
        ("scoring", config.analysis.enable_scoring),
        ("structure", config.analysis.enable_structure_analysis),
        ("refactoring", config.analysis.enable_refactoring_analysis),
        ("impact", config.analysis.enable_graph_analysis),
        ("clones", config.analysis.enable_lsh_analysis),
        ("coverage", config.analysis.enable_coverage_analysis),
    ];

    checks
        .iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(name, _)| *name)
        .collect()
}

/// Display clone analysis details if enabled.
fn display_clone_details(config: &ValknutConfig) {
    if !config.analysis.enable_lsh_analysis {
        return;
    }

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

/// Display coverage analysis details if enabled.
fn display_coverage_details(config: &ValknutConfig) {
    if !config.analysis.enable_coverage_analysis {
        return;
    }

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

/// Returns a human-readable coverage status string for display.
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

/// Run comprehensive analysis with optional progress tracking.
async fn run_comprehensive_analysis(
    paths: &[PathBuf],
    config: ValknutConfig,
    with_progress: bool,
) -> anyhow::Result<AnalysisResults> {
    let mut engine = ValknutEngine::new_from_valknut_config(config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create analysis engine: {}", e))?;

    let all_results = if with_progress {
        let multi_progress = MultiProgress::new();
        let main_progress = multi_progress.add(ProgressBar::new(100));
        if let Ok(style) = ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>3}/{len:3} {msg}")
        {
            main_progress.set_style(style.progress_chars("##-"));
        }

        let progress = main_progress.clone();
        let results = run_analysis_core(&mut engine, paths, Some(|msg: &str, pct: f64| {
            progress.set_position((pct * 100.0) as u64);
            progress.set_message(msg.to_string());
        }))
        .await?;

        main_progress.finish_with_message("Analysis complete");
        results
    } else {
        run_analysis_core(&mut engine, paths, None::<fn(&str, f64)>).await?
    };

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


// Re-export config commands for backward compatibility
pub use super::config::{init_config, load_configuration, print_default_config, validate_config};

// Re-export MCP commands from the dedicated module for backward compatibility
pub use super::mcp::{mcp_manifest_command, mcp_stdio_command};

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

// Re-export doc_audit_command from the dedicated module for backward compatibility
pub use super::doc_audit::doc_audit_command;

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

// Import Oracle functions from the dedicated module
use super::oracle::{run_oracle_analysis, run_oracle_dry_run};

#[allow(dead_code)]
/// Generate output reports in various formats (legacy version for compatibility).
async fn generate_reports(result: &AnalysisResults, args: &AnalyzeArgs) -> anyhow::Result<()> {
    generate_reports_with_oracle(result, &None, args).await
}


#[cfg(test)]
#[path = "analyze_tests.rs"]
mod tests;
