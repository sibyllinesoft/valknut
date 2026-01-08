//! Analysis results and reporting structures.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::pipeline::StageResultsBundle;
use crate::core::pipeline::{CloneVerificationResults, HealthMetrics};
use crate::core::scoring::Priority;
// use crate::detectors::names::{RenamePack, ContractMismatchPack, ConsistencyIssue};

#[cfg(test)]
#[path = "result_types_tests.rs"]
mod tests;

/// High-level analysis results for public API consumption
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResults {
    /// Root directory of the analyzed project. All file paths are relative to this.
    #[serde(default)]
    pub project_root: PathBuf,

    /// Summary of the analysis
    pub summary: AnalysisSummary,

    /// Optional normalized snapshot of results for downstream consumers
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized: Option<NormalizedAnalysisResults>,

    /// Per-pass outputs from the analysis pipeline
    pub passes: StageResultsBundle,

    /// Detailed results for entities that need refactoring
    pub refactoring_candidates: Vec<RefactoringCandidate>,

    /// Analysis statistics
    pub statistics: AnalysisStatistics,

    /// Aggregated health metrics computed by the pipeline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_metrics: Option<HealthMetrics>,

    /// Per-directory health scores (0-100, using same formula as overall health)
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub directory_health: std::collections::HashMap<String, f64>,

    /// Per-file health scores (0-100, using same formula as overall health)
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub file_health: std::collections::HashMap<String, f64>,

    /// Per-entity health scores (0-100, using same formula as overall health)
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub entity_health: std::collections::HashMap<String, f64>,

    /// Directory health tree structure for file browser visualization
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory_health_tree: Option<DirectoryHealthTree>,

    /// Code quality analysis results (simple pattern-based analysis)
    // pub naming_results: Option<NamingAnalysisResults>,

    /// Clone detection and denoising analysis results
    pub clone_analysis: Option<CloneAnalysisResults>,

    /// Coverage analysis results - test gap analysis with prioritized packs
    pub coverage_packs: Vec<crate::detectors::coverage::CoveragePack>,

    /// Documentation analysis results (lightweight view for reports)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<DocumentationResults>,

    /// Any warnings or issues encountered
    pub warnings: Vec<String>,

    /// Dictionary describing issue/suggestion codes for downstream consumers
    #[serde(default, skip_serializing_if = "CodeDictionary::is_empty")]
    pub code_dictionary: CodeDictionary,
}

/// Lightweight documentation results for public consumers
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DocumentationResults {
    /// Total documentation issues (doc gaps)
    pub issues_count: usize,
    /// Documentation health score (0-100)
    pub doc_health_score: f64,
    /// Per-file documentation health
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub file_doc_health: HashMap<String, f64>,
    /// Per-file doc issue counts
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub file_doc_issues: HashMap<String, usize>,
    /// Per-directory doc health (0-100)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub directory_doc_health: HashMap<String, f64>,
    /// Per-directory doc issue counts
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub directory_doc_issues: HashMap<String, usize>,
}

/// Summary of analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSummary {
    /// Total number of files processed
    pub files_processed: usize,

    /// Total number of entities analyzed
    pub entities_analyzed: usize,

    /// Number of entities that need refactoring
    pub refactoring_needed: usize,

    /// Number of high-priority refactoring candidates
    pub high_priority: usize,

    /// Number of critical refactoring candidates
    pub critical: usize,

    /// Average refactoring score across all entities
    pub avg_refactoring_score: f64,

    /// Overall code health score (0.0 = poor, 1.0 = excellent)
    pub code_health_score: f64,

    /// Total files analyzed (pipeline aggregate)
    pub total_files: usize,

    /// Total entities analyzed (pipeline aggregate)
    pub total_entities: usize,

    /// Total lines of code analyzed
    pub total_lines_of_code: usize,

    /// Languages detected in the project
    pub languages: Vec<String>,

    /// Total issues detected during analysis
    pub total_issues: usize,

    /// Number of high-priority issues detected
    pub high_priority_issues: usize,

    /// Number of critical issues detected
    pub critical_issues: usize,

    /// Documentation health score (0.0 = poor, 1.0 = excellent) - future use
    #[serde(default)]
    pub doc_health_score: f64,

    /// Documentation issue count (files/dirs/readmes with gaps)
    #[serde(default)]
    pub doc_issue_count: usize,
}

/// Methods for updating [`AnalysisSummary`] with additional metrics.
impl AnalysisSummary {
    /// Add doc issues and recompute code health based on total entities
    pub fn apply_doc_issues(&mut self, doc_issue_count: usize) {
        self.total_issues += doc_issue_count;
        self.doc_issue_count += doc_issue_count;
        if self.total_entities > 0 {
            let penalty = (self.total_issues as f64 / self.total_entities as f64).min(1.0);
            self.code_health_score = (1.0 - penalty).clamp(0.0, 1.0);
        }
    }
}

