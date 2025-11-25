//! Analysis results and reporting structures.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::pipeline::StageResultsBundle;
use crate::core::pipeline::{CloneVerificationResults, HealthMetrics};
use crate::core::scoring::Priority;
// use crate::detectors::names::{RenamePack, ContractMismatchPack, ConsistencyIssue};

/// High-level analysis results for public API consumption
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResults {
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

impl CodeDictionary {
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty() && self.suggestions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scoring::Priority;

    fn sample_candidate(path: &str, severity: f64, priority: Priority) -> RefactoringCandidate {
        RefactoringCandidate {
            entity_id: format!("{path}::entity"),
            name: "entity".to_string(),
            file_path: path.to_string(),
            line_range: Some((1, 20)),
            priority,
            score: severity * 20.0,
            confidence: 0.85,
            issues: vec![RefactoringIssue {
                code: "CMPLX".to_string(),
                category: "complexity".to_string(),
                severity,
                contributing_features: vec![FeatureContribution {
                    feature_name: "cyclomatic_complexity".to_string(),
                    value: 18.0,
                    normalized_value: 0.7,
                    contribution: 1.3,
                }],
            }],
            suggestions: vec![RefactoringSuggestion {
                refactoring_type: "extract_method".to_string(),
                code: "XTRMTH".to_string(),
                priority: 0.9,
                effort: 0.4,
                impact: 0.85,
            }],
            issue_count: 1,
            suggestion_count: 1,
        }
    }

    #[test]
    fn code_dictionary_reports_when_empty() {
        let mut dictionary = CodeDictionary::default();
        assert!(dictionary.is_empty());

        dictionary.issues.insert(
            "CMPLX".to_string(),
            CodeDefinition {
                code: "CMPLX".to_string(),
                title: "High Complexity".to_string(),
                summary: "Cyclomatic complexity exceeded target".to_string(),
                category: Some("complexity".to_string()),
            },
        );
        assert!(!dictionary.is_empty());
    }

    #[test]
    fn memory_stats_merge_preserves_extremes_and_averages() {
        let mut base = MemoryStats {
            peak_memory_bytes: 5_000_000,
            final_memory_bytes: 3_000_000,
            efficiency_score: 0.8,
        };
        let other = MemoryStats {
            peak_memory_bytes: 7_500_000,
            final_memory_bytes: 2_000_000,
            efficiency_score: 0.4,
        };

        base.merge(other);
        assert_eq!(base.peak_memory_bytes, 7_500_000);
        assert_eq!(base.final_memory_bytes, 3_000_000);
        assert!((base.efficiency_score - 0.6).abs() < f64::EPSILON);
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

impl DirectoryHealthTree {
    /// Overlay documentation health/issue data onto the tree
    pub fn apply_doc_overlays(
        &mut self,
        doc_scores: &HashMap<String, f64>,
        doc_issues: &HashMap<String, usize>,
    ) {
        for (path, score) in doc_scores {
            let key = PathBuf::from(path);
            if let Some(dir) = self.directories.get_mut(&key) {
                dir.doc_health_score = (*score / 100.0).clamp(0.0, 1.0);
            } else if key == self.root.path {
                self.root.doc_health_score = (*score / 100.0).clamp(0.0, 1.0);
            }
        }

        for (path, issues) in doc_issues {
            let key = PathBuf::from(path);
            if let Some(dir) = self.directories.get_mut(&key) {
                dir.doc_issue_count = *issues;
            } else if key == self.root.path {
                self.root.doc_issue_count = *issues;
            }
        }
    }

    /// Create a minimal directory health tree from refactoring candidates.
    pub fn from_candidates(candidates: &[RefactoringCandidate]) -> Self {
        let file_count = candidates.len();
        let entity_count = candidates.len();
        let refactoring_needed = candidates.len();
        let avg_score = if candidates.is_empty() {
            0.0
        } else {
            candidates.iter().map(|c| c.score).sum::<f64>() / candidates.len() as f64
        };

        let mut root = DirectoryHealthScore {
            path: PathBuf::from("."),
            health_score: if entity_count > 0 {
                (1.0 - (refactoring_needed as f64 / entity_count as f64)).clamp(0.0, 1.0)
            } else {
                1.0
            },
            file_count,
            entity_count,
            refactoring_needed,
            critical_issues: candidates
                .iter()
                .filter(|c| matches!(c.priority, Priority::Critical))
                .count(),
            high_priority_issues: candidates
                .iter()
                .filter(|c| matches!(c.priority, Priority::High | Priority::Critical))
                .count(),
            avg_refactoring_score: avg_score,
            weight: (entity_count as f64).max(1.0),
            children: Vec::new(),
            parent: None,
            issue_categories: HashMap::new(),
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };
        // Build per-directory aggregates
        let mut directories: HashMap<PathBuf, DirectoryHealthScore> = HashMap::new();

        for candidate in candidates {
            let path = PathBuf::from(&candidate.file_path);
            let dir_path = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            let entry = directories
                .entry(dir_path.clone())
                .or_insert(DirectoryHealthScore {
                    path: dir_path.clone(),
                    health_score: 1.0,
                    file_count: 0,
                    entity_count: 0,
                    refactoring_needed: 0,
                    critical_issues: 0,
                    high_priority_issues: 0,
                    avg_refactoring_score: 0.0,
                    weight: 0.0,
                    children: Vec::new(),
                    parent: None,
                    issue_categories: HashMap::new(),
                    doc_health_score: 1.0,
                    doc_issue_count: 0,
                });

            entry.file_count += 1;
            entry.entity_count += 1;
            entry.refactoring_needed += 1;
            entry.critical_issues += usize::from(matches!(candidate.priority, Priority::Critical));
            entry.high_priority_issues += usize::from(matches!(
                candidate.priority,
                Priority::High | Priority::Critical
            ));
            entry.avg_refactoring_score += candidate.score;
            entry.weight += 1.0;
        }

        // Finalize averages and parent/child links
        for entry in directories.values_mut() {
            if entry.entity_count > 0 {
                entry.avg_refactoring_score /= entry.entity_count as f64;
                entry.health_score = (1.0
                    - (entry.refactoring_needed as f64 / entry.entity_count as f64))
                    .clamp(0.0, 1.0);
            } else {
                entry.health_score = 1.0;
            }

            let parent_path = entry
                .path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(PathBuf::from("."));
            entry.parent = Some(parent_path.clone());
        }

        // Populate children vectors
        let keys: Vec<PathBuf> = directories.keys().cloned().collect();
        for dir_path in keys {
            let parent_path = directories
                .get(&dir_path)
                .and_then(|d| d.parent.clone())
                .unwrap_or(PathBuf::from("."));
            if let Some(parent_dir) = directories.get_mut(&parent_path) {
                parent_dir.children.push(dir_path.clone());
            } else if parent_path == PathBuf::from(".") {
                // Attach to root
                root.children.push(dir_path.clone());
            }
        }

        let total_dirs = directories.len() + 1;

        DirectoryHealthTree {
            root: root.clone(),
            directories,
            tree_statistics: TreeStatistics {
                total_directories: total_dirs,
                max_depth: 2,
                avg_health_score: root.health_score,
                health_score_std_dev: 0.0,
                hotspot_directories: Vec::new(),
                health_by_depth: HashMap::new(),
            },
        }
    }

    /// Get the health score for a directory path, defaulting to root.
    pub fn get_health_score(&self, path: &Path) -> f64 {
        if let Some(dir) = self.directories.get(path) {
            dir.health_score
        } else if path == self.root.path {
            self.root.health_score
        } else {
            self.root.health_score
        }
    }

    /// Get all children directories for a given path (empty in minimal tree).
    pub fn get_children(&self, path: &Path) -> Vec<&DirectoryHealthScore> {
        let mut children = Vec::new();

        // Match root
        let path_buf = path.to_path_buf();
        for dir in self.directories.values() {
            if let Some(parent) = &dir.parent {
                if *parent == path_buf {
                    children.push(dir);
                }
            }
        }

        // If asking for root and no directory entries, return empty
        children
    }

    /// Generate a simple tree representation as text.
    pub fn to_tree_string(&self) -> String {
        let mut dirs: Vec<String> = self
            .directories
            .keys()
            .map(|p| p.display().to_string())
            .collect();
        dirs.sort();
        format!(
            "root: {} (health: {:.1}%) dirs: {:?}",
            self.root.path.display(),
            self.root.health_score * 100.0,
            dirs
        )
    }
}

/// Simplified normalized issue used for report compatibility
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormalizedIssue {
    pub code: String,
    pub category: String,
    pub severity: f64,
}

/// Simplified normalized suggestion used for report compatibility
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormalizedSuggestion {
    #[serde(rename = "type")]
    pub refactoring_type: String,
    pub code: String,
    pub priority: f64,
    pub effort: f64,
    pub impact: f64,
}

/// Normalized entity representation for legacy report consumers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEntity {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub line_range: Option<(usize, usize)>,
    pub score: f64,
    #[serde(default = "default_priority_low")]
    pub priority: Priority,
    #[serde(default)]
    pub metrics: Option<serde_json::Value>,
    pub issues: Vec<NormalizedIssue>,
    pub suggestions: Vec<NormalizedSuggestion>,
    #[serde(default)]
    pub issue_count: usize,
    #[serde(default)]
    pub suggestion_count: usize,
}

fn default_priority_low() -> Priority {
    Priority::Low
}

impl Default for NormalizedEntity {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            file_path: None,
            file: None,
            kind: None,
            line_range: None,
            score: 0.0,
            priority: Priority::Low,
            metrics: None,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 0,
            suggestion_count: 0,
        }
    }
}

