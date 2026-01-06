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
    args.quiet || args.format.is_machine_readable()
}

/// Generate reports with optional oracle data.
pub async fn generate_reports_with_oracle(
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
    } else if args.format == OutputFormat::Json {
        // Use streaming for JSON to avoid building large string in memory
        let (filename, _) = format_file_info(&args.format);
        let path = args.out.join(filename);
        write_json_streaming(&path, result, oracle_response)?;
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

/// Generate output reports in various formats (legacy version for compatibility).
#[allow(dead_code)]
pub async fn generate_reports(result: &AnalysisResults, args: &AnalyzeArgs) -> anyhow::Result<()> {
    generate_reports_with_oracle(result, &None, args).await
}

// Re-export format_to_string from output module for backwards compatibility
pub use super::output::format_to_string;
