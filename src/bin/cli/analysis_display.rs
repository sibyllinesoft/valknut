//! Display and output formatting functions for CLI.
//!
//! This module contains functions for formatting and displaying
//! analysis results, summaries, and configuration information.

use std::cmp::Ordering;
use std::path::Path;

use owo_colors::OwoColorize;
use tabled::{settings::Style as TableStyle, Table, Tabled};

use valknut_rs::api::results::{AnalysisResults, RefactoringCandidate};
use valknut_rs::core::pipeline::AnalysisConfig as PipelineAnalysisConfig;
use valknut_rs::core::scoring::Priority;
use valknut_rs::detectors::structure::StructureConfig;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Print Valknut header with version info
pub fn print_header() {
    println!("Valknut v{VERSION}");
}

/// Build the stylized header lines to fit the given terminal width.
pub fn header_lines_for_width(width: u16) -> Vec<String> {
    let _ = width; // width retained for test call signature
    vec![format!("Valknut v{VERSION}")]
}

/// Display comprehensive analysis results
pub fn display_comprehensive_results(result: &AnalysisResults, detailed: bool) {
    println!("Results:");
    display_analysis_summary(result, detailed);
}

/// Display analysis summary
pub fn display_analysis_summary(result: &AnalysisResults, detailed: bool) {
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
        display_detailed_metrics(result);
    }
}

/// Display detailed metrics when verbose mode is enabled
fn display_detailed_metrics(result: &AnalysisResults) {
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

    display_hotspots(result);
    display_warnings(result);
}

/// Display top hotspots from analysis
fn display_hotspots(result: &AnalysisResults) {
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
}

/// Display warnings from analysis
fn display_warnings(result: &AnalysisResults) {
    if !result.warnings.is_empty() {
        println!("  warnings:");
        for warning in &result.warnings {
            println!("    - {}", warning);
        }
    }
}

/// Human-friendly label for a `Priority` value.
pub fn priority_label(priority: Priority) -> &'static str {
    match priority {
        Priority::None => "none",
        Priority::Low => "low",
        Priority::Medium => "medium",
        Priority::High => "high",
        Priority::Critical => "critical",
    }
}

/// Display configuration summary in a formatted table
pub fn display_config_summary(config: &StructureConfig) {
    /// Row for configuration display table.
    #[derive(Tabled)]
    struct ConfigRow {
        setting: String,
        value: String,
    }

    let config_rows = vec![
        ConfigRow {
            setting: "Languages".to_string(),
            value: "Auto-detected".to_string(),
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

/// Display analysis configuration summary.
pub fn display_analysis_config(pipeline_config: &PipelineAnalysisConfig, cohesion_enabled: bool) {
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
pub fn log_analysis_completion(result: &valknut_rs::core::pipeline::ComprehensiveAnalysisResult) {
    tracing::info!("Analysis completed successfully");
    tracing::info!("Total files: {}", result.summary.total_files);
    tracing::info!("Total issues: {}", result.summary.total_issues);
    tracing::info!(
        "Overall health score: {:.1}",
        result.health_metrics.overall_health_score
    );
}
