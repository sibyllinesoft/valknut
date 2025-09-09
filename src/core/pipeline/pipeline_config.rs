//! Configuration types and defaults for the analysis pipeline.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use crate::core::config::ValknutConfig;

/// Configuration for comprehensive analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Enable structure analysis
    pub enable_structure_analysis: bool,
    /// Enable complexity analysis
    pub enable_complexity_analysis: bool,
    /// Enable refactoring analysis
    pub enable_refactoring_analysis: bool,
    /// Enable impact analysis
    pub enable_impact_analysis: bool,
    /// File extensions to include
    pub file_extensions: Vec<String>,
    /// Directories to exclude
    pub exclude_directories: Vec<String>,
    /// Maximum files to analyze (0 = no limit)
    pub max_files: usize,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            enable_structure_analysis: true,
            enable_complexity_analysis: true,
            enable_refactoring_analysis: true,
            enable_impact_analysis: true,
            file_extensions: vec![
                "py".to_string(),
                "js".to_string(),
                "ts".to_string(),
                "tsx".to_string(),
                "jsx".to_string(),
                "rs".to_string(),
                "go".to_string(),
                "java".to_string(),
            ],
            exclude_directories: vec![
                "node_modules".to_string(),
                "target".to_string(),
                "__pycache__".to_string(),
                ".git".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
            max_files: 5000,
        }
    }
}

impl From<ValknutConfig> for AnalysisConfig {
    fn from(config: ValknutConfig) -> Self {
        Self::default() // For now, use defaults and extend as needed
    }
}

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