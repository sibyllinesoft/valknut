//! HTML report generation functions.
//!
//! This module contains functions for generating HTML analysis reports
//! with embedded CSS styling.

use super::helpers::{format_refactoring_type, refactoring_type_emoji};
use super::report_helpers::{metric_class, render_issues_html, render_metric_card, render_recommendations_html};

/// Build the main report details HTML.
fn build_report_details(result: &serde_json::Value) -> String {
    let mut html = String::new();

    // Health metrics section
    if let Some(health_metrics) = result.get("health_metrics") {
        html.push_str(&build_health_metrics_section(health_metrics));
    }

    // High priority files section
    if let Some(complexity) = result.get("complexity") {
        html.push_str(&build_high_priority_files_section(complexity));
    }

    // Refactoring opportunities section
    if let Some(refactoring) = result.get("refactoring") {
        html.push_str(&build_refactoring_section(refactoring));
    }

    // Summary statistics section
    if let Some(complexity) = result.get("complexity") {
        html.push_str(&build_summary_stats_section(complexity));
    }

    // Static recommendations
    html.push_str("<h2>💡 Recommendations</h2>");
    html.push_str("<ol class='recommendations-list'>");
    html.push_str("<li><strong>Start with Critical Issues</strong>: Focus on files with critical and high-severity issues first</li>");
    html.push_str("<li><strong>Reduce Complexity</strong>: Break down large functions and simplify complex conditionals</li>");
    html.push_str("<li><strong>Improve Maintainability</strong>: Address technical debt systematically</li>");
    html.push_str("<li><strong>Regular Monitoring</strong>: Run analysis regularly to track improvements</li>");
    html.push_str("</ol>");

    html
}

/// Build the health metrics section.
fn build_health_metrics_section(health_metrics: &serde_json::Value) -> String {
    let mut html = String::from("<h2>📊 Health Metrics</h2><div class='metrics-grid'>");

    let metrics: &[(&str, &str, f64, f64, bool, Option<&str>)] = &[
        ("overall_health_score", "Overall Health", 80.0, 60.0, true, None),
        ("complexity_score", "Complexity Score", 25.0, 50.0, false, Some("lower is better")),
        ("technical_debt_ratio", "Technical Debt", 20.0, 40.0, false, Some("lower is better")),
        ("maintainability_score", "Maintainability", 60.0, 40.0, true, None),
    ];

    for (key, label, good, warn, higher_is_better, hint) in metrics {
        if let Some(v) = health_metrics.get(*key).and_then(|v| v.as_f64()) {
            let class = metric_class(v, *good, *warn, *higher_is_better);
            let suffix = if *key == "technical_debt_ratio" { "%" } else { "/100" };
            html.push_str(&render_metric_card(label, v, class, suffix, *hint));
        }
    }

    html.push_str("</div>");
    html
}

/// Build the high priority files section.
fn build_high_priority_files_section(complexity: &serde_json::Value) -> String {
    let Some(detailed_results) = complexity.get("detailed_results").and_then(|v| v.as_array()) else {
        return String::new();
    };

    let high_priority_files: Vec<_> = detailed_results
        .iter()
        .filter(|f| {
            f.get("issues")
                .and_then(|i| i.as_array())
                .map(|i| !i.is_empty())
                .unwrap_or(false)
        })
        .collect();

    if high_priority_files.is_empty() {
        return String::new();
    }

    let mut html = String::from("<h2>🔥 High Priority Files</h2>");
    html.push_str("<p>Files with complexity issues that should be addressed first:</p>");

    for (i, file_result) in high_priority_files.iter().take(10).enumerate() {
        let Some(file_path) = file_result.get("file_path").and_then(|v| v.as_str()) else {
            continue;
        };
        html.push_str(&format!(
            "<div class='file-section'><h3>{}.&nbsp;<code>{}</code></h3>",
            i + 1,
            file_path
        ));

        if let Some(issues) = file_result.get("issues").and_then(|v| v.as_array()) {
            html.push_str(&render_issues_html(issues, 5));
        }
        if let Some(recs) = file_result.get("recommendations").and_then(|v| v.as_array()) {
            html.push_str(&render_recommendations_html(recs, 3));
        }
        html.push_str("</div>");
    }

    html
}