/// A candidate entity that may need refactoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringCandidate {
    /// Entity identifier
    pub entity_id: String,

    /// Entity name (function, class, etc.)
    pub name: String,

    /// File path containing this entity
    pub file_path: String,

    /// Line range in the file
    pub line_range: Option<(usize, usize)>,

    /// Overall refactoring priority
    pub priority: Priority,

    /// Overall refactoring score
    pub score: f64,

    /// Confidence in this assessment
    pub confidence: f64,

    /// Breakdown of issues by category
    pub issues: Vec<RefactoringIssue>,

    /// Suggested refactoring actions
    pub suggestions: Vec<RefactoringSuggestion>,

    /// Count of issues (for React-safe templates)
    pub issue_count: usize,

    /// Count of suggestions (for React-safe templates)
    pub suggestion_count: usize,

    /// Test coverage percentage (0-100), if coverage data available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage_percentage: Option<f64>,
}

/// A specific refactoring issue within an entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringIssue {
    /// Machine-readable code identifying the issue type
    pub code: String,

    /// Issue category (complexity, structure, etc.)
    pub category: String,

    /// Severity score
    pub severity: f64,

    /// Contributing features
    pub contributing_features: Vec<FeatureContribution>,
}

/// Contribution of a specific feature to an issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureContribution {
    /// Feature name
    pub feature_name: String,

    /// Feature value
    pub value: f64,

    /// Normalized value
    pub normalized_value: f64,

    /// Contribution to the overall score
    pub contribution: f64,
}

/// A suggested refactoring action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringSuggestion {
    /// Type of refactoring (extract_method, reduce_complexity, etc.)
    pub refactoring_type: String,

    /// Machine-readable code identifying the suggestion type
    pub code: String,

    /// Priority level (0.0-1.0)
    pub priority: f64,

    /// Estimated effort level (0.0-1.0)
    pub effort: f64,

    /// Expected impact (0.0-1.0)
    pub impact: f64,
}

/// Dictionary of refactoring issue and suggestion codes
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeDictionary {
    /// Issue code definitions keyed by code
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub issues: std::collections::HashMap<String, CodeDefinition>,

    /// Suggestion code definitions keyed by code
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub suggestions: std::collections::HashMap<String, CodeDefinition>,
}

/// Query methods for [`CodeDictionary`].
impl CodeDictionary {
    /// Checks if both issue and suggestion dictionaries are empty.
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty() && self.suggestions.is_empty()
    }
}

/// Human-friendly description of a code emitted by the analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeDefinition {
    /// Short machine-readable code
    pub code: String,

    /// Concise human-facing title
    pub title: String,

    /// Longer explanation or remediation guidance
    pub summary: String,

    /// Optional category the code belongs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// Refactoring candidates grouped by file for reporting
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileRefactoringGroup {
    /// File path on disk
    pub file_path: String,

    /// File name without path components
    pub file_name: String,

    /// Number of entities flagged in this file
    pub entity_count: usize,

    /// Highest priority refactoring issue found in the file
    pub highest_priority: Priority,

    /// Average score across all entities in this file
    pub avg_score: f64,

    /// Total number of individual issues contributing to the score
    pub total_issues: usize,

    /// Detailed list of entities that require attention
    pub entities: Vec<RefactoringCandidate>,
}

/// Detailed analysis statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisStatistics {
    /// Total execution time
    pub total_duration: Duration,

    /// Average processing time per file
    pub avg_file_processing_time: Duration,

    /// Average processing time per entity
    pub avg_entity_processing_time: Duration,

    /// Number of features extracted per entity
    pub features_per_entity: HashMap<String, f64>,

    /// Distribution of refactoring priorities
    pub priority_distribution: HashMap<String, usize>,

    /// Distribution of issues by category
    pub issue_distribution: HashMap<String, usize>,

    /// Memory usage statistics
    pub memory_stats: MemoryStats,
}

/// Memory usage statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Peak memory usage in bytes
    pub peak_memory_bytes: usize,

    /// Final memory usage in bytes
    pub final_memory_bytes: usize,

    /// Memory efficiency score
    pub efficiency_score: f64,
}

/// Aggregation methods for [`MemoryStats`].
impl MemoryStats {
    /// Merge memory statistics, keeping worst-case usage and averaging efficiency
    pub fn merge(&mut self, other: MemoryStats) {
        self.peak_memory_bytes = self.peak_memory_bytes.max(other.peak_memory_bytes);
        self.final_memory_bytes = self.final_memory_bytes.max(other.final_memory_bytes);
        self.efficiency_score =
            ((self.efficiency_score + other.efficiency_score) / 2.0).clamp(0.0, 1.0);
    }
}

