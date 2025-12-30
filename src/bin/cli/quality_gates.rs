//! Quality gate evaluation, violation checking, and display logic.

use std::cmp::Ordering;
use std::path::PathBuf;

use owo_colors::OwoColorize;

use valknut_rs::api::results::{AnalysisResults, RefactoringCandidate};
use valknut_rs::core::pipeline::{QualityGateConfig, QualityGateResult, QualityGateViolation};
use valknut_rs::core::scoring::Priority;

use crate::cli::args::{AnalyzeArgs, QualityGateArgs};

/// Build a quality gate violation with common structure.
pub fn build_violation(
    rule_name: &str,
    description: String,
    current_value: f64,
    threshold: f64,
    severity: &str,
    affected_files: Vec<PathBuf>,
    recommended_actions: Vec<&str>,
) -> QualityGateViolation {
    QualityGateViolation {
        rule_name: rule_name.to_string(),
        description,
        current_value,
        threshold,
        severity: severity.to_string(),
        affected_files,
        recommended_actions: recommended_actions.into_iter().map(String::from).collect(),
    }
}

/// Evaluate quality gates against analysis results.
pub fn evaluate_quality_gates(
    result: &AnalysisResults,
    config: &QualityGateConfig,
    verbose: bool,
) -> anyhow::Result<QualityGateResult> {
    let default_score = (result.summary.code_health_score * 100.0).clamp(0.0, 100.0);

    if !config.enabled {
        let score = result
            .health_metrics
            .as_ref()
            .map(|m| m.overall_health_score)
            .unwrap_or(default_score);

        return Ok(QualityGateResult {
            passed: true,
            violations: Vec::new(),
            overall_score: score,
        });
    }

    let mut violations = Vec::new();
    let high_priority_files = || {
        top_issue_files(
            result,
            |c| matches!(c.priority, Priority::High | Priority::Critical),
            5,
        )
    };

    // Check health metrics if available
    if let Some(metrics) = result.health_metrics.as_ref() {
        check_metric_violations(&mut violations, metrics, config, &high_priority_files);
    } else if verbose {
        println!(
            "{}",
            "⚠️ Quality gate metrics unavailable; skipping maintainability and complexity checks."
                .yellow()
        );
    }

    // Check issue count violations
    check_issue_count_violations(&mut violations, result, config);

    let overall_score = result
        .health_metrics
        .as_ref()
        .map(|m| m.overall_health_score)
        .unwrap_or(default_score)
        .clamp(0.0, 100.0);

    Ok(QualityGateResult {
        passed: violations.is_empty(),
        violations,
        overall_score,
    })
}

/// Check health metric-based violations.
pub fn check_metric_violations(
    violations: &mut Vec<QualityGateViolation>,
    metrics: &valknut_rs::core::pipeline::HealthMetrics,
    config: &QualityGateConfig,
    high_priority_files: &impl Fn() -> Vec<PathBuf>,
) {
    if metrics.complexity_score > config.max_complexity_score {
        violations.push(build_violation(
            "Complexity Threshold",
            format!(
                "Average complexity score ({:.1}) exceeds configured limit ({:.1})",
                metrics.complexity_score, config.max_complexity_score
            ),
            metrics.complexity_score,
            config.max_complexity_score,
            severity_for_excess(metrics.complexity_score, config.max_complexity_score),
            high_priority_files(),
            vec![
                "Break down the highest complexity functions highlighted above",
                "Introduce guard clauses or helper methods to reduce nesting",
            ],
        ));
    }

    if metrics.technical_debt_ratio > config.max_technical_debt_ratio {
        violations.push(build_violation(
            "Technical Debt Ratio",
            format!(
                "Technical debt ratio ({:.1}%) exceeds maximum allowed ({:.1}%)",
                metrics.technical_debt_ratio, config.max_technical_debt_ratio
            ),
            metrics.technical_debt_ratio,
            config.max_technical_debt_ratio,
            severity_for_excess(metrics.technical_debt_ratio, config.max_technical_debt_ratio),
            high_priority_files(),
            vec![
                "Triage the listed hotspots and schedule debt paydown work",
                "Ensure tests cover recent refactors to prevent regression",
            ],
        ));
    }

    if metrics.maintainability_score < config.min_maintainability_score {
        violations.push(build_violation(
            "Maintainability Score",
            format!(
                "Maintainability score ({:.1}) fell below required minimum ({:.1})",
                metrics.maintainability_score, config.min_maintainability_score
            ),
            metrics.maintainability_score,
            config.min_maintainability_score,
            severity_for_shortfall(metrics.maintainability_score, config.min_maintainability_score),
            high_priority_files(),
            vec![
                "Refactor low-cohesion modules to improve readability",
                "Document intent for complex code paths flagged in the report",
            ],
        ));
    }
}

