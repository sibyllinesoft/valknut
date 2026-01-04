//! Output Formatting, Report Generation, and Display Functions
//!
//! This module contains all output formatting functions, report generation for
//! various formats (HTML, Markdown, CSV, Sonar), and display utilities.

mod csv_export;
mod display;
mod helpers;
mod html_report;
mod markdown_report;
mod report_helpers;
mod reports;
mod sonar;
mod writers;

use std::path::Path;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

use crate::cli::args::OutputFormat;
use valknut_rs::api::results::AnalysisResults;

// Re-export public items from submodules
pub use display::{
    display_analysis_results, display_completion_summary, display_complexity_recommendations,
    display_file_complexity_recommendations, display_refactoring_suggestions,
    display_top_structure_issues, print_comprehensive_results_pretty, print_human_readable_results,
};
pub use helpers::format_to_string;
pub use csv_export::{generate_ci_summary_report, generate_csv_report};
pub use html_report::generate_html_report;
pub use markdown_report::generate_markdown_report;
pub use sonar::generate_sonar_report;
pub use writers::{
    build_report_generator, write_ci_summary, write_csv, write_html, write_json, write_jsonl,
    write_markdown, write_sonar, write_yaml,
};

/// Generate outputs with progress feedback
#[allow(dead_code)]
pub async fn generate_outputs_with_feedback(
    result: &serde_json::Value,
    out_path: &Path,
    output_format: &OutputFormat,
    quiet: bool,
) -> anyhow::Result<()> {
    if !quiet {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::with_template("{spinner:.blue} {msg}")?);
        pb.set_message(format!(
            "Generating {} output...",
            format_to_string(output_format).to_uppercase()
        ));
        pb.enable_steady_tick(Duration::from_millis(100));

        generate_outputs(result, out_path, output_format).await?;

        pb.finish_with_message(format!(
            "{} report generated",
            format_to_string(output_format).to_uppercase()
        ));
    } else {
        generate_outputs(result, out_path, output_format).await?;
    }

    Ok(())
}

/// Generate output files from analysis result
#[allow(dead_code)]
pub async fn generate_outputs(
    result: &serde_json::Value,
    out_path: &Path,
    output_format: &OutputFormat,
) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(out_path).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Jsonl | OutputFormat::Yaml => {
            write_data_format(result, out_path, output_format).await
        }
        OutputFormat::Markdown | OutputFormat::Html => {
            write_rich_report(result, out_path, output_format).await
        }
        OutputFormat::Sonar | OutputFormat::Csv | OutputFormat::CiSummary => {
            write_integration_format(result, out_path, output_format).await
        }
        OutputFormat::Pretty => {
            print_comprehensive_results_pretty(result);
            Ok(())
        }
    }
}

/// Write structured data formats (JSON, JSONL, YAML).
async fn write_data_format(
    result: &serde_json::Value,
    out_path: &Path,
    format: &OutputFormat,
) -> anyhow::Result<()> {
    let analysis_results = serde_json::from_value::<AnalysisResults>(result.clone()).ok();
    let generator = build_report_generator()?;

    match format {
        OutputFormat::Jsonl => write_jsonl(result, out_path).await,
        OutputFormat::Json => write_json(&generator, analysis_results.as_ref(), result, out_path).await,
        OutputFormat::Yaml => write_yaml(&generator, analysis_results.as_ref(), result, out_path).await,
        _ => unreachable!(),
    }
}

/// Write rich document reports (Markdown, HTML).
async fn write_rich_report(
    result: &serde_json::Value,
    out_path: &Path,
    format: &OutputFormat,
) -> anyhow::Result<()> {
    let analysis_results = serde_json::from_value::<AnalysisResults>(result.clone()).ok();
    let generator = build_report_generator()?;

    match format {
        OutputFormat::Markdown => write_markdown(&generator, analysis_results.as_ref(), result, out_path).await,
        OutputFormat::Html => write_html(&generator, analysis_results.as_ref(), result, out_path).await,
        _ => unreachable!(),
    }
}

/// Write CI/integration formats (Sonar, CSV, CI Summary).
async fn write_integration_format(
    result: &serde_json::Value,
    out_path: &Path,
    format: &OutputFormat,
) -> anyhow::Result<()> {
    let analysis_results = serde_json::from_value::<AnalysisResults>(result.clone()).ok();
    let generator = build_report_generator()?;

    match format {
        OutputFormat::Sonar => write_sonar(&generator, analysis_results.as_ref(), result, out_path).await,
        OutputFormat::Csv => write_csv(&generator, analysis_results.as_ref(), result, out_path).await,
        OutputFormat::CiSummary => write_ci_summary(result, out_path).await,
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[path = "../output_tests.rs"]
mod tests;
