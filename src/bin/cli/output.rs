//! Output Formatting, Report Generation, and Display Functions
//!
//! This module contains all output formatting functions, report generation for
//! various formats (HTML, Markdown, CSV, Sonar), and display utilities.

use crate::cli::args::OutputFormat;
use anyhow;
use chrono;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde_json;
use serde_yaml;
use std::path::Path;
use std::time::Duration;
use tabled::{settings::Style as TableStyle, Table, Tabled};

// Import our proper report generator
use valknut_rs::api::results::AnalysisResults;
use valknut_rs::core::config::ReportFormat;
use valknut_rs::io::reports::ReportGenerator;

/// Generate outputs with progress feedback
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
pub async fn generate_outputs(
    result: &serde_json::Value,
    out_path: &Path,
    output_format: &OutputFormat,
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

            // Use the proper ReportGenerator with Sibylline theme
            let templates_dir = std::path::Path::new("templates");
            let generator = ReportGenerator::new()
                .with_templates_dir(templates_dir)
                .map_err(|e| anyhow::anyhow!("Failed to load templates: {}", e))?;

            // Convert JSON back to AnalysisResults (this is not ideal but works)
            if let Ok(analysis_results) = serde_json::from_value::<AnalysisResults>(result.clone())
            {
                generator.generate_report(&analysis_results, &report_file, ReportFormat::Html)?;
            } else {
                // Fallback to old HTML generation if conversion fails
                let content = generate_html_report(result).await?;
                tokio::fs::write(&report_file, content).await?;
            }

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

    let health_emoji = if health_score >= 80 {
        "🟢"
    } else if health_score >= 60 {
        "🟡"
    } else {
        "🔴"
    };
    let priority_emoji = if total_issues == 0 {
        "✅"
    } else if total_issues < 5 {
        "⚠️"
    } else {
        "❌"
    };

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
    output_format: &OutputFormat,
) {
    println!("{}", "✅ Analysis Complete!".bright_green().bold());
    println!();
    println!(
        "{} {}",
        "📁 Results saved to:".bold(),
        out_path.display().to_string().cyan()
    );
    println!();

    let total_issues = result["summary"]["total_issues"].as_u64().unwrap_or(0);

    if total_issues > 0 {
        println!("{}", "📊 Quick Insights:".bright_blue().bold());
        println!();
        println!(
            "{} {}",
            "🔥 Issues requiring attention:".bright_red().bold(),
            total_issues
        );

        // Show top issues if available
        if let Some(structure) = result["comprehensive_analysis"]["structure"].as_object() {
            if let Some(packs) = structure["packs"].as_array() {
                if !packs.is_empty() {
                    println!();
                    println!(
                        "{}",
                        "🔥 Top Issues Requiring Attention:".bright_red().bold()
                    );
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
        println!(
            "{}",
            "🎉 Great job! No significant issues found.".bright_green()
        );
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
                println!(
                    "💻 Tip: Open {} in your browser",
                    html_file.display().to_string().cyan()
                );
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
            println!(
                "   1. Review the generated {} report for detailed findings",
                format_str
            );
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
    content.push_str(&format!(
        "- **Analysis Date**: {}\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    content.push_str("\n");

    if total_issues == 0 {
        content.push_str("✅ **Excellent!** No significant issues found in your codebase.\n");
    } else {
        content.push_str("## Issues Requiring Attention\n\n");

        // Add health metrics
        if let Some(health_metrics) = result.get("health_metrics") {
            content.push_str("### Health Metrics\n\n");
            if let Some(overall_health) = health_metrics
                .get("overall_health_score")
                .and_then(|v| v.as_f64())
            {
                let health_emoji = if overall_health >= 80.0 {
                    "🟢"
                } else if overall_health >= 60.0 {
                    "🟡"
                } else {
                    "🔴"
                };
                content.push_str(&format!(
                    "- **Overall Health Score**: {} {:.1}/100\n",
                    health_emoji, overall_health
                ));
            }
            if let Some(complexity_score) = health_metrics
                .get("complexity_score")
                .and_then(|v| v.as_f64())
            {
                content.push_str(&format!(
                    "- **Complexity Score**: {:.1}/100 (lower is better)\n",
                    complexity_score
                ));
            }
            if let Some(debt_ratio) = health_metrics
                .get("technical_debt_ratio")
                .and_then(|v| v.as_f64())
            {
                content.push_str(&format!(
                    "- **Technical Debt Ratio**: {:.1}% (lower is better)\n",
                    debt_ratio
                ));
            }
            if let Some(maintainability) = health_metrics
                .get("maintainability_score")
                .and_then(|v| v.as_f64())
            {
                content.push_str(&format!(
                    "- **Maintainability Score**: {:.1}/100\n",
                    maintainability
                ));
            }
            content.push_str("\n");
        }

        // Add complexity analysis results
        if let Some(complexity) = result.get("complexity") {
            if let Some(detailed_results) = complexity
                .get("detailed_results")
                .and_then(|v| v.as_array())
            {
                let high_priority_files: Vec<_> = detailed_results
                    .iter()
                    .filter(|file_result| {
                        file_result
                            .get("issues")
                            .and_then(|issues| issues.as_array())
                            .map(|issues| !issues.is_empty())
                            .unwrap_or(false)
                    })
                    .collect();

                if !high_priority_files.is_empty() {
                    content.push_str("### High Priority Files\n\n");
                    content.push_str(
                        "Files with complexity issues that should be addressed first:\n\n",
                    );

                    for (i, file_result) in high_priority_files.iter().take(10).enumerate() {
                        if let Some(file_path) =
                            file_result.get("file_path").and_then(|v| v.as_str())
                        {
                            content.push_str(&format!("#### {}. `{}`\n\n", i + 1, file_path));

                            if let Some(issues) =
                                file_result.get("issues").and_then(|v| v.as_array())
                            {
                                for issue in issues.iter().take(5) {
                                    // Limit to top 5 issues per file
                                    if let (Some(description), Some(severity)) = (
                                        issue.get("description").and_then(|v| v.as_str()),
                                        issue.get("severity").and_then(|v| v.as_str()),
                                    ) {
                                        let severity_emoji = match severity {
                                            "Critical" => "🔴",
                                            "VeryHigh" => "🟠",
                                            "High" => "🟡",
                                            _ => "⚠️",
                                        };
                                        content.push_str(&format!(
                                            "- {} **{}**: {}\n",
                                            severity_emoji, severity, description
                                        ));
                                    }
                                }
                            }

                            if let Some(recommendations) = file_result
                                .get("recommendations")
                                .and_then(|v| v.as_array())
                            {
                                if !recommendations.is_empty() {
                                    content.push_str("\n**Recommended Actions:**\n");
                                    for (j, rec) in recommendations.iter().take(3).enumerate() {
                                        if let Some(desc) =
                                            rec.get("description").and_then(|v| v.as_str())
                                        {
                                            let effort = rec
                                                .get("effort")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(1);
                                            content.push_str(&format!(
                                                "{}. {} (Effort: {})\n",
                                                j + 1,
                                                desc,
                                                effort
                                            ));
                                        }
                                    }
                                }
                            }
                            content.push_str("\n");
                        }
                    }
                }
            }

            // Add summary statistics
            content.push_str("### Summary Statistics\n\n");
            if let Some(avg_cyclomatic) = complexity
                .get("average_cyclomatic_complexity")
                .and_then(|v| v.as_f64())
            {
                content.push_str(&format!(
                    "- **Average Cyclomatic Complexity**: {:.1}\n",
                    avg_cyclomatic
                ));
            }
            if let Some(avg_cognitive) = complexity
                .get("average_cognitive_complexity")
                .and_then(|v| v.as_f64())
            {
                content.push_str(&format!(
                    "- **Average Cognitive Complexity**: {:.1}\n",
                    avg_cognitive
                ));
            }
            if let Some(avg_debt) = complexity
                .get("average_technical_debt_score")
                .and_then(|v| v.as_f64())
            {
                content.push_str(&format!(
                    "- **Average Technical Debt Score**: {:.1}\n",
                    avg_debt
                ));
            }
            content.push_str("\n");
        }

        // Add refactoring opportunities
        if let Some(refactoring) = result.get("refactoring") {
            if let Some(opportunities_count) = refactoring
                .get("opportunities_count")
                .and_then(|v| v.as_u64())
            {
                if opportunities_count > 0 {
                    content.push_str("### Refactoring Opportunities\n\n");
                    content.push_str(&format!(
                        "Found **{}** refactoring opportunities across the codebase.\n\n",
                        opportunities_count
                    ));
                }
            }
        }

        content.push_str("## Recommendations\n\n");
        content.push_str("1. **Start with Critical Issues**: Focus on files with critical and high-severity issues first\n");
        content.push_str("2. **Reduce Complexity**: Break down large functions and simplify complex conditionals\n");
        content.push_str("3. **Improve Maintainability**: Address technical debt systematically\n");
        content.push_str(
            "4. **Regular Monitoring**: Run analysis regularly to track improvements\n\n",
        );

        content.push_str("---\n\n");
        content.push_str("*Report generated by [Valknut](https://github.com/nathanricedev/valknut) - AI-Powered Code Analysis*\n");
    }

    Ok(content)
}

pub async fn generate_html_report(result: &serde_json::Value) -> anyhow::Result<String> {
    let total_issues = result["summary"]["total_issues"].as_u64().unwrap_or(0);
    let total_files = result["summary"]["total_files"].as_u64().unwrap_or(0);

    let mut details_html = String::new();

    if total_issues == 0 {
        details_html.push_str("<div class='success-message'>✅ <strong>Excellent!</strong> No significant issues found in your codebase.</div>");
    } else {
        // Add health metrics section
        if let Some(health_metrics) = result.get("health_metrics") {
            details_html.push_str("<h2>📊 Health Metrics</h2>");
            details_html.push_str("<div class='metrics-grid'>");

            if let Some(overall_health) = health_metrics
                .get("overall_health_score")
                .and_then(|v| v.as_f64())
            {
                let health_class = if overall_health >= 80.0 {
                    "metric-good"
                } else if overall_health >= 60.0 {
                    "metric-warning"
                } else {
                    "metric-critical"
                };
                details_html.push_str(&format!(
                    "<div class='metric-card {}'><h3>Overall Health</h3><div class='metric-value'>{:.1}/100</div></div>",
                    health_class, overall_health
                ));
            }

            if let Some(complexity_score) = health_metrics
                .get("complexity_score")
                .and_then(|v| v.as_f64())
            {
                let complexity_class = if complexity_score <= 25.0 {
                    "metric-good"
                } else if complexity_score <= 50.0 {
                    "metric-warning"
                } else {
                    "metric-critical"
                };
                details_html.push_str(&format!(
                    "<div class='metric-card {}'><h3>Complexity Score</h3><div class='metric-value'>{:.1}/100</div><small>lower is better</small></div>",
                    complexity_class, complexity_score
                ));
            }

            if let Some(debt_ratio) = health_metrics
                .get("technical_debt_ratio")
                .and_then(|v| v.as_f64())
            {
                let debt_class = if debt_ratio <= 20.0 {
                    "metric-good"
                } else if debt_ratio <= 40.0 {
                    "metric-warning"
                } else {
                    "metric-critical"
                };
                details_html.push_str(&format!(
                    "<div class='metric-card {}'><h3>Technical Debt</h3><div class='metric-value'>{:.1}%</div><small>lower is better</small></div>",
                    debt_class, debt_ratio
                ));
            }

            if let Some(maintainability) = health_metrics
                .get("maintainability_score")
                .and_then(|v| v.as_f64())
            {
                let maintainability_class = if maintainability >= 60.0 {
                    "metric-good"
                } else if maintainability >= 40.0 {
                    "metric-warning"
                } else {
                    "metric-critical"
                };
                details_html.push_str(&format!(
                    "<div class='metric-card {}'><h3>Maintainability</h3><div class='metric-value'>{:.1}/100</div></div>",
                    maintainability_class, maintainability
                ));
            }

            details_html.push_str("</div>");
        }

        // Add complexity analysis details
        if let Some(complexity) = result.get("complexity") {
            if let Some(detailed_results) = complexity
                .get("detailed_results")
                .and_then(|v| v.as_array())
            {
                let high_priority_files: Vec<_> = detailed_results
                    .iter()
                    .filter(|file_result| {
                        file_result
                            .get("issues")
                            .and_then(|issues| issues.as_array())
                            .map(|issues| !issues.is_empty())
                            .unwrap_or(false)
                    })
                    .collect();

                if !high_priority_files.is_empty() {
                    details_html.push_str("<h2>🔥 High Priority Files</h2>");
                    details_html.push_str(
                        "<p>Files with complexity issues that should be addressed first:</p>",
                    );

                    for (i, file_result) in high_priority_files.iter().take(10).enumerate() {
                        if let Some(file_path) =
                            file_result.get("file_path").and_then(|v| v.as_str())
                        {
                            details_html.push_str(&format!(
                                "<div class='file-section'><h3>{}.&nbsp;<code>{}</code></h3>",
                                i + 1,
                                file_path
                            ));

                            if let Some(issues) =
                                file_result.get("issues").and_then(|v| v.as_array())
                            {
                                details_html.push_str("<div class='issues-list'>");
                                for issue in issues.iter().take(5) {
                                    if let (Some(description), Some(severity)) = (
                                        issue.get("description").and_then(|v| v.as_str()),
                                        issue.get("severity").and_then(|v| v.as_str()),
                                    ) {
                                        let (severity_emoji, severity_class) = match severity {
                                            "Critical" => ("🔴", "severity-critical"),
                                            "VeryHigh" => ("🟠", "severity-very-high"),
                                            "High" => ("🟡", "severity-high"),
                                            _ => ("⚠️", "severity-medium"),
                                        };
                                        details_html.push_str(&format!(
                                            "<div class='issue-item {}'><span class='severity-indicator'>{} {}</span><span class='issue-description'>{}</span></div>",
                                            severity_class, severity_emoji, severity, description
                                        ));
                                    }
                                }
                                details_html.push_str("</div>");
                            }

                            if let Some(recommendations) = file_result
                                .get("recommendations")
                                .and_then(|v| v.as_array())
                            {
                                if !recommendations.is_empty() {
                                    details_html.push_str("<div class='recommendations'><h4>💡 Recommended Actions:</h4><ol>");
                                    for rec in recommendations.iter().take(3) {
                                        if let Some(desc) =
                                            rec.get("description").and_then(|v| v.as_str())
                                        {
                                            let effort = rec
                                                .get("effort")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(1);
                                            let effort_class = match effort {
                                                1..=3 => "effort-low",
                                                4..=6 => "effort-medium",
                                                7..=10 => "effort-high",
                                                _ => "effort-unknown",
                                            };
                                            details_html.push_str(&format!(
                                                "<li><span class='recommendation-text'>{}</span> <span class='effort-indicator {}'>(Effort: {})</span></li>",
                                                desc, effort_class, effort
                                            ));
                                        }
                                    }
                                    details_html.push_str("</ol></div>");
                                }
                            }
                            details_html.push_str("</div>");
                        }
                    }
                }
            }
        }

        // Add refactoring opportunities
        if let Some(refactoring) = result.get("refactoring") {
            if let Some(opportunities_count) = refactoring
                .get("opportunities_count")
                .and_then(|v| v.as_u64())
            {
                if opportunities_count > 0 {
                    details_html.push_str("<h2>🔧 Refactoring Opportunities</h2>");
                    details_html.push_str(&format!("<p>Found <strong>{}</strong> refactoring opportunities across the codebase.</p>", opportunities_count));

                    if let Some(detailed_results) = refactoring
                        .get("detailed_results")
                        .and_then(|v| v.as_array())
                    {
                        details_html.push_str("<div class='refactoring-list'>");
                        for file_result in detailed_results.iter().take(8) {
                            if let Some(file_path) =
                                file_result.get("file_path").and_then(|v| v.as_str())
                            {
                                if let Some(recommendations) = file_result
                                    .get("recommendations")
                                    .and_then(|v| v.as_array())
                                {
                                    if recommendations.is_empty() {
                                        continue;
                                    }

                                    details_html.push_str(&format!(
                                        "<div class='refactoring-file'><h4>📄 {}</h4>",
                                        file_path
                                    ));
                                    details_html.push_str("<div class='refactoring-items'>");

                                    for rec in recommendations.iter().take(3) {
                                        if let (
                                            Some(description),
                                            Some(refactoring_type),
                                            Some(impact),
                                            Some(effort),
                                        ) = (
                                            rec.get("description").and_then(|v| v.as_str()),
                                            rec.get("refactoring_type").and_then(|v| v.as_str()),
                                            rec.get("estimated_impact").and_then(|v| v.as_f64()),
                                            rec.get("estimated_effort").and_then(|v| v.as_f64()),
                                        ) {
                                            let type_emoji = match refactoring_type {
                                                "ExtractMethod" => "⚡",
                                                "ExtractClass" => "📦",
                                                "ReduceComplexity" => "🎯",
                                                "EliminateDuplication" => "🔄",
                                                "ImproveNaming" => "📝",
                                                "SimplifyConditionals" => "🔀",
                                                "RemoveDeadCode" => "🧹",
                                                _ => "🔧",
                                            };

                                            let priority_score = rec
                                                .get("priority_score")
                                                .and_then(|v| v.as_f64())
                                                .unwrap_or(0.0);

                                            details_html.push_str(&format!(
                                                "<div class='refactoring-item'><div class='refactoring-header'>{} <strong>{}</strong></div><div class='refactoring-description'>{}</div><div class='refactoring-metrics'>Impact: {:.1}/10 | Effort: {:.1}/10 | Priority: {:.2}</div></div>",
                                                type_emoji, refactoring_type.replace("Extract", "Extract ").replace("Reduce", "Reduce ").replace("Eliminate", "Eliminate ").replace("Improve", "Improve ").replace("Simplify", "Simplify ").replace("Remove", "Remove "), description, impact, effort, priority_score
                                            ));
                                        }
                                    }
                                    details_html.push_str("</div></div>");
                                }
                            }
                        }
                        details_html.push_str("</div>");
                    }
                }
            }
        }

        // Add summary statistics
        if let Some(complexity) = result.get("complexity") {
            details_html.push_str("<h2>📈 Summary Statistics</h2>");
            details_html.push_str("<div class='stats-grid'>");

            if let Some(avg_cyclomatic) = complexity
                .get("average_cyclomatic_complexity")
                .and_then(|v| v.as_f64())
            {
                details_html.push_str(&format!("<div class='stat-item'><span class='stat-label'>Average Cyclomatic Complexity</span><span class='stat-value'>{:.1}</span></div>", avg_cyclomatic));
            }
            if let Some(avg_cognitive) = complexity
                .get("average_cognitive_complexity")
                .and_then(|v| v.as_f64())
            {
                details_html.push_str(&format!("<div class='stat-item'><span class='stat-label'>Average Cognitive Complexity</span><span class='stat-value'>{:.1}</span></div>", avg_cognitive));
            }
            if let Some(avg_debt) = complexity
                .get("average_technical_debt_score")
                .and_then(|v| v.as_f64())
            {
                details_html.push_str(&format!("<div class='stat-item'><span class='stat-label'>Average Technical Debt Score</span><span class='stat-value'>{:.1}</span></div>", avg_debt));
            }

            details_html.push_str("</div>");
        }

        // Add recommendations
        details_html.push_str("<h2>💡 Recommendations</h2>");
        details_html.push_str("<ol class='recommendations-list'>");
        details_html.push_str("<li><strong>Start with Critical Issues</strong>: Focus on files with critical and high-severity issues first</li>");
        details_html.push_str("<li><strong>Reduce Complexity</strong>: Break down large functions and simplify complex conditionals</li>");
        details_html.push_str("<li><strong>Improve Maintainability</strong>: Address technical debt systematically</li>");
        details_html.push_str("<li><strong>Regular Monitoring</strong>: Run analysis regularly to track improvements</li>");
        details_html.push_str("</ol>");
    }

    Ok(format!(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Valknut Analysis Report</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        * {{
            box-sizing: border-box;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
            line-height: 1.6;
            margin: 0;
            padding: 20px;
            background-color: #f8fafc;
            color: #1a202c;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            border-radius: 12px;
            box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
            overflow: hidden;
        }}
        .header {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 2rem;
            text-align: center;
        }}
        .header h1 {{
            margin: 0;
            font-size: 2.5rem;
            font-weight: 600;
        }}
        .content {{
            padding: 2rem;
        }}
        .summary {{
            background: #f7fafc;
            border: 1px solid #e2e8f0;
            border-radius: 8px;
            padding: 1.5rem;
            margin-bottom: 2rem;
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1rem;
        }}
        .summary-item {{
            text-align: center;
        }}
        .summary-label {{
            display: block;
            font-size: 0.875rem;
            color: #64748b;
            margin-bottom: 0.5rem;
        }}
        .summary-value {{
            display: block;
            font-size: 2rem;
            font-weight: 700;
            color: #1e293b;
        }}
        .metrics-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1rem;
            margin-bottom: 2rem;
        }}
        .metric-card {{
            padding: 1.5rem;
            border-radius: 8px;
            text-align: center;
            border: 2px solid transparent;
        }}
        .metric-good {{
            background: #f0fdf4;
            border-color: #22c55e;
        }}
        .metric-warning {{
            background: #fffbeb;
            border-color: #f59e0b;
        }}
        .metric-critical {{
            background: #fef2f2;
            border-color: #ef4444;
        }}
        .metric-card h3 {{
            margin: 0 0 0.5rem;
            font-size: 1rem;
            color: #64748b;
        }}
        .metric-value {{
            font-size: 2rem;
            font-weight: 700;
            margin-bottom: 0.25rem;
        }}
        .metric-good .metric-value {{ color: #16a34a; }}
        .metric-warning .metric-value {{ color: #d97706; }}
        .metric-critical .metric-value {{ color: #dc2626; }}
        .file-section {{
            background: white;
            border: 1px solid #e2e8f0;
            border-radius: 8px;
            margin-bottom: 1.5rem;
            overflow: hidden;
        }}
        .file-section h3 {{
            background: #f8fafc;
            padding: 1rem 1.5rem;
            margin: 0;
            border-bottom: 1px solid #e2e8f0;
            color: #1e293b;
        }}
        .file-section h3 code {{
            background: #1e293b;
            color: #f1f5f9;
            padding: 0.25rem 0.5rem;
            border-radius: 4px;
            font-weight: normal;
        }}
        .issues-list {{
            padding: 1rem 1.5rem;
        }}
        .issue-item {{
            padding: 0.75rem;
            margin-bottom: 0.5rem;
            border-radius: 6px;
            display: flex;
            align-items: center;
            gap: 1rem;
        }}
        .severity-critical {{
            background: #fef2f2;
            border-left: 4px solid #dc2626;
        }}
        .severity-very-high {{
            background: #fff7ed;
            border-left: 4px solid #ea580c;
        }}
        .severity-high {{
            background: #fffbeb;
            border-left: 4px solid #d97706;
        }}
        .severity-medium {{
            background: #f8fafc;
            border-left: 4px solid #64748b;
        }}
        .severity-indicator {{
            font-weight: 600;
            min-width: 100px;
        }}
        .issue-description {{
            flex: 1;
        }}
        .recommendations {{
            padding: 1rem 1.5rem;
            border-top: 1px solid #e2e8f0;
            background: #f8fafc;
        }}
        .recommendations h4 {{
            margin: 0 0 1rem;
            color: #1e293b;
        }}
        .effort-low {{ color: #16a34a; }}
        .effort-medium {{ color: #d97706; }}
        .effort-high {{ color: #dc2626; }}
        .refactoring-list {{
            display: grid;
            gap: 1.5rem;
        }}
        .refactoring-file {{
            background: white;
            border: 1px solid #e2e8f0;
            border-radius: 8px;
            overflow: hidden;
        }}
        .refactoring-file h4 {{
            background: #f1f5f9;
            padding: 1rem 1.5rem;
            margin: 0;
            border-bottom: 1px solid #e2e8f0;
        }}
        .refactoring-items {{
            padding: 1rem 1.5rem;
        }}
        .refactoring-item {{
            padding: 1rem;
            background: #f8fafc;
            border-radius: 6px;
            margin-bottom: 1rem;
        }}
        .refactoring-header {{
            font-weight: 600;
            margin-bottom: 0.5rem;
            color: #1e293b;
        }}
        .refactoring-description {{
            color: #475569;
            margin-bottom: 0.5rem;
        }}
        .refactoring-metrics {{
            font-size: 0.875rem;
            color: #64748b;
        }}
        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 1rem;
            margin-bottom: 2rem;
        }}
        .stat-item {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 1rem;
            background: #f8fafc;
            border-radius: 6px;
            border-left: 4px solid #3b82f6;
        }}
        .stat-label {{
            font-weight: 500;
            color: #475569;
        }}
        .stat-value {{
            font-size: 1.5rem;
            font-weight: 700;
            color: #1e293b;
        }}
        .recommendations-list {{
            background: #f0f9ff;
            border: 1px solid #0ea5e9;
            border-radius: 8px;
            padding: 1.5rem 2rem;
            margin: 0;
        }}
        .recommendations-list li {{
            margin-bottom: 1rem;
            color: #1e293b;
        }}
        .success-message {{
            background: #f0fdf4;
            border: 2px solid #22c55e;
            color: #15803d;
            padding: 2rem;
            border-radius: 8px;
            text-align: center;
            font-size: 1.125rem;
        }}
        h2 {{
            color: #1e293b;
            border-bottom: 2px solid #e2e8f0;
            padding-bottom: 0.5rem;
            margin: 2rem 0 1rem;
        }}
        @media (max-width: 768px) {{
            body {{
                padding: 10px;
            }}
            .header h1 {{
                font-size: 2rem;
            }}
            .content {{
                padding: 1rem;
            }}
            .summary {{
                grid-template-columns: 1fr;
            }}
            .metrics-grid {{
                grid-template-columns: 1fr;
            }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🔍 Valknut Analysis Report</h1>
        </div>
        <div class="content">
            <div class="summary">
                <div class="summary-item">
                    <span class="summary-label">Files Analyzed</span>
                    <span class="summary-value">{}</span>
                </div>
                <div class="summary-item">
                    <span class="summary-label">Issues Found</span>
                    <span class="summary-value">{}</span>
                </div>
                <div class="summary-item">
                    <span class="summary-label">Analysis Date</span>
                    <span class="summary-value" style="font-size: 1rem; font-weight: 500;">{}</span>
                </div>
            </div>
            {}
            <footer style="text-align: center; margin-top: 3rem; padding: 2rem; border-top: 1px solid #e2e8f0; color: #64748b;">
                <em>Report generated by <a href="https://github.com/nathanricedev/valknut" style="color: #3b82f6;">Valknut</a> - AI-Powered Code Analysis</em>
            </footer>
        </div>
    </div>
</body>
</html>
"#,
        total_files,
        total_issues,
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        details_html
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
    println!(
        "{}",
        "🏗️  Valknut Structure Analysis Results"
            .bright_blue()
            .bold()
    );
    println!("{}", "=====================================".dimmed());
    println!();

    if let Some(packs) = results.get("packs").and_then(|p| p.as_array()) {
        if packs.is_empty() {
            println!("{}", "✅ No structural issues found!".bright_green());
            return;
        }

        println!(
            "{}",
            format!("📊 Found {} potential improvements:", packs.len()).bold()
        );
        println!();

        for (i, pack) in packs.iter().enumerate() {
            let kind = pack
                .get("kind")
                .and_then(|k| k.as_str())
                .unwrap_or("unknown");
            let empty_vec = vec![];
            let reasons = pack
                .get("reasons")
                .and_then(|r| r.as_array())
                .unwrap_or(&empty_vec);

            println!(
                "{}",
                format!(
                    "{}. {} Analysis",
                    i + 1,
                    match kind {
                        "branch" => "🌿 Directory Branch",
                        "file_split" => "📄 File Split",
                        _ => "🔍 General",
                    }
                )
                .bold()
            );

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
    println!(
        "{}",
        "📊 Comprehensive Analysis Results".bright_blue().bold()
    );
    println!("{}", "=================================".dimmed());
    println!();

    let total_issues = results["summary"]["total_issues"].as_u64().unwrap_or(0);
    let total_files = results["summary"]["total_files"].as_u64().unwrap_or(0);

    println!("{}", "🎯 Analysis Summary:".bold());
    println!(
        "   • {} total issues found",
        total_issues.to_string().bright_yellow()
    );
    println!(
        "   • {} files analyzed",
        total_files.to_string().bright_green()
    );
    println!();

    if total_issues == 0 {
        println!(
            "{}",
            "🎉 Great job! No significant issues found across all analyzers.".bright_green()
        );
        println!("   Your code appears to be well-structured and maintainable.");
    } else {
        println!(
            "{}",
            "📈 Recommendation: Address high-priority issues first for maximum impact."
                .bright_blue()
        );
        println!(
            "   Use detailed analyzers (structure, names, impact) for specific recommendations."
        );
    }

    // Display refactoring suggestions prominently
    display_refactoring_suggestions(results);

    // Display complexity recommendations
    display_complexity_recommendations(results);
}

/// Display refactoring suggestions prominently
pub fn display_refactoring_suggestions(results: &serde_json::Value) {
    // Check if refactoring analysis was enabled and has results
    if let Some(refactoring) = results.get("refactoring") {
        if let Some(enabled) = refactoring.get("enabled").and_then(|v| v.as_bool()) {
            if !enabled {
                return; // Skip if refactoring analysis was disabled
            }
        }

        if let Some(detailed_results) = refactoring
            .get("detailed_results")
            .and_then(|v| v.as_array())
        {
            if detailed_results.is_empty() {
                return; // No refactoring opportunities found
            }

            println!();
            println!("{}", "🔧 Refactoring Opportunities".bright_magenta().bold());
            println!("{}", "=============================".dimmed());
            println!();

            let opportunities_count = refactoring
                .get("opportunities_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            if opportunities_count > 0 {
                println!(
                    "{} {}",
                    "🎯 Total opportunities found:".bold(),
                    opportunities_count.to_string().bright_yellow()
                );
                println!();
            }

            // Group recommendations by file and display top opportunities
            let mut _file_count = 0;
            for file_result in detailed_results.iter().take(10) {
                // Show top 10 files
                if let Some(file_path) = file_result.get("file_path").and_then(|v| v.as_str()) {
                    if let Some(recommendations) = file_result
                        .get("recommendations")
                        .and_then(|v| v.as_array())
                    {
                        if recommendations.is_empty() {
                            continue;
                        }

                        _file_count += 1;
                        println!("{}", format!("📄 {}", file_path).bright_cyan().bold());

                        // Sort recommendations by priority score (highest first)
                        let mut sorted_recommendations: Vec<_> = recommendations.iter().collect();
                        sorted_recommendations.sort_by(|a, b| {
                            let priority_a = a
                                .get("priority_score")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            let priority_b = b
                                .get("priority_score")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            priority_b
                                .partial_cmp(&priority_a)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });

                        for (i, recommendation) in sorted_recommendations.iter().take(3).enumerate()
                        {
                            // Top 3 per file
                            if let (
                                Some(description),
                                Some(refactoring_type),
                                Some(impact),
                                Some(effort),
                            ) = (
                                recommendation.get("description").and_then(|v| v.as_str()),
                                recommendation
                                    .get("refactoring_type")
                                    .and_then(|v| v.as_str()),
                                recommendation
                                    .get("estimated_impact")
                                    .and_then(|v| v.as_f64()),
                                recommendation
                                    .get("estimated_effort")
                                    .and_then(|v| v.as_f64()),
                            ) {
                                let priority_score = recommendation
                                    .get("priority_score")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);

                                // Format refactoring type with emoji
                                let type_emoji = match refactoring_type {
                                    "ExtractMethod" => "⚡",
                                    "ExtractClass" => "📦",
                                    "ReduceComplexity" => "🎯",
                                    "EliminateDuplication" => "🔄",
                                    "ImproveNaming" => "📝",
                                    "SimplifyConditionals" => "🔀",
                                    "RemoveDeadCode" => "🧹",
                                    _ => "🔧",
                                };

                                // Get location if available
                                let location_str = if let Some(location) =
                                    recommendation.get("location").and_then(|v| v.as_array())
                                {
                                    if location.len() >= 2 {
                                        if let (Some(start), Some(end)) =
                                            (location[0].as_u64(), location[1].as_u64())
                                        {
                                            if start == end {
                                                format!(" (line {})", start)
                                            } else {
                                                format!(" (lines {}-{})", start, end)
                                            }
                                        } else {
                                            String::new()
                                        }
                                    } else {
                                        String::new()
                                    }
                                } else {
                                    String::new()
                                };

                                println!(
                                    "   {}. {} {} {}",
                                    i + 1,
                                    type_emoji,
                                    format!(
                                        "{}: {}",
                                        refactoring_type
                                            .replace("Extract", "Extract ")
                                            .replace("Reduce", "Reduce ")
                                            .replace("Eliminate", "Eliminate ")
                                            .replace("Improve", "Improve ")
                                            .replace("Simplify", "Simplify ")
                                            .replace("Remove", "Remove "),
                                        description
                                    )
                                    .yellow(),
                                    location_str.dimmed()
                                );

                                println!("      {} Impact: {:.1}/10 | Effort: {:.1}/10 | Priority: {:.2}", 
                                    "📊".dimmed(),
                                    impact,
                                    effort,
                                    priority_score
                                );
                            }
                        }
                        println!();
                    }
                }
            }

            if _file_count == 0 {
                println!(
                    "{}",
                    "✅ No refactoring opportunities found - code quality looks good!"
                        .bright_green()
                );
            } else if detailed_results.len() > 10 {
                println!("{}", format!("📋 Showing top 10 files with opportunities ({} more files have suggestions)", detailed_results.len() - 10).dimmed());
            }
        }
    }
}

/// Display complexity-based recommendations
pub fn display_complexity_recommendations(results: &serde_json::Value) {
    if let Some(complexity) = results.get("complexity") {
        if let Some(enabled) = complexity.get("enabled").and_then(|v| v.as_bool()) {
            if !enabled {
                return; // Skip if complexity analysis was disabled
            }
        }

        if let Some(detailed_results) = complexity
            .get("detailed_results")
            .and_then(|v| v.as_array())
        {
            // Collect files with recommendations
            let files_with_recommendations: Vec<_> = detailed_results
                .iter()
                .filter(|file_result| {
                    file_result
                        .get("recommendations")
                        .and_then(|rec| rec.as_array())
                        .map(|arr| !arr.is_empty())
                        .unwrap_or(false)
                })
                .collect();

            if files_with_recommendations.is_empty() {
                return; // No complexity recommendations found
            }

            println!();
            println!("{}", "🏗️  Complexity Recommendations".bright_red().bold());
            println!("{}", "===============================".dimmed());
            println!();

            let mut _file_count = 0;
            for file_result in files_with_recommendations.iter().take(8) {
                // Show top 8 files
                if let Some(file_path) = file_result.get("file_path").and_then(|v| v.as_str()) {
                    if let Some(recommendations) = file_result
                        .get("recommendations")
                        .and_then(|v| v.as_array())
                    {
                        if recommendations.is_empty() {
                            continue;
                        }

                        _file_count += 1;
                        println!("{}", format!("📄 {}", file_path).bright_cyan().bold());

                        for (i, recommendation) in recommendations.iter().take(2).enumerate() {
                            // Top 2 per file
                            if let Some(description) =
                                recommendation.get("description").and_then(|v| v.as_str())
                            {
                                let effort = recommendation
                                    .get("effort")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(1);
                                let effort_emoji = match effort {
                                    1..=3 => "🟢 Low",
                                    4..=6 => "🟡 Medium",
                                    7..=10 => "🔴 High",
                                    _ => "⚪ Unknown",
                                };

                                println!("   {}. {} {}", i + 1, "🎯".yellow(), description.white());
                                println!("      {} Effort: {}", "📊".dimmed(), effort_emoji);
                            }
                        }
                        println!();
                    }
                }
            }

            if files_with_recommendations.len() > 8 {
                println!("{}", format!("📋 Showing top 8 files with recommendations ({} more files have suggestions)", files_with_recommendations.len() - 8).dimmed());
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::{NamedTempFile, TempDir};
    use tokio;

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
    fn test_display_analysis_results() {
        let result = json!({
            "summary": {
                "total_files": 10,
                "total_lines": 1000,
                "health_score": 75.5,
                "complexity_score": 82.3,
                "technical_debt_ratio": 15.2,
                "maintainability_score": 68.1,
                "total_issues": 25,
                "critical_issues": 3,
                "high_priority_issues": 8
            },
            "timestamp": "2024-01-15T10:30:00Z"
        });

        // Test that display_analysis_results doesn't panic
        display_analysis_results(&result);
    }

    #[test]
    fn test_display_analysis_results_minimal() {
        let result = json!({});

        // Test that display_analysis_results handles missing fields gracefully
        display_analysis_results(&result);
    }

    #[test]
    fn test_display_completion_summary() {
        let result = json!({
            "summary": {
                "total_files": 100,
                "issues_count": 5
            }
        });
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path();

        // Test that display_completion_summary doesn't panic
        display_completion_summary(&result, out_path, &OutputFormat::Json);
    }

    #[tokio::test]
    async fn test_generate_markdown_report() {
        let result = json!({
            "summary": {
                "total_files": 10,
                "total_lines": 1000,
                "health_score": 75.5
            },
            "issues": [],
            "refactoring_opportunities": []
        });

        let markdown = generate_markdown_report(&result).await.unwrap();
        assert!(markdown.contains("# Valknut Analysis Report"));
        assert!(markdown.contains("Files Analyzed**: 10"));
        assert!(markdown.contains("Issues Found**: 0"));
    }

    #[tokio::test]
    async fn test_generate_html_report() {
        let result = json!({
            "summary": {
                "total_files": 5,
                "total_lines": 500,
                "health_score": 85.0
            },
            "issues": []
        });

        let html = generate_html_report(&result).await.unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<title>Valknut Analysis Report</title>"));
        assert!(html.contains("5"));
        assert!(html.contains("body"));
    }

    #[tokio::test]
    async fn test_generate_sonar_report() {
        let result = json!({
            "issues": [
                {
                    "file": "test.rs",
                    "line": 10,
                    "column": 5,
                    "severity": "major",
                    "rule": "complexity",
                    "message": "High complexity function"
                }
            ]
        });

        let sonar = generate_sonar_report(&result).await.unwrap();
        assert!(sonar.contains("\"issues\": []"));
        assert!(sonar.contains("\"version\": \"1.0\""));
        assert!(sonar.contains("\"summary\""));
    }

    #[tokio::test]
    async fn test_generate_csv_report() {
        let result = json!({
            "issues": [
                {
                    "file": "main.rs",
                    "line": 20,
                    "severity": "high",
                    "category": "complexity",
                    "description": "Function too complex"
                },
                {
                    "file": "utils.rs",
                    "line": 35,
                    "severity": "medium",
                    "category": "maintainability",
                    "description": "Poor naming"
                }
            ]
        });

        let csv = generate_csv_report(&result).await.unwrap();
        assert!(csv.contains("File,Issue Type,Severity,Description"));
    }

    #[tokio::test]
    async fn test_generate_csv_report_empty() {
        let result = json!({
            "issues": []
        });

        let csv = generate_csv_report(&result).await.unwrap();
        assert!(csv.contains("File,Issue Type,Severity,Description"));
        assert_eq!(csv.lines().count(), 2); // Header + "No issues found" line
    }

    #[tokio::test]
    async fn test_generate_ci_summary_report() {
        let result = json!({
            "summary": {
                "total_files": 15,
                "total_issues": 0,
                "critical_issues": 0,
                "high_priority_issues": 0
            },
            "health_metrics": {
                "overall_health_score": 72.5
            }
        });

        let summary = generate_ci_summary_report(&result).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&summary).unwrap();

        assert_eq!(parsed["status"], "success");
        assert_eq!(parsed["summary"]["total_files"], 15);
        assert_eq!(parsed["summary"]["total_issues"], 0);
        assert_eq!(parsed["summary"]["critical_issues"], 0);
        assert_eq!(parsed["metrics"]["overall_health_score"], 72.5);
    }

    #[tokio::test]
    async fn test_generate_ci_summary_report_fail() {
        let result = json!({
            "summary": {
                "total_files": 10,
                "total_issues": 25,
                "critical_issues": 8,
                "high_priority_issues": 12,
                "health_score": 45.0
            }
        });

        let summary = generate_ci_summary_report(&result).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&summary).unwrap();

        assert_eq!(parsed["status"], "issues_found");
        assert_eq!(parsed["summary"]["total_issues"], 25);
        assert_eq!(parsed["summary"]["critical_issues"], 8);
    }

    #[test]
    fn test_print_human_readable_results() {
        let results = json!({
            "summary": {
                "total_files": 20,
                "total_lines": 2000,
                "health_score": 88.5
            },
            "issues": [
                {
                    "severity": "high",
                    "description": "Test issue"
                }
            ]
        });

        // Test that print_human_readable_results doesn't panic
        print_human_readable_results(&results);
    }

    #[test]
    fn test_print_comprehensive_results_pretty() {
        let results = json!({
            "summary": {
                "total_files": 15,
                "health_score": 75.0,
                "complexity_score": 65.2,
                "technical_debt_ratio": 20.1
            },
            "issues": []
        });

        // Test that print_comprehensive_results_pretty doesn't panic
        print_comprehensive_results_pretty(&results);
    }

    #[test]
    fn test_display_refactoring_suggestions() {
        let results = json!({
            "refactoring_opportunities": [
                {
                    "type": "extract_method",
                    "file": "main.rs",
                    "line": 50,
                    "description": "Extract complex method",
                    "impact": "high"
                },
                {
                    "type": "reduce_complexity",
                    "file": "utils.rs",
                    "line": 25,
                    "description": "Simplify conditional logic",
                    "impact": "medium"
                }
            ]
        });

        // Test that display_refactoring_suggestions doesn't panic
        display_refactoring_suggestions(&results);
    }

    #[test]
    fn test_display_refactoring_suggestions_empty() {
        let results = json!({
            "refactoring_opportunities": []
        });

        // Test that display_refactoring_suggestions handles empty list
        display_refactoring_suggestions(&results);
    }

    #[test]
    fn test_display_complexity_recommendations() {
        let results = json!({
            "complexity_issues": [
                {
                    "file": "complex.rs",
                    "function": "process_data",
                    "complexity": 15,
                    "recommendation": "Split into smaller functions"
                }
            ]
        });

        // Test that display_complexity_recommendations doesn't panic
        display_complexity_recommendations(&results);
    }

    #[test]
    fn test_display_complexity_recommendations_empty() {
        let results = json!({
            "complexity_issues": []
        });

        // Test that display_complexity_recommendations handles empty data
        display_complexity_recommendations(&results);
    }

    #[tokio::test]
    async fn test_generate_outputs_json() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 5
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Json).await;
        assert!(result.is_ok());

        let json_file = out_path.join("analysis_results.json");
        assert!(json_file.exists());

        let content = fs::read_to_string(&json_file).unwrap();
        assert!(content.contains("total_files"));
    }

    #[tokio::test]
    async fn test_generate_outputs_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "health_score": 85.5
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Yaml).await;
        assert!(result.is_ok());

        let yaml_file = out_path.join("analysis_results.yaml");
        assert!(yaml_file.exists());

        let content = fs::read_to_string(&yaml_file).unwrap();
        assert!(content.contains("health_score"));
    }

    #[tokio::test]
    async fn test_generate_outputs_markdown() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 10,
                "health_score": 70.0
            },
            "issues": []
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Markdown).await;
        assert!(result.is_ok());

        let md_file = out_path.join("team_report.md");
        assert!(md_file.exists());

        let content = fs::read_to_string(&md_file).unwrap();
        assert!(content.contains("# Valknut Analysis Report"));
        assert!(content.contains("Files Analyzed**: 10"));
    }

    #[tokio::test]
    async fn test_generate_outputs_html() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 8,
                "health_score": 92.1
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Html).await;
        assert!(result.is_ok());

        let html_file = out_path.join("team_report.html");
        assert!(html_file.exists());

        let content = fs::read_to_string(&html_file).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("html"));
    }

    #[tokio::test]
    async fn test_generate_outputs_csv() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "issues": [
                {
                    "file": "test.rs",
                    "line": 15,
                    "severity": "high",
                    "category": "complexity",
                    "description": "Too complex"
                }
            ]
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Csv).await;
        assert!(result.is_ok());

        let csv_file = out_path.join("analysis_data.csv");
        assert!(csv_file.exists());

        let content = fs::read_to_string(&csv_file).unwrap();
        assert!(content.contains("File,Issue Type,Severity,Description"));
    }

    #[tokio::test]
    async fn test_generate_outputs_sonar() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "issues": [
                {
                    "file": "main.rs",
                    "line": 20,
                    "severity": "major",
                    "rule": "complexity",
                    "message": "High complexity"
                }
            ]
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Sonar).await;
        assert!(result.is_ok());

        let sonar_file = out_path.join("sonarqube_issues.json");
        assert!(sonar_file.exists());

        let content = fs::read_to_string(&sonar_file).unwrap();
        assert!(content.contains("\"issues\": []"));
        assert!(content.contains("\"version\": \"1.0\""));
    }

    #[tokio::test]
    async fn test_generate_outputs_ci_summary() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 12,
                "total_issues": 3,
                "critical_issues": 0,
                "health_score": 88.5
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::CiSummary).await;
        assert!(result.is_ok());

        let ci_file = out_path.join("ci_summary.json");
        assert!(ci_file.exists());

        let content = fs::read_to_string(&ci_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["status"], "issues_found");
        assert_eq!(parsed["summary"]["total_files"], 12);
    }

    #[tokio::test]
    async fn test_generate_outputs_with_feedback_quiet() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 3
            }
        });

        let result =
            generate_outputs_with_feedback(&result, &out_path, &OutputFormat::Json, true).await;
        assert!(result.is_ok());

        let json_file = out_path.join("analysis_results.json");
        assert!(json_file.exists());
    }

    #[tokio::test]
    async fn test_generate_outputs_with_feedback_not_quiet() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 7
            }
        });

        let result =
            generate_outputs_with_feedback(&result, &out_path, &OutputFormat::Yaml, false).await;
        assert!(result.is_ok());

        let yaml_file = out_path.join("analysis_results.yaml");
        assert!(yaml_file.exists());
    }

    #[tokio::test]
    async fn test_generate_outputs_pretty() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 25,
                "health_score": 78.3
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Pretty).await;
        assert!(result.is_ok());

        // Pretty format should not create files, just display
        assert!(!out_path.join("analysis.txt").exists());
    }

    #[tokio::test]
    async fn test_generate_outputs_jsonl() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 6
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Jsonl).await;
        assert!(result.is_ok());

        let jsonl_file = out_path.join("report.jsonl");
        assert!(jsonl_file.exists());

        let content = fs::read_to_string(&jsonl_file).unwrap();
        assert!(content.contains("total_files"));
    }

    // Test edge cases and error conditions
    #[tokio::test]
    async fn test_generate_outputs_missing_fields() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({});

        // Should handle missing fields gracefully
        let result = generate_outputs(&result, &out_path, &OutputFormat::Json).await;
        assert!(result.is_ok());
    }
}