/// Build the refactoring opportunities section.
fn build_refactoring_section(refactoring: &serde_json::Value) -> String {
    let opportunities_count = refactoring
        .get("opportunities_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if opportunities_count == 0 {
        return String::new();
    }

    let mut html = String::from("<h2>🔧 Refactoring Opportunities</h2>");
    html.push_str(&format!(
        "<p>Found <strong>{}</strong> refactoring opportunities across the codebase.</p>",
        opportunities_count
    ));

    let Some(detailed_results) = refactoring.get("detailed_results").and_then(|v| v.as_array()) else {
        return html;
    };

    html.push_str("<div class='refactoring-list'>");
    for file_result in detailed_results.iter().take(8) {
        html.push_str(&render_refactoring_file(file_result));
    }
    html.push_str("</div>");

    html
}

/// Render a single refactoring file entry.
fn render_refactoring_file(file_result: &serde_json::Value) -> String {
    let Some(file_path) = file_result.get("file_path").and_then(|v| v.as_str()) else {
        return String::new();
    };
    let Some(recommendations) = file_result.get("recommendations").and_then(|v| v.as_array()) else {
        return String::new();
    };
    if recommendations.is_empty() {
        return String::new();
    }

    let mut html = format!("<div class='refactoring-file'><h4>📄 {}</h4>", file_path);
    html.push_str("<div class='refactoring-items'>");

    for rec in recommendations.iter().take(3) {
        html.push_str(&render_refactoring_item(rec));
    }

    html.push_str("</div></div>");
    html
}

/// Render a single refactoring item.
fn render_refactoring_item(rec: &serde_json::Value) -> String {
    let description = rec.get("description").and_then(|v| v.as_str());
    let refactoring_type = rec.get("refactoring_type").and_then(|v| v.as_str());
    let impact = rec.get("estimated_impact").and_then(|v| v.as_f64());
    let effort = rec.get("estimated_effort").and_then(|v| v.as_f64());

    let (Some(description), Some(refactoring_type), Some(impact), Some(effort)) =
        (description, refactoring_type, impact, effort)
    else {
        return String::new();
    };

    let type_emoji = refactoring_type_emoji(refactoring_type);
    let display_type = format_refactoring_type(refactoring_type);
    let priority_score = rec.get("priority_score").and_then(|v| v.as_f64()).unwrap_or(0.0);

    format!(
        "<div class='refactoring-item'>\
         <div class='refactoring-header'>{} <strong>{}</strong></div>\
         <div class='refactoring-description'>{}</div>\
         <div class='refactoring-metrics'>Impact: {:.1}/10 | Effort: {:.1}/10 | Priority: {:.2}</div>\
         </div>",
        type_emoji, display_type, description, impact, effort, priority_score
    )
}

/// Build the summary statistics section.
fn build_summary_stats_section(complexity: &serde_json::Value) -> String {
    let mut html = String::from("<h2>📈 Summary Statistics</h2><div class='stats-grid'>");

    let stats: &[(&str, &str)] = &[
        ("average_cyclomatic_complexity", "Average Cyclomatic Complexity"),
        ("average_cognitive_complexity", "Average Cognitive Complexity"),
        ("average_technical_debt_score", "Average Technical Debt Score"),
    ];

    for (key, label) in stats {
        if let Some(v) = complexity.get(*key).and_then(|v| v.as_f64()) {
            html.push_str(&format!(
                "<div class='stat-item'><span class='stat-label'>{}</span><span class='stat-value'>{:.1}</span></div>",
                label, v
            ));
        }
    }

    html.push_str("</div>");
    html
}

/// Render the analysis result as an interactive HTML document.
#[allow(dead_code)]
pub async fn generate_html_report(result: &serde_json::Value) -> anyhow::Result<String> {
    let total_issues = result["summary"]["total_issues"].as_u64().unwrap_or(0);
    let total_files = result["summary"]["total_files"].as_u64().unwrap_or(0);

    let details_html = if total_issues == 0 {
        "<div class='success-message'>✅ <strong>Excellent!</strong> No significant issues found in your codebase.</div>".to_string()
    } else {
        build_report_details(result)
    };

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
        <div class="hero-container">
            <canvas id="neural-network" class="neural-background"></canvas>
            <div class="hero-content">
                <h1 class="hero-title">🔍 Valknut Analysis Report</h1>
                <p class="hero-subtitle">Comprehensive code quality analysis and refactoring guidance</p>
            </div>
        </div>
        <hr class="hero-divider">
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
        <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
        <script src="./webpage_files/trefoil-animation.js"></script>
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