/// Clone detection and denoising analysis results reported by the API layer
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CloneAnalysisResults {
    /// Whether clone denoising heuristics were enabled for this run
    pub denoising_enabled: bool,

    /// Whether auto-calibration logic was applied (None if unavailable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_calibration_applied: Option<bool>,

    /// Candidate count prior to denoising (None when telemetry unavailable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates_before_denoising: Option<usize>,

    /// Candidate count after denoising and ranking
    pub candidates_after_denoising: usize,

    /// Calibrated threshold reported by the detector (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calibrated_threshold: Option<f64>,

    /// Composite quality score produced by the detector (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_score: Option<f64>,

    /// Average similarity across reported clone pairs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_similarity: Option<f64>,

    /// Maximum similarity observed amongst clone pairs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_similarity: Option<f64>,

    /// Structural verification summary
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<CloneVerificationResults>,

    /// Phase-level filtering statistics (when telemetry captured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_filtering_stats: Option<PhaseFilteringStats>,

    /// Performance metrics for the clone analysis stages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance_metrics: Option<CloneAnalysisPerformance>,

    /// Additional context to explain missing fields or configuration
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,

    /// Reported clone pairs (as emitted by LSH analysis)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clone_pairs: Vec<serde_json::Value>,
}

/// Statistics for filtering performed by each denoising phase
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PhaseFilteringStats {
    /// Phase 1: Weighted shingling results
    pub phase1_weighted_signature: usize,

    /// Phase 2: Structural gate filtering
    pub phase2_structural_gates: usize,

    /// Phase 3: Stop-motifs cache filtering
    pub phase3_stop_motifs_filter: usize,

    /// Phase 4: Payoff ranking results
    pub phase4_payoff_ranking: usize,
}

/// Performance metrics emitted by the clone analysis pipeline
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CloneAnalysisPerformance {
    /// Total analysis time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time_ms: Option<u64>,

    /// Peak memory usage in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_usage_bytes: Option<u64>,

    /// Entities processed per second
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities_per_second: Option<f64>,
}

/// Summary of issues in a directory by category
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DirectoryIssueSummary {
    /// Category name
    pub category: String,
    /// Number of entities with this issue type
    pub affected_entities: usize,
    /// Average severity score for this category
    pub avg_severity: f64,
    /// Maximum severity score for this category
    pub max_severity: f64,
    /// Contribution to overall directory health score
    pub health_impact: f64,
}

/// Health statistics for a specific depth level
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DepthHealthStats {
    /// Directory tree depth (0 = root)
    pub depth: usize,
    /// Number of directories at this depth
    pub directory_count: usize,
    /// Average health score at this depth
    pub avg_health_score: f64,
    /// Minimum health score at this depth
    pub min_health_score: f64,
    /// Maximum health score at this depth
    pub max_health_score: f64,
}

/// A directory identified as a hotspot (low health score)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DirectoryHotspot {
    /// Directory path
    pub path: PathBuf,
    /// Health score
    pub health_score: f64,
    /// Rank among all directories (1 = worst)
    pub rank: usize,
    /// Primary issue category contributing to low health
    pub primary_issue_category: String,
    /// Recommended action
    pub recommendation: String,
}

/// Statistics for the entire directory tree
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TreeStatistics {
    /// Total number of directories
    pub total_directories: usize,
    /// Maximum depth of the directory tree
    pub max_depth: usize,
    /// Average health score across all directories
    pub avg_health_score: f64,
    /// Standard deviation of health scores
    pub health_score_std_dev: f64,
    /// Directories with health scores below threshold (configurable)
    pub hotspot_directories: Vec<DirectoryHotspot>,
    /// Health score distribution by depth level
    pub health_by_depth: HashMap<usize, DepthHealthStats>,
}

/// Health score for a single directory
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DirectoryHealthScore {
    /// Directory path
    pub path: PathBuf,
    /// Health score for this directory (0.0 = poor, 1.0 = excellent)
    pub health_score: f64,
    /// Number of files directly in this directory
    pub file_count: usize,
    /// Number of entities in files directly in this directory
    pub entity_count: usize,
    /// Number of entities needing refactoring in this directory
    pub refactoring_needed: usize,
    /// Number of critical issues in this directory
    pub critical_issues: usize,
    /// Number of high-priority issues in this directory
    pub high_priority_issues: usize,
    /// Average refactoring score for entities in this directory
    pub avg_refactoring_score: f64,
    /// Weight used for aggregation (typically based on entity count or file size)
    pub weight: f64,
    /// Child directory paths
    pub children: Vec<PathBuf>,
    /// Parent directory path (None for root)
    pub parent: Option<PathBuf>,
    /// Breakdown by issue category
    pub issue_categories: HashMap<String, DirectoryIssueSummary>,

    /// Documentation health score for this directory (0.0 = poor, 1.0 = excellent)
    #[serde(default)]
    pub doc_health_score: f64,

    /// Documentation issue count in this directory
    #[serde(default)]
    pub doc_issue_count: usize,
}

/// Hierarchical directory health score tree
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DirectoryHealthTree {
    /// Root directory health scores
    pub root: DirectoryHealthScore,
    /// Mapping of directory paths to their health scores
    pub directories: HashMap<PathBuf, DirectoryHealthScore>,
    /// Statistics for the entire tree
    pub tree_statistics: TreeStatistics,
}

// DirectoryHealthTree implementation is in health_tree.rs

// Normalized types for legacy report compatibility are in normalized_types.rs
pub use super::normalized_types::{
    NormalizedAnalysisResults, NormalizedEntity, NormalizedIssue, NormalizedIssues,
    NormalizedIssueTotals, NormalizedMeta, NormalizedSuggestion, NormalizedSummary,
};
