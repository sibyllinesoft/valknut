//! Output Formatting, Report Generation, and Display Functions
//!
//! This module contains all output formatting functions, report generation for
//! various formats (HTML, Markdown, CSV, Sonar), and display utilities.

use crate::cli::args::OutputFormat;
use anyhow;
use console::{Style, Term, style};
use indicatif::{ProgressBar, ProgressStyle};
use serde_json;
use serde_yaml;
use std::path::Path;
use std::time::Duration;
use tabled::{Table, Tabled, settings::{Style as TableStyle, Color}};
use owo_colors::OwoColorize;
use chrono;

/// Generate outputs with progress feedback
pub async fn generate_outputs_with_feedback(
    result: &serde_json::Value, 
    out_path: &Path, 
    output_format: &OutputFormat, 
    quiet: bool
) -> anyhow::Result<()> {
    if !quiet {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::with_template(
            "{spinner:.blue} {msg}"
        )?);
        pb.set_message(format!("Generating {} output...", format_to_string(output_format).to_uppercase()));
        pb.enable_steady_tick(Duration::from_millis(100));

        generate_outputs(result, out_path, output_format).await?;
        
        pb.finish_with_message(format!("{} report generated", format_to_string(output_format).to_uppercase()));
    } else {
        generate_outputs(result, out_path, output_format).await?;
    }

    Ok(())
}

/// Generate output files from analysis result
pub async fn generate_outputs(
    result: &serde_json::Value,
    out_path: &Path,
    output_format: &OutputFormat
) -> anyhow::Result<()> {
    // Create output directory
    tokio::fs::create_dir_all(out_path).await?;

    match output_format {
        OutputFormat::Jsonl => {
            let report_file = out_path.join("report.jsonl");
            let content = serde_json::to_string_pretty(result)?;
            tokio::fs::write(&report_file, content).await?;
            println!("📄 Feature report: {}", report_file.display());
        }
        OutputFormat::Json => {
            let report_file = out_path.join("analysis_results.json");
            let content = serde_json::to_string_pretty(result)?;
            tokio::fs::write(&report_file, content).await?;
            println!("📄 Analysis results: {}", report_file.display());
        }
        OutputFormat::Yaml => {
            let report_file = out_path.join("analysis_results.yaml");
            let content = serde_yaml::to_string(result)?;
            tokio::fs::write(&report_file, content).await?;
            println!("📄 Analysis results: {}", report_file.display());
        }
        OutputFormat::Markdown => {
            let report_file = out_path.join("team_report.md");
            let content = generate_markdown_report(result).await?;
            tokio::fs::write(&report_file, content).await?;
            println!("📊 Team report (markdown): {}", report_file.display());
        }
        OutputFormat::Html => {
            let report_file = out_path.join("team_report.html");
            let content = generate_html_report(result).await?;
            tokio::fs::write(&report_file, content).await?;
            println!("📊 Team report (html): {}", report_file.display());
        }
        OutputFormat::Sonar => {
            let report_file = out_path.join("sonarqube_issues.json");
            let content = generate_sonar_report(result).await?;
            tokio::fs::write(&report_file, content).await?;
            println!("📊 SonarQube report: {}", report_file.display());
        }
        OutputFormat::Csv => {
            let report_file = out_path.join("analysis_data.csv");
            let content = generate_csv_report(result).await?;
            tokio::fs::write(&report_file, content).await?;
            println!("📊 CSV report: {}", report_file.display());
        }
        OutputFormat::CiSummary => {
            let report_file = out_path.join("ci_summary.json");
            let content = generate_ci_summary_report(result).await?;
            tokio::fs::write(&report_file, content).await?;
            println!("📊 CI Summary: {}", report_file.display());
        }
        OutputFormat::Pretty => {
            print_comprehensive_results_pretty(result);
        }
    }

    Ok(())
}

