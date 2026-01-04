use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::config::validate_coverage_discovery;
use crate::core::errors::Result;
use crate::detectors::coverage::types::ScoringWeights;

/// Configuration for coverage analysis and automatic file discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageConfig {
    /// Enable automatic coverage file discovery
    pub auto_discover: bool,

    /// Search paths for coverage files (relative to analysis root)
    pub search_paths: Vec<String>,

    /// File patterns to search for
    pub file_patterns: Vec<String>,

    /// Maximum age of coverage files in days (0 = no age limit)
    pub max_age_days: u32,

    /// Specific coverage file path (overrides auto discovery)
    pub coverage_file: Option<PathBuf>,

    /// Whether coverage gap analysis is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Explicit report locations to analyze
    #[serde(default)]
    pub report_paths: Vec<PathBuf>,

    /// Maximum number of gaps to surface per file
    #[serde(default = "default_max_gaps_per_file")]
    pub max_gaps_per_file: usize,

    /// Minimum gap length (in LOC) to consider actionable
    #[serde(default = "default_min_gap_loc")]
    pub min_gap_loc: usize,

    /// Context lines to include before/after a gap in previews
    #[serde(default = "default_snippet_context_lines")]
    pub snippet_context_lines: usize,

    /// Number of head/tail lines to include for long gaps
    #[serde(default = "default_long_gap_head_tail")]
    pub long_gap_head_tail: usize,

    /// Whether to group gaps across files into packs
    #[serde(default)]
    pub group_cross_file: bool,

    /// Target repo coverage gain used for prioritization
    #[serde(default = "default_target_repo_gain")]
    pub target_repo_gain: f64,

    /// Scoring weights for gap prioritization
    #[serde(default)]
    pub weights: ScoringWeights,

    /// Patterns to exclude from coverage analysis
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

impl Default for CoverageConfig {
    fn default() -> Self {
        Self {
            auto_discover: true,
            search_paths: vec![
                "./coverage/".to_string(),
                "./target/coverage/".to_string(),
                "./target/tarpaulin/".to_string(),
                "./target/".to_string(),
                "./.coverage/".to_string(),
                "./htmlcov/".to_string(),
                "./coverage-reports/".to_string(),
                "./reports/".to_string(),
                "./test-results/".to_string(),
                "./build/coverage/".to_string(),
                "./build/test-results/".to_string(),
                "./".to_string(),
            ],
            file_patterns: vec![
                "coverage.xml".to_string(),
                "lcov.info".to_string(),
                "coverage.json".to_string(),
                "coverage.lcov".to_string(),
                "cobertura.xml".to_string(),
                "coverage-final.json".to_string(),
                "coverage-summary.json".to_string(),
                ".coverage".to_string(),
                "junit.xml".to_string(),
                "jacoco.xml".to_string(),
                "clover.xml".to_string(),
                "**/coverage.xml".to_string(),
                "**/lcov.info".to_string(),
                "**/coverage.json".to_string(),
                "**/cobertura.xml".to_string(),
                "**/jacoco.xml".to_string(),
                "**/clover.xml".to_string(),
                "target/coverage/*.xml".to_string(),
                "target/tarpaulin/coverage.xml".to_string(),
                "target/llvm-cov/coverage.lcov".to_string(),
                "build/coverage/*.xml".to_string(),
                "coverage/coverage-final.json".to_string(),
                "htmlcov/coverage.json".to_string(),
                "**/build/jacoco/*.xml".to_string(),
                "**/build/reports/jacoco/test/*.xml".to_string(),
                "**/build/test-results/test/*.xml".to_string(),
            ],
            max_age_days: 7,
            coverage_file: None,
            enabled: false,
            report_paths: vec![
                PathBuf::from("coverage.xml"),
                PathBuf::from("lcov.info"),
                PathBuf::from("coverage-final.json"),
            ],
            max_gaps_per_file: default_max_gaps_per_file(),
            min_gap_loc: default_min_gap_loc(),
            snippet_context_lines: default_snippet_context_lines(),
            long_gap_head_tail: default_long_gap_head_tail(),
            group_cross_file: false,
            target_repo_gain: default_target_repo_gain(),
            weights: ScoringWeights::default(),
            exclude_patterns: vec!["**/tests/**".to_string(), "**/spec/**".to_string()],
        }
    }
}

impl CoverageConfig {
    /// Validate coverage configuration
    pub fn validate(&self) -> Result<()> {
        validate_coverage_discovery(self.auto_discover, &self.file_patterns, &self.search_paths)
    }
}

fn default_max_gaps_per_file() -> usize {
    5
}

fn default_min_gap_loc() -> usize {
    3
}

fn default_snippet_context_lines() -> usize {
    5
}

fn default_long_gap_head_tail() -> usize {
    2
}

fn default_target_repo_gain() -> f64 {
    0.02
}
