//! CSV report generation.
//!
//! This module provides functions for generating CSV reports from valknut analysis results.

use serde_json::Value;

/// Render the analysis result as CSV rows.
pub async fn generate_csv_report(result: &Value) -> anyhow::Result<String> {
    let mut rows = Vec::new();

    if let Some(complexity) = result.get("complexity") {
        rows.extend(extract_complexity_csv_rows(complexity));
    }

    if let Some(refactoring) = result.get("refactoring") {
        rows.extend(extract_refactoring_csv_rows(refactoring));
    }

    if let Some(structure) = result.get("structure") {
        rows.extend(extract_structure_csv_rows(structure));
    }

    let mut content = String::from("File,Issue Type,Severity,Description,Line,Impact,Effort\n");

    if rows.is_empty() {
        content.push_str(
            "\"No issues found\",\"Info\",\"Info\",\"Code quality is excellent\",0,\"\",\"\"\n",
        );
    } else {
        for row in rows {
            content.push_str(&row);
        }
    }

    Ok(content)
}

/// Escapes double quotes in a string for CSV format.
fn escape_csv(s: &str) -> String {
    s.replace('"', "\"\"")
}

/// Extracts complexity issues and formats them as CSV rows.
fn extract_complexity_csv_rows(complexity: &Value) -> Vec<String> {
    let Some(detailed_results) = complexity.get("detailed_results").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    for file_result in detailed_results {
        let Some(file_path) = file_result.get("file_path").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(file_issues) = file_result.get("issues").and_then(|v| v.as_array()) else {
            continue;
        };

        for issue in file_issues {
            let issue_type = issue.get("category").and_then(|v| v.as_str()).unwrap_or("Complexity");
            let severity = issue.get("severity").and_then(|v| v.as_str()).unwrap_or("Medium");
            let description = issue.get("description").and_then(|v| v.as_str()).unwrap_or("Complexity issue");
            let line = issue.get("line").and_then(|v| v.as_u64()).unwrap_or(0);

            rows.push(format!(
                "\"{}\",\"{}\",\"{}\",\"{}\",{},\"\",\"\"\n",
                escape_csv(file_path), issue_type, severity, escape_csv(description), line
            ));
        }
    }
    rows
}

/// Extracts refactoring recommendations and formats them as CSV rows.
fn extract_refactoring_csv_rows(refactoring: &Value) -> Vec<String> {
    let Some(detailed_results) = refactoring.get("detailed_results").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    for file_result in detailed_results {
        let Some(file_path) = file_result.get("file_path").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(recommendations) = file_result.get("recommendations").and_then(|v| v.as_array()) else {
            continue;
        };

        for rec in recommendations {
            rows.push(build_refactoring_csv_row(rec, file_path));
        }
    }
    rows
}

/// Builds a CSV row from a refactoring recommendation.
fn build_refactoring_csv_row(rec: &Value, file_path: &str) -> String {
    let refactoring_type = rec.get("refactoring_type").and_then(|v| v.as_str()).unwrap_or("Refactoring");
    let description = rec.get("description").and_then(|v| v.as_str()).unwrap_or("Refactoring opportunity");
    let priority_score = rec.get("priority_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let impact = rec.get("estimated_impact").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let effort = rec.get("estimated_effort").and_then(|v| v.as_f64()).unwrap_or(0.0);

    let severity = if priority_score > 0.8 {
        "High"
    } else if priority_score > 0.5 {
        "Medium"
    } else {
        "Low"
    };

    let line = rec
        .get("location")
        .and_then(|v| v.as_array())
        .and_then(|loc| loc.first())
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    format!(
        "\"{}\",\"{}\",\"{}\",\"{}\",{},\"{:.1}\",\"{:.1}\"\n",
        escape_csv(file_path), refactoring_type, severity, escape_csv(description), line, impact, effort
    )
}

/// Extracts structure pack issues and formats them as CSV rows.
fn extract_structure_csv_rows(structure: &Value) -> Vec<String> {
    let Some(packs) = structure.get("packs").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    packs
        .iter()
        .map(|pack| {
            let kind = pack.get("kind").and_then(|v| v.as_str()).unwrap_or("Structure");
            let file_or_dir = pack
                .get("file")
                .and_then(|v| v.as_str())
                .or_else(|| pack.get("directory").and_then(|v| v.as_str()))
                .unwrap_or("Unknown");

            let reasons = pack
                .get("reasons")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|r| r.as_str()).collect::<Vec<_>>().join("; "))
                .unwrap_or_else(|| "Structure issue".to_string());

            format!(
                "\"{}\",\"{}\",\"Medium\",\"{}\",0,\"\",\"\"\n",
                escape_csv(file_or_dir), kind, escape_csv(&reasons)
            )
        })
        .collect()
}

/// Render a concise JSON summary suitable for CI systems.
pub async fn generate_ci_summary_report(result: &Value) -> anyhow::Result<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_csv_report_empty() {
        let result = serde_json::json!({});
        let report = generate_csv_report(&result).await.unwrap();
        assert!(report.contains("No issues found"));
    }

    #[test]
    fn test_escape_csv() {
        assert_eq!(escape_csv("hello"), "hello");
        assert_eq!(escape_csv("hello\"world"), "hello\"\"world");
    }
}