/// Check issue count violations.
pub fn check_issue_count_violations(
    violations: &mut Vec<QualityGateViolation>,
    result: &AnalysisResults,
    config: &QualityGateConfig,
) {
    let summary = &result.summary;

    if summary.critical as usize > config.max_critical_issues {
        let affected = top_issue_files(result, |c| matches!(c.priority, Priority::Critical), 5);
        violations.push(build_violation(
            "Critical Issues",
            format!(
                "{} critical issues detected (limit: {})",
                summary.critical, config.max_critical_issues
            ),
            summary.critical as f64,
            config.max_critical_issues as f64,
            severity_for_excess(summary.critical as f64, config.max_critical_issues as f64),
            affected,
            vec![
                "Prioritise fixes for the critical hotspots above",
                "Add regression tests before merging related fixes",
            ],
        ));
    }

    if summary.high_priority as usize > config.max_high_priority_issues {
        let affected = top_issue_files(
            result,
            |c| matches!(c.priority, Priority::High | Priority::Critical),
            5,
        );
        violations.push(build_violation(
            "High Priority Issues",
            format!(
                "{} high-priority issues detected (limit: {})",
                summary.high_priority, config.max_high_priority_issues
            ),
            summary.high_priority as f64,
            config.max_high_priority_issues as f64,
            severity_for_excess(
                summary.high_priority as f64,
                config.max_high_priority_issues as f64,
            ),
            affected,
            vec![
                "Address the highlighted high-priority candidates before release",
                "Break work into smaller refactors to keep velocity high",
            ],
        ));
    }
}

/// Display quality gate failures and recommended remediation steps.
pub fn display_quality_failures(result: &QualityGateResult, detailed: bool) {
    for violation in &result.violations {
        println!(
            "  {} - {} (current: {:.1}, threshold: {:.1})",
            violation.rule_name,
            violation.description,
            violation.current_value,
            violation.threshold
        );

        if detailed && !violation.recommended_actions.is_empty() {
            println!("     actions:");
            for action in &violation.recommended_actions {
                println!("       - {}", action);
            }
        }
    }

    if !result.violations.is_empty() {
        println!("  overall quality score: {:.1}/100", result.overall_score);
    }
}

/// Assigns a severity string when a metric exceeds its threshold.
pub fn severity_for_excess(current: f64, threshold: f64) -> &'static str {
    let delta = current - threshold;
    if threshold == 0.0 {
        if delta >= 5.0 {
            "Critical"
        } else if delta >= 1.0 {
            "High"
        } else {
            "Medium"
        }
    } else if delta >= threshold * 0.5 || delta >= 20.0 {
        "Critical"
    } else if delta >= threshold * 0.25 || delta >= 10.0 {
        "High"
    } else {
        "Medium"
    }
}

/// Assigns a severity string when a metric falls short of its threshold.
pub fn severity_for_shortfall(current: f64, threshold: f64) -> &'static str {
    let delta = threshold - current;
    if delta >= 20.0 {
        "Critical"
    } else if delta >= 10.0 {
        "High"
    } else {
        "Medium"
    }
}

/// Returns a ranked list of issue file paths filtered by the provided predicate.
pub fn top_issue_files<F>(result: &AnalysisResults, filter: F, limit: usize) -> Vec<PathBuf>
where
    F: Fn(&RefactoringCandidate) -> bool,
{
    let mut ranked: Vec<_> = result
        .refactoring_candidates
        .iter()
        .filter(|candidate| filter(candidate))
        .map(|candidate| {
            (
                candidate.priority,
                candidate.score,
                PathBuf::from(&candidate.file_path),
            )
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal))
    });

    let mut files = Vec::new();
    for (_, _, path) in ranked {
        if !files.iter().any(|existing| existing == &path) {
            files.push(path);
        }
        if files.len() >= limit {
            break;
        }
    }

    files
}

