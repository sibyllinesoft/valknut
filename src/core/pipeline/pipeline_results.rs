//! Result types and data structures for analysis pipeline outputs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::pipeline_config::AnalysisConfig;
use super::result_types::AnalysisSummary;
use crate::core::featureset::FeatureVector;
use crate::core::scoring::ScoringResult;
use crate::detectors::complexity::ComplexityAnalysisResult;
use crate::detectors::refactoring::RefactoringAnalysisResult;

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
    /// LSH analysis results for clone detection
    pub lsh: LshAnalysisResults,
    /// Coverage analysis results
    pub coverage: CoverageAnalysisResults,
    /// Documentation analysis results
    #[serde(default)]
    pub documentation: DocumentationAnalysisResults,
    /// Overall health metrics
    pub health_metrics: HealthMetrics,
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
    /// Force-directed module dependency graph (nodes + links with layout)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_force_graph: Option<ForceGraph>,
    /// Clone groups
    pub clone_groups: Vec<serde_json::Value>,
    /// Impact issues count
    pub issues_count: usize,
}

/// Force-directed graph payload for module-level dependency visualization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ForceGraph {
    /// Graph nodes with layout coordinates
    pub nodes: Vec<ForceGraphNode>,
    /// Graph edges/links between nodes
    pub links: Vec<ForceGraphLink>,
    /// Graph metadata (layout, pruning, counts)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ForceGraphMetadata>,
}

/// Node entry for a force-directed module graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceGraphNode {
    /// Stable identifier (path-based)
    pub id: String,
    /// Short display label (e.g., file name)
    pub label: String,
    /// Full module path
    pub path: String,
    /// Number of functions contained in the module
    pub functions: usize,
    /// Aggregate incoming dependency weight
    pub fan_in: usize,
    /// Aggregate outgoing dependency weight
    pub fan_out: usize,
    /// Max chokepoint/betweenness score among contained entities
    pub chokepoint_score: f64,
    /// Whether any contained entity participates in a cycle
    pub in_cycle: bool,
    /// Optional size hint for front-end scaling
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// X coordinate from force-directed layout (normalized -1..1)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    /// Y coordinate from force-directed layout (normalized -1..1)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

/// Edge/link entry for a force-directed module graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceGraphLink {
    /// Source node id
    pub source: String,
    /// Target node id
    pub target: String,
    /// Weighted edge count between modules
    pub weight: usize,
}

/// Metadata for a force-directed graph payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceGraphMetadata {
    /// Layout algorithm label
    pub layout: String,
    /// Nodes included in the payload
    pub node_count: usize,
    /// Links included in the payload
    pub edge_count: usize,
    /// Nodes omitted during pruning (if any)
    pub pruned_nodes: usize,
}

/// LSH analysis results for clone detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LshAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Clone detection results
    pub clone_pairs: Vec<serde_json::Value>,
    /// Maximum similarity found
    pub max_similarity: f64,
    /// Average similarity across all comparisons
    pub avg_similarity: f64,
    /// Total potential duplicates found
    pub duplicate_count: usize,
    /// Whether APTED verification was applied
    pub apted_verification_enabled: bool,
    /// Verification summary (e.g. APTED)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<CloneVerificationResults>,
    /// Whether denoise mode was active
    pub denoising_enabled: bool,
    /// TF-IDF statistics (if denoising enabled)
    pub tfidf_stats: Option<TfIdfStats>,
}

/// Summary of structural verification applied to clone pairs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneVerificationResults {
    /// Verification method identifier (e.g. "apted")
    pub method: String,
    /// Total clone pairs produced by LSH
    pub pairs_considered: usize,
    /// Clone pairs where structural verification was attempted
    pub pairs_evaluated: usize,
    /// Clone pairs that produced a verification similarity score
    pub pairs_scored: usize,
    /// Average structural similarity across scored pairs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_similarity: Option<f64>,
}

/// TF-IDF statistics for denoise mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TfIdfStats {
    /// Total k-grams processed
    pub total_grams: usize,
    /// Unique k-grams found
    pub unique_grams: usize,
    /// Top 1% contribution percentage
    pub top1pct_contribution: f64,
}

/// Coverage analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Coverage files discovered and used
    pub coverage_files_used: Vec<CoverageFileInfo>,
    /// Coverage gaps found
    pub coverage_gaps: Vec<serde_json::Value>,
    /// Total number of coverage gaps
    pub gaps_count: usize,
    /// Overall coverage percentage (if calculable)
    pub overall_coverage_percentage: Option<f64>,
    /// Coverage analysis method used
    pub analysis_method: String,
}

/// Documentation analysis results (placeholder until full doc wiring is complete)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentationAnalysisResults {
    /// Whether doc analysis ran
    pub enabled: bool,
    /// Total documentation issues (doc gaps)
    pub issues_count: usize,
    /// Documentation health score (0-100)
    pub doc_health_score: f64,
    /// Per-file documentation health (0-100)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub file_doc_health: HashMap<String, f64>,
    /// Per-file doc issue counts
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub file_doc_issues: HashMap<String, usize>,
    /// Per-directory doc health
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub directory_doc_health: HashMap<String, f64>,
    /// Per-directory doc issue counts
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub directory_doc_issues: HashMap<String, usize>,
}

/// Information about coverage files used in analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageFileInfo {
    /// Path to the coverage file
    pub path: String,
    /// Detected format
    pub format: String,
    /// File size in bytes
    pub size: u64,
    /// Last modified timestamp
    pub modified: String,
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
    /// Documentation health score (0-100, higher is better)
    #[serde(default = "default_health_hundred")]
    pub doc_health_score: f64,
}

fn default_health_hundred() -> f64 {
    100.0
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
    /// Current memory usage in bytes at sampling time
    pub current_memory_bytes: usize,
    /// Peak memory usage in bytes during execution
    pub peak_memory_bytes: usize,
    /// Final memory usage in bytes once execution completed
    pub final_memory_bytes: usize,
    /// Heuristic efficiency score (0.0 - 1.0)
    pub efficiency_score: f64,
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
