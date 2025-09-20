//! Quality gate configuration types for pipeline evaluation.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Quality gate configuration for CI/CD integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateConfig {
    /// Whether quality gates are enabled
    pub enabled: bool,
    /// Maximum allowed complexity score (0-100, lower is better)
    pub max_complexity_score: f64,
    /// Maximum allowed technical debt ratio (0-100, lower is better)
    pub max_technical_debt_ratio: f64,
    /// Minimum required maintainability score (0-100, higher is better)
    pub min_maintainability_score: f64,
    /// Maximum allowed critical issues
    pub max_critical_issues: usize,
    /// Maximum allowed high-priority issues
    pub max_high_priority_issues: usize,
}

impl Default for QualityGateConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_complexity_score: 70.0,
            max_technical_debt_ratio: 50.0,
            min_maintainability_score: 60.0,
            max_critical_issues: 5,
            max_high_priority_issues: 20,
        }
    }
}

/// Quality gate violation details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateViolation {
    /// Name of the violated rule
    pub rule_name: String,
    /// Description of the violation
    pub description: String,
    /// Current value that violated the threshold
    pub current_value: f64,
    /// The threshold that was violated
    pub threshold: f64,
    /// Severity of the violation
    pub severity: String,
    /// Files or components that contribute to this violation
    pub affected_files: Vec<PathBuf>,
    /// Recommended actions to fix this violation
    pub recommended_actions: Vec<String>,
}

/// Result of quality gate evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateResult {
    /// Whether all quality gates passed
    pub passed: bool,
    /// List of violations (empty if all gates passed)
    pub violations: Vec<QualityGateViolation>,
    /// Overall quality score
    pub overall_score: f64,
}
