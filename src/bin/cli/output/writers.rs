//! File writing functions for various output formats

use std::path::Path;

use valknut_rs::api::results::AnalysisResults;
use valknut_rs::core::config::ReportFormat;
use valknut_rs::io::reports::assets::copy_webpage_assets_to_output;
use valknut_rs::io::reports::ReportGenerator;

use super::csv_export::{generate_ci_summary_report, generate_csv_report};
use super::reports::{generate_html_report, generate_markdown_report};
use super::sonar::generate_sonar_report;

/// Build a report generator, optionally loading templates from a directory.
pub fn build_report_generator() -> anyhow::Result<ReportGenerator> {
    let templates_dir = std::path::Path::new("templates");
    if templates_dir.exists() {
        ReportGenerator::new()
            .with_templates_dir(templates_dir)
            .map_err(|e| anyhow::anyhow!("Failed to load templates: {}", e))
    } else {
        Ok(ReportGenerator::new())
    }
}

/// Write JSONL format output.
pub async fn write_jsonl(result: &serde_json::Value, out_path: &Path) -> anyhow::Result<()> {
    let report_file = out_path.join("report.jsonl");
    let content = serde_json::to_string_pretty(result)?;
    tokio::fs::write(&report_file, content).await?;
    println!("📄 Feature report: {}", report_file.display());
    Ok(())
}

/// Write JSON format output.
pub async fn write_json(
    generator: &ReportGenerator,
    analysis_results: Option<&AnalysisResults>,
    result: &serde_json::Value,
    out_path: &Path,
) -> anyhow::Result<()> {
    let report_file = out_path.join("analysis_results.json");
    if let Some(results) = analysis_results {
        generator.generate_report(results, &report_file, ReportFormat::Json)?;
    } else {
        let content = serde_json::to_string_pretty(result)?;
        tokio::fs::write(&report_file, content).await?;
    }
    println!("📄 Analysis results: {}", report_file.display());
    Ok(())
}

/// Write YAML format output.
pub async fn write_yaml(
    generator: &ReportGenerator,
    analysis_results: Option<&AnalysisResults>,
    result: &serde_json::Value,
    out_path: &Path,
) -> anyhow::Result<()> {
    let report_file = out_path.join("analysis_results.yaml");
    if let Some(results) = analysis_results {
        generator.generate_report(results, &report_file, ReportFormat::Yaml)?;
    } else {
        let content = serde_yaml::to_string(result)?;
        tokio::fs::write(&report_file, content).await?;
    }
    println!("📄 Analysis results: {}", report_file.display());
    Ok(())
}

/// Write Markdown format output.
pub async fn write_markdown(
    generator: &ReportGenerator,
    analysis_results: Option<&AnalysisResults>,
    result: &serde_json::Value,
    out_path: &Path,
) -> anyhow::Result<()> {
    let report_file = out_path.join("team_report.md");
    if let Some(results) = analysis_results {
        generator.generate_markdown_report(results, &report_file)?;
    } else {
        let content = generate_markdown_report(result).await?;
        tokio::fs::write(&report_file, content).await?;
    }
    println!("📊 Team report (markdown): {}", report_file.display());
    Ok(())
}

/// Write HTML format output.
pub async fn write_html(
    generator: &ReportGenerator,
    analysis_results: Option<&AnalysisResults>,
    result: &serde_json::Value,
    out_path: &Path,
) -> anyhow::Result<()> {
    let report_file = out_path.join("team_report.html");
    copy_webpage_assets_to_output(out_path).map_err(anyhow::Error::msg)?;

    if let Some(results) = analysis_results {
        generator.generate_report(results, &report_file, ReportFormat::Html)?;
    } else {
        let content = generate_html_report(result).await?;
        tokio::fs::write(&report_file, content).await?;
    }

    println!("📊 Team report (html): {}", report_file.display());
    Ok(())
}

/// Write SonarQube format output.
pub async fn write_sonar(
    generator: &ReportGenerator,
    analysis_results: Option<&AnalysisResults>,
    result: &serde_json::Value,
    out_path: &Path,
) -> anyhow::Result<()> {
    let report_file = out_path.join("sonarqube_issues.json");
    if let Some(results) = analysis_results {
        generator.generate_sonar_report(results, &report_file)?;
    } else {
        let content = generate_sonar_report(result).await?;
        tokio::fs::write(&report_file, content).await?;
    }
    println!("📊 SonarQube report: {}", report_file.display());
    Ok(())
}

/// Write CSV format output.
pub async fn write_csv(
    generator: &ReportGenerator,
    analysis_results: Option<&AnalysisResults>,
    result: &serde_json::Value,
    out_path: &Path,
) -> anyhow::Result<()> {
    let report_file = out_path.join("analysis_data.csv");
    if let Some(results) = analysis_results {
        generator.generate_csv_table(results, &report_file)?;
    } else {
        let content = generate_csv_report(result).await?;
        tokio::fs::write(&report_file, content).await?;
    }
    println!("📊 CSV report: {}", report_file.display());
    Ok(())
}

/// Write CI summary format output.
pub async fn write_ci_summary(result: &serde_json::Value, out_path: &Path) -> anyhow::Result<()> {
    let report_file = out_path.join("ci_summary.json");
    let content = generate_ci_summary_report(result).await?;
    tokio::fs::write(&report_file, content).await?;
    println!("📊 CI Summary: {}", report_file.display());
    Ok(())
}