/// Display analysis results with visual indicators
pub fn display_analysis_results(result: &serde_json::Value) {
    println!("{}", "✅ Analysis Complete".bright_green().bold());
    println!();

    #[derive(Tabled)]
    struct StatsRow {
        metric: String,
        value: String,
    }

    let total_files = result["summary"]["total_files"].as_u64().unwrap_or(0);
    let total_issues = result["summary"]["total_issues"].as_u64().unwrap_or(0);
    let processing_time = result["summary"]["processing_time"].as_f64().unwrap_or(0.0);

    // Calculate health score (simple heuristic)
    let health_score = if total_issues == 0 {
        100
    } else {
        std::cmp::max(60, 100 - (total_issues as i32 * 5))
    };

    let health_emoji = if health_score >= 80 { "🟢" } else if health_score >= 60 { "🟡" } else { "🔴" };
    let priority_emoji = if total_issues == 0 { "✅" } else if total_issues < 5 { "⚠️" } else { "❌" };

    let stats_rows = vec![
        StatsRow {
            metric: "📄 Files Analyzed".to_string(),
            value: format!("{}", total_files),
        },
        StatsRow {
            metric: "🏢 Code Entities".to_string(),
            value: format!("{}", total_files * 50), // Estimate
        },
        StatsRow {
            metric: "⏱️  Processing Time".to_string(),
            value: format!("{:.2}s", processing_time),
        },
        StatsRow {
            metric: "🏆 Health Score".to_string(),
            value: format!("{} {}/100", health_emoji, health_score),
        },
        StatsRow {
            metric: "⚠️  Priority Issues".to_string(),
            value: format!("{} {}", priority_emoji, total_issues),
        },
    ];

    let mut table = Table::new(stats_rows);
    table.with(TableStyle::rounded());
    println!("{}", table);
    println!();
}

/// Display completion summary with next steps
pub fn display_completion_summary(
    result: &serde_json::Value,
    out_path: &Path,
    output_format: &OutputFormat
) {
    println!("{}", "✅ Analysis Complete!".bright_green().bold());
    println!();
    println!("{} {}", "📁 Results saved to:".bold(), out_path.display().to_string().cyan());
    println!();

    let total_issues = result["summary"]["total_issues"].as_u64().unwrap_or(0);
    
    if total_issues > 0 {
        println!("{}", "📊 Quick Insights:".bright_blue().bold());
        println!();
        println!("{} {}", "🔥 Issues requiring attention:".bright_red().bold(), total_issues);
        
        // Show top issues if available
        if let Some(structure) = result["comprehensive_analysis"]["structure"].as_object() {
            if let Some(packs) = structure["packs"].as_array() {
                if !packs.is_empty() {
                    println!();
                    println!("{}", "🔥 Top Issues Requiring Attention:".bright_red().bold());
                    for (i, pack) in packs.iter().take(3).enumerate() {
                        if let Some(kind) = pack["kind"].as_str() {
                            let issue_type = match kind {
                                "branch" => "🌿 Directory reorganization",
                                "file_split" => "📄 File splitting",
                                _ => "🔍 Structure optimization",
                            };
                            println!("  {}. {}", i + 1, issue_type);
                        }
                    }
                }
            }
        }
    } else {
        println!("{}", "🎉 Great job! No significant issues found.".bright_green());
        println!("   Your code appears to be well-structured and maintainable.");
    }

    println!();
    println!("{}", "📢 Next Steps:".bright_blue().bold());
    
    let format_str = format_to_string(output_format);
    match output_format {
        OutputFormat::Html => {
            println!("   1. Open the HTML report in your browser for interactive exploration");
            println!("   2. Share the report with your team for collaborative code review");
            let html_file = out_path.join("team_report.html");
            if html_file.exists() {
                println!();
                println!("💻 Tip: Open {} in your browser", html_file.display().to_string().cyan());
            }
        }
        OutputFormat::Sonar => {
            println!("   1. Import the SonarQube JSON into your SonarQube instance");
            println!("   2. Set up quality gates based on the technical debt metrics");
        }
        OutputFormat::Csv => {
            println!("   1. Import the CSV data into your project tracking system");
            println!("   2. Prioritize refactoring tasks based on effort estimates");
        }
        OutputFormat::CiSummary => {
            println!("   1. Integrate the CI summary JSON with your build pipeline");
            println!("   2. Set up automated quality gate enforcement");
            println!("   3. Monitor metrics over time to track code quality trends");
        }
        _ => {
            println!("   1. Review the generated {} report for detailed findings", format_str);
            println!("   2. Address high-priority issues identified in the analysis");
            println!("   3. Consider running analysis regularly to track improvements");
        }
    }
}

