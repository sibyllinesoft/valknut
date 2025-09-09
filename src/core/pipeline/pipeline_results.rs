//! Result types and data structures for analysis pipeline outputs.

use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::core::featureset::FeatureVector;
use crate::core::scoring::ScoringResult;
use crate::detectors::complexity::ComplexityAnalysisResult;
use crate::detectors::refactoring::RefactoringAnalysisResult;
use super::pipeline_config::AnalysisConfig;

/// Comprehensive analysis result containing all analysis types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveAnalysisResult {
    /// Unique identifier for this analysis run
    pub analysis_id: String,
    /// Timestamp when analysis started
    pub timestamp: DateTime<Utc>,
    /// Total processing time in seconds
    pub processing_time: f64,
    /// Analysis configuration used
    pub config: AnalysisConfig,
    /// Summary statistics
    pub summary: AnalysisSummary,
    /// Structure analysis results
    pub structure: StructureAnalysisResults,
    /// Complexity analysis results
    pub complexity: ComplexityAnalysisResults,
    /// Refactoring analysis results
    pub refactoring: RefactoringAnalysisResults,
    /// Impact analysis results  
    pub impact: ImpactAnalysisResults,
    /// Overall health metrics
    pub health_metrics: HealthMetrics,
}

/// Summary statistics for the analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSummary {
    /// Total files analyzed
    pub total_files: usize,
    /// Total entities analyzed (functions, classes, etc.)
    pub total_entities: usize,
    /// Total lines of code
    pub total_lines_of_code: usize,
    /// Languages detected
    pub languages: Vec<String>,
    /// Total issues found
    pub total_issues: usize,
    /// High-priority issues
    pub high_priority_issues: usize,
    /// Critical issues
    pub critical_issues: usize,
}

/// Structure analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Directory reorganization recommendations
    pub directory_recommendations: Vec<serde_json::Value>,
    /// File splitting recommendations
    pub file_splitting_recommendations: Vec<serde_json::Value>,
    /// Structure issues count
    pub issues_count: usize,
}

/// Complexity analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Detailed complexity results per file/entity
    pub detailed_results: Vec<ComplexityAnalysisResult>,
    /// Average cyclomatic complexity
    pub average_cyclomatic_complexity: f64,
    /// Average cognitive complexity
    pub average_cognitive_complexity: f64,
    /// Average technical debt score
    pub average_technical_debt_score: f64,
    /// Average maintainability index
    pub average_maintainability_index: f64,
    /// Complexity issues count
    pub issues_count: usize,
}

/// Refactoring analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Detailed refactoring results
    pub detailed_results: Vec<RefactoringAnalysisResult>,
    /// Refactoring opportunities count
    pub opportunities_count: usize,
}

/// Impact analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Dependency cycles detected
    pub dependency_cycles: Vec<serde_json::Value>,
    /// Chokepoint modules
    pub chokepoints: Vec<serde_json::Value>,
    /// Clone groups
    pub clone_groups: Vec<serde_json::Value>,
    /// Impact issues count
    pub issues_count: usize,
}

/// Overall health metrics for the codebase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// Overall health score (0-100, higher is better)
    pub overall_health_score: f64,
    /// Maintainability score (0-100, higher is better)
    pub maintainability_score: f64,
    /// Technical debt ratio (0-100, lower is better)
    pub technical_debt_ratio: f64,
    /// Complexity score (0-100, lower is better)
    pub complexity_score: f64,
    /// Structure quality score (0-100, higher is better)
    pub structure_quality_score: f64,
}

/// Pipeline execution results wrapper
#[derive(Debug)]
pub struct PipelineResults {
    /// Analysis ID
    pub analysis_id: String,
    /// Execution timestamp
    pub timestamp: DateTime<Utc>,
    /// Comprehensive analysis results
    pub results: ComprehensiveAnalysisResult,
    /// Pipeline execution statistics
    pub statistics: PipelineStatistics,
    /// Errors encountered during analysis
    pub errors: Vec<String>,
    /// Scoring results
    pub scoring_results: ScoringResults,
    /// Feature vectors extracted
    pub feature_vectors: Vec<FeatureVector>,
}

impl PipelineResults {
    /// Get a summary of the results
    pub fn summary(&self) -> ResultSummary {
        let refactoring_needed = self.results.refactoring.opportunities_count;
        let total_entities = self.results.summary.total_entities;
        let avg_score = if total_entities > 0 {
            (100.0 - self.results.health_metrics.complexity_score) / 100.0
        } else {
            1.0
        };

        ResultSummary {
            total_files: self.results.summary.total_files,
            total_issues: self.results.summary.total_issues,
            health_score: self.results.health_metrics.overall_health_score,
            processing_time: self.results.processing_time,
            total_entities,
            refactoring_needed,
            avg_score,
        }
    }
}

/// Pipeline execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatistics {
    /// Memory usage statistics
    pub memory_stats: MemoryStats,
    /// Number of files processed
    pub files_processed: usize,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
}

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Current memory usage in bytes
    pub current_memory_bytes: usize,
    /// Peak memory usage in bytes
    pub peak_memory_bytes: usize,
}

/// Summary of analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultSummary {
    /// Total files analyzed
    pub total_files: usize,
    /// Total issues found
    pub total_issues: usize,
    /// Health score
    pub health_score: f64,
    /// Processing time in seconds
    pub processing_time: f64,
    /// Total entities analyzed (legacy compatibility)
    pub total_entities: usize,
    /// Refactoring needed count (legacy compatibility)
    pub refactoring_needed: usize,
    /// Average score (legacy compatibility)
    pub avg_score: f64,
}

/// Scoring results container
#[derive(Debug, Clone)]
pub struct ScoringResults {
    /// File scores
    pub files: Vec<ScoringResult>,
}

impl ScoringResults {
    /// Iterate over scoring results
    pub fn iter(&self) -> std::slice::Iter<'_, ScoringResult> {
        self.files.iter()
    }
}

/// Individual file scoring result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileScore {
    /// File path
    pub path: PathBuf,
    /// Overall score
    pub score: f64,
    /// Individual metric scores
    pub metrics: HashMap<String, f64>,
}

impl FileScore {
    /// Check if this file needs refactoring based on score thresholds
    pub fn needs_refactoring(&self) -> bool {
        self.score < 60.0 // Files with score below 60 need attention
    }
}

/// Pipeline execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatus {
    /// Whether pipeline is ready to execute
    pub ready: bool,
    /// Current status message
    pub status: String,
    /// Errors if any
    pub errors: Vec<String>,
    /// Issues (legacy compatibility)
    pub issues: Vec<String>,
    /// Is ready flag (legacy compatibility)
    pub is_ready: bool,
    /// Configuration valid (legacy compatibility)
    pub config_valid: bool,
}