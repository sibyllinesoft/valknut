//! Command Execution Logic and Analysis Operations
//!
//! This module contains the main command execution logic, analysis operations,
//! and progress tracking functionality.

use crate::cli::args::{
    AIFeaturesArgs, AdvancedCloneArgs, AnalysisControlArgs, AnalyzeArgs, CloneDetectionArgs,
    CoverageArgs, DocAuditArgs, DocAuditFormat, InitConfigArgs, McpManifestArgs, McpStdioArgs,
    OutputFormat, PerformanceProfile, QualityGateArgs, SurveyVerbosity, ValidateConfigArgs,
};
use crate::cli::config_layer::build_layered_valknut_config;
use anyhow::{self, Context};
use chrono;
use console::Term;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde_json;
use serde_yaml;
use std::cmp::Ordering;
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
use valknut_rs::oracle::{OracleConfig, RefactoringOracle};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Determines whether CLI output should be suppressed for the given args.
fn is_quiet(args: &AnalyzeArgs) -> bool {
    args.quiet || args.format.is_machine_readable()
}

/// Main analyze command implementation with comprehensive analysis pipeline
pub async fn analyze_command(
    args: AnalyzeArgs,
    _survey: bool,
    _survey_verbosity: SurveyVerbosity,
) -> anyhow::Result<()> {
    let quiet_mode = is_quiet(&args);

    // Print header
    if !quiet_mode {
        print_header();
    }

    // Build comprehensive configuration from CLI args and file
    let valknut_config = build_valknut_config(&args).await?;

    if !quiet_mode {
        println!(
            "{}",
            "✅ Configuration loaded with comprehensive analysis enabled".green()
        );
        display_analysis_config_summary(&valknut_config);
    }
    warn_for_unsupported_languages(&valknut_config, quiet_mode);

    // Validate and prepare paths
    if !quiet_mode {
        println!("{}", "📂 Validating Input Paths".bright_blue().bold());
        println!();
    }

    let mut valid_paths = Vec::new();
    for path in &args.paths {
        if path.exists() {
            valid_paths.push(path.clone());
            if !quiet_mode {
                let path_type = if path.is_dir() {
                    "📁 Directory"
                } else {
                    "📄 File"
                };
                println!("  {}: {}", path_type, path.display().to_string().green());
            }
        } else {
            return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
        }
    }

    if valid_paths.is_empty() {
        return Err(anyhow::anyhow!("No valid paths provided"));
    }

    // Create output directory
    tokio::fs::create_dir_all(&args.out).await?;

    if !quiet_mode {
        println!();
        println!(
            "{} {}",
            "📁 Output directory:".bold(),
            args.out.display().to_string().cyan()
        );
        println!(
            "{} {}",
            "📊 Report format:".bold(),
            format_to_string(&args.format).to_uppercase().cyan()
        );
        println!();
    }

    // Preview coverage file discovery if enabled
    if valknut_config.analysis.enable_coverage_analysis && !quiet_mode {
        preview_coverage_discovery(&valid_paths, &valknut_config.coverage).await?;
    }

    // Run comprehensive analysis with enhanced progress tracking
    if !quiet_mode {
        println!(
            "{}",
            "🔍 Starting Comprehensive Analysis Pipeline"
                .bright_blue()
                .bold()
        );
        display_enabled_analyses(&valknut_config);
        println!();
    }

    let analysis_result = if quiet_mode {
        run_comprehensive_analysis_without_progress(&valid_paths, valknut_config, &args).await?
    } else {
        run_comprehensive_analysis_with_progress(&valid_paths, valknut_config, &args).await?
    };

    // Handle quality gates
    let quality_gate_result = if args.quality_gate.quality_gate || args.quality_gate.fail_on_issues
    {
        let quality_config = build_quality_gate_config(&args);
        Some(evaluate_quality_gates(
            &analysis_result,
            &quality_config,
            !quiet_mode,
        )?)
    } else {
        None
    };

    // Display analysis results
    if !quiet_mode {
        display_comprehensive_results(&analysis_result);
    }

    // Run Oracle analysis if requested
    let oracle_response = if args.ai_features.oracle {
        if !quiet_mode {
            println!(
                "{}",
                "🧠 Running AI Refactoring Oracle Analysis..."
                    .bright_blue()
                    .bold()
            );
        }
        run_oracle_analysis(&valid_paths, &analysis_result, &args).await?
    } else {
        None
    };

    // Generate output reports (with oracle results if available)
    generate_reports_with_oracle(&analysis_result, &oracle_response, &args).await?;

    // Handle quality gate failures
    if let Some(quality_result) = quality_gate_result {
        if !quality_result.passed {
            if !quiet_mode {
                println!("{}", "❌ Quality gates failed!".red().bold());
                display_quality_failures(&quality_result);
            }
            return Err(anyhow::anyhow!("Quality gates failed"));
        } else if !quiet_mode {
            println!("{}", "✅ All quality gates passed!".green().bold());
        }
    }

    if !quiet_mode {
        println!("{}", "🎉 Analysis completed successfully!".green().bold());
    }

    Ok(())
}

/// Load doc-audit settings from a YAML file.
fn load_doc_audit_config_file(path: &Path) -> anyhow::Result<DocAuditConfigFile> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read doc audit config at {}", path.display()))?;
    serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse doc audit config {}", path.display()))
}

/// Build comprehensive ValknutConfig from CLI arguments.
async fn build_valknut_config(args: &AnalyzeArgs) -> anyhow::Result<ValknutConfig> {
    // Use the new layered configuration approach
    let mut config = build_layered_valknut_config(args)?;

    // Apply performance profile optimizations
    apply_performance_profile(&mut config, &args.profile);

    Ok(config)
}

/// Apply performance profile optimizations to the configuration.
fn apply_performance_profile(config: &mut ValknutConfig, profile: &PerformanceProfile) {
    match profile {
        PerformanceProfile::Fast => {
            // Fast mode - minimal analysis, optimized for speed
            config.analysis.max_files = 500; // Limit file count
            config.lsh.num_bands = 10; // Reduce LSH precision for speed
            config.lsh.num_hashes = 50; // Fewer hash functions
            info!("🚀 Performance profile: Fast mode - optimized for speed");
        }
        PerformanceProfile::Balanced => {
            // Balanced mode - good default (no changes needed)
            info!("⚖️  Performance profile: Balanced mode - default settings");
        }
        PerformanceProfile::Thorough => {
            // Thorough mode - more comprehensive analysis
            config.analysis.max_files = 2000; // Allow more files
            config.lsh.num_bands = 20; // Higher LSH precision
            config.lsh.num_hashes = 150; // More hash functions
            config.denoise.enabled = true; // Enable all denoising
            info!("🔍 Performance profile: Thorough mode - comprehensive analysis");
        }
        PerformanceProfile::Extreme => {
            // Extreme mode - maximum analysis depth
            config.analysis.max_files = 5000; // Maximum files
            config.lsh.num_bands = 50; // Highest LSH precision
            config.lsh.num_hashes = 200; // Maximum hash functions
            config.denoise.enabled = true;
            info!("🔥 Performance profile: Extreme mode - maximum analysis depth");
        }
    }
}

/// Preview coverage file discovery to show what will be analyzed
async fn preview_coverage_discovery(
    paths: &[PathBuf],
    coverage_config: &CoverageConfig,
) -> anyhow::Result<()> {
    println!(
        "{}",
        "📋 Coverage File Discovery Preview".bright_blue().bold()
    );

    // Use the first path as root for discovery
    let default_path = PathBuf::from(".");
    let root_path = paths.first().unwrap_or(&default_path);

    let discovered_files = CoverageDiscovery::discover_coverage_files(root_path, coverage_config)
        .map_err(|e| anyhow::anyhow!("Coverage discovery failed: {}", e))?;

    if discovered_files.is_empty() {
        println!(
            "  {} No coverage files found - coverage analysis will be skipped",
            "⚠️".yellow()
        );
        println!("  💡 Tip: Generate coverage files using your test runner, e.g.:");
        println!("    - Rust: cargo tarpaulin --out xml");
        println!("    - Python: pytest --cov --cov-report=xml");
        println!("    - JavaScript: npm test -- --coverage --coverageReporters=cobertura");
    } else {
        println!(
            "  {} Found {} coverage files:",
            "✅".green(),
            discovered_files.len()
        );
        for (i, file) in discovered_files.iter().take(3).enumerate() {
            println!(
                "    {}. {} (format: {:?}, size: {} KB)",
                i + 1,
                file.path.display(),
                file.format,
                file.size / 1024
            );
        }
        if discovered_files.len() > 3 {
            println!("    ... and {} more files", discovered_files.len() - 3);
        }
    }

    println!();
    Ok(())
}

/// Display which analyses are enabled
fn display_enabled_analyses(config: &ValknutConfig) {
    println!("  Enabled Analyses:");

    if config.analysis.enable_scoring {
        println!("    ✅ Complexity Analysis - Cyclomatic and cognitive complexity scoring");
    }
    if config.analysis.enable_structure_analysis {
        println!("    ✅ Structure Analysis - Directory organization and architectural patterns");
    }
    if config.analysis.enable_refactoring_analysis {
        println!("    ✅ Refactoring Analysis - Refactoring opportunity detection");
    }
    if config.analysis.enable_graph_analysis {
        println!("    ✅ Impact Analysis - Dependency graphs, cycles, and centrality");
    }
    if config.analysis.enable_lsh_analysis {
        let mut note = if config.denoise.enabled {
            " (with denoising)".to_string()
        } else {
            String::new()
        };
        if config.lsh.verify_with_apted {
            if note.is_empty() {
                note.push_str(" (APTED verification)");
            } else {
                note.push_str(" + APTED verification");
            }
        }
        println!(
            "    ✅ Clone Detection - LSH-based similarity analysis{}",
            note
        );
    }
    if config.analysis.enable_coverage_analysis {
        let auto_status = if config.coverage.auto_discover {
            " (auto-discovery enabled)"
        } else {
            ""
        };
        println!(
            "    ✅ Coverage Analysis - Test gap analysis{}",
            auto_status
        );
    }

    // Count enabled analyses
    let enabled_count = [
        config.analysis.enable_scoring,
        config.analysis.enable_structure_analysis,
        config.analysis.enable_refactoring_analysis,
        config.analysis.enable_graph_analysis,
        config.analysis.enable_lsh_analysis,
        config.analysis.enable_coverage_analysis,
    ]
    .iter()
    .filter(|&&enabled| enabled)
    .count();

    println!("  📊 Total: {} analyses enabled", enabled_count);
}

