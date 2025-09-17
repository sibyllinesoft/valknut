//! Report generation for live reachability analysis
//!
//! Creates JSON, HTML, and Markdown reports for shadow island analysis

use crate::core::errors::{Result, ValknutError};
use crate::live::{
    community::{CommunityDetection, CommunityId},
    graph::CallGraph,
    types::{Community, CommunityNode, LiveReachReport, LiveReachScore, ReportStats},
};

use chrono::{DateTime, Utc};
use handlebars::Handlebars;
use serde_json::json;
use std::collections::HashMap;

/// Report generation formats
#[derive(Debug, Clone)]
pub enum ReportFormat {
    Json,
    Html,
    Markdown,
    Csv,
}

/// Live reachability report generator
pub struct LiveReachReporter {
    handlebars: Handlebars<'static>,
}

impl LiveReachReporter {
    /// Create a new report generator
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();

        // Register helper for percentage calculation
        handlebars.register_helper(
            "percent",
            Box::new(
                |h: &handlebars::Helper,
                 _: &handlebars::Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let numerator = h.param(0).and_then(|v| v.value().as_f64()).unwrap_or(0.0);
                    let denominator = h.param(1).and_then(|v| v.value().as_f64()).unwrap_or(1.0);
                    let percent = if denominator > 0.0 {
                        (numerator / denominator * 100.0).round()
                    } else {
                        0.0
                    };
                    out.write(&format!("{:.0}", percent))?;
                    Ok(())
                },
            ),
        );

        // Register HTML template
        if let Err(e) = handlebars.register_template_string("html_report", HTML_TEMPLATE) {
            eprintln!("Warning: Failed to register HTML template: {}", e);
        }

        // Register Markdown template
        if let Err(e) = handlebars.register_template_string("markdown_report", MARKDOWN_TEMPLATE) {
            eprintln!("Warning: Failed to register Markdown template: {}", e);
        }

        Self { handlebars }
    }

    /// Generate complete analysis report
    pub fn generate_report(
        &self,
        graph: &CallGraph,
        detection: &CommunityDetection,
        live_reach_scores: &HashMap<String, LiveReachScore>,
        shadow_scores: &HashMap<CommunityId, f64>,
        service: &str,
        window: (DateTime<Utc>, DateTime<Utc>),
    ) -> Result<LiveReachReport> {
        let graph_stats = graph.get_stats();

        // Build communities with shadow island scores
        let mut communities = Vec::new();

        for (community_id, community_info) in &detection.communities {
            if community_info.size() < 3 {
                continue; // Skip very small communities
            }

            let shadow_score = shadow_scores.get(community_id).copied().unwrap_or(0.0);

            // Build community nodes
            let nodes: Vec<CommunityNode> = community_info
                .nodes
                .iter()
                .filter_map(|&node_idx| graph.get_symbol(node_idx))
                .filter_map(|symbol| {
                    live_reach_scores
                        .get(symbol)
                        .map(|score| {
                            let stats = graph.get_node_stats(symbol)?;
                            Some(CommunityNode {
                                id: symbol.to_string(),
                                live_reach: score.score,
                                last_seen: stats
                                    .last_seen
                                    .map(|dt| dt.format("%Y-%m-%d").to_string()),
                                seed_reachable: stats.seed_reachable,
                            })
                        })
                        .flatten()
                })
                .collect();

            if nodes.is_empty() {
                continue; // Skip empty communities
            }

            // Generate analysis notes
            let notes = self.generate_community_notes(community_info, shadow_score, &nodes);

            // Build top inbound/outbound edges (simplified)
            let top_inbound = Vec::new(); // TODO: Implement cross-community edge analysis
            let top_outbound = Vec::new();

            let community = Community {
                id: format!("c_{}", community_id),
                size: nodes.len(),
                score: shadow_score,
                cut_ratio: community_info.cut_ratio(),
                runtime_internal: community_info.runtime_internal_fraction(),
                nodes,
                top_inbound,
                top_outbound,
                notes,
            };

            communities.push(community);
        }

        // Sort communities by shadow island score (descending)
        communities.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Calculate overall statistics
        let all_scores: Vec<f64> = live_reach_scores.values().map(|s| s.score).collect();
        let median_live_reach = calculate_median(&all_scores);

        let shadow_islands = communities
            .iter()
            .filter(|c| c.score >= 0.6 && c.size >= 5)
            .count();

        let stats = ReportStats {
            total_nodes: graph_stats.total_nodes,
            total_edges: graph_stats.total_edges,
            runtime_edges: graph_stats.runtime_edges,
            static_edges: graph_stats.static_edges,
            communities: communities.len(),
            shadow_islands,
            median_live_reach,
        };

        Ok(LiveReachReport {
            generated_at: Utc::now(),
            svc: service.to_string(),
            window,
            communities,
            stats,
        })
    }

    /// Generate HTML report
    pub fn generate_html_report(&self, report: &LiveReachReport) -> Result<String> {
        let data = json!({
            "report": report,
            "title": format!("Live Reachability Report - {}", report.svc),
            "generated_date": report.generated_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            "window_start": report.window.0.format("%Y-%m-%d").to_string(),
            "window_end": report.window.1.format("%Y-%m-%d").to_string(),
            "has_shadow_islands": report.stats.shadow_islands > 0,
            "top_islands": report.communities.iter().take(10).collect::<Vec<_>>(),
        });

        self.handlebars
            .render("html_report", &data)
            .map_err(|e| ValknutError::validation(format!("Failed to render HTML report: {}", e)))
    }

    /// Generate Markdown report
    pub fn generate_markdown_report(&self, report: &LiveReachReport) -> Result<String> {
        let data = json!({
            "report": report,
            "title": format!("Live Reachability Report - {}", report.svc),
            "generated_date": report.generated_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            "window_start": report.window.0.format("%Y-%m-%d").to_string(),
            "window_end": report.window.1.format("%Y-%m-%d").to_string(),
            "top_islands": report.communities.iter().take(10).collect::<Vec<_>>(),
        });

        self.handlebars
            .render("markdown_report", &data)
            .map_err(|e| {
                ValknutError::validation(format!("Failed to render Markdown report: {}", e))
            })
    }

    /// Generate analysis notes for a community
    fn generate_community_notes(
        &self,
        community_info: &crate::live::community::CommunityInfo,
        shadow_score: f64,
        nodes: &[CommunityNode],
    ) -> Vec<String> {
        let mut notes = Vec::new();

        // Shadow island severity
        if shadow_score >= 0.8 {
            notes.push("🔴 Critical shadow island - immediate refactoring recommended".to_string());
        } else if shadow_score >= 0.6 {
            notes.push("🟡 Shadow island detected - consider refactoring".to_string());
        }

        // Coupling analysis
        if community_info.cut_ratio() < 0.1 {
            notes.push("🔗 Tightly coupled - few external dependencies".to_string());
        }

        // Runtime vs static analysis
        let runtime_fraction = community_info.runtime_internal_fraction();
        if runtime_fraction < 0.2 {
            notes.push("📊 >80% static-only edges - potentially unused code".to_string());
        } else if runtime_fraction > 0.8 {
            notes.push("⚡ >80% runtime edges - actively used code".to_string());
        }

        // Staleness analysis
        let stale_nodes = nodes
            .iter()
            .filter(|node| node.last_seen.is_none() || node.live_reach < 0.1)
            .count();

        if stale_nodes > nodes.len() / 2 {
            notes.push(format!("🕰️ {} nodes appear stale or unused", stale_nodes));
        }

        // Size analysis
        if nodes.len() >= 20 {
            notes.push("📏 Large community - consider breaking into smaller modules".to_string());
        }

        // Reachability analysis
        let unreachable_nodes = nodes.iter().filter(|node| !node.seed_reachable).count();

        if unreachable_nodes > 0 {
            notes.push(format!(
                "🚫 {} nodes not reachable from entrypoints",
                unreachable_nodes
            ));
        }

        notes
    }
}

