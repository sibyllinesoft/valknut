//! Report generation logic for various output formats.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use valknut_rs::api::results::AnalysisResults;
use valknut_rs::core::config::ReportFormat;
use valknut_rs::core::config::ValknutConfig;
use valknut_rs::io::agent_report::build_agent_report;
use valknut_rs::io::reports::ReportGenerator;

use crate::cli::args::{AnalyzeArgs, OutputFormat};

/// Helper to write content to a file with consistent error handling.
pub async fn write_report(path: &Path, content: &str, format_name: &str) -> anyhow::Result<()> {
    tokio::fs::write(path, content)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to write {} report: {}", format_name, e))
}

/// Write JSON report directly to file (streaming, avoids building string in memory).
pub fn write_json_streaming(path: &Path, result: &AnalysisResults) -> anyhow::Result<()> {
    let file =
        File::create(path).map_err(|e| anyhow::anyhow!("Failed to create JSON file: {}", e))?;
    let writer = BufWriter::new(file);

    serde_json::to_writer_pretty(writer, result)
        .map_err(|e| anyhow::anyhow!("Failed to write JSON: {}", e))
}

/// Generate JSON report content.
pub fn generate_json_content(result: &AnalysisResults) -> anyhow::Result<String> {
    serde_json::to_string_pretty(result)
        .map_err(|e| anyhow::anyhow!("Failed to serialize JSON: {}", e))
}

/// Generate the canonical agent-first JSON artifact.
pub fn generate_agent_json_content(
    result: &AnalysisResults,
    config: &ValknutConfig,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<String> {
    let report = build_agent_report(result, config, timestamp)?;
    serde_json::to_string_pretty(&report)
        .map_err(|e| anyhow::anyhow!("Failed to serialize agent report: {}", e))
}

/// Generate JSONL report content.
pub fn generate_jsonl_content(result: &AnalysisResults) -> anyhow::Result<String> {
    serde_json::to_string(result).map_err(|e| anyhow::anyhow!("Failed to serialize JSONL: {}", e))
}

/// Generate YAML report content.
pub fn generate_yaml_content(result: &AnalysisResults) -> anyhow::Result<String> {
    serde_yaml::to_string(result).map_err(|e| anyhow::anyhow!("Failed to serialize YAML: {}", e))
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
    file_path: &Path,
    config: Option<&ValknutConfig>,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<()> {
    let default_config = valknut_rs::api::config_types::AnalysisConfig::default();
    let generator = ReportGenerator::new().with_config(default_config);

    generator
        .generate_report(result, file_path, ReportFormat::Html)
        .map_err(|e| anyhow::anyhow!("Failed to generate HTML report: {}", e))?;

    if let Some(config) = config {
        embed_agent_report(file_path, result, config, timestamp)?;
    }
    Ok(())
}

fn embed_agent_report(
    file_path: &Path,
    result: &AnalysisResults,
    config: &ValknutConfig,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<()> {
    let report = build_agent_report(result, config, timestamp)?;
    let payload = serde_json::to_string(&report)?.replace("</", "<\\/");
    let script =
        format!("<script id=\"valknut-agent-report\" type=\"application/json\">{payload}</script>");
    let html = std::fs::read_to_string(file_path)?;
    let updated = if let Some(position) = html.rfind("</body>") {
        format!("{}{}{}", &html[..position], script, &html[position..])
    } else {
        format!("{html}{script}")
    };
    std::fs::write(file_path, updated)?;
    Ok(())
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
) -> anyhow::Result<String> {
    match format {
        OutputFormat::Json => generate_json_content(result),
        OutputFormat::Jsonl => generate_jsonl_content(result),
        OutputFormat::Yaml => generate_yaml_content(result),
        OutputFormat::Markdown => generate_markdown_content(result).await,
        OutputFormat::Sonar => generate_sonar_content(result).await,
        OutputFormat::Csv => generate_csv_content(result).await,
        _ => generate_json_content(result),
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
    out_dir: &std::path::Path,
    config: Option<&ValknutConfig>,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<std::path::PathBuf> {
    let path = match format {
        OutputFormat::Html => {
            let filename_timestamp = timestamp.format("%Y%m%d_%H%M%S");
            let path = out_dir.join(format!("report_{}.html", filename_timestamp));
            generate_html_file(result, &path, config, timestamp)?;
            path
        }
        OutputFormat::Json => {
            let (filename, _) = format_file_info(format);
            let path = out_dir.join(filename);
            if let Some(config) = config {
                let content = generate_agent_json_content(result, config, timestamp)?;
                std::fs::write(&path, content)?;
            } else {
                write_json_streaming(&path, result)?;
            }
            path
        }
        OutputFormat::CiSummary => {
            let path = out_dir.join("ci-summary.json");
            let content = generate_ci_summary_content(result)?;
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
            let content = generate_format_content(format, result).await?;
            write_report(&path, &content, format_label).await?;
            path
        }
    };
    Ok(path)
}

/// Generate CI summary content (concise JSON for automated systems).
fn generate_ci_summary_content(result: &AnalysisResults) -> anyhow::Result<String> {
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
    });
    serde_json::to_string_pretty(&summary)
        .map_err(|e| anyhow::anyhow!("Failed to serialize CI summary: {}", e))
}

/// Generate reports in the requested formats.
/// Supports multiple output formats via --format (repeatable) and --output-bundle.
pub async fn generate_reports(result: &AnalysisResults, args: &AnalyzeArgs) -> anyhow::Result<()> {
    generate_reports_with_config(result, args, None).await
}

/// Generate reports with the resolved configuration embedded in machine-readable artifacts.
pub async fn generate_reports_with_config(
    result: &AnalysisResults,
    args: &AnalyzeArgs,
    config: Option<&ValknutConfig>,
) -> anyhow::Result<()> {
    let quiet_mode = is_quiet(args);
    let formats = args.effective_formats();
    let timestamp = chrono::Utc::now();

    if !quiet_mode {
        if formats.len() == 1 {
            println!("Saving report...");
        } else {
            println!("Saving {} reports...", formats.len());
        }
    }

    let mut output_files = Vec::new();

    for format in &formats {
        let path = generate_single_report(format, result, &args.out, config, timestamp).await?;
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

// Re-export format_to_string from output module for backwards compatibility
pub use super::output::format_to_string;