/// Display analysis configuration summary
fn display_analysis_config_summary(config: &ValknutConfig) {
    println!("  📊 Analysis Configuration:");
    println!(
        "    • Confidence threshold: {:.1}%",
        config.analysis.confidence_threshold * 100.0
    );
    println!(
        "    • Max files: {}",
        if config.analysis.max_files == 0 {
            "unlimited".to_string()
        } else {
            config.analysis.max_files.to_string()
        }
    );

    if config.analysis.enable_coverage_analysis {
        println!(
            "    • Coverage max age: {} days",
            config.coverage.max_age_days
        );
        println!(
            "    • Coverage patterns: {} patterns",
            config.coverage.file_patterns.len()
        );
    }

    if config.analysis.enable_lsh_analysis && config.denoise.enabled {
        println!(
            "    • Clone detection: denoising enabled (similarity: {:.0}%)",
            config.denoise.similarity * 100.0
        );
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

    // Create engine and run analysis using the full ValknutConfig to preserve clone settings
    let mut engine = ValknutEngine::new_from_valknut_config(config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create analysis engine: {}", e))?;

    // Set up progress callback
    let progress_callback = {
        let progress = main_progress.clone();
        Box::new(move |message: &str, percentage: f64| {
            progress.set_position((percentage * 100.0) as u64);
            progress.set_message(message.to_string());
        })
    };

    // Run analysis for each path
    let mut all_results = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        progress_callback(
            &format!("Analyzing {} ({}/{})", path.display(), i + 1, paths.len()),
            (i as f64) / (paths.len() as f64),
        );

        let result = engine
            .analyze_directory(path)
            .await
            .map_err(|e| anyhow::anyhow!("Analysis failed for {}: {}", path.display(), e))?;

        all_results.push(result);
    }

    main_progress.finish_with_message("Analysis complete");

    // Combine results if multiple paths
    let combined_result = if all_results.len() == 1 {
        all_results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Expected at least one analysis result"))?
    } else {
        combine_analysis_results(all_results)?
    };

    Ok(combined_result)
}

/// Run comprehensive analysis without progress tracking.
async fn run_comprehensive_analysis_without_progress(
    paths: &[PathBuf],
    config: ValknutConfig,
    _args: &AnalyzeArgs,
) -> anyhow::Result<AnalysisResults> {
    // Create engine and run analysis using the full ValknutConfig to preserve clone settings
    let mut engine = ValknutEngine::new_from_valknut_config(config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create analysis engine: {}", e))?;

    // Run analysis for each path
    let mut all_results = Vec::new();
    for path in paths.iter() {
        let result = engine
            .analyze_directory(path)
            .await
            .map_err(|e| anyhow::anyhow!("Analysis failed for {}: {}", path.display(), e))?;

        all_results.push(result);
    }

    // Combine results if multiple paths
    let combined_result = if all_results.len() == 1 {
        all_results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Expected at least one analysis result"))?
    } else {
        combine_analysis_results(all_results)?
    };

    Ok(combined_result)
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
            .map(|metrics| metrics.overall_health_score)
            .unwrap_or(default_score);

        return Ok(QualityGateResult {
            passed: true,
            violations: Vec::new(),
            overall_score: score,
        });
    }

    let mut violations = Vec::new();

    if let Some(metrics) = result.health_metrics.as_ref() {
        if metrics.complexity_score > config.max_complexity_score {
            violations.push(QualityGateViolation {
                rule_name: "Complexity Threshold".to_string(),
                description: format!(
                    "Average complexity score ({:.1}) exceeds configured limit ({:.1})",
                    metrics.complexity_score, config.max_complexity_score
                ),
                current_value: metrics.complexity_score,
                threshold: config.max_complexity_score,
                severity: severity_for_excess(
                    metrics.complexity_score,
                    config.max_complexity_score,
                )
                .to_string(),
                affected_files: top_issue_files(
                    result,
                    |candidate| matches!(candidate.priority, Priority::High | Priority::Critical),
                    5,
                ),
                recommended_actions: vec![
                    "Break down the highest complexity functions highlighted above".to_string(),
                    "Introduce guard clauses or helper methods to reduce nesting".to_string(),
                ],
            });
        }

        if metrics.technical_debt_ratio > config.max_technical_debt_ratio {
            violations.push(QualityGateViolation {
                rule_name: "Technical Debt Ratio".to_string(),
                description: format!(
                    "Technical debt ratio ({:.1}%) exceeds maximum allowed ({:.1}%)",
                    metrics.technical_debt_ratio, config.max_technical_debt_ratio
                ),
                current_value: metrics.technical_debt_ratio,
                threshold: config.max_technical_debt_ratio,
                severity: severity_for_excess(
                    metrics.technical_debt_ratio,
                    config.max_technical_debt_ratio,
                )
                .to_string(),
                affected_files: top_issue_files(
                    result,
                    |candidate| matches!(candidate.priority, Priority::High | Priority::Critical),
                    5,
                ),
                recommended_actions: vec![
                    "Triage the listed hotspots and schedule debt paydown work".to_string(),
                    "Ensure tests cover recent refactors to prevent regression".to_string(),
                ],
            });
        }

        if metrics.maintainability_score < config.min_maintainability_score {
            violations.push(QualityGateViolation {
                rule_name: "Maintainability Score".to_string(),
                description: format!(
                    "Maintainability score ({:.1}) fell below required minimum ({:.1})",
                    metrics.maintainability_score, config.min_maintainability_score
                ),
                current_value: metrics.maintainability_score,
                threshold: config.min_maintainability_score,
                severity: severity_for_shortfall(
                    metrics.maintainability_score,
                    config.min_maintainability_score,
                )
                .to_string(),
                affected_files: top_issue_files(
                    result,
                    |candidate| matches!(candidate.priority, Priority::High | Priority::Critical),
                    5,
                ),
                recommended_actions: vec![
                    "Refactor low-cohesion modules to improve readability".to_string(),
                    "Document intent for complex code paths flagged in the report".to_string(),
                ],
            });
        }
    } else if verbose {
        println!(
            "{}",
            "⚠️ Quality gate metrics unavailable; skipping maintainability and complexity checks."
                .yellow()
        );
    }

    let summary = &result.summary;

    if summary.critical as usize > config.max_critical_issues {
        let affected_files = top_issue_files(
            result,
            |candidate| matches!(candidate.priority, Priority::Critical),
            5,
        );
        violations.push(QualityGateViolation {
            rule_name: "Critical Issues".to_string(),
            description: format!(
                "{} critical issues detected (limit: {})",
                summary.critical, config.max_critical_issues
            ),
            current_value: summary.critical as f64,
            threshold: config.max_critical_issues as f64,
            severity: severity_for_excess(
                summary.critical as f64,
                config.max_critical_issues as f64,
            )
            .to_string(),
            affected_files,
            recommended_actions: vec![
                "Prioritise fixes for the critical hotspots above".to_string(),
                "Add regression tests before merging related fixes".to_string(),
            ],
        });
    }

    if summary.high_priority as usize > config.max_high_priority_issues {
        let affected_files = top_issue_files(
            result,
            |candidate| matches!(candidate.priority, Priority::High | Priority::Critical),
            5,
        );
        violations.push(QualityGateViolation {
            rule_name: "High Priority Issues".to_string(),
            description: format!(
                "{} high-priority issues detected (limit: {})",
                summary.high_priority, config.max_high_priority_issues
            ),
            current_value: summary.high_priority as f64,
            threshold: config.max_high_priority_issues as f64,
            severity: severity_for_excess(
                summary.high_priority as f64,
                config.max_high_priority_issues as f64,
            )
            .to_string(),
            affected_files,
            recommended_actions: vec![
                "Address the highlighted high-priority candidates before release".to_string(),
                "Break work into smaller refactors to keep velocity high".to_string(),
            ],
        });
    }

    let overall_score = result
        .health_metrics
        .as_ref()
        .map(|metrics| metrics.overall_health_score)
        .unwrap_or(default_score)
        .clamp(0.0, 100.0);

    Ok(QualityGateResult {
        passed: violations.is_empty(),
        violations,
        overall_score,
    })
}

/// Display comprehensive analysis results
fn display_comprehensive_results(result: &AnalysisResults) {
    println!("{}", "📊 Analysis Results".bright_blue().bold());
    println!();

    // Display summary information
    display_analysis_summary(result);

    println!();
}

/// Display analysis summary
fn display_analysis_summary(result: &AnalysisResults) {
    let summary = &result.summary;

    println!(
        "  Files analyzed: {} | Entities: {} | Candidates: {}",
        summary.files_processed, summary.entities_analyzed, summary.refactoring_needed
    );
    println!(
        "  High priority issues: {} ({} critical)",
        summary.high_priority, summary.critical
    );
    println!(
        "  Code health score: {:.1}% | Avg refactor score: {:.1}",
        summary.code_health_score * 100.0,
        summary.avg_refactoring_score
    );

    if let Some(metrics) = result.health_metrics.as_ref() {
        println!(
            "  Maintainability: {:.1} | Technical debt: {:.1}% | Complexity: {:.1} | Structure: {:.1}",
            metrics.maintainability_score,
            metrics.technical_debt_ratio,
            metrics.complexity_score,
            metrics.structure_quality_score
        );
    }

    if let Some(clone_analysis) = result.clone_analysis.as_ref() {
        println!(
            "  Clone candidates after denoising: {}",
            clone_analysis.candidates_after_denoising
        );
        if let Some(avg_similarity) = clone_analysis.avg_similarity {
            println!("  Avg clone similarity: {:.2}", avg_similarity);
        }
        if let Some(max_similarity) = clone_analysis.max_similarity {
            println!("  Max clone similarity: {:.2}", max_similarity);
        }
        if let Some(verification) = clone_analysis.verification.as_ref() {
            let scored = verification.pairs_scored;
            let evaluated = verification.pairs_evaluated;
            let considered = verification.pairs_considered;
            if let Some(avg) = verification.avg_similarity {
                println!(
                    "  Verification ({}): scored {}/{} pairs ({}) avg {:.2}",
                    verification.method, scored, evaluated, considered, avg
                );
            } else {
                println!(
                    "  Verification ({}): scored {}/{} pairs ({})",
                    verification.method, scored, evaluated, considered
                );
            }
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
        println!();
        println!("  Top hotspots:");
        for candidate in hotspots {
            let file_name = Path::new(&candidate.file_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&candidate.file_path);

            println!(
                "    • {} ({}) — score {:.1} • {}",
                candidate.name,
                priority_label(candidate.priority),
                candidate.score,
                file_name
            );
        }
    }

    if !result.warnings.is_empty() {
        println!();
        println!("  ⚠️ Warnings:");
        for warning in &result.warnings {
            println!("    • {}", warning.yellow());
        }
    }
}

/// Display quality gate failures and recommended remediation steps.
fn display_quality_failures(result: &QualityGateResult) {
    for violation in &result.violations {
        println!(
            "  ❌ {} - {} (current: {:.1}, threshold: {:.1})",
            violation.rule_name,
            violation.description,
            violation.current_value,
            violation.threshold
        );

        if !violation.recommended_actions.is_empty() {
            println!("     💡 Recommended actions:");
            for action in &violation.recommended_actions {
                println!("       • {}", action);
            }
        }
    }

    if !result.violations.is_empty() {
        println!(
            "  📊 Overall quality score: {:.1}/100",
            result.overall_score
        );
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
async fn generate_reports_with_oracle(
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
    args: &AnalyzeArgs,
) -> anyhow::Result<()> {
    let quiet_mode = is_quiet(args);

    if !quiet_mode {
        println!("{}", "📝 Generating Reports".bright_blue().bold());
    }

    let output_file = match args.format {
        OutputFormat::Json => {
            let file_path = args.out.join("analysis-results.json");
            let json_content = serde_json::to_string_pretty(result)
                .map_err(|e| anyhow::anyhow!("Failed to serialize JSON: {}", e))?;
            tokio::fs::write(&file_path, json_content)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write JSON report: {}", e))?;
            file_path
        }
        OutputFormat::Jsonl => {
            let file_path = args.out.join("analysis-results.jsonl");
            let json_content = serde_json::to_string(result)
                .map_err(|e| anyhow::anyhow!("Failed to serialize JSONL: {}", e))?;
            tokio::fs::write(&file_path, json_content)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write JSONL report: {}", e))?;
            file_path
        }
        OutputFormat::Yaml => {
            let file_path = args.out.join("analysis-results.yaml");
            let yaml_content = serde_yaml::to_string(result)
                .map_err(|e| anyhow::anyhow!("Failed to serialize YAML: {}", e))?;
            tokio::fs::write(&file_path, yaml_content)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write YAML report: {}", e))?;
            file_path
        }
        OutputFormat::Markdown => {
            let file_path = args.out.join("team-report.md");
            let result_json = serde_json::to_value(result)?;
            let markdown_content = super::output::generate_markdown_report(&result_json)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to generate markdown report: {}", e))?;
            tokio::fs::write(&file_path, markdown_content)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write markdown report: {}", e))?;
            file_path
        }
        OutputFormat::Html => {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let file_path = args.out.join(format!("report_{}.html", timestamp));

            // Use the proper ReportGenerator with Sibylline theme and oracle data
            let default_config = valknut_rs::api::config_types::AnalysisConfig::default();
            let generator = ReportGenerator::new().with_config(default_config);
            if let Some(oracle) = oracle_response {
                generator
                    .generate_report_with_oracle(result, oracle, &file_path, ReportFormat::Html)
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to generate HTML report with oracle: {}", e)
                    })?
            } else {
                generator
                    .generate_report(result, &file_path, ReportFormat::Html)
                    .map_err(|e| anyhow::anyhow!("Failed to generate HTML report: {}", e))?
            };

            file_path
        }
        OutputFormat::Sonar => {
            let file_path = args.out.join("sonarqube-issues.json");
            let result_json = serde_json::to_value(result)?;
            let sonar_content = super::output::generate_sonar_report(&result_json)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to generate SonarQube report: {}", e))?;
            tokio::fs::write(&file_path, sonar_content)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write SonarQube report: {}", e))?;
            file_path
        }
        OutputFormat::Csv => {
            let file_path = args.out.join("analysis-data.csv");
            let result_json = serde_json::to_value(result)?;
            let csv_content = super::output::generate_csv_report(&result_json)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to generate CSV report: {}", e))?;
            tokio::fs::write(&file_path, csv_content)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write CSV report: {}", e))?;
            file_path
        }
        _ => {
            // Default to JSON for other formats (with oracle data if available)
            let file_path = args.out.join("analysis-results.json");
            let combined_result = if let Some(oracle) = oracle_response {
                serde_json::json!({
                    "oracle_refactoring_plan": oracle,
                    "analysis_results": result
                })
            } else {
                serde_json::to_value(result)
                    .map_err(|e| anyhow::anyhow!("Failed to convert analysis to JSON: {}", e))?
            };
            let json_content = serde_json::to_string_pretty(&combined_result)
                .map_err(|e| anyhow::anyhow!("Failed to serialize JSON: {}", e))?;
            tokio::fs::write(&file_path, json_content)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write JSON report: {}", e))?;
            file_path
        }
    };

    if !quiet_mode {
        println!(
            "  ✅ Report saved: {}",
            output_file.display().to_string().cyan()
        );
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
    let DocAuditArgs {
        root,
        complexity_threshold,
        max_readme_commits,
        strict,
        format,
        ignore_dir,
        ignore_suffix,
        ignore,
        config,
    } = args;

    let implicit_config = [".valknut.docaudit.yml", ".valknut.docaudit.yaml"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists());
    let file_config = match config.or(implicit_config) {
        Some(path) => Some(load_doc_audit_config_file(&path)?),
        None => None,
    };

    let root_override = if root != PathBuf::from(".") {
        root.clone()
    } else {
        file_config
            .as_ref()
            .and_then(|c| c.root.clone())
            .unwrap_or_else(|| root.clone())
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

    let mut config = doc_audit::DocAuditConfig::new(root_path);

    if let Some(file_cfg) = file_config {
        if let Some(threshold) = file_cfg.complexity_threshold {
            config.complexity_threshold = threshold;
        }
        if let Some(commits) = file_cfg.max_readme_commits {
            config.max_readme_commits = commits;
        }
        for dir in file_cfg.ignore_dir {
            if !dir.trim().is_empty() {
                config.ignore_dirs.insert(dir);
            }
        }
        for suffix in file_cfg.ignore_suffix {
            if !suffix.trim().is_empty() {
                config.ignore_suffixes.insert(suffix);
            }
        }
        for glob in file_cfg.ignore {
            if !glob.trim().is_empty() {
                config.ignore_globs.push(glob);
            }
        }
    }

    config.complexity_threshold = complexity_threshold;
    config.max_readme_commits = max_readme_commits;

    for dir in ignore_dir {
        if !dir.trim().is_empty() {
            config.ignore_dirs.insert(dir);
        }
    }

    for suffix in ignore_suffix {
        if !suffix.trim().is_empty() {
            config.ignore_suffixes.insert(suffix);
        }
    }

    for glob in ignore {
        if !glob.trim().is_empty() {
            config.ignore_globs.push(glob);
        }
    }

    let result = doc_audit::run_audit(&config)?;

    match format {
        DocAuditFormat::Text => {
            println!("{}", doc_audit::render_text(&result));
        }
        DocAuditFormat::Json => {
            let payload = doc_audit::render_json(&result)?;
            println!("{payload}");
        }
    }

    if strict && result.has_issues() {
        anyhow::bail!("Documentation audit found issues");
    }

    Ok(())
}

/// Print Valknut header with version info
pub fn print_header() {
    let width = Term::stdout().size().1;
    for line in header_lines_for_width(width) {
        println!("{line}");
    }
    println!();
}

/// Build the stylized header lines to fit the given terminal width.
fn header_lines_for_width(width: u16) -> Vec<String> {
    if width >= 80 {
        vec![
            format!(
                "{}",
                "┌".cyan().bold().to_string()
                    + &"─".repeat(60).cyan().to_string()
                    + &"┐".cyan().bold().to_string()
            ),
            format!(
                "{} {} {}",
                "│".cyan().bold(),
                format!("⚙️  Valknut v{} - AI-Powered Code Analysis", VERSION)
                    .bright_cyan()
                    .bold(),
                "│".cyan().bold()
            ),
            format!(
                "{}",
                "└".cyan().bold().to_string()
                    + &"─".repeat(60).cyan().to_string()
                    + &"┘".cyan().bold().to_string()
            ),
        ]
    } else {
        vec![format!(
            "{} {}",
            "⚙️".bright_cyan(),
            format!("Valknut v{}", VERSION).bright_cyan().bold()
        )]
    }
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

#[allow(dead_code)]
/// Run comprehensive analysis with detailed progress tracking.
pub async fn run_analysis_with_progress(
    paths: &[PathBuf],
    _config: StructureConfig,
    args: &AnalyzeArgs,
) -> anyhow::Result<serde_json::Value> {
    use valknut_rs::core::config::{DenoiseConfig, ValknutConfig};
    use valknut_rs::core::pipeline::{AnalysisConfig, AnalysisPipeline, ProgressCallback};

    let quiet_mode = is_quiet(args);
    let multi_progress = MultiProgress::new();

    // Create main progress bar
    let main_pb = multi_progress.add(ProgressBar::new(100));
    main_pb.set_style(ProgressStyle::with_template(
        "🚀 {msg} [{bar:40.bright_blue/blue}] {pos:>3}% {elapsed_precise}",
    )?);
    main_pb.set_message("Comprehensive Analysis");

    // Create full ValknutConfig to properly configure denoising
    let mut valknut_config = ValknutConfig::default();
    valknut_config.analysis.enable_lsh_analysis = true;
    valknut_config.analysis.enable_coverage_analysis = true;
    if valknut_config.analysis.max_files == 0 {
        valknut_config.analysis.max_files = 5000;
    }
    if valknut_config.analysis.max_files == 0 {
        valknut_config.analysis.max_files = 5000;
    }

    // Apply CLI args to denoise configuration (enabled by default)
    let denoise_enabled = true; // force denoise defaults on when semantic clones are requested
    let auto_enabled = !args.advanced_clone.no_auto;

    if args.advanced_clone.no_apted_verify {
        valknut_config.lsh.verify_with_apted = false;
    } else if args.advanced_clone.apted_verify {
        valknut_config.lsh.verify_with_apted = true;
    }
    if let Some(max_nodes) = args.advanced_clone.apted_max_nodes {
        valknut_config.lsh.apted_max_nodes = max_nodes;
    }
    if let Some(max_pairs) = args.advanced_clone.apted_max_pairs {
        valknut_config.lsh.apted_max_pairs_per_entity = max_pairs;
    }

    if denoise_enabled {
        info!("Clone denoising enabled (advanced analysis mode)");
    } else {
        info!("Clone denoising disabled via --no-denoise flag");
    }

    // Configure denoise settings from CLI args with defaults
    let min_function_tokens = args.clone_detection.min_function_tokens.unwrap_or(40);
    let min_match_tokens = args.clone_detection.min_match_tokens.unwrap_or(24);
    let require_blocks = args.clone_detection.require_blocks.unwrap_or(2);
    let similarity = args.clone_detection.similarity.unwrap_or(0.82);

    // Apply advanced configuration if provided
    let mut weights = valknut_rs::core::config::DenoiseWeights::default();
    if let Some(ast_weight) = args.advanced_clone.ast_weight {
        weights.ast = ast_weight;
    }
    if let Some(pdg_weight) = args.advanced_clone.pdg_weight {
        weights.pdg = pdg_weight;
    }
    if let Some(emb_weight) = args.advanced_clone.emb_weight {
        weights.emb = emb_weight;
    }

    let io_mismatch_penalty = args.advanced_clone.io_mismatch_penalty.unwrap_or(0.25);

    // Configure auto-calibration settings
    let mut auto_calibration = valknut_rs::core::config::AutoCalibrationConfig {
        enabled: auto_enabled,
        ..Default::default()
    };
    if let Some(quality_target) = args.advanced_clone.quality_target {
        auto_calibration.quality_target = quality_target;
    }
    if let Some(sample_size) = args.advanced_clone.sample_size {
        auto_calibration.sample_size = sample_size;
    }

    // Configure ranking settings
    let mut ranking = valknut_rs::core::config::RankingConfig::default();
    if let Some(min_saved_tokens) = args.advanced_clone.min_saved_tokens {
        ranking.min_saved_tokens = min_saved_tokens;
    }
    if let Some(min_rarity_gain) = args.advanced_clone.min_rarity_gain {
        ranking.min_rarity_gain = min_rarity_gain;
    }

    valknut_config.denoise = DenoiseConfig {
        enabled: denoise_enabled,
        auto: auto_enabled,
        min_function_tokens,
        min_match_tokens,
        require_blocks,
        similarity,
        weights,
        io_mismatch_penalty,
        threshold_s: similarity,
        stop_motifs: valknut_rs::core::config::StopMotifsConfig::default(),
        auto_calibration,
        ranking,
        dry_run: args.clone_detection.denoise_dry_run,
    };

    // Enable rarity weighting when denoise is enabled
    if denoise_enabled {
        valknut_config.dedupe.adaptive.rarity_weighting = true;

        // Update LSH config to use k=9 for k-grams when denoising
        valknut_config.lsh.shingle_size = 9;

        info!("Denoise config - min_function_tokens: {}, min_match_tokens: {}, require_blocks: {}, similarity: {:.2}", 
              min_function_tokens, min_match_tokens, require_blocks, similarity);

        // Create denoise cache directories
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
    }

    // Apply CLI analysis disable/enable flags
    if args.coverage.no_coverage {
        valknut_config.analysis.enable_coverage_analysis = false;
    }
    if args.analysis_control.no_complexity {
        valknut_config.analysis.enable_scoring = false;
    }
    if args.analysis_control.no_structure {
        valknut_config.analysis.enable_structure_analysis = false;
    }
    if args.analysis_control.no_refactoring {
        valknut_config.analysis.enable_refactoring_analysis = false;
    }
    if args.analysis_control.no_impact {
        valknut_config.analysis.enable_graph_analysis = false;
    }
    if args.analysis_control.no_lsh {
        valknut_config.analysis.enable_lsh_analysis = false;
    }

    // Configure coverage analysis from CLI args
    let mut coverage_config = valknut_rs::core::config::CoverageConfig::default();
    if let Some(coverage_file) = &args.coverage.coverage_file {
        coverage_config.coverage_file = Some(coverage_file.clone());
        coverage_config.auto_discover = false; // Explicit file overrides discovery
    }
    if args.coverage.no_coverage_auto_discover {
        coverage_config.auto_discover = false;
    }
    if let Some(max_age_days) = args.coverage.coverage_max_age_days {
        coverage_config.max_age_days = max_age_days;
    }

    valknut_config.coverage = coverage_config;

    let pipeline_config = PipelineAnalysisConfig::from(valknut_config.clone());

    // Log analysis configuration
    let enabled_analyses = vec![
        ("Complexity", pipeline_config.enable_complexity_analysis),
        ("Structure", pipeline_config.enable_structure_analysis),
        ("Refactoring", pipeline_config.enable_refactoring_analysis),
        ("Impact", pipeline_config.enable_impact_analysis),
        ("Clone Detection (LSH)", pipeline_config.enable_lsh_analysis),
        ("Coverage", pipeline_config.enable_coverage_analysis),
    ];

    if !quiet_mode {
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

    let pipeline = AnalysisPipeline::new_with_config(pipeline_config, valknut_config);

    // Create progress callback
    let progress_callback: ProgressCallback = Box::new({
        let pb = main_pb.clone();
        move |stage: &str, progress: f64| {
            pb.set_message(stage.to_string());
            pb.set_position(progress as u64);
        }
    });

    // Run comprehensive analysis
    info!("Starting comprehensive analysis for {} paths", paths.len());
    let analysis_result = pipeline
        .analyze_paths(paths, Some(progress_callback))
        .await
        .map_err(|e| anyhow::anyhow!("Analysis failed: {}", e))?;

    // Finish progress bar
    main_pb.finish_with_message("Analysis Complete");

    // Convert to JSON format matching the expected structure
    let result_json = serde_json::to_value(&analysis_result)?;

    info!("Analysis completed successfully");
    info!("Total files: {}", analysis_result.summary.total_files);
    info!("Total issues: {}", analysis_result.summary.total_issues);
    info!(
        "Overall health score: {:.1}",
        analysis_result.health_metrics.overall_health_score
    );

    Ok(result_json)
}

#[allow(dead_code)]
/// Run analysis without progress bars for quiet mode.
pub async fn run_analysis_without_progress(
    paths: &[PathBuf],
    _config: StructureConfig,
    args: &AnalyzeArgs,
) -> anyhow::Result<serde_json::Value> {
    use valknut_rs::core::config::{DenoiseConfig, ValknutConfig};
    use valknut_rs::core::pipeline::{AnalysisConfig as PipelineAnalysisConfig, AnalysisPipeline};

    // Create full ValknutConfig to properly configure denoising
    let mut valknut_config = ValknutConfig::default();
    valknut_config.analysis.enable_lsh_analysis = true;
    valknut_config.analysis.enable_coverage_analysis = true;

    // Apply CLI args to denoise configuration (enabled by default)
    let denoise_enabled = true; // force denoise defaults on when semantic clones are requested
    let auto_enabled = !args.advanced_clone.no_auto;

    if args.advanced_clone.no_apted_verify {
        valknut_config.lsh.verify_with_apted = false;
    } else if args.advanced_clone.apted_verify {
        valknut_config.lsh.verify_with_apted = true;
    }
    if let Some(max_nodes) = args.advanced_clone.apted_max_nodes {
        valknut_config.lsh.apted_max_nodes = max_nodes;
    }
    if let Some(max_pairs) = args.advanced_clone.apted_max_pairs {
        valknut_config.lsh.apted_max_pairs_per_entity = max_pairs;
    }

    if denoise_enabled {
        info!("Clone denoising enabled (advanced analysis mode)");
    } else {
        info!("Clone denoising disabled via --no-denoise flag");
    }

    // Configure denoise settings from CLI args with defaults
    let min_function_tokens = args.clone_detection.min_function_tokens.unwrap_or(40);
    let min_match_tokens = args.clone_detection.min_match_tokens.unwrap_or(24);
    let require_blocks = args.clone_detection.require_blocks.unwrap_or(2);
    let similarity = args.clone_detection.similarity.unwrap_or(0.82);

    // Apply advanced configuration if provided
    let mut weights = valknut_rs::core::config::DenoiseWeights::default();
    if let Some(ast_weight) = args.advanced_clone.ast_weight {
        weights.ast = ast_weight;
    }
    if let Some(pdg_weight) = args.advanced_clone.pdg_weight {
        weights.pdg = pdg_weight;
    }
    if let Some(emb_weight) = args.advanced_clone.emb_weight {
        weights.emb = emb_weight;
    }

    let io_mismatch_penalty = args.advanced_clone.io_mismatch_penalty.unwrap_or(0.25);

    // Configure auto-calibration settings
    let mut auto_calibration = valknut_rs::core::config::AutoCalibrationConfig {
        enabled: auto_enabled,
        ..Default::default()
    };
    if let Some(quality_target) = args.advanced_clone.quality_target {
        auto_calibration.quality_target = quality_target;
    }
    if let Some(sample_size) = args.advanced_clone.sample_size {
        auto_calibration.sample_size = sample_size;
    }

    // Configure ranking settings
    let mut ranking = valknut_rs::core::config::RankingConfig::default();
    if let Some(min_saved_tokens) = args.advanced_clone.min_saved_tokens {
        ranking.min_saved_tokens = min_saved_tokens;
    }
    if let Some(min_rarity_gain) = args.advanced_clone.min_rarity_gain {
        ranking.min_rarity_gain = min_rarity_gain;
    }

    valknut_config.denoise = DenoiseConfig {
        enabled: denoise_enabled,
        auto: auto_enabled,
        min_function_tokens,
        min_match_tokens,
        require_blocks,
        similarity,
        weights,
        io_mismatch_penalty,
        threshold_s: similarity,
        stop_motifs: valknut_rs::core::config::StopMotifsConfig::default(),
        auto_calibration,
        ranking,
        dry_run: args.clone_detection.denoise_dry_run,
    };

    // Enable rarity weighting when denoise is enabled
    if denoise_enabled {
        valknut_config.dedupe.adaptive.rarity_weighting = true;

        // Update LSH config to use k=9 for k-grams when denoising
        valknut_config.lsh.shingle_size = 9;

        info!("Denoise config - min_function_tokens: {}, min_match_tokens: {}, require_blocks: {}, similarity: {:.2}", 
              min_function_tokens, min_match_tokens, require_blocks, similarity);

        // Create denoise cache directories
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
    }

    // Apply CLI analysis disable/enable flags
    if args.coverage.no_coverage {
        valknut_config.analysis.enable_coverage_analysis = false;
    }
    if args.analysis_control.no_complexity {
        valknut_config.analysis.enable_scoring = false;
    }
    if args.analysis_control.no_structure {
        valknut_config.analysis.enable_structure_analysis = false;
    }
    if args.analysis_control.no_refactoring {
        valknut_config.analysis.enable_refactoring_analysis = false;
    }
    if args.analysis_control.no_impact {
        valknut_config.analysis.enable_graph_analysis = false;
    }
    if args.analysis_control.no_lsh {
        valknut_config.analysis.enable_lsh_analysis = false;
    }

    // Configure coverage analysis from CLI args
    let mut coverage_config = valknut_rs::core::config::CoverageConfig::default();
    if let Some(coverage_file) = &args.coverage.coverage_file {
        coverage_config.coverage_file = Some(coverage_file.clone());
        coverage_config.auto_discover = false; // Explicit file overrides discovery
    }
    if args.coverage.no_coverage_auto_discover {
        coverage_config.auto_discover = false;
    }
    if let Some(max_age_days) = args.coverage.coverage_max_age_days {
        coverage_config.max_age_days = max_age_days;
    }

    valknut_config.coverage = coverage_config;

    let pipeline_config = PipelineAnalysisConfig::from(valknut_config.clone());
    let pipeline = AnalysisPipeline::new_with_config(pipeline_config, valknut_config);

    // Run comprehensive analysis without progress callback
    info!("Starting comprehensive analysis for {} paths", paths.len());
    let analysis_result = pipeline
        .analyze_paths(paths, None)
        .await
        .map_err(|e| anyhow::anyhow!("Analysis failed: {}", e))?;

    // Convert to JSON format matching the expected structure
    let result_json = serde_json::to_value(&analysis_result)?;

    info!("Analysis completed successfully");
    info!("Total files: {}", analysis_result.summary.total_files);
    info!("Total issues: {}", analysis_result.summary.total_issues);
    info!(
        "Overall health score: {:.1}",
        analysis_result.health_metrics.overall_health_score
    );

    Ok(result_json)
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
        println!("{} {}", "⚠️".yellow(), message);
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

    // Group violations by severity
    let blockers: Vec<_> = result
        .violations
        .iter()
        .filter(|v| v.severity == "Blocker")
        .collect();
    let criticals: Vec<_> = result
        .violations
        .iter()
        .filter(|v| v.severity == "Critical")
        .collect();
    let warnings: Vec<_> = result
        .violations
        .iter()
        .filter(|v| v.severity == "Warning" || v.severity == "High")
        .collect();

    if !blockers.is_empty() {
        println!("{}", "🚫 BLOCKER Issues:".red().bold());
        for violation in blockers {
            println!(
                "  • {}: {:.1} (threshold: {:.1})",
                violation.rule_name.yellow(),
                violation.current_value,
                violation.threshold
            );
            println!("    {}", violation.description.dimmed());
        }
        println!();
    }

    if !criticals.is_empty() {
        println!("{}", "🔴 CRITICAL Issues:".red().bold());
        for violation in criticals {
            println!(
                "  • {}: {:.1} (threshold: {:.1})",
                violation.rule_name.yellow(),
                violation.current_value,
                violation.threshold
            );
            println!("    {}", violation.description.dimmed());
        }
        println!();
    }

    if !warnings.is_empty() {
        println!("{}", "⚠️  WARNING Issues:".yellow().bold());
        for violation in warnings {
            println!(
                "  • {}: {:.1} (threshold: {:.1})",
                violation.rule_name.yellow(),
                violation.current_value,
                violation.threshold
            );
            println!("    {}", violation.description.dimmed());
        }
        println!();
    }

    println!("{}", "To fix these issues:".bold());
    println!("  1. Reduce code complexity by refactoring large functions");
    println!("  2. Address critical and high-priority issues first");
    println!("  3. Improve code maintainability through better structure");
    println!("  4. Reduce technical debt by following best practices");
    println!();
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
            config
        }
        Err(e) => {
            eprintln!("{} {}", "❌ Oracle configuration failed:".red(), e);
            eprintln!(
                "   {}",
                "Set the GEMINI_API_KEY environment variable to use the oracle feature".dimmed()
            );
            return Ok(None);
        }
    };

    let oracle = RefactoringOracle::new(oracle_config);

    // Use the first path as the project root for analysis
    let project_path = paths.first().unwrap();

    if !quiet_mode {
        println!(
            "  🔍 Analyzing project: {}",
            project_path.display().to_string().cyan()
        );
        println!("  🧠 Sending to Gemini 2.5 Pro for intelligent refactoring suggestions...");
    }

    match oracle
        .generate_suggestions(project_path, analysis_result)
        .await
    {
        Ok(response) => {
            if !quiet_mode {
                println!("  ✅ Oracle analysis completed successfully!");
                println!(
                    "  📊 Generated {} refactoring phases with {} total tasks",
                    response.refactoring_plan.phases.len().to_string().green(),
                    response
                        .refactoring_plan
                        .phases
                        .iter()
                        .map(|p| p.subsystems.iter().map(|s| s.tasks.len()).sum::<usize>())
                        .sum::<usize>()
                        .to_string()
                        .green()
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
                    println!(
                        "  💾 Oracle recommendations saved to: {}",
                        oracle_path.display().to_string().cyan()
                    );
                }
            }

            Ok(Some(response))
        }
        Err(e) => {
            if !quiet_mode {
                eprintln!("{} Oracle analysis failed: {}", "⚠️".yellow(), e);
                eprintln!(
                    "   {}",
                    "Analysis will continue without oracle suggestions".dimmed()
                );
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
mod tests {
    use super::*;
    use anyhow::Result;
    use gag::BufferRedirect;
    use serial_test::serial;
    use std::collections::HashMap;
    use std::time::Duration;
    use std::{
        env, fs,
        io::{Read, Write},
    };
    use tempfile::{NamedTempFile, TempDir};
    use tokio::runtime::Runtime;
    use valknut_rs::api::results::{
        AnalysisResults, AnalysisStatistics, AnalysisSummary, CloneAnalysisResults,
        FeatureContribution, MemoryStats, RefactoringIssue, RefactoringSuggestion,
    };
    use valknut_rs::core::config::{CoverageConfig, ValknutConfig};
    use valknut_rs::core::pipeline::{
        CodeDictionary, HealthMetrics, QualityGateConfig, QualityGateViolation,
    };
    use valknut_rs::oracle::{
        CodebaseAssessment, IdentifiedRisk, RefactoringOracleResponse, RefactoringPhase,
        RefactoringPlan, RefactoringSubsystem, RefactoringTask, RiskAssessment,
    };

    struct ColorOverrideGuard {
        previous: Option<String>,
    }

    impl ColorOverrideGuard {
        fn new() -> Self {
            let previous = env::var("NO_COLOR").ok();
            env::set_var("NO_COLOR", "1");
            Self { previous }
        }
    }

    impl Drop for ColorOverrideGuard {
        fn drop(&mut self) {
            if let Some(ref value) = self.previous {
                env::set_var("NO_COLOR", value);
            } else {
                env::remove_var("NO_COLOR");
            }
        }
    }

    fn capture_stdout<F: FnOnce()>(action: F) -> String {
        let mut buffer = Vec::new();
        {
            let _color_guard = ColorOverrideGuard::new();
            if let Ok(mut redirect) = BufferRedirect::stdout() {
                action();
                std::io::stdout().flush().expect("flush stdout");
                redirect
                    .read_to_end(&mut buffer)
                    .expect("read captured stdout");
            } else {
                // If stdout is already redirected (rare in concurrent tests), just run the action.
                action();
            }
        }
        String::from_utf8(buffer).expect("stdout should be valid utf8")
    }

    fn sample_candidate(path: &str, priority: Priority, score: f64) -> RefactoringCandidate {
        RefactoringCandidate {
            entity_id: format!("{path}::entity"),
            name: "entity".to_string(),
            file_path: path.to_string(),
            line_range: Some((1, 20)),
            priority,
            score,
            confidence: 0.85,
            issues: vec![RefactoringIssue {
                code: "CMPLX".to_string(),
                category: "complexity".to_string(),
                severity: 1.2,
                contributing_features: vec![FeatureContribution {
                    feature_name: "cyclomatic_complexity".to_string(),
                    value: 18.0,
                    normalized_value: 0.7,
                    contribution: 1.3,
                }],
            }],
            suggestions: vec![RefactoringSuggestion {
                refactoring_type: "extract_method".to_string(),
                code: "XTRMTH".to_string(),
                priority: 0.9,
                effort: 0.4,
                impact: 0.85,
            }],
            issue_count: 1,
            suggestion_count: 1,
        }
    }

    // Helper function to create default AnalyzeArgs for tests
    fn create_default_analyze_args() -> AnalyzeArgs {
        AnalyzeArgs {
            paths: vec![PathBuf::from("test")],
            out: PathBuf::from("output"),
            format: OutputFormat::Json,
            config: None,
            quiet: false,
            profile: PerformanceProfile::Balanced,
            quality_gate: QualityGateArgs {
                quality_gate: false,
                fail_on_issues: false,
                max_complexity: None,
                min_health: None,
                min_doc_health: None,
                max_debt: None,
                min_maintainability: None,
                max_issues: None,
                max_critical: None,
                max_high_priority: None,
            },
            clone_detection: CloneDetectionArgs {
                semantic_clones: false,
                strict_dedupe: false,
                denoise: false,
                min_function_tokens: None,
                min_match_tokens: None,
                require_blocks: None,
                similarity: None,
                denoise_dry_run: false,
            },
            advanced_clone: AdvancedCloneArgs {
                no_auto: false,
                loose_sweep: false,
                rarity_weighting: false,
                structural_validation: false,
                apted_verify: false,
                apted_max_nodes: None,
                apted_max_pairs: None,
                no_apted_verify: false,
                live_reach_boost: false,
                ast_weight: None,
                pdg_weight: None,
                emb_weight: None,
                io_mismatch_penalty: None,
                quality_target: None,
                sample_size: None,
                min_saved_tokens: None,
                min_rarity_gain: None,
            },
            coverage: CoverageArgs {
                no_coverage: false,
                coverage_file: None,
                no_coverage_auto_discover: false,
                coverage_max_age_days: None,
            },
            analysis_control: AnalysisControlArgs {
                no_complexity: false,
                no_structure: false,
                no_refactoring: false,
                no_impact: false,
                no_lsh: false,
            },
            ai_features: AIFeaturesArgs {
                oracle: false,
                oracle_max_tokens: None,
            },
        }
    }

    fn create_doc_args(root: PathBuf) -> DocAuditArgs {
        DocAuditArgs {
            root,
            complexity_threshold: usize::MAX,
            max_readme_commits: usize::MAX,
            strict: false,
            format: DocAuditFormat::Text,
            ignore_dir: vec![],
            ignore_suffix: vec![],
            ignore: vec![],
            config: None,
        }
    }

    struct DirGuard {
        original: PathBuf,
    }

    impl DirGuard {
        fn change_to(path: &Path) -> Self {
            let original = env::current_dir().expect("read current dir");
            env::set_current_dir(path).expect("set current dir");
            Self { original }
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.original);
        }
    }

    fn create_sample_analysis_project() -> TempDir {
        let project = TempDir::new().expect("temp project");
        let root = project.path();

        fs::write(
            root.join("analytics.py"),
            r#"
def compute(values):
    total = sum(values)
    return total / max(len(values), 1)

def duplicate(values):
    return [value for value in values if value > 0]
"#,
        )
        .expect("write python file");

        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn helper(value: i32) -> i32 {
    if value > 0 {
        value + 1
    } else {
        value - 1
    }
}
"#,
        )
        .expect("write rust file");

        fs::write(
            root.join("metrics.ts"),
            r#"
export function accumulate(values: number[]): number {
    return values.reduce((sum, value) => sum + value, 0);
}
"#,
        )
        .expect("write ts file");

        project
    }

    fn write_lcov_fixture(root: &Path) -> PathBuf {
        let coverage_dir = root.join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        let file = coverage_dir.join("coverage.lcov");
        fs::write(
            &file,
            "TN:valknut\nSF:src/lib.rs\nFN:2,helper\nFNF:1\nFNH:1\nFNDA:4,helper\nDA:2,4\nDA:3,4\nDA:4,4\nDA:5,4\nLF:4\nLH:4\nend_of_record\n",
        )
        .expect("write coverage");
        file
    }

    fn sample_analysis_results() -> AnalysisResults {
        let candidate = sample_candidate("src/lib.rs", Priority::High, 2.5);

        AnalysisResults {
            summary: AnalysisSummary {
                files_processed: 1,
                entities_analyzed: 1,
                refactoring_needed: 1,
                high_priority: 1,
                critical: 0,
                avg_refactoring_score: 0.75,
                code_health_score: 0.65,
                total_files: 1,
                total_entities: 1,
                total_lines_of_code: 120,
                languages: vec!["Rust".to_string()],
                total_issues: 1,
                high_priority_issues: 1,
                critical_issues: 0,
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
            normalized: None,
            passes: valknut_rs::api::results::StageResultsBundle::disabled(),
            refactoring_candidates: vec![candidate],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_millis(25),
                avg_file_processing_time: Duration::from_millis(25),
                avg_entity_processing_time: Duration::from_millis(25),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 2048,
                    final_memory_bytes: 1024,
                    efficiency_score: 0.9,
                },
            },
            health_metrics: Some(HealthMetrics {
                overall_health_score: 72.0,
                maintainability_score: 70.0,
                technical_debt_ratio: 25.0,
                complexity_score: 45.0,
                structure_quality_score: 78.0,
                doc_health_score: 100.0,
            }),
            clone_analysis: None,
            coverage_packs: Vec::new(),
            warnings: Vec::new(),
            code_dictionary: CodeDictionary::default(),
            documentation: None,
        }
    }

    fn sample_oracle_response() -> RefactoringOracleResponse {
        RefactoringOracleResponse {
            assessment: CodebaseAssessment {
                health_score: 70,
                strengths: vec!["Modular design".to_string()],
                weaknesses: vec!["Clone density".to_string()],
                architecture_quality: "Improving".to_string(),
                organization_quality: "Good".to_string(),
            },
            refactoring_plan: RefactoringPlan {
                phases: vec![RefactoringPhase {
                    id: "phase-1".to_string(),
                    name: "Stabilize Core Modules".to_string(),
                    description: "Address hotspots in core utilities.".to_string(),
                    priority: 1,
                    subsystems: vec![RefactoringSubsystem {
                        id: "core-utils".to_string(),
                        name: "Core Utilities".to_string(),
                        affected_files: vec!["src/lib.rs".to_string()],
                        tasks: vec![RefactoringTask {
                            id: "task-1".to_string(),
                            title: "Extract helper utilities".to_string(),
                            description: "Split monolithic helper into focused modules."
                                .to_string(),
                            task_type: "refactor".to_string(),
                            files: vec!["src/lib.rs".to_string()],
                            risk_level: "Low".to_string(),
                            benefits: vec!["Improved readability".to_string()],
                        }],
                    }],
                }],
            },
            risk_assessment: RiskAssessment {
                overall_risk: "Moderate".to_string(),
                risks: vec![IdentifiedRisk {
                    category: "Regression".to_string(),
                    description: "Potential behaviour regressions during extraction".to_string(),
                    probability: "Medium".to_string(),
                    impact: "Medium".to_string(),
                    mitigation: "Add focused regression tests".to_string(),
                }],
                mitigation_strategies: vec!["Increase automated test coverage".to_string()],
            },
        }
    }

    #[test]
    fn output_format_machine_readable_detection() {
        assert!(OutputFormat::Json.is_machine_readable());
        assert!(OutputFormat::Jsonl.is_machine_readable());
        assert!(OutputFormat::Yaml.is_machine_readable());
        assert!(OutputFormat::Csv.is_machine_readable());
        assert!(OutputFormat::Sonar.is_machine_readable());
        assert!(OutputFormat::CiSummary.is_machine_readable());
        assert!(!OutputFormat::Markdown.is_machine_readable());
        assert!(!OutputFormat::Html.is_machine_readable());
        assert!(!OutputFormat::Pretty.is_machine_readable());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_respects_quiet_mode() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = true;
        args.format = OutputFormat::Json;

        let result = sample_analysis_results();
        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("json report generation should succeed");

        assert!(temp.path().join("analysis-results.json").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_html_with_ai_data() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Html;

        let result = sample_analysis_results();
        let oracle = sample_oracle_response();

        generate_reports_with_oracle(&result, &Some(oracle), &args)
            .await
            .expect("html report generation should succeed");

        let html_count = fs::read_dir(temp.path())
            .expect("read output dir")
            .filter(|entry| {
                entry
                    .as_ref()
                    .ok()
                    .and_then(|e| e.path().extension().map(|ext| ext == "html"))
                    .unwrap_or(false)
            })
            .count();

        assert!(
            html_count > 0,
            "expected at least one html report in {:?}",
            temp.path()
        );
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_markdown() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Markdown;

        let result = sample_analysis_results();

        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("markdown report generation should succeed");

        assert!(temp.path().join("team-report.md").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_csv() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Csv;

        let result = sample_analysis_results();

        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("csv report generation should succeed");

        assert!(temp.path().join("analysis-data.csv").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_yaml() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Yaml;

        let result = sample_analysis_results();

        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("yaml report generation should succeed");

        assert!(temp.path().join("analysis-results.yaml").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_jsonl() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Jsonl;

        let result = sample_analysis_results();

        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("jsonl report generation should succeed");

        assert!(temp.path().join("analysis-results.jsonl").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_sonar() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Sonar;

        let result = sample_analysis_results();

        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("sonar report generation should succeed");

        assert!(temp.path().join("sonarqube-issues.json").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_combines_for_ci_summary() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::CiSummary;
        args.ai_features.oracle = true;

        let result = sample_analysis_results();
        let oracle = sample_oracle_response();

        generate_reports_with_oracle(&result, &Some(oracle), &args)
            .await
            .expect("ci summary should fall back to combined json");

        let combined_path = temp.path().join("analysis-results.json");
        assert!(combined_path.exists());
        let contents = fs::read_to_string(combined_path).expect("read combined output");
        assert!(
            contents.contains("oracle_refactoring_plan"),
            "combined report should include oracle data"
        );
    }

    #[test]
    fn evaluate_quality_gates_disabled_returns_health_score() {
        let result = sample_analysis_results();
        let expected = result
            .health_metrics
            .as_ref()
            .map(|m| m.overall_health_score)
            .unwrap();

        let config = QualityGateConfig {
            enabled: false,
            ..Default::default()
        };

        let gate = evaluate_quality_gates(&result, &config, false)
            .expect("quality gate evaluation succeeds");

        assert!(gate.passed);
        assert!(gate.violations.is_empty());
        assert_eq!(gate.overall_score, expected);
    }

    #[test]
    fn evaluate_quality_gates_reports_violations() {
        let mut result = sample_analysis_results();
        result.summary.critical = 3;
        result.summary.high_priority = 4;
        result.summary.total_issues = 7;
        if let Some(metrics) = result.health_metrics.as_mut() {
            metrics.complexity_score = 88.0;
            metrics.technical_debt_ratio = 65.0;
            metrics.maintainability_score = 52.0;
            metrics.doc_health_score = 10.0;
        }

        let config = QualityGateConfig {
            enabled: true,
            min_health_score: QualityGateConfig::default().min_health_score,
            min_doc_health_score: 50.0,
            max_complexity_score: 55.0,
            max_technical_debt_ratio: 25.0,
            min_maintainability_score: 85.0,
            max_critical_issues: 1,
            max_high_priority_issues: 2,
        };

        let gate = evaluate_quality_gates(&result, &config, false)
            .expect("quality gate evaluation succeeds");

        assert!(!gate.passed);
        assert!(!gate.violations.is_empty());

        let rule_names: Vec<_> = gate
            .violations
            .iter()
            .map(|v| v.rule_name.as_str())
            .collect();
        assert!(rule_names.contains(&"Complexity Threshold"));
        assert!(rule_names.contains(&"Technical Debt Ratio"));
        assert!(rule_names.contains(&"Maintainability Score"));
        assert!(rule_names.contains(&"Critical Issues"));
        assert!(rule_names.contains(&"High Priority Issues"));

        assert!(
            gate.violations
                .iter()
                .all(|v| !v.recommended_actions.is_empty()),
            "violations should include actionable guidance"
        );
    }

    #[test]
    fn evaluate_quality_gates_handles_missing_metrics_when_verbose() {
        let mut result = sample_analysis_results();
        result.health_metrics = None;

        let config = QualityGateConfig {
            enabled: true,
            min_health_score: QualityGateConfig::default().min_health_score,
            min_doc_health_score: 0.0,
            max_complexity_score: 90.0,
            max_technical_debt_ratio: 90.0,
            min_maintainability_score: 10.0,
            max_critical_issues: 10,
            max_high_priority_issues: 10,
        };

        let gate = evaluate_quality_gates(&result, &config, true)
            .expect("quality gate evaluation succeeds");

        assert!(gate.passed);
        assert!(gate.violations.is_empty());
        assert!(
            (gate.overall_score - (result.summary.code_health_score * 100.0)).abs() < f64::EPSILON
        );
    }

    #[tokio::test]
    #[serial]
    async fn run_oracle_analysis_returns_none_without_api_key() {
        // Ensure GEMINI_API_KEY is unset for this test
        std::env::remove_var("GEMINI_API_KEY");

        let project = create_sample_analysis_project();
        let mut args = create_default_analyze_args();
        args.paths = vec![project.path().to_path_buf()];
        args.ai_features.oracle = true;

        let result = run_oracle_analysis(
            &[project.path().to_path_buf()],
            &sample_analysis_results(),
            &args,
        )
        .await
        .expect("oracle analysis should not error when key missing");

        assert!(
            result.is_none(),
            "Oracle should be skipped when GEMINI_API_KEY is absent"
        );
    }

    #[tokio::test]
    #[serial]
    async fn run_oracle_analysis_handles_generation_error() {
        // Provide a dummy API key to exercise request failure path
        std::env::set_var("GEMINI_API_KEY", "test-api-key");

        let project = create_sample_analysis_project();
        let mut args = create_default_analyze_args();
        args.paths = vec![project.path().to_path_buf()];
        args.ai_features.oracle = true;
        args.ai_features.oracle_max_tokens = Some(256);

        let oracle_result = run_oracle_analysis(
            &[project.path().to_path_buf()],
            &sample_analysis_results(),
            &args,
        )
        .await
        .expect("oracle analysis should gracefully handle request failures");

        assert!(
            oracle_result.is_none(),
            "Oracle failures should not propagate fatal errors"
        );

        std::env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn doc_audit_command_rejects_missing_root() {
        let args = create_doc_args(PathBuf::from("./does-not-exist"));
        assert!(doc_audit_command(args).is_err());
    }

    #[test]
    fn doc_audit_command_generates_report() {
        let temp = TempDir::new().expect("temp dir");
        fs::write(
            temp.path().join("lib.rs"),
            "/// docs\npub fn documented() {}\n",
        )
        .expect("write file");

        let mut args = create_doc_args(temp.path().to_path_buf());
        args.format = DocAuditFormat::Json;
        doc_audit_command(args).expect("doc audit should succeed");
    }

    #[test]
    fn doc_audit_command_strict_flags_issues() {
        let temp = TempDir::new().expect("temp dir");
        fs::write(temp.path().join("main.rs"), "pub fn missing_docs() {}\n").expect("write file");

        let mut args = create_doc_args(temp.path().to_path_buf());
        args.strict = true;
        let err = doc_audit_command(args).expect_err("strict mode should fail");
        assert!(err.to_string().contains("Documentation audit found issues"));
    }

    #[test]
    fn is_quiet_respects_format_overrides() {
        let mut args = create_default_analyze_args();
        args.quiet = false;
        args.format = OutputFormat::Json;
        assert!(super::is_quiet(&args));

        args.format = OutputFormat::Pretty;
        assert!(!super::is_quiet(&args));

        args.quiet = true;
        assert!(super::is_quiet(&args));
    }

    #[test]
    fn test_print_header() {
        // Test that print_header doesn't panic
        print_header();
    }

    #[test]
    fn test_header_lines_for_wide_terminal() {
        let lines = header_lines_for_width(120);
        assert_eq!(lines.len(), 3);
        assert!(
            lines[1].contains("Valknut"),
            "expected middle header line to mention Valknut"
        );
    }

    #[test]
    fn test_header_lines_for_narrow_terminal() {
        let lines = header_lines_for_width(40);
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].contains("Valknut"),
            "expected compact header to mention Valknut"
        );
    }

    #[test]
    fn test_format_to_string() {
        assert_eq!(format_to_string(&OutputFormat::Json), "json");
        assert_eq!(format_to_string(&OutputFormat::Yaml), "yaml");
        assert_eq!(format_to_string(&OutputFormat::Markdown), "markdown");
        assert_eq!(format_to_string(&OutputFormat::Html), "html");
        assert_eq!(format_to_string(&OutputFormat::Jsonl), "jsonl");
        assert_eq!(format_to_string(&OutputFormat::Sonar), "sonar");
        assert_eq!(format_to_string(&OutputFormat::Csv), "csv");
        assert_eq!(format_to_string(&OutputFormat::CiSummary), "ci-summary");
        assert_eq!(format_to_string(&OutputFormat::Pretty), "pretty");
    }

    #[test]
    fn test_display_config_summary() {
        let config = StructureConfig::default();
        // Test that display_config_summary doesn't panic
        display_config_summary(&config);
    }

    #[tokio::test]
    async fn test_load_configuration_default() {
        let result = load_configuration(None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_configuration_yaml_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let result = load_configuration(Some(temp_file.path())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_configuration_json_file() {
        let temp_dir = TempDir::new().unwrap();
        let json_path = temp_dir.path().join("config.json");
        let config = StructureConfig::default();
        let json_content = serde_json::to_string(&config).unwrap();
        fs::write(&json_path, json_content).unwrap();

        let result = load_configuration(Some(&json_path)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_configuration_invalid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), "invalid: yaml: content:").unwrap();

        let result = load_configuration(Some(temp_file.path())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_print_default_config() {
        let result = print_default_config().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_init_config_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yml");

        let args = InitConfigArgs {
            output: config_path.clone(),
            force: false,
        };

        let result = init_config(args).await;
        assert!(result.is_ok());
        assert!(config_path.exists());

        // Verify file contains valid YAML
        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: serde_yaml::Result<valknut_rs::core::config::ValknutConfig> =
            serde_yaml::from_str(&content);
        assert!(parsed.is_ok());
    }

    #[tokio::test]
    async fn test_init_config_force_overwrite() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("existing_config.yml");

        // Create existing file
        fs::write(&config_path, "existing content").unwrap();

        let args = InitConfigArgs {
            output: config_path.clone(),
            force: true,
        };

        let result = init_config(args).await;
        assert!(result.is_ok());

        // Verify file was overwritten with valid YAML
        let content = fs::read_to_string(&config_path).unwrap();
        assert_ne!(content, "existing content");
        let parsed: serde_yaml::Result<valknut_rs::core::config::ValknutConfig> =
            serde_yaml::from_str(&content);
        assert!(parsed.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_valid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let args = ValidateConfigArgs {
            config: temp_file.path().to_path_buf(),
            verbose: false,
        };

        let result = validate_config(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_verbose() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let args = ValidateConfigArgs {
            config: temp_file.path().to_path_buf(),
            verbose: true,
        };

        let result = validate_config(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_stdio_command() {
        let args = McpStdioArgs { config: None };

        let result = mcp_stdio_command(args, false, SurveyVerbosity::Low).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_stdio_command_with_config() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let args = McpStdioArgs {
            config: Some(temp_file.path().to_path_buf()),
        };

        let result = mcp_stdio_command(args, true, SurveyVerbosity::High).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_manifest_command_stdout() {
        let args = McpManifestArgs { output: None };

        let result = mcp_manifest_command(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_manifest_command_file_output() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("manifest.json");

        let args = McpManifestArgs {
            output: Some(manifest_path.clone()),
        };

        let result = mcp_manifest_command(args).await;
        assert!(result.is_ok());
        assert!(manifest_path.exists());

        // Verify file contains valid JSON
        let content = fs::read_to_string(&manifest_path).unwrap();
        let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(&content);
        assert!(parsed.is_ok());

        let manifest = parsed.unwrap();
        assert_eq!(manifest["name"], "valknut");
        assert!(manifest["capabilities"]["tools"].is_array());
    }

    #[tokio::test]
    async fn test_list_languages() {
        let result = list_languages().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_quality_gate_config_defaults() {
        let args = create_default_analyze_args();

        let config = build_quality_gate_config(&args);
        assert!(!config.enabled);
    }

    #[test]
    fn test_build_quality_gate_config_quality_gate_enabled() {
        let mut args = create_default_analyze_args();
        args.quality_gate.quality_gate = true;
        args.quality_gate.max_complexity = Some(75.0);
        args.quality_gate.min_health = Some(60.0);
        args.quality_gate.max_debt = Some(30.0);
        args.quality_gate.min_maintainability = Some(65.0);
        args.quality_gate.max_issues = Some(10);
        args.quality_gate.max_critical = Some(5);
        args.quality_gate.max_high_priority = Some(15);

        let config = build_quality_gate_config(&args);
        assert!(config.enabled);
        assert_eq!(config.max_complexity_score, 75.0);
        assert_eq!(config.min_maintainability_score, 65.0);
        assert_eq!(config.max_technical_debt_ratio, 30.0);
        assert_eq!(config.max_critical_issues, 5);
        assert_eq!(config.max_high_priority_issues, 15);
    }

    #[test]
    fn test_build_quality_gate_config_fail_on_issues() {
        let mut args = create_default_analyze_args();
        args.quality_gate.fail_on_issues = true;

        let config = build_quality_gate_config(&args);
        assert!(config.enabled);
        assert_eq!(config.max_critical_issues, 0);
        assert_eq!(config.max_high_priority_issues, 0);
    }

    #[test]
    fn test_severity_for_excess_handles_zero_threshold() {
        assert_eq!(severity_for_excess(10.0, 0.0), "Critical");
        assert_eq!(severity_for_excess(2.0, 0.0), "High");
        assert_eq!(severity_for_excess(0.5, 0.0), "Medium");
    }

    #[test]
    fn test_severity_for_excess_relative_thresholds() {
        assert_eq!(severity_for_excess(150.0, 200.0), "Medium");
        assert_eq!(severity_for_excess(108.0, 100.0), "Medium");
        assert_eq!(severity_for_excess(75.0, 60.0), "High");
        assert_eq!(severity_for_excess(95.0, 60.0), "Critical");
    }

    #[test]
    fn test_severity_for_shortfall_levels() {
        assert_eq!(severity_for_shortfall(95.0, 100.0), "Medium");
        assert_eq!(severity_for_shortfall(85.0, 100.0), "High");
        assert_eq!(severity_for_shortfall(70.0, 100.0), "Critical");
    }

    #[test]
    fn test_top_issue_files_ranks_and_limits() {
        let mut results = AnalysisResults::empty();
        results.refactoring_candidates = vec![
            sample_candidate("src/a.rs", Priority::High, 0.82),
            sample_candidate("src/a.rs", Priority::Medium, 0.65),
            sample_candidate("src/b.rs", Priority::Critical, 0.91),
            sample_candidate("src/c.rs", Priority::Low, 0.15),
        ];

        let top = top_issue_files(
            &results,
            |candidate| matches!(candidate.priority, Priority::High | Priority::Critical),
            2,
        );

        assert_eq!(top.len(), 2);
        assert_eq!(top[0], PathBuf::from("src/b.rs"));
        assert_eq!(top[1], PathBuf::from("src/a.rs"));
    }

    #[test]
    fn test_priority_label_variants() {
        assert_eq!(priority_label(Priority::None), "none");
        assert_eq!(priority_label(Priority::Low), "low");
        assert_eq!(priority_label(Priority::Medium), "medium");
        assert_eq!(priority_label(Priority::High), "high");
        assert_eq!(priority_label(Priority::Critical), "critical");
    }

    #[test]
    fn test_is_quiet_considers_flag_and_format() {
        let mut args = create_default_analyze_args();
        assert!(is_quiet(&args)); // machine-readable default

        args.quiet = true;
        args.format = OutputFormat::Markdown;
        assert!(is_quiet(&args)); // explicit quiet flag

        args.quiet = false;
        args.format = OutputFormat::Markdown;
        assert!(!is_quiet(&args)); // human-readable without quiet flag
    }

    #[test]
    fn test_display_quality_gate_violations_with_violations() {
        let violations = vec![
            QualityGateViolation {
                rule_name: "Test Rule".to_string(),
                current_value: 85.0,
                threshold: 70.0,
                description: "Test violation".to_string(),
                severity: "Critical".to_string(),
                affected_files: vec![],
                recommended_actions: vec!["Fix the issue".to_string()],
            },
            QualityGateViolation {
                rule_name: "Warning Rule".to_string(),
                current_value: 25.0,
                threshold: 20.0,
                description: "Warning violation".to_string(),
                severity: "Warning".to_string(),
                affected_files: vec![],
                recommended_actions: vec!["Consider fixing".to_string()],
            },
        ];

        let result = QualityGateResult {
            passed: false,
            violations,
            overall_score: 65.0,
        };

        let _ = capture_stdout(|| display_quality_gate_violations(&result));
    }

    #[test]
    fn test_display_quality_gate_violations_no_violations() {
        let result = QualityGateResult {
            passed: true,
            violations: vec![],
            overall_score: 85.0,
        };

        let _ = capture_stdout(|| display_quality_gate_violations(&result));
    }

    #[test]
    fn test_preview_coverage_discovery_reports_absence_stdout() {
        let runtime = Runtime::new().expect("runtime");
        let workspace = TempDir::new().expect("temp workspace");

        let mut coverage_config = CoverageConfig::default();
        coverage_config.search_paths = vec![".".into()];
        coverage_config.file_patterns = vec!["coverage.lcov".into()];
        coverage_config.auto_discover = true;

        let paths = vec![workspace.path().to_path_buf()];
        let _ = capture_stdout(|| {
            runtime.block_on(async {
                preview_coverage_discovery(&paths, &coverage_config)
                    .await
                    .expect("preview discovery");
            });
        });
    }

    #[test]
    fn test_preview_coverage_discovery_lists_files_stdout() {
        let runtime = Runtime::new().expect("runtime");
        let workspace = TempDir::new().expect("temp workspace");
        let coverage_dir = workspace.path().join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        let coverage_file = coverage_dir.join("coverage.lcov");
        fs::write(
            &coverage_file,
            "TN:valknut\nSF:src/lib.rs\nFN:1,foo\nFNF:1\nFNH:1\nDA:1,1\nLF:1\nLH:1\n",
        )
        .expect("write coverage file");

        let mut coverage_config = CoverageConfig::default();
        coverage_config.search_paths = vec!["coverage".into()];
        coverage_config.file_patterns = vec!["coverage.lcov".into()];
        coverage_config.auto_discover = true;

        let paths = vec![workspace.path().to_path_buf()];
        let _ = capture_stdout(|| {
            runtime.block_on(async {
                preview_coverage_discovery(&paths, &coverage_config)
                    .await
                    .expect("preview discovery");
            });
        });
    }

    #[test]
    fn test_display_quality_gate_violations_blocker_severity() {
        let violations = vec![QualityGateViolation {
            rule_name: "Blocker Rule".to_string(),
            current_value: 95.0,
            threshold: 70.0,
            description: "Blocker violation".to_string(),
            severity: "Blocker".to_string(),
            affected_files: vec!["test.rs".to_string().into()],
            recommended_actions: vec!["Immediate fix required".to_string()],
        }];

        let result = QualityGateResult {
            passed: false,
            violations,
            overall_score: 30.0,
        };

        let _ = capture_stdout(|| display_quality_gate_violations(&result));
    }

    #[test]
    fn test_display_quality_failures_with_recommendations() {
        let result = QualityGateResult {
            passed: false,
            violations: vec![
                QualityGateViolation {
                    rule_name: "Maintainability Score".to_string(),
                    description: "Maintainability below threshold".to_string(),
                    current_value: 55.0,
                    threshold: 75.0,
                    severity: "Critical".to_string(),
                    affected_files: vec![],
                    recommended_actions: vec![
                        "Refactor large modules".to_string(),
                        "Improve documentation".to_string(),
                    ],
                },
                QualityGateViolation {
                    rule_name: "High Priority Issues".to_string(),
                    description: "High-priority issues exceed limit".to_string(),
                    current_value: 8.0,
                    threshold: 3.0,
                    severity: "High".to_string(),
                    affected_files: vec![],
                    recommended_actions: Vec::new(),
                },
            ],
            overall_score: 62.5,
        };

        let _ = capture_stdout(|| display_quality_failures(&result));
    }

    #[test]
    fn test_display_quality_failures_without_violations() {
        let result = QualityGateResult {
            passed: true,
            violations: Vec::new(),
            overall_score: 91.0,
        };

        let _ = capture_stdout(|| display_quality_failures(&result));
    }

    // Mock test for handle_quality_gates since it requires complex analysis result structure
    #[tokio::test]
    async fn test_handle_quality_gates_basic() {
        let mut args = create_default_analyze_args();
        args.quality_gate.quality_gate = true;

        // Create a minimal analysis result
        let analysis_result = serde_json::json!({
            "summary": {
                "total_issues": 5,
                "total_files": 10
            },
            "health_metrics": {
                "overall_health_score": 75.0,
                "complexity_score": 65.0,
                "technical_debt_ratio": 15.0
            }
        });

        let result = handle_quality_gates(&args, &analysis_result).await;
        assert!(result.is_ok());

        let quality_result = result.unwrap();
        assert!(quality_result.passed); // Should pass with default thresholds
    }

    #[tokio::test]
    async fn test_handle_quality_gates_violations() {
        let mut args = create_default_analyze_args();
        args.quality_gate.quality_gate = true;
        args.quality_gate.max_complexity = Some(50.0); // Set low threshold to trigger violation
        args.quality_gate.min_health = Some(80.0); // Set high threshold to trigger violation
        args.quality_gate.max_issues = Some(3); // Set low threshold to trigger violation

        // Create analysis result that will violate quality gates
        let analysis_result = serde_json::json!({
            "summary": {
                "total_issues": 5, // Exceeds max_issues of 3
                "total_files": 10
            },
            "health_metrics": {
                "overall_health_score": 75.0, // Below min_health of 80
                "complexity_score": 65.0, // Exceeds max_complexity of 50
                "technical_debt_ratio": 15.0
            }
        });

        let result = handle_quality_gates(&args, &analysis_result).await;
        assert!(result.is_ok());

        let quality_result = result.unwrap();
        assert!(!quality_result.passed); // Should fail due to violations
        assert!(!quality_result.violations.is_empty());
    }

    #[tokio::test]
    async fn test_handle_quality_gates_missing_summary() {
        let mut args = create_default_analyze_args();
        args.quality_gate.quality_gate = true;

        // Create analysis result without summary
        let analysis_result = serde_json::json!({
            "health_metrics": {
                "overall_health_score": 75.0
            }
        });

        let result = handle_quality_gates(&args, &analysis_result).await;
        assert!(result.is_err()); // Should fail due to missing summary
    }

    #[tokio::test]
    #[serial]
    async fn analyze_command_errors_on_missing_path() {
        let temp_out = TempDir::new().expect("temp out dir");
        let mut args = create_default_analyze_args();
        args.paths = vec![PathBuf::from("definitely_missing_path")];
        args.out = temp_out.path().join("reports");
        args.quiet = false;
        args.format = OutputFormat::Json;

        let result = analyze_command(args, false, SurveyVerbosity::Low).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn analyze_command_errors_when_no_paths() {
        let temp_out = TempDir::new().expect("temp out dir");
        let mut args = create_default_analyze_args();
        args.paths.clear();
        args.out = temp_out.path().join("reports");
        args.quiet = false;
        args.format = OutputFormat::Json;

        let result = analyze_command(args, false, SurveyVerbosity::Low).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_analyze_command_quiet_mode_on_minimal_project() {
        let project = TempDir::new().expect("temp project");
        let project_root = project.path().to_path_buf();
        fs::write(
            project_root.join("lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        )
        .expect("write sample file");

        let output = TempDir::new().expect("output dir");
        let out_path = output.path().join("reports");

        let mut args = create_default_analyze_args();
        args.paths = vec![project_root];
        args.out = out_path;
        args.quiet = true;
        args.format = OutputFormat::Json;
        args.profile = PerformanceProfile::Fast;
        args.coverage.no_coverage = true;
        args.coverage.no_coverage_auto_discover = true;
        args.analysis_control.no_complexity = true;
        args.analysis_control.no_structure = true;
        args.analysis_control.no_refactoring = true;
        args.analysis_control.no_impact = true;
        args.analysis_control.no_lsh = true;

        let result = analyze_command(args, false, SurveyVerbosity::Low).await;
        assert!(
            result.is_ok(),
            "analyze_command should succeed for minimal quiet invocation: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[serial]
    async fn run_analysis_with_progress_handles_denoise_configuration() -> Result<()> {
        let project = create_sample_analysis_project();
        let project_path = project.path().to_path_buf();
        let coverage_file = write_lcov_fixture(project.path());
        let output_dir = TempDir::new().expect("output dir");

        let mut args = create_default_analyze_args();
        args.paths = vec![project_path.clone()];
        args.out = output_dir.path().to_path_buf();
        args.format = OutputFormat::Pretty;
        args.clone_detection.denoise = true;
        args.clone_detection.denoise_dry_run = true;
        args.clone_detection.min_function_tokens = Some(12);
        args.clone_detection.min_match_tokens = Some(4);
        args.clone_detection.require_blocks = Some(1);
        args.clone_detection.similarity = Some(0.88);
        args.advanced_clone.ast_weight = Some(0.6);
        args.advanced_clone.pdg_weight = Some(0.25);
        args.advanced_clone.emb_weight = Some(0.15);
        args.advanced_clone.apted_verify = true;
        args.advanced_clone.apted_max_nodes = Some(256);
        args.advanced_clone.apted_max_pairs = Some(24);
        args.advanced_clone.quality_target = Some(0.92);
        args.advanced_clone.sample_size = Some(42);
        args.advanced_clone.min_saved_tokens = Some(3);
        args.advanced_clone.min_rarity_gain = Some(0.05);
        args.advanced_clone.io_mismatch_penalty = Some(0.33);
        args.coverage.coverage_file = Some(coverage_file.clone());
        args.coverage.coverage_max_age_days = Some(30);
        args.analysis_control.no_complexity = true;
        args.analysis_control.no_refactoring = true;

        let _guard = DirGuard::change_to(&project_path);
        let result =
            run_analysis_with_progress(&args.paths, StructureConfig::default(), &args).await?;

        assert!(
            result["summary"]["total_files"]
                .as_u64()
                .unwrap_or_default()
                >= 1
        );
        let cache_dir = project_path.join(".valknut/cache/denoise");
        assert!(cache_dir.join("stop_motifs.v1.json").exists());
        assert!(cache_dir.join("auto_calibration.v1.json").exists());

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn run_analysis_without_progress_toggles_modules() -> Result<()> {
        let project = create_sample_analysis_project();
        let project_path = project.path().to_path_buf();
        let output_dir = TempDir::new().expect("output dir");

        let mut args = create_default_analyze_args();
        args.paths = vec![project_path.clone()];
        args.out = output_dir.path().to_path_buf();
        args.format = OutputFormat::Json;
        args.clone_detection.denoise = true;
        args.clone_detection.min_function_tokens = Some(8);
        args.clone_detection.min_match_tokens = Some(4);
        args.clone_detection.require_blocks = Some(1);
        args.clone_detection.similarity = Some(0.9);
        args.advanced_clone.no_auto = true;
        args.advanced_clone.no_apted_verify = true;
        args.coverage.no_coverage = true;
        args.coverage.no_coverage_auto_discover = true;
        args.coverage.coverage_max_age_days = Some(14);
        args.analysis_control.no_complexity = true;
        args.analysis_control.no_structure = true;
        args.analysis_control.no_refactoring = true;
        args.analysis_control.no_impact = true;
        args.analysis_control.no_lsh = true;

        {
            let _guard = DirGuard::change_to(&project_path);
            let summary =
                run_analysis_without_progress(&args.paths, StructureConfig::default(), &args)
                    .await?;
            assert!(
                summary["summary"]["total_files"]
                    .as_u64()
                    .unwrap_or_default()
                    >= 1
            );
        }

        let mut args_no_denoise = create_default_analyze_args();
        args_no_denoise.paths = vec![project_path.clone()];
        args_no_denoise.out = output_dir.path().to_path_buf();
        args_no_denoise.format = OutputFormat::Json;
        args_no_denoise.clone_detection.denoise = false;
        args_no_denoise.clone_detection.denoise_dry_run = false;
        args_no_denoise.coverage.no_coverage = true;
        args_no_denoise.coverage.no_coverage_auto_discover = true;
        args_no_denoise.coverage.coverage_max_age_days = Some(14);
        args_no_denoise.analysis_control.no_complexity = true;
        args_no_denoise.analysis_control.no_structure = true;
        args_no_denoise.analysis_control.no_refactoring = true;
        args_no_denoise.analysis_control.no_impact = true;
        args_no_denoise.analysis_control.no_lsh = true;

        {
            let _guard = DirGuard::change_to(&project_path);
            let summary = run_analysis_without_progress(
                &args_no_denoise.paths,
                StructureConfig::default(),
                &args_no_denoise,
            )
            .await?;
            assert!(
                summary["summary"]["total_files"]
                    .as_u64()
                    .unwrap_or_default()
                    >= 1
            );
        }

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create_denoise_cache_directories_is_idempotent() -> Result<()> {
        let temp = TempDir::new().expect("temp dir");
        let _guard = DirGuard::change_to(temp.path());
        create_denoise_cache_directories().await?;
        let stop_file = temp
            .path()
            .join(".valknut/cache/denoise/stop_motifs.v1.json");
        let auto_file = temp
            .path()
            .join(".valknut/cache/denoise/auto_calibration.v1.json");
        assert!(stop_file.exists());
        assert!(auto_file.exists());

        create_denoise_cache_directories().await?;
        assert!(stop_file.exists());
        assert!(auto_file.exists());

        Ok(())
    }

    #[test]
    fn apply_performance_profile_adjusts_configuration() {
        let mut config = ValknutConfig::default();
        apply_performance_profile(&mut config, &PerformanceProfile::Fast);
        assert_eq!(config.analysis.max_files, 500);
        apply_performance_profile(&mut config, &PerformanceProfile::Balanced);
        apply_performance_profile(&mut config, &PerformanceProfile::Thorough);
        assert!(config.denoise.enabled);
        apply_performance_profile(&mut config, &PerformanceProfile::Extreme);
        assert_eq!(config.lsh.num_hashes, 200);
    }

    #[test]
    fn test_display_enabled_analyses_all_features() {
        let mut config = ValknutConfig::default();
        config.analysis.enable_scoring = true;
        config.analysis.enable_structure_analysis = true;
        config.analysis.enable_refactoring_analysis = true;
        config.analysis.enable_graph_analysis = true;
        config.analysis.enable_lsh_analysis = true;
        config.analysis.enable_coverage_analysis = true;
        config.coverage.auto_discover = true;
        config.denoise.enabled = true;
        config.lsh.verify_with_apted = true;

        display_enabled_analyses(&config);
    }

    #[test]
    fn test_display_analysis_config_summary_with_flags() {
        let mut config = ValknutConfig::default();
        config.analysis.enable_coverage_analysis = true;
        config.coverage.max_age_days = 7;
        config.coverage.file_patterns = vec!["coverage.lcov".into()];
        config.analysis.max_files = 42;
        config.denoise.enabled = true;
        config.denoise.similarity = 0.87;
        config.analysis.enable_lsh_analysis = true;

        display_analysis_config_summary(&config);
    }

    #[tokio::test]
    async fn test_preview_coverage_discovery_handles_absence() {
        let temp_dir = TempDir::new().unwrap();
        let config = CoverageConfig::default();

        let result = preview_coverage_discovery(&[temp_dir.path().to_path_buf()], &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_preview_coverage_discovery_lists_files() {
        let coverage_dir = TempDir::new().unwrap();
        let root = coverage_dir.path();
        let nested = root.join("coverage");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("coverage.lcov"), "TN:demo\nend_of_record\n").unwrap();

        let mut config = CoverageConfig::default();
        config.auto_discover = true;
        config.file_patterns = vec!["coverage.lcov".into()];

        let result = preview_coverage_discovery(&[root.to_path_buf()], &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_preview_coverage_discovery_truncates_listing() {
        let coverage_dir = TempDir::new().unwrap();
        let root = coverage_dir.path();
        let nested = root.join("coverage");
        fs::create_dir_all(&nested).unwrap();

        for idx in 0..4 {
            let file_path = nested.join(format!("report_{idx}.lcov"));
            fs::write(&file_path, "TN:demo\nend_of_record\n").unwrap();
        }

        let mut config = CoverageConfig::default();
        config.auto_discover = true;
        config.search_paths = vec!["coverage".to_string()];
        config.file_patterns = vec!["*.lcov".to_string()];

        let result = preview_coverage_discovery(&[root.to_path_buf()], &config).await;
        assert!(result.is_ok());
    }

    #[test]
    fn severity_for_excess_covers_threshold_cases() {
        assert_eq!(severity_for_excess(10.0, 0.0), "Critical");
        assert_eq!(severity_for_excess(20.0, 10.0), "Critical");
        assert_eq!(severity_for_excess(26.0, 20.0), "High");
        assert_eq!(severity_for_excess(22.0, 20.0), "Medium");
    }

    #[test]
    fn severity_for_shortfall_respects_delta() {
        assert_eq!(severity_for_shortfall(50.0, 80.0), "Critical");
        assert_eq!(severity_for_shortfall(65.0, 80.0), "High");
        assert_eq!(severity_for_shortfall(75.0, 80.0), "Medium");
    }

    #[test]
    fn display_analysis_summary_prints_hotspots_and_metrics() {
        let mut result = sample_analysis_results();
        result.summary.refactoring_needed = 2;
        result.summary.high_priority = 2;
        result.summary.critical = 1;

        result.refactoring_candidates.push(sample_candidate(
            "src/utils.rs",
            Priority::Critical,
            3.8,
        ));

        result.refactoring_candidates.push(sample_candidate(
            "src/helpers/mod.rs",
            Priority::High,
            2.9,
        ));

        result.clone_analysis = Some(CloneAnalysisResults {
            denoising_enabled: true,
            auto_calibration_applied: Some(true),
            candidates_before_denoising: Some(10),
            candidates_after_denoising: 4,
            calibrated_threshold: Some(0.75),
            quality_score: Some(0.82),
            avg_similarity: Some(0.68),
            max_similarity: Some(0.91),
            verification: None,
            phase_filtering_stats: None,
            performance_metrics: None,
            notes: vec!["Filtered duplicates".to_string()],
            clone_pairs: Vec::new(),
        });

        result.warnings = vec!["Sample warning".to_string()];

        display_comprehensive_results(&result);
    }

    #[test]
    fn combine_analysis_results_merges_runs() {
        let mut first = sample_analysis_results();
        first.summary.files_processed = 2;
        first.summary.entities_analyzed = 4;
        first.summary.avg_refactoring_score = 0.6;
        first.summary.code_health_score = 0.7;
        first.statistics.total_duration = Duration::from_millis(30);
        first
            .statistics
            .features_per_entity
            .insert("cyclomatic".into(), 3.0);
        first.summary.refactoring_needed = 1;
        first.summary.high_priority = 1;
        first.statistics.memory_stats = MemoryStats {
            peak_memory_bytes: 2048,
            final_memory_bytes: 1024,
            efficiency_score: 0.8,
        };

        let mut second = sample_analysis_results();
        second.summary.files_processed = 3;
        second.summary.entities_analyzed = 6;
        second.summary.avg_refactoring_score = 0.9;
        second.summary.code_health_score = 0.5;
        second.statistics.total_duration = Duration::from_millis(60);
        second
            .statistics
            .features_per_entity
            .insert("cyclomatic".into(), 5.0);
        second
            .statistics
            .features_per_entity
            .insert("maintainability".into(), 2.0);
        second.summary.refactoring_needed = 2;
        second.summary.high_priority = 1;
        second.statistics.memory_stats = MemoryStats {
            peak_memory_bytes: 4096,
            final_memory_bytes: 2048,
            efficiency_score: 0.6,
        };
        second.warnings.push("Second warning".into());

        let expected_files = first.summary.files_processed + second.summary.files_processed;
        let expected_entities = first.summary.entities_analyzed + second.summary.entities_analyzed;
        let expected_refactoring =
            first.summary.refactoring_needed + second.summary.refactoring_needed;
        let expected_high_priority = first.summary.high_priority + second.summary.high_priority;
        let expected_duration = first.statistics.total_duration + second.statistics.total_duration;

        let combined = combine_analysis_results(vec![first, second]).expect("merge succeeds");

        assert_eq!(combined.summary.files_processed, expected_files);
        assert_eq!(combined.summary.entities_analyzed, expected_entities);
        assert_eq!(combined.summary.refactoring_needed, expected_refactoring);
        assert_eq!(combined.summary.high_priority, expected_high_priority);
        assert!(
            combined.summary.avg_refactoring_score >= 0.6
                && combined.summary.avg_refactoring_score <= 0.9
        );
        assert!(
            combined.summary.code_health_score >= 0.5 && combined.summary.code_health_score <= 0.7
        );
        assert_eq!(combined.statistics.total_duration, expected_duration);
        assert!(combined
            .statistics
            .features_per_entity
            .contains_key("maintainability"));
        assert_eq!(combined.warnings.len(), 1);
        assert_eq!(combined.refactoring_candidates.len(), 2);
    }

    #[test]
    fn combine_analysis_results_errors_on_empty() {
        let err = combine_analysis_results(vec![]);
        assert!(err.is_err());
    }
}