/// Normalized meta summary used for legacy report structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedMeta {
    pub files_scanned: usize,
    pub entities_analyzed: usize,
    pub code_health: f64,
    pub languages: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub issues: NormalizedIssues,
}

/// Normalized issue counts
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormalizedIssues {
    pub total: usize,
    pub high: usize,
    pub critical: usize,
}

/// Backwards-compatible alias for normalized issue totals
pub type NormalizedIssueTotals = NormalizedIssues;

/// Backwards-compatible alias for normalized meta summary
pub type NormalizedSummary = NormalizedMeta;

impl From<(String, f64)> for NormalizedIssue {
    fn from(value: (String, f64)) -> Self {
        NormalizedIssue {
            code: value.0,
            category: String::new(),
            severity: value.1,
        }
    }
}

impl From<(&str, f64)> for NormalizedIssue {
    fn from(value: (&str, f64)) -> Self {
        NormalizedIssue {
            code: value.0.to_string(),
            category: String::new(),
            severity: value.1,
        }
    }
}

/// Normalized analysis results used by report generator compatibility path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedAnalysisResults {
    pub meta: NormalizedMeta,
    pub entities: Vec<NormalizedEntity>,
    #[serde(default)]
    pub clone: Option<serde_json::Value>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub dictionary: CodeDictionary,
}

impl Default for NormalizedMeta {
    fn default() -> Self {
        Self {
            files_scanned: 0,
            entities_analyzed: 0,
            code_health: 1.0,
            languages: Vec::new(),
            timestamp: Utc::now(),
            issues: NormalizedIssues::default(),
        }
    }
}

impl Default for NormalizedAnalysisResults {
    fn default() -> Self {
        Self {
            meta: NormalizedMeta::default(),
            entities: Vec::new(),
            clone: None,
            warnings: Vec::new(),
            dictionary: CodeDictionary::default(),
        }
    }
}
