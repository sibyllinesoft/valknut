//! Terminal display functions for analysis results
//!
//! This module contains functions for displaying analysis results
//! in the terminal with colored output and formatting.

use std::path::Path;

use owo_colors::OwoColorize;
use tabled::{settings::Style as TableStyle, Table, Tabled};

use crate::cli::args::OutputFormat;
use super::helpers::{
    format_location, format_refactoring_type, format_to_string, refactoring_type_emoji,
};

/// Display complexity recommendations for a single file.
pub fn display_file_complexity_recommendations(file_result: &serde_json::Value) {
    let Some(file_path) = file_result.get("file_path").and_then(|v| v.as_str()) else {
        return;
    };
    let Some(recommendations) = file_result.get("recommendations").and_then(|v| v.as_array())
    else {
        return;
    };
    if recommendations.is_empty() {
        return;
    }

    println!("{}", format!("📄 {}", file_path).bright_cyan().bold());

    for (i, recommendation) in recommendations.iter().take(2).enumerate() {
        let Some(description) = recommendation.get("description").and_then(|v| v.as_str()) else {
            continue;
        };
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
    println!();
}

/// Display top structure issues from comprehensive analysis results.
pub fn display_top_structure_issues(result: &serde_json::Value) {
    let Some(structure) = result["comprehensive_analysis"]["structure"].as_object() else {
        return;
    };
    let Some(packs) = structure["packs"].as_array() else {
        return;
    };
    if packs.is_empty() {
        return;
    }

    println!();
    println!(
        "{}",
        "🔥 Top Issues Requiring Attention:".bright_red().bold()
    );
    for (i, pack) in packs.iter().take(3).enumerate() {
        let Some(kind) = pack["kind"].as_str() else {
            continue;
        };
        let issue_type = match kind {
            "branch" => "🌿 Directory reorganization",
            "file_split" => "📄 File splitting",
            _ => "🔍 Structure optimization",
        };
        println!("  {}. {}", i + 1, issue_type);
    }
}

/// Display analysis results with visual indicators
#[allow(dead_code)]
pub fn display_analysis_results(result: &serde_json::Value) {
    println!("{}", "✅ Analysis Complete".bright_green().bold());
    println!();

    #[derive(Tabled)]
    /// Row used when printing the basic stats table.
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
#[allow(dead_code)]
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
        display_top_structure_issues(result);
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

/// Pretty-print analysis results for interactive terminal use.
#[allow(dead_code)]
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

/// Pretty multi-section renderer for comprehensive analysis results.
#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn display_refactoring_suggestions(results: &serde_json::Value) {
    // Check if refactoring analysis was enabled and has results
    if let Some(refactoring) = results.get("refactoring") {
        if let Some(enabled) = refactoring.get("enabled").and_then(|v| v.as_bool()) {
            if !enabled {
                return;
            }
        }

        if let Some(detailed_results) = refactoring
            .get("detailed_results")
            .and_then(|v| v.as_array())
        {
            if detailed_results.is_empty() {
                return;
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

            let mut _file_count = 0;
            for file_result in detailed_results.iter().take(10) {
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

                                let type_emoji = refactoring_type_emoji(refactoring_type);
                                let display_type = format_refactoring_type(refactoring_type);
                                let location_str = format_location(recommendation);

                                println!(
                                    "   {}. {} {} {}",
                                    i + 1,
                                    type_emoji,
                                    format!("{}: {}", display_type, description).yellow(),
                                    location_str.dimmed()
                                );

                                println!(
                                    "      {} Impact: {:.1}/10 | Effort: {:.1}/10 | Priority: {:.2}",
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
                println!(
                    "{}",
                    format!(
                        "📋 Showing top 10 files with opportunities ({} more files have suggestions)",
                        detailed_results.len() - 10
                    )
                    .dimmed()
                );
            }
        }
    }
}

/// Display complexity-based recommendations
#[allow(dead_code)]
pub fn display_complexity_recommendations(results: &serde_json::Value) {
    if let Some(complexity) = results.get("complexity") {
        if let Some(enabled) = complexity.get("enabled").and_then(|v| v.as_bool()) {
            if !enabled {
                return;
            }
        }

        if let Some(detailed_results) = complexity
            .get("detailed_results")
            .and_then(|v| v.as_array())
        {
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
                return;
            }

            println!();
            println!("{}", "🏗️  Complexity Recommendations".bright_red().bold());
            println!("{}", "===============================".dimmed());
            println!();

            for file_result in files_with_recommendations.iter().take(8) {
                display_file_complexity_recommendations(file_result);
            }

            if files_with_recommendations.len() > 8 {
                println!(
                    "{}",
                    format!(
                        "📋 Showing top 8 files with recommendations ({} more files have suggestions)",
                        files_with_recommendations.len() - 8
                    )
                    .dimmed()
                );
            }
        }
    }
}