impl Default for LiveReachReporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate median of a vector of f64 values
fn calculate_median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let len = sorted.len();
    if len % 2 == 0 {
        (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
    } else {
        sorted[len / 2]
    }
}

/// HTML template for live reachability reports
const HTML_TEMPLATE: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{title}}</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            line-height: 1.6;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            color: #333;
        }
        .header {
            border-bottom: 3px solid #007acc;
            padding-bottom: 20px;
            margin-bottom: 30px;
        }
        .header h1 {
            margin: 0;
            color: #007acc;
        }
        .meta {
            color: #666;
            font-size: 0.9em;
            margin-top: 10px;
        }
        .stats {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }
        .stat-card {
            background: #f8f9fa;
            padding: 20px;
            border-radius: 8px;
            border-left: 4px solid #007acc;
        }
        .stat-value {
            font-size: 2em;
            font-weight: bold;
            color: #007acc;
        }
        .stat-label {
            color: #666;
            font-size: 0.9em;
        }
        .communities {
            margin-top: 30px;
        }
        .community {
            background: #fff;
            border: 1px solid #ddd;
            border-radius: 8px;
            margin-bottom: 20px;
            overflow: hidden;
        }
        .community-header {
            background: #f8f9fa;
            padding: 15px 20px;
            border-bottom: 1px solid #ddd;
        }
        .community-title {
            font-weight: bold;
            font-size: 1.1em;
        }
        .community-score {
            float: right;
            padding: 4px 12px;
            border-radius: 20px;
            font-size: 0.8em;
            color: white;
        }
        .score-high { background: #dc3545; }
        .score-medium { background: #ffc107; color: #000; }
        .score-low { background: #28a745; }
        .community-body {
            padding: 20px;
        }
        .notes {
            margin-bottom: 15px;
        }
        .note {
            background: #e3f2fd;
            padding: 8px 12px;
            margin: 5px 0;
            border-radius: 4px;
            font-size: 0.9em;
        }
        .nodes {
            margin-top: 15px;
        }
        .node {
            font-family: monospace;
            background: #f8f9fa;
            padding: 8px 12px;
            margin: 2px 0;
            border-radius: 4px;
            font-size: 0.8em;
        }
        .node-score {
            float: right;
            color: #666;
        }
        .warning {
            background: #fff3cd;
            border: 1px solid #ffeaa7;
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 20px;
        }
        .success {
            background: #d4edda;
            border: 1px solid #c3e6cb;
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 20px;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1>{{title}}</h1>
        <div class="meta">
            Generated: {{generated_date}} | 
            Analysis Window: {{window_start}} to {{window_end}}
        </div>
    </div>

    <div class="stats">
        <div class="stat-card">
            <div class="stat-value">{{report.stats.total_nodes}}</div>
            <div class="stat-label">Total Nodes</div>
        </div>
        <div class="stat-card">
            <div class="stat-value">{{report.stats.total_edges}}</div>
            <div class="stat-label">Total Edges</div>
        </div>
        <div class="stat-card">
            <div class="stat-value">{{report.stats.communities}}</div>
            <div class="stat-label">Communities</div>
        </div>
        <div class="stat-card">
            <div class="stat-value">{{report.stats.shadow_islands}}</div>
            <div class="stat-label">Shadow Islands</div>
        </div>
        <div class="stat-card">
            <div class="stat-value">{{report.stats.median_live_reach}}</div>
            <div class="stat-label">Median Live Reach</div>
        </div>
    </div>

    {{#if has_shadow_islands}}
    <div class="warning">
        <strong>⚠️ Shadow Islands Detected</strong><br>
        Found {{report.stats.shadow_islands}} communities with low live reach and tight coupling. 
        These may represent unused or problematic code that should be refactored.
    </div>
    {{else}}
    <div class="success">
        <strong>✅ No Critical Shadow Islands</strong><br>
        Your codebase shows good live reachability patterns with healthy coupling.
    </div>
    {{/if}}

    <div class="communities">
        <h2>Community Analysis</h2>
        {{#each top_islands}}
        <div class="community">
            <div class="community-header">
                <div class="community-title">Community {{id}} ({{size}} nodes)</div>
                <div class="community-score {{#if (gte score 0.8)}}score-high{{else}}{{#if (gte score 0.6)}}score-medium{{else}}score-low{{/if}}{{/if}}">
                    Score: {{score}}
                </div>
                <div style="clear: both;"></div>
            </div>
            <div class="community-body">
                {{#if notes}}
                <div class="notes">
                    {{#each notes}}
                    <div class="note">{{this}}</div>
                    {{/each}}
                </div>
                {{/if}}
                
                <div><strong>Cut Ratio:</strong> {{cut_ratio}} | <strong>Runtime Internal:</strong> {{runtime_internal}}</div>
                
                <div class="nodes">
                    <strong>Top Nodes:</strong>
                    {{#each (slice nodes 0 10)}}
                    <div class="node">
                        {{id}}
                        <span class="node-score">Live Reach: {{live_reach}}</span>
                        <div style="clear: both;"></div>
                    </div>
                    {{/each}}
                    {{#if (gt nodes.length 10)}}
                    <div class="note">... and {{sub nodes.length 10}} more nodes</div>
                    {{/if}}
                </div>
            </div>
        </div>
        {{/each}}
    </div>
</body>
</html>
"#;

/// Markdown template for live reachability reports
const MARKDOWN_TEMPLATE: &str = r#"# {{title}}

**Generated:** {{generated_date}}  
**Analysis Window:** {{window_start}} to {{window_end}}

## Summary

| Metric | Value |
|--------|-------|
| Total Nodes | {{report.stats.total_nodes}} |
| Total Edges | {{report.stats.total_edges}} |
| Communities | {{report.stats.communities}} |
| Shadow Islands | {{report.stats.shadow_islands}} |
| Median Live Reach | {{report.stats.median_live_reach}} |
| Runtime Edges | {{report.stats.runtime_edges}} ({{percent report.stats.runtime_edges report.stats.total_edges}}%) |
| Static Edges | {{report.stats.static_edges}} ({{percent report.stats.static_edges report.stats.total_edges}}%) |

{{#if (gt report.stats.shadow_islands 0)}}
## ⚠️ Shadow Islands Detected

Found **{{report.stats.shadow_islands}}** communities with low live reach and tight coupling. These may represent unused or problematic code that should be refactored.
{{else}}
## ✅ No Critical Shadow Islands

Your codebase shows good live reachability patterns with healthy coupling.
{{/if}}

## Community Analysis

{{#each top_islands}}
### Community {{id}} (Score: {{score}}, Size: {{size}})

{{#if notes}}
**Analysis Notes:**
{{#each notes}}
- {{this}}
{{/each}}
{{/if}}

**Metrics:**
- Cut Ratio: {{cut_ratio}}
- Runtime Internal: {{runtime_internal}}

**Top Nodes:**
{{#each (slice nodes 0 5)}}
- `{{id}}` (Live Reach: {{live_reach}})
{{/each}}
{{#if (gt nodes.length 5)}}
- ... and {{sub nodes.length 5}} more nodes
{{/if}}

---

{{/each}}

## Recommendations

{{#if (gt report.stats.shadow_islands 0)}}
1. **Priority Refactoring:** Focus on communities with scores ≥ 0.8
2. **Code Review:** Examine nodes with low live reach (< 0.3)
3. **Monitoring:** Set up alerts for new shadow islands in CI/CD
{{/if}}

4. **Architectural Health:** Maintain median live reach > 0.5
5. **Coupling Management:** Keep cut ratios > 0.2 for healthy modularity
6. **Runtime Coverage:** Ensure critical paths have runtime data (not just static)

---

*Generated by Valknut Live Reachability Analysis*
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_median_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(calculate_median(&values), 3.0);

        let even_values = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(calculate_median(&even_values), 2.5);

        let empty_values: Vec<f64> = vec![];
        assert_eq!(calculate_median(&empty_values), 0.0);

        let single_value = vec![42.0];
        assert_eq!(calculate_median(&single_value), 42.0);
    }

    #[test]
    fn test_report_generation() {
        let reporter = LiveReachReporter::new();

        // Create minimal test report
        let report = LiveReachReport {
            generated_at: Utc::now(),
            svc: "test-service".to_string(),
            window: (Utc::now() - chrono::Duration::days(30), Utc::now()),
            communities: vec![],
            stats: ReportStats {
                total_nodes: 100,
                total_edges: 200,
                runtime_edges: 150,
                static_edges: 50,
                communities: 5,
                shadow_islands: 2,
                median_live_reach: 0.75,
            },
        };

        // Test HTML generation
        let html_result = reporter.generate_html_report(&report);
        assert!(html_result.is_ok());
        let html = html_result.unwrap();
        assert!(html.contains("test-service"));
        assert!(html.contains("100")); // Total nodes

        // Test Markdown generation
        let md_result = reporter.generate_markdown_report(&report);
        if let Err(e) = &md_result {
            eprintln!("Markdown generation error: {:?}", e);
        }
        assert!(md_result.is_ok());
        let markdown = md_result.unwrap();
        assert!(markdown.contains("# Live Reachability Report - test-service"));
        assert!(markdown.contains("| Total Nodes | 100 |"));
    }

    #[test]
    fn test_community_notes_generation() {
        let reporter = LiveReachReporter::new();

        // Mock community info
        let community_info = crate::live::community::CommunityInfo {
            id: 1,
            nodes: vec![], // Simplified for test
            internal_weight: 10.0,
            cut_weight: 1.0, // Low cut ratio
            total_degree: 20.0,
            runtime_internal_count: 1,
            static_internal_count: 9, // High static ratio
        };

        let nodes = vec![CommunityNode {
            id: "test::node1".to_string(),
            live_reach: 0.05, // Low live reach
            last_seen: None,  // Stale
            seed_reachable: false,
        }];

        let notes = reporter.generate_community_notes(&community_info, 0.85, &nodes);

        // Should detect multiple issues
        assert!(!notes.is_empty());
        assert!(notes
            .iter()
            .any(|note| note.contains("Critical shadow island")));
        assert!(notes.iter().any(|note| note.contains("Tightly coupled")));
        assert!(notes.iter().any(|note| note.contains("static-only edges")));
        assert!(notes.iter().any(|note| note.contains("not reachable")));
    }
}
