//! Command Execution Logic and Analysis Operations
//!
//! This module contains the main command execution logic, analysis operations,
//! and progress tracking functionality.

use crate::cli::args::{
    AIFeaturesArgs, AdvancedCloneArgs, AnalysisControlArgs, AnalyzeArgs, CloneDetectionArgs,
    CoverageArgs, InitConfigArgs, McpManifestArgs, McpStdioArgs, OutputFormat, PerformanceProfile, 
    QualityGateArgs, SurveyVerbosity, ValidateConfigArgs,
};
use crate::cli::config_layer::build_layered_valknut_config;
use anyhow;
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
use valknut_rs::api::config_types::AnalysisConfig as ApiAnalysisConfig;
use valknut_rs::api::engine::ValknutEngine;
use valknut_rs::api::results::{AnalysisResults, RefactoringCandidate};
use valknut_rs::core::config::ReportFormat;
use valknut_rs::core::config::{CoverageConfig, ValknutConfig};
use valknut_rs::core::file_utils::CoverageDiscovery;
use valknut_rs::core::pipeline::{QualityGateConfig, QualityGateResult, QualityGateViolation};
use valknut_rs::core::scoring::Priority;
use valknut_rs::detectors::structure::StructureConfig;
use valknut_rs::io::reports::ReportGenerator;
use valknut_rs::oracle::{OracleConfig, RefactoringOracle};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main analyze command implementation with comprehensive analysis pipeline
pub async fn analyze_command(
    args: AnalyzeArgs,
    _survey: bool,
    _survey_verbosity: SurveyVerbosity,
) -> anyhow::Result<()> {
    // Print header
    if !args.quiet {
        print_header();
    }

    // Build comprehensive configuration from CLI args and file
    let valknut_config = build_valknut_config(&args).await?;

    if !args.quiet {
        println!(
            "{}",
            "✅ Configuration loaded with comprehensive analysis enabled".green()
        );
        display_analysis_config_summary(&valknut_config);
    }

    // Validate and prepare paths
    if !args.quiet {
        println!("{}", "📂 Validating Input Paths".bright_blue().bold());
        println!();
    }

    let mut valid_paths = Vec::new();
    for path in &args.paths {
        if path.exists() {
            valid_paths.push(path.clone());
            if !args.quiet {
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

    if !args.quiet {
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
    if valknut_config.analysis.enable_coverage_analysis && !args.quiet {
        preview_coverage_discovery(&valid_paths, &valknut_config.coverage).await?;
    }

    // Run comprehensive analysis with enhanced progress tracking
    if !args.quiet {
        println!(
            "{}",
            "🔍 Starting Comprehensive Analysis Pipeline"
                .bright_blue()
                .bold()
        );
        display_enabled_analyses(&valknut_config);
        println!();
    }

    let analysis_result = if args.quiet {
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
            !args.quiet,
        )?)
    } else {
        None
    };

    // Display analysis results
    if !args.quiet {
        display_comprehensive_results(&analysis_result);
    }

    // Run Oracle analysis if requested
    let oracle_response = if args.ai_features.oracle {
        if !args.quiet {
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
            if !args.quiet {
                println!("{}", "❌ Quality gates failed!".red().bold());
                display_quality_failures(&quality_result);
            }
            return Err(anyhow::anyhow!("Quality gates failed"));
        } else if !args.quiet {
            println!("{}", "✅ All quality gates passed!".green().bold());
        }
    }

    if !args.quiet {
        println!("{}", "🎉 Analysis completed successfully!".green().bold());
    }

    Ok(())
}

/// Build comprehensive ValknutConfig from CLI arguments
async fn build_valknut_config(args: &AnalyzeArgs) -> anyhow::Result<ValknutConfig> {
    // Use the new layered configuration approach
    let mut config = build_layered_valknut_config(args)?;
    
    // Apply performance profile optimizations
    apply_performance_profile(&mut config, &args.profile);
    
    Ok(config)
}

/// Apply performance profile optimizations to the configuration
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
        let denoise_status = if config.denoise.enabled {
            " (with denoising)"
        } else {
            ""
        };
        println!(
            "    ✅ Clone Detection - LSH-based similarity analysis{}",
            denoise_status
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

/// Run comprehensive analysis with progress tracking
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

    // Convert to API config
    let api_config = ApiAnalysisConfig::from_valknut_config(config)?;

    // Create engine and run analysis
    let mut engine = ValknutEngine::new(api_config)
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

/// Run comprehensive analysis without progress tracking  
async fn run_comprehensive_analysis_without_progress(
    paths: &[PathBuf],
    config: ValknutConfig,
    _args: &AnalyzeArgs,
) -> anyhow::Result<AnalysisResults> {
    // Convert to API config
    let api_config = ApiAnalysisConfig::from_valknut_config(config)?;

    // Create engine and run analysis
    let mut engine = ValknutEngine::new(api_config)
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

/// Display quality gate failures
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

fn priority_label(priority: Priority) -> &'static str {
    match priority {
        Priority::None => "none",
        Priority::Low => "low",
        Priority::Medium => "medium",
        Priority::High => "high",
        Priority::Critical => "critical",
    }
}

/// Generate output reports in various formats with optional Oracle results
async fn generate_reports_with_oracle(
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
    args: &AnalyzeArgs,
) -> anyhow::Result<()> {
    println!("{}", "📝 Generating Reports".bright_blue().bold());

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

    println!(
        "  ✅ Report saved: {}",
        output_file.display().to_string().cyan()
    );
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

/// Run MCP server over stdio for IDE integration
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

/// Generate MCP manifest JSON
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

/// List supported programming languages and their status
pub async fn list_languages() -> anyhow::Result<()> {
    println!(
        "{}",
        "🔤 Supported Programming Languages".bright_blue().bold()
    );
    println!("   Found {} supported languages", 8); // TODO: Dynamic count
    println!();

    #[derive(Tabled)]
    struct LanguageRow {
        language: String,
        extension: String,
        status: String,
        features: String,
    }

    let languages = vec![
        LanguageRow {
            language: "Python".to_string(),
            extension: ".py".to_string(),
            status: "✅ Full Support".to_string(),
            features: "Full analysis, refactoring suggestions".to_string(),
        },
        LanguageRow {
            language: "TypeScript".to_string(),
            extension: ".ts, .tsx".to_string(),
            status: "✅ Full Support".to_string(),
            features: "Full analysis, type checking".to_string(),
        },
        LanguageRow {
            language: "JavaScript".to_string(),
            extension: ".js, .jsx".to_string(),
            status: "✅ Full Support".to_string(),
            features: "Full analysis, complexity metrics".to_string(),
        },
        LanguageRow {
            language: "Rust".to_string(),
            extension: ".rs".to_string(),
            status: "✅ Full Support".to_string(),
            features: "Full analysis, memory safety checks".to_string(),
        },
        LanguageRow {
            language: "Go".to_string(),
            extension: ".go".to_string(),
            status: "🚧 Experimental".to_string(),
            features: "Basic analysis".to_string(),
        },
        LanguageRow {
            language: "Java".to_string(),
            extension: ".java".to_string(),
            status: "🚧 Experimental".to_string(),
            features: "Basic analysis".to_string(),
        },
        LanguageRow {
            language: "C++".to_string(),
            extension: ".cpp, .cxx".to_string(),
            status: "🚧 Experimental".to_string(),
            features: "Basic analysis".to_string(),
        },
        LanguageRow {
            language: "C#".to_string(),
            extension: ".cs".to_string(),
            status: "🚧 Experimental".to_string(),
            features: "Basic analysis".to_string(),
        },
    ];

    let mut table = Table::new(languages);
    table.with(TableStyle::rounded());
    println!("{}", table);

    println!();
    println!("{}", "📝 Usage Notes:".bright_blue().bold());
    println!("   • Full Support: Complete feature set with refactoring suggestions");
    println!("   • Experimental: Basic complexity analysis, limited features");
    println!("   • Configure languages in your config file with language-specific settings");
    println!();
    println!(
        "{}",
        "💡 Tip: Use 'valknut init-config' to create a configuration file".dimmed()
    );

    Ok(())
}

/// Print Valknut header with version info
pub fn print_header() {
    if Term::stdout().size().1 >= 80 {
        // Full header for wide terminals
        println!(
            "{}",
            "┌".cyan().bold().to_string()
                + &"─".repeat(60).cyan().to_string()
                + &"┐".cyan().bold().to_string()
        );
        println!(
            "{} {} {}",
            "│".cyan().bold(),
            format!("⚙️  Valknut v{} - AI-Powered Code Analysis", VERSION)
                .bright_cyan()
                .bold(),
            "│".cyan().bold()
        );
        println!(
            "{}",
            "└".cyan().bold().to_string()
                + &"─".repeat(60).cyan().to_string()
                + &"┘".cyan().bold().to_string()
        );
    } else {
        // Compact header for narrow terminals
        println!(
            "{} {}",
            "⚙️".bright_cyan(),
            format!("Valknut v{}", VERSION).bright_cyan().bold()
        );
    }
    println!();
}

/// Display configuration summary in a formatted table
pub fn display_config_summary(config: &StructureConfig) {
    #[derive(Tabled)]
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

/// Run comprehensive analysis with detailed progress tracking
#[allow(dead_code)]
pub async fn run_analysis_with_progress(
    paths: &[PathBuf],
    _config: StructureConfig,
    args: &AnalyzeArgs,
) -> anyhow::Result<serde_json::Value> {
    use valknut_rs::core::config::{DenoiseConfig, ValknutConfig};
    use valknut_rs::core::pipeline::{AnalysisConfig, AnalysisPipeline, ProgressCallback};

    let multi_progress = MultiProgress::new();

    // Create main progress bar
    let main_pb = multi_progress.add(ProgressBar::new(100));
    main_pb.set_style(ProgressStyle::with_template(
        "🚀 {msg} [{bar:40.bright_blue/blue}] {pos:>3}% {elapsed_precise}",
    )?);
    main_pb.set_message("Comprehensive Analysis");

    // Create full ValknutConfig to properly configure denoising
    let mut valknut_config = ValknutConfig::default();
    let mut analysis_config = AnalysisConfig {
        enable_lsh_analysis: true,
        ..Default::default()
    };

    // Apply CLI args to denoise configuration (enabled by default)
    let denoise_enabled = !args.clone_detection.no_denoise;
    let auto_enabled = !args.advanced_clone.no_auto;

    if denoise_enabled {
        info!("Clone denoising enabled (default behavior)");
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
        analysis_config.enable_coverage_analysis = false;
    }
    if args.analysis_control.no_complexity {
        analysis_config.enable_complexity_analysis = false; // Complexity is part of scoring
    }
    if args.analysis_control.no_structure {
        analysis_config.enable_structure_analysis = false;
    }
    if args.analysis_control.no_refactoring {
        analysis_config.enable_refactoring_analysis = false;
    }
    if args.analysis_control.no_impact {
        analysis_config.enable_impact_analysis = false; // Impact analysis uses graph analysis
    }
    if args.analysis_control.no_lsh {
        analysis_config.enable_lsh_analysis = false;
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

    // Log analysis configuration
    let enabled_analyses = vec![
        ("Complexity", analysis_config.enable_complexity_analysis),
        ("Structure", analysis_config.enable_structure_analysis),
        ("Refactoring", analysis_config.enable_refactoring_analysis),
        ("Impact", analysis_config.enable_impact_analysis),
        ("Clone Detection (LSH)", analysis_config.enable_lsh_analysis),
        ("Coverage", analysis_config.enable_coverage_analysis),
    ];

    if !args.quiet {
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

    let pipeline = AnalysisPipeline::new_with_config(analysis_config, valknut_config);

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

/// Run analysis without progress bars for quiet mode
#[allow(dead_code)]
pub async fn run_analysis_without_progress(
    paths: &[PathBuf],
    _config: StructureConfig,
    args: &AnalyzeArgs,
) -> anyhow::Result<serde_json::Value> {
    use valknut_rs::core::config::{DenoiseConfig, ValknutConfig};
    use valknut_rs::core::pipeline::{AnalysisConfig, AnalysisPipeline};

    // Create full ValknutConfig to properly configure denoising
    let mut valknut_config = ValknutConfig::default();
    let mut analysis_config = AnalysisConfig {
        enable_lsh_analysis: true,
        ..Default::default()
    };

    // Apply CLI args to denoise configuration (enabled by default)
    let denoise_enabled = !args.clone_detection.no_denoise;
    let auto_enabled = !args.advanced_clone.no_auto;

    if denoise_enabled {
        info!("Clone denoising enabled (default behavior)");
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
        analysis_config.enable_coverage_analysis = false;
    }
    if args.analysis_control.no_complexity {
        analysis_config.enable_complexity_analysis = false; // Complexity is part of scoring
    }
    if args.analysis_control.no_structure {
        analysis_config.enable_structure_analysis = false;
    }
    if args.analysis_control.no_refactoring {
        analysis_config.enable_refactoring_analysis = false;
    }
    if args.analysis_control.no_impact {
        analysis_config.enable_impact_analysis = false; // Impact analysis uses graph analysis
    }
    if args.analysis_control.no_lsh {
        analysis_config.enable_lsh_analysis = false;
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

    let pipeline = AnalysisPipeline::new_with_config(analysis_config, valknut_config);

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

/// Create denoise cache directories if they don't exist
#[allow(dead_code)]
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

/// Handle quality gate evaluation
#[allow(dead_code)]
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
        ..Default::default()
    };

    // Override defaults with CLI values if provided
    if let Some(max_complexity) = args.quality_gate.max_complexity {
        config.max_complexity_score = max_complexity;
    }
    if let Some(min_health) = args.quality_gate.min_health {
        config.min_maintainability_score = min_health;
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

/// Display quality gate violations in a user-friendly format
#[allow(dead_code)]
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

    if !args.quiet {
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
            if !args.quiet {
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
                } else if !args.quiet {
                    println!(
                        "  💾 Oracle recommendations saved to: {}",
                        oracle_path.display().to_string().cyan()
                    );
                }
            }

            Ok(Some(response))
        }
        Err(e) => {
            if !args.quiet {
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

/// Generate output reports in various formats (legacy version for compatibility)
#[allow(dead_code)]
async fn generate_reports(result: &AnalysisResults, args: &AnalyzeArgs) -> anyhow::Result<()> {
    generate_reports_with_oracle(result, &None, args).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{NamedTempFile, TempDir};
    use valknut_rs::core::pipeline::QualityGateViolation;

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
                max_debt: None,
                min_maintainability: None,
                max_issues: None,
                max_critical: None,
                max_high_priority: None,
            },
            clone_detection: CloneDetectionArgs {
                semantic_clones: false,
                strict_dedupe: false,
                no_denoise: false,
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

    #[test]
    fn test_print_header() {
        // Test that print_header doesn't panic
        print_header();
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

        // Test that display_quality_gate_violations doesn't panic
        display_quality_gate_violations(&result);
    }

    #[test]
    fn test_display_quality_gate_violations_no_violations() {
        let result = QualityGateResult {
            passed: true,
            violations: vec![],
            overall_score: 85.0,
        };

        // Test that display_quality_gate_violations doesn't panic
        display_quality_gate_violations(&result);
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

        // Test that display_quality_gate_violations doesn't panic with blocker
        display_quality_gate_violations(&result);
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
}
