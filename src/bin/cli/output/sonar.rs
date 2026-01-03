//! SonarQube report generation.
//!
//! This module provides functions for generating SonarQube-compatible JSON reports
//! from valknut analysis results.

use serde_json::Value;

/// Render the analysis result as SonarQube JSON.
pub async fn generate_sonar_report(result: &Value) -> anyhow::Result<String> {
    let mut issues = Vec::new();

    if let Some(complexity) = result.get("complexity") {
        issues.extend(extract_complexity_sonar_issues(complexity));
    }

    if let Some(refactoring) = result.get("refactoring") {
        issues.extend(extract_refactoring_sonar_issues(refactoring));
    }

    let sonar_format = serde_json::json!({
        "issues": issues,
        "version": "1.0",
        "summary": {
            "total_issues": issues.len(),
            "analysis_date": chrono::Utc::now().to_rfc3339(),
            "rules_used": issues.iter()
                .filter_map(|issue| issue.get("ruleId").and_then(|v| v.as_str()))
                .collect::<std::collections::HashSet<_>>()
                .len()
        }
    });

    Ok(serde_json::to_string_pretty(&sonar_format)?)
}

/// Extracts complexity issues and formats them as SonarQube issues.
fn extract_complexity_sonar_issues(complexity: &Value) -> Vec<Value> {
    let Some(detailed_results) = complexity.get("detailed_results").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut issues = Vec::new();
    for file_result in detailed_results {
        let Some(file_path) = file_result.get("file_path").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(file_issues) = file_result.get("issues").and_then(|v| v.as_array()) else {
            continue;
        };

        for issue in file_issues {
            issues.push(build_sonar_issue_from_complexity(issue, file_path));
        }
    }
    issues
}

/// Builds a SonarQube issue from a complexity issue.
fn build_sonar_issue_from_complexity(issue: &Value, file_path: &str) -> Value {
    let severity = map_severity_to_sonar(issue.get("severity").and_then(|v| v.as_str()));
    let rule_key = issue.get("category").and_then(|v| v.as_str()).unwrap_or("complexity");
    let description = issue.get("description").and_then(|v| v.as_str()).unwrap_or("Complexity issue");
    let line = issue.get("line").and_then(|v| v.as_u64()).unwrap_or(1);

    build_sonar_issue(rule_key, severity, description, file_path, line)
}

/// Maps valknut severity levels to SonarQube severity strings.
fn map_severity_to_sonar(severity: Option<&str>) -> &'static str {
    match severity {
        Some("Critical") => "BLOCKER",
        Some("VeryHigh") => "CRITICAL",
        Some("High") => "MAJOR",
        Some("Medium") => "MINOR",
        _ => "INFO",
    }
}

/// Extracts refactoring recommendations and formats them as SonarQube issues.
fn extract_refactoring_sonar_issues(refactoring: &Value) -> Vec<Value> {
    let Some(detailed_results) = refactoring.get("detailed_results").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut issues = Vec::new();
    for file_result in detailed_results {
        let Some(file_path) = file_result.get("file_path").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(recommendations) = file_result.get("recommendations").and_then(|v| v.as_array()) else {
            continue;
        };

        for rec in recommendations {
            issues.push(build_sonar_issue_from_recommendation(rec, file_path));
        }
    }
    issues
}

/// Builds a SonarQube issue from a refactoring recommendation.
fn build_sonar_issue_from_recommendation(rec: &Value, file_path: &str) -> Value {
    let priority_score = rec.get("priority_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let severity = if priority_score > 0.8 {
        "MAJOR"
    } else if priority_score > 0.5 {
        "MINOR"
    } else {
        "INFO"
    };

    let refactoring_type = rec.get("refactoring_type").and_then(|v| v.as_str()).unwrap_or("refactoring");
    let description = rec.get("description").and_then(|v| v.as_str()).unwrap_or("Refactoring opportunity");
    let line = rec
        .get("location")
        .and_then(|v| v.as_array())
        .and_then(|loc| loc.first())
        .and_then(|v| v.as_u64())
        .unwrap_or(1);

    build_sonar_issue(&refactoring_type.to_lowercase(), severity, description, file_path, line)
}

/// Builds a SonarQube-formatted issue JSON object.
fn build_sonar_issue(rule_key: &str, severity: &str, message: &str, file_path: &str, line: u64) -> Value {
    serde_json::json!({
        "engineId": "valknut",
        "ruleId": format!("valknut:{}", rule_key),
        "severity": severity,
        "type": "CODE_SMELL",
        "primaryLocation": {
            "message": message,
            "filePath": file_path,
            "textRange": {
                "startLine": line,
                "endLine": line
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_sonar_report_empty() {
        let result = serde_json::json!({});
        let report = generate_sonar_report(&result).await.unwrap();
        let parsed: Value = serde_json::from_str(&report).unwrap();
        assert!(parsed.get("issues").unwrap().as_array().unwrap().is_empty());
    }

    #[test]
    fn test_map_severity_to_sonar() {
        assert_eq!(map_severity_to_sonar(Some("Critical")), "BLOCKER");
        assert_eq!(map_severity_to_sonar(Some("VeryHigh")), "CRITICAL");
        assert_eq!(map_severity_to_sonar(Some("High")), "MAJOR");
        assert_eq!(map_severity_to_sonar(Some("Medium")), "MINOR");
        assert_eq!(map_severity_to_sonar(None), "INFO");
    }
}
