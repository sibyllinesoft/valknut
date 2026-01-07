//! Report generation logic for various output formats.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use valknut_rs::api::results::AnalysisResults;
use valknut_rs::core::config::ReportFormat;
use valknut_rs::io::reports::ReportGenerator;

use crate::cli::args::{AnalyzeArgs, OutputFormat};

/// Helper to write content to a file with consistent error handling.
pub async fn write_report(path: &Path, content: &str, format_name: &str) -> anyhow::Result<()> {
    tokio::fs::write(path, content)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to write {} report: {}", format_name, e))
}

/// Write JSON report directly to file (streaming, avoids building string in memory).
pub fn write_json_streaming(
    path: &Path,
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
) -> anyhow::Result<()> {
    let file = File::create(path)
        .map_err(|e| anyhow::anyhow!("Failed to create JSON file: {}", e))?;
    let writer = BufWriter::new(file);

    let combined = match oracle_response {
        Some(oracle) => serde_json::json!({
            "oracle_refactoring_plan": oracle,
            "analysis_results": result
        }),
        None => serde_json::to_value(result)
            .map_err(|e| anyhow::anyhow!("Failed to convert analysis to JSON: {}", e))?,
    };

    serde_json::to_writer_pretty(writer, &combined)
        .map_err(|e| anyhow::anyhow!("Failed to write JSON: {}", e))
}

/// Generate JSON report content.
pub fn generate_json_content(result: &AnalysisResults) -> anyhow::Result<String> {
    serde_json::to_string_pretty(result)
        .map_err(|e| anyhow::anyhow!("Failed to serialize JSON: {}", e))
}

/// Generate JSONL report content.
pub fn generate_jsonl_content(result: &AnalysisResults) -> anyhow::Result<String> {
    serde_json::to_string(result)
        .map_err(|e| anyhow::anyhow!("Failed to serialize JSONL: {}", e))
}

/// Generate YAML report content.
pub fn generate_yaml_content(result: &AnalysisResults) -> anyhow::Result<String> {
    serde_yaml::to_string(result)
        .map_err(|e| anyhow::anyhow!("Failed to serialize YAML: {}", e))
}

/// Generate markdown report content.
pub async fn generate_markdown_content(result: &AnalysisResults) -> anyhow::Result<String> {
    let result_json = serde_json::to_value(result)?;
    super::output::generate_markdown_report(&result_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to generate markdown report: {}", e))
}

/// Generate HTML report file.
pub fn generate_html_file(
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
    file_path: &Path,
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
pub async fn generate_sonar_content(result: &AnalysisResults) -> anyhow::Result<String> {
    let result_json = serde_json::to_value(result)?;
    super::output::generate_sonar_report(&result_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to generate SonarQube report: {}", e))
}

/// Generate CSV report content.
pub async fn generate_csv_content(result: &AnalysisResults) -> anyhow::Result<String> {
    let result_json = serde_json::to_value(result)?;
    super::output::generate_csv_report(&result_json)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to generate CSV report: {}", e))
}

/// Generate default JSON report with optional oracle data.
pub fn generate_default_content(
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
pub fn format_file_info(format: &OutputFormat) -> (&'static str, &'static str) {
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
pub async fn generate_format_content(
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

/// Determines whether CLI output should be suppressed for the given args.
pub fn is_quiet(args: &AnalyzeArgs) -> bool {
    args.quiet || args.has_machine_readable_format()
}

/// Generate a single report for a specific format.
async fn generate_single_report(
    format: &OutputFormat,
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
    out_dir: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    let path = match format {
        OutputFormat::Html => {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let path = out_dir.join(format!("report_{}.html", timestamp));
            generate_html_file(result, oracle_response, &path)?;
            path
        }
        OutputFormat::Json => {
            let (filename, _) = format_file_info(format);
            let path = out_dir.join(filename);
            write_json_streaming(&path, result, oracle_response)?;
            path
        }
        OutputFormat::CiSummary => {
            let path = out_dir.join("ci-summary.json");
            let content = generate_ci_summary_content(result, oracle_response)?;
            write_report(&path, &content, "CI Summary").await?;
            path
        }
        OutputFormat::Pretty => {
            // Pretty format is for terminal display, not file output
            // Skip file generation but don't error
            return Ok(out_dir.join("(terminal output)"));
        }
        _ => {
            let (filename, format_label) = format_file_info(format);
            let path = out_dir.join(filename);
            let content = generate_format_content(format, result, oracle_response).await?;
            write_report(&path, &content, format_label).await?;
            path
        }
    };
    Ok(path)
}

/// Generate CI summary content (concise JSON for automated systems).
fn generate_ci_summary_content(
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
) -> anyhow::Result<String> {
    let summary = serde_json::json!({
        "status": if result.summary.critical > 0 { "critical" }
                  else if result.summary.high_priority > 0 { "warning" }
                  else { "ok" },
        "files_analyzed": result.summary.files_processed,
        "entities_analyzed": result.summary.entities_analyzed,
        "issues": {
            "total": result.summary.total_issues,
            "critical": result.summary.critical,
            "high": result.summary.high_priority,
        },
        "code_health_score": result.summary.code_health_score,
        "refactoring_candidates": result.refactoring_candidates.len(),
        "has_oracle_analysis": oracle_response.is_some(),
    });
    serde_json::to_string_pretty(&summary)
        .map_err(|e| anyhow::anyhow!("Failed to serialize CI summary: {}", e))
}

/// Generate reports with optional oracle data.
/// Supports multiple output formats via --format (repeatable) and --output-bundle.
pub async fn generate_reports_with_oracle(
    result: &AnalysisResults,
    oracle_response: &Option<valknut_rs::oracle::RefactoringOracleResponse>,
    args: &AnalyzeArgs,
) -> anyhow::Result<()> {
    let quiet_mode = is_quiet(args);
    let formats = args.effective_formats();

    if !quiet_mode {
        if formats.len() == 1 {
            println!("Saving report...");
        } else {
            println!("Saving {} reports...", formats.len());
        }
    }

    let mut output_files = Vec::new();

    for format in &formats {
        let path = generate_single_report(format, result, oracle_response, &args.out).await?;
        output_files.push((format.clone(), path));
    }

    if !quiet_mode {
        if output_files.len() == 1 {
            println!("Report: {}", output_files[0].1.display());
        } else {
            println!("Reports:");
            for (format, path) in &output_files {
                let format_name = super::output::format_to_string(format);
                println!("  {}: {}", format_name.to_uppercase(), path.display());
            }
        }
    }
    Ok(())
}

/// Generate output reports in various formats (legacy version for compatibility).
#[allow(dead_code)]
pub async fn generate_reports(result: &AnalysisResults, args: &AnalyzeArgs) -> anyhow::Result<()> {
    generate_reports_with_oracle(result, &None, args).await
}

// Re-export format_to_string from output module for backwards compatibility
pub use super::output::format_to_string;