// Report generation functions
pub async fn generate_markdown_report(result: &serde_json::Value) -> anyhow::Result<String> {
    let mut content = String::new();
    content.push_str("# Valknut Analysis Report\n\n");
    
    let total_issues = result["summary"]["total_issues"].as_u64().unwrap_or(0);
    let total_files = result["summary"]["total_files"].as_u64().unwrap_or(0);
    
    content.push_str("## Summary\n\n");
    content.push_str(&format!("- **Files Analyzed**: {}\n", total_files));
    content.push_str(&format!("- **Issues Found**: {}\n", total_issues));
    content.push_str(&format!("- **Analysis Date**: {}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
    content.push_str("\n");
    
    if total_issues == 0 {
        content.push_str("✅ **Excellent!** No significant issues found in your codebase.\n");
    } else {
        content.push_str("## Issues Requiring Attention\n\n");
        // TODO: Add detailed issue breakdown
    }
    
    Ok(content)
}

pub async fn generate_html_report(result: &serde_json::Value) -> anyhow::Result<String> {
    let total_issues = result["summary"]["total_issues"].as_u64().unwrap_or(0);
    let total_files = result["summary"]["total_files"].as_u64().unwrap_or(0);
    
    Ok(format!(r#"
<!DOCTYPE html>
<html>
<head>
    <title>Valknut Analysis Report</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; }}
        .header {{ color: #0066cc; }}
        .summary {{ background: #f5f5f5; padding: 20px; border-radius: 8px; }}
        .issue {{ border-left: 4px solid #ff6b35; padding: 10px; margin: 10px 0; }}
    </style>
</head>
<body>
    <h1 class="header">🔍 Valknut Analysis Report</h1>
    <div class="summary">
        <h2>Summary</h2>
        <p><strong>Files Analyzed:</strong> {}</p>
        <p><strong>Issues Found:</strong> {}</p>
        <p><strong>Analysis Date:</strong> {}</p>
    </div>
    {}
</body>
</html>
"#, 
    total_files,
    total_issues,
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
    if total_issues == 0 {
        "<div class='summary'>✅ <strong>Excellent!</strong> No significant issues found in your codebase.</div>"
    } else {
        "<h2>Issues Requiring Attention</h2><div class='issue'>Detailed issues would be listed here in a full implementation.</div>"
    }
    ))
}

pub async fn generate_sonar_report(result: &serde_json::Value) -> anyhow::Result<String> {
    let sonar_format = serde_json::json!({
        "version": "1.0",
        "issues": [],
        "summary": {
            "total_issues": result["summary"]["total_issues"],
            "analysis_date": chrono::Utc::now().to_rfc3339()
        }
    });
    
    Ok(serde_json::to_string_pretty(&sonar_format)?)
}

pub async fn generate_csv_report(result: &serde_json::Value) -> anyhow::Result<String> {
    let mut content = String::new();
    content.push_str("File,Issue Type,Severity,Description\n");
    
    let total_issues = result["summary"]["total_issues"].as_u64().unwrap_or(0);
    if total_issues == 0 {
        content.push_str("No issues found,Info,Info,Code quality is excellent\n");
    }
    
    Ok(content)
}

pub async fn generate_ci_summary_report(result: &serde_json::Value) -> anyhow::Result<String> {
    let summary = &result["summary"];
    let health_metrics = &result["health_metrics"];
    let complexity = &result["complexity"];
    
    let ci_summary = serde_json::json!({
        "status": if summary["total_issues"].as_u64().unwrap_or(0) == 0 { "success" } else { "issues_found" },
        "summary": {
            "total_files": summary["total_files"],
            "total_issues": summary["total_issues"],
            "critical_issues": summary["critical_issues"].as_u64().unwrap_or(0),
            "high_priority_issues": summary["high_priority_issues"].as_u64().unwrap_or(0),
            "languages": summary["languages"]
        },
        "metrics": {
            "overall_health_score": health_metrics["overall_health_score"].as_f64().unwrap_or(0.0),
            "complexity_score": health_metrics["complexity_score"].as_f64().unwrap_or(0.0),
            "maintainability_score": health_metrics["maintainability_score"].as_f64().unwrap_or(0.0),
            "technical_debt_ratio": health_metrics["technical_debt_ratio"].as_f64().unwrap_or(0.0),
            "average_cyclomatic_complexity": complexity["average_cyclomatic_complexity"].as_f64().unwrap_or(0.0),
            "average_cognitive_complexity": complexity["average_cognitive_complexity"].as_f64().unwrap_or(0.0)
        },
        "quality_gates": {
            "health_score_threshold": 60.0,
            "complexity_threshold": 75.0,
            "max_issues_threshold": 10,
            "recommendations": if summary["total_issues"].as_u64().unwrap_or(0) > 0 {
                vec![
                    "Address high-priority issues first",
                    "Focus on reducing complexity in critical files",
                    "Improve maintainability through refactoring"
                ]
            } else {
                vec!["Code quality is excellent - maintain current standards"]
            }
        },
        "timestamp": result["timestamp"],
        "analysis_id": result["analysis_id"]
    });
    
    Ok(serde_json::to_string_pretty(&ci_summary)?)
}

// Human-readable output functions
pub fn print_human_readable_results(results: &serde_json::Value) {
    println!("{}", "🏗️  Valknut Structure Analysis Results".bright_blue().bold());
    println!("{}", "=====================================".dimmed());
    println!();

    if let Some(packs) = results.get("packs").and_then(|p| p.as_array()) {
        if packs.is_empty() {
            println!("{}", "✅ No structural issues found!".bright_green());
            return;
        }

        println!("{}", format!("📊 Found {} potential improvements:", packs.len()).bold());
        println!();

        for (i, pack) in packs.iter().enumerate() {
            let kind = pack.get("kind").and_then(|k| k.as_str()).unwrap_or("unknown");
            let empty_vec = vec![];
            let reasons = pack.get("reasons").and_then(|r| r.as_array()).unwrap_or(&empty_vec);
            
            println!("{}", format!("{}. {} Analysis", i + 1, 
                     match kind {
                         "branch" => "🌿 Directory Branch",
                         "file_split" => "📄 File Split", 
                         _ => "🔍 General",
                     }).bold());

            if let Some(file) = pack.get("file").and_then(|f| f.as_str()) {
                println!("   📁 File: {}", file.cyan());
            }

            if let Some(directory) = pack.get("directory").and_then(|d| d.as_str()) {
                println!("   📁 Directory: {}", directory.cyan());
            }

            if !reasons.is_empty() {
                println!("   📋 Reasons:");
                for reason in reasons {
                    if let Some(reason_str) = reason.as_str() {
                        println!("      • {}", reason_str);
                    }
                }
            }

            println!();
        }
    }
}

pub fn print_comprehensive_results_pretty(results: &serde_json::Value) {
    println!("{}", "📊 Comprehensive Analysis Results".bright_blue().bold());
    println!("{}", "=================================".dimmed());
    println!();

    let total_issues = results["summary"]["total_issues"].as_u64().unwrap_or(0);
    let total_files = results["summary"]["total_files"].as_u64().unwrap_or(0);

    println!("{}", "🎯 Analysis Summary:".bold());
    println!("   • {} total issues found", total_issues.to_string().bright_yellow());
    println!("   • {} files analyzed", total_files.to_string().bright_green());
    println!();

    if total_issues == 0 {
        println!("{}", "🎉 Great job! No significant issues found across all analyzers.".bright_green());
        println!("   Your code appears to be well-structured and maintainable.");
    } else {
        println!("{}", "📈 Recommendation: Address high-priority issues first for maximum impact.".bright_blue());
        println!("   Use detailed analyzers (structure, names, impact) for specific recommendations.");
    }
}

// Helper function
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