/// Build quality gate configuration from CLI arguments.
pub fn build_quality_gate_config(args: &AnalyzeArgs) -> QualityGateConfig {
    let mut config = QualityGateConfig {
        enabled: args.quality_gate.quality_gate || args.quality_gate.fail_on_issues,
        min_health_score: QualityGateConfig::default().min_health_score,
        min_doc_health_score: QualityGateConfig::default().min_doc_health_score,
        ..Default::default()
    };

    // Override defaults with CLI values if provided
    if let Some(max_complexity) = args.quality_gate.max_complexity {
        config.max_complexity_score = max_complexity;
    }
    if let Some(min_health) = args.quality_gate.min_health {
        config.min_maintainability_score = min_health;
        config.min_health_score = min_health;
    }
    if let Some(min_doc_health) = args.quality_gate.min_doc_health {
        config.min_doc_health_score = min_doc_health;
    }
    if let Some(max_debt) = args.quality_gate.max_debt {
        config.max_technical_debt_ratio = max_debt;
    }
    if let Some(min_maintainability) = args.quality_gate.min_maintainability {
        config.min_maintainability_score = min_maintainability;
    }
    if let Some(max_issues) = args.quality_gate.max_issues {
        config.max_critical_issues = max_issues;
    }
    if let Some(max_critical) = args.quality_gate.max_critical {
        config.max_critical_issues = max_critical;
    }
    if let Some(max_high_priority) = args.quality_gate.max_high_priority {
        config.max_high_priority_issues = max_high_priority;
    }

    // Handle fail_on_issues flag (sets max_issues to 0)
    if args.quality_gate.fail_on_issues {
        config.max_critical_issues = 0;
        config.max_high_priority_issues = 0;
    }

    config
}

/// Print a group of violations with a header.
pub fn print_violation_group(header: &str, violations: &[&QualityGateViolation]) {
    if violations.is_empty() {
        return;
    }
    println!("{}", header);
    for v in violations {
        println!(
            "  • {}: {:.1} (threshold: {:.1})",
            v.rule_name.yellow(),
            v.current_value,
            v.threshold
        );
        println!("    {}", v.description.dimmed());
    }
    println!();
}

/// Display quality gate violations in a user-friendly format.
#[allow(dead_code)]
pub fn display_quality_gate_violations(result: &QualityGateResult) {
    println!();
    println!("{}", "❌ Quality Gate Failed".red().bold());
    println!(
        "{} {:.1}",
        "Quality Score:".dimmed(),
        result.overall_score.to_string().yellow()
    );
    println!();

    // Group and display violations by severity
    let (blockers, criticals, warnings): (Vec<_>, Vec<_>, Vec<_>) = result
        .violations
        .iter()
        .fold((vec![], vec![], vec![]), |(mut b, mut c, mut w), v| {
            match v.severity.as_str() {
                "Blocker" => b.push(v),
                "Critical" => c.push(v),
                "Warning" | "High" => w.push(v),
                _ => {}
            }
            (b, c, w)
        });

    print_violation_group(&"🚫 BLOCKER Issues:".red().bold().to_string(), &blockers);
    print_violation_group(&"🔴 CRITICAL Issues:".red().bold().to_string(), &criticals);
    print_violation_group(&"⚠️  WARNING Issues:".yellow().bold().to_string(), &warnings);

    println!("{}", "To fix these issues:".bold());
    println!("  1. Reduce code complexity by refactoring large functions");
    println!("  2. Address critical and high-priority issues first");
    println!("  3. Improve code maintainability through better structure");
    println!("  4. Reduce technical debt by following best practices");
    println!();
}

/// Evaluate quality gates if enabled.
pub fn evaluate_quality_gates_if_enabled(
    result: &AnalysisResults,
    args: &AnalyzeArgs,
    quiet_mode: bool,
) -> anyhow::Result<Option<QualityGateResult>> {
    if !args.quality_gate.quality_gate && !args.quality_gate.fail_on_issues {
        return Ok(None);
    }
    let quality_config = build_quality_gate_config(args);
    let gate_result = evaluate_quality_gates(result, &quality_config, !quiet_mode)?;
    Ok(Some(gate_result))
}

/// Handle quality gate result and return error if failed.
pub fn handle_quality_gate_result(
    result: Option<QualityGateResult>,
    quiet_mode: bool,
    detail_mode: bool,
) -> anyhow::Result<()> {
    let Some(quality_result) = result else {
        return Ok(());
    };

    if !quality_result.passed {
        if !quiet_mode {
            println!("Quality gate: failed");
            display_quality_failures(&quality_result, detail_mode);
        }
        return Err(anyhow::anyhow!("Quality gates failed"));
    }

    if !quiet_mode {
        println!("Quality gate: passed");
    }
    Ok(())
}

/// Generate status string for quality gate configuration.
pub fn quality_status(args: &QualityGateArgs) -> String {
    if args.fail_on_issues {
        "fail on issues".to_string()
    } else if args.quality_gate {
        "on".to_string()
    } else {
        "off".to_string()
    }
}

