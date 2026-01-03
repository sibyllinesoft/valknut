//! Shared helper functions for report generation.
//!
//! This module contains helper functions used by both markdown and HTML report generators.

/// Determine CSS class based on metric value and thresholds.
pub fn metric_class(value: f64, good_threshold: f64, warning_threshold: f64, higher_is_better: bool) -> &'static str {
    if higher_is_better {
        match value {
            v if v >= good_threshold => "metric-good",
            v if v >= warning_threshold => "metric-warning",
            _ => "metric-critical",
        }
    } else {
        match value {
            v if v <= good_threshold => "metric-good",
            v if v <= warning_threshold => "metric-warning",
            _ => "metric-critical",
        }
    }
}

/// Render a metric card HTML element.
pub fn render_metric_card(title: &str, value: f64, class: &str, suffix: &str, note: Option<&str>) -> String {
    let note_html = note.map(|n| format!("<span class='metric-note'>{}</span>", n)).unwrap_or_default();
    format!(
        "<div class='metric-card {}'><span class='metric-title'>{}</span><span class='metric-value'>{:.1}{}</span>{}</div>",
        class, title, value, suffix, note_html
    )
}

/// Get severity indicator emoji and CSS class.
pub fn severity_indicator(severity: &str) -> (&'static str, &'static str) {
    match severity {
        "Critical" => ("🔴", "severity-critical"),
        "VeryHigh" => ("🟠", "severity-very-high"),
        "High" => ("🟡", "severity-high"),
        _ => ("⚠️", "severity-medium"),
    }
}

/// Determine effort class based on effort level.
pub fn effort_class(effort: u64) -> &'static str {
    match effort {
        1..=3 => "effort-low",
        4..=6 => "effort-medium",
        7..=10 => "effort-high",
        _ => "effort-unknown",
    }
}

/// Render issues list HTML from a JSON array of issues.
pub fn render_issues_html(issues: &[serde_json::Value], limit: usize) -> String {
    let mut html = String::from("<div class='issues-list'>");
    for issue in issues.iter().take(limit) {
        let Some(description) = issue.get("description").and_then(|v| v.as_str()) else {
            continue;
        };
        let severity = issue.get("severity").and_then(|v| v.as_str()).unwrap_or("Medium");
        let (emoji, class) = severity_indicator(severity);
        html.push_str(&format!(
            "<div class='issue-item {}'><span class='severity-indicator'>{} {}</span><span class='issue-description'>{}</span></div>",
            class, emoji, severity, description
        ));
    }
    html.push_str("</div>");
    html
}

/// Render issues list as markdown from a JSON array.
pub fn render_issues_markdown(issues: &[serde_json::Value], limit: usize) -> String {
    let mut md = String::new();
    for issue in issues.iter().take(limit) {
        let Some(description) = issue.get("description").and_then(|v| v.as_str()) else {
            continue;
        };
        let severity = issue.get("severity").and_then(|v| v.as_str()).unwrap_or("Medium");
        let (emoji, _) = severity_indicator(severity);
        md.push_str(&format!("- {} **{}**: {}\n", emoji, severity, description));
    }
    md
}

/// Render recommendations list as markdown from a JSON array.
pub fn render_recommendations_markdown(recommendations: &[serde_json::Value], limit: usize) -> String {
    if recommendations.is_empty() {
        return String::new();
    }
    let mut md = String::from("\n**Recommended Actions:**\n");
    for (i, rec) in recommendations.iter().take(limit).enumerate() {
        let Some(desc) = rec.get("description").and_then(|v| v.as_str()) else {
            continue;
        };
        let effort = rec.get("effort").and_then(|v| v.as_u64()).unwrap_or(1);
        md.push_str(&format!("{}. {} (Effort: {})\n", i + 1, desc, effort));
    }
    md
}

/// Render recommendations list HTML from a JSON array.
pub fn render_recommendations_html(recommendations: &[serde_json::Value], limit: usize) -> String {
    if recommendations.is_empty() {
        return String::new();
    }
    let mut html = String::from("<div class='recommendations'><h4>💡 Recommended Actions:</h4><ol>");
    for rec in recommendations.iter().take(limit) {
        let Some(desc) = rec.get("description").and_then(|v| v.as_str()) else {
            continue;
        };
        let effort = rec.get("effort").and_then(|v| v.as_u64()).unwrap_or(1);
        let class = effort_class(effort);
        html.push_str(&format!(
            "<li><span class='recommendation-text'>{}</span> <span class='effort-indicator {}'>(Effort: {})</span></li>",
            desc, class, effort
        ));
    }
    html.push_str("</ol></div>");
    html
}