/// Handle quality gate evaluation for JSON results emitted by tests.
#[allow(dead_code)]
pub async fn handle_quality_gates(
    args: &AnalyzeArgs,
    result: &serde_json::Value,
) -> anyhow::Result<QualityGateResult> {
    // Build quality gate configuration from CLI args
    let quality_gate_config = build_quality_gate_config(args);

    let mut violations = Vec::new();

    // Extract summary data (this should always be present)
    let summary = result
        .get("summary")
        .ok_or_else(|| anyhow::anyhow!("Summary not found in analysis result"))?;

    let total_issues = summary
        .get("total_issues")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    // Check available metrics against thresholds
    if quality_gate_config.max_critical_issues > 0
        && total_issues > quality_gate_config.max_critical_issues
    {
        violations.push(QualityGateViolation {
            rule_name: "Total Issues Count".to_string(),
            current_value: total_issues as f64,
            threshold: quality_gate_config.max_critical_issues as f64,
            description: format!(
                "Total issues ({}) exceeds maximum allowed ({})",
                total_issues, quality_gate_config.max_critical_issues
            ),
            severity: if total_issues > quality_gate_config.max_critical_issues * 2 {
                "Critical".to_string()
            } else {
                "High".to_string()
            },
            affected_files: Vec::new(),
            recommended_actions: vec!["Review and address high-priority issues".to_string()],
        });
    }

    // Try to extract health metrics if available (for more comprehensive analysis)
    if let Some(health_metrics) = result.get("health_metrics") {
        if let Some(overall_health) = health_metrics
            .get("overall_health_score")
            .and_then(|v| v.as_f64())
        {
            if overall_health < quality_gate_config.min_maintainability_score {
                violations.push(QualityGateViolation {
                    rule_name: "Overall Health Score".to_string(),
                    current_value: overall_health,
                    threshold: quality_gate_config.min_maintainability_score,
                    description: format!(
                        "Health score ({:.1}) is below minimum required ({:.1})",
                        overall_health, quality_gate_config.min_maintainability_score
                    ),
                    severity: if overall_health
                        < quality_gate_config.min_maintainability_score - 20.0
                    {
                        "Blocker".to_string()
                    } else {
                        "Critical".to_string()
                    },
                    affected_files: Vec::new(),
                    recommended_actions: vec![
                        "Improve code structure and reduce technical debt".to_string()
                    ],
                });
            }
        }

        if let Some(complexity_score) = health_metrics
            .get("complexity_score")
            .and_then(|v| v.as_f64())
        {
            if complexity_score > quality_gate_config.max_complexity_score {
                violations.push(QualityGateViolation {
                    rule_name: "Complexity Score".to_string(),
                    current_value: complexity_score,
                    threshold: quality_gate_config.max_complexity_score,
                    description: format!(
                        "Complexity score ({:.1}) exceeds maximum allowed ({:.1})",
                        complexity_score, quality_gate_config.max_complexity_score
                    ),
                    severity: if complexity_score > quality_gate_config.max_complexity_score + 10.0
                    {
                        "Critical".to_string()
                    } else {
                        "High".to_string()
                    },
                    affected_files: Vec::new(),
                    recommended_actions: vec![
                        "Simplify complex functions and reduce nesting".to_string()
                    ],
                });
            }
        }

        if let Some(debt_ratio) = health_metrics
            .get("technical_debt_ratio")
            .and_then(|v| v.as_f64())
        {
            if debt_ratio > quality_gate_config.max_technical_debt_ratio {
                violations.push(QualityGateViolation {
                    rule_name: "Technical Debt Ratio".to_string(),
                    current_value: debt_ratio,
                    threshold: quality_gate_config.max_technical_debt_ratio,
                    description: format!(
                        "Technical debt ratio ({:.1}%) exceeds maximum allowed ({:.1}%)",
                        debt_ratio, quality_gate_config.max_technical_debt_ratio
                    ),
                    severity: if debt_ratio > quality_gate_config.max_technical_debt_ratio + 20.0 {
                        "Critical".to_string()
                    } else {
                        "High".to_string()
                    },
                    affected_files: Vec::new(),
                    recommended_actions: vec!["Refactor code to reduce technical debt".to_string()],
                });
            }
        }
    }

    let passed = violations.is_empty();
    let overall_score = result
        .get("health_metrics")
        .and_then(|hm| hm.get("overall_health_score"))
        .and_then(|v| v.as_f64())
        .unwrap_or(50.0); // Default score if not available

    Ok(QualityGateResult {
        passed,
        violations,
        overall_score,
    })
}
