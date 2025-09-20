//! Analysis results and reporting structures.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::pipeline::HealthMetrics;
use crate::core::scoring::Priority;
// use crate::detectors::names::{RenamePack, ContractMismatchPack, ConsistencyIssue};

/// High-level analysis results for public API consumption
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResults {
    /// Summary of the analysis
    pub summary: AnalysisSummary,

    /// Detailed results for entities that need refactoring
    pub refactoring_candidates: Vec<RefactoringCandidate>,

    /// Refactoring candidates grouped by file
    pub refactoring_candidates_by_file: Vec<FileRefactoringGroup>,

    /// Analysis statistics
    pub statistics: AnalysisStatistics,

    /// Aggregated health metrics computed by the pipeline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_metrics: Option<HealthMetrics>,

    /// Directory health score tree (hierarchical breakdown)
    pub directory_health_tree: Option<DirectoryHealthTree>,

    /// Code quality analysis results (simple pattern-based analysis)
    // pub naming_results: Option<NamingAnalysisResults>,

    /// Clone detection and denoising analysis results
    pub clone_analysis: Option<CloneAnalysisResults>,

    /// Coverage analysis results - test gap analysis with prioritized packs
    pub coverage_packs: Vec<crate::detectors::coverage::CoveragePack>,

    /// Unified hierarchy for tree-based UI rendering
    pub unified_hierarchy: Vec<serde_json::Value>,

    /// Any warnings or issues encountered
    pub warnings: Vec<String>,
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
    /// Issue category (complexity, structure, etc.)
    pub category: String,

    /// Issue description
    pub description: String,

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

    /// Human-readable description
    pub description: String,

    /// Priority level (0.0-1.0)
    pub priority: f64,

    /// Estimated effort level (0.0-1.0)
    pub effort: f64,

    /// Expected impact (0.0-1.0)
    pub impact: f64,
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

    /// Phase-level filtering statistics (when telemetry captured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_filtering_stats: Option<PhaseFilteringStats>,

    /// Performance metrics for the clone analysis stages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance_metrics: Option<CloneAnalysisPerformance>,

    /// Additional context to explain missing fields or configuration
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
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

/// Hierarchical directory health score tree
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DirectoryHealthTree {
    /// Root directory health scores
    pub root: DirectoryHealthScore,

    /// Mapping of directory paths to their health scores
    pub directories: HashMap<PathBuf, DirectoryHealthScore>,

    /// Statistics for the entire tree
    pub tree_statistics: TreeStatistics,
}

/// Health score for a single directory
#[derive(Debug, Serialize, Deserialize, Clone)]
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
}

/// Summary of issues in a directory by category
#[derive(Debug, Serialize, Deserialize, Clone)]
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

/// Statistics for the entire directory tree
#[derive(Debug, Serialize, Deserialize, Clone)]
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

/// A directory identified as a hotspot (low health score)
#[derive(Debug, Serialize, Deserialize, Clone)]
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

/// Health statistics for a specific depth level
#[derive(Debug, Serialize, Deserialize, Clone)]
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

impl DirectoryHealthTree {
    /// Create directory health tree from refactoring candidates
    pub fn from_candidates(refactoring_candidates: &[RefactoringCandidate]) -> Self {
        use std::collections::{BTreeMap, BTreeSet};
        use std::path::Path;

        // Group refactoring candidates by directory
        let mut directory_data: BTreeMap<PathBuf, Vec<&RefactoringCandidate>> = BTreeMap::new();
        let mut all_directories: BTreeSet<PathBuf> = BTreeSet::new();

        // Extract directories from file paths
        for candidate in refactoring_candidates {
            let file_path = Path::new(&candidate.file_path);
            if let Some(dir_path) = file_path.parent() {
                let dir_path = dir_path.to_path_buf();
                directory_data
                    .entry(dir_path.clone())
                    .or_default()
                    .push(candidate);

                // Add all parent directories, but filter out empty paths
                let mut current = Some(dir_path);
                while let Some(dir) = current {
                    // Only add non-empty paths
                    if !dir.as_os_str().is_empty() {
                        all_directories.insert(dir.clone());
                    }
                    current = dir
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .map(|p| p.to_path_buf());
                }
            }
        }

        // Handle case where no candidates exist - use current directory
        if all_directories.is_empty() {
            all_directories.insert(PathBuf::from("."));
        }

        // Build directory scores
        let mut directories = HashMap::new();
        let mut depth_stats: HashMap<usize, DepthHealthStats> = HashMap::new();

        for dir in &all_directories {
            let dir_candidates = directory_data.get(dir).map(|v| v.as_slice()).unwrap_or(&[]);

            // Calculate directory health score
            let (total_issues, health_score) = if dir_candidates.is_empty() {
                // For directories without direct candidates, check if they have children with issues
                let has_children_with_issues = directory_data
                    .keys()
                    .any(|path| path.starts_with(dir) && path != dir);

                if has_children_with_issues {
                    (0, 0.8) // Indirect issues
                } else {
                    (0, 1.0) // No issues
                }
            } else {
                let total_issues = dir_candidates.len();
                let avg_score =
                    dir_candidates.iter().map(|c| c.confidence).sum::<f64>() / total_issues as f64;
                (total_issues, 1.0 - (avg_score * 0.5)) // Simple health calculation
            };

            let depth = dir.components().count();

            // Update depth statistics
            let depth_stat = depth_stats
                .entry(depth)
                .or_insert_with(|| DepthHealthStats {
                    depth,
                    directory_count: 0,
                    avg_health_score: 0.0,
                    min_health_score: 1.0,
                    max_health_score: 0.0,
                });

            depth_stat.directory_count += 1;
            depth_stat.avg_health_score += health_score;
            depth_stat.min_health_score = depth_stat.min_health_score.min(health_score);
            depth_stat.max_health_score = depth_stat.max_health_score.max(health_score);

            // Create issue categories
            let mut issue_categories: HashMap<String, DirectoryIssueSummary> = HashMap::new();
            for candidate in dir_candidates {
                for issue in &candidate.issues {
                    let summary = issue_categories
                        .entry(issue.category.clone())
                        .or_insert_with(|| DirectoryIssueSummary {
                            category: issue.category.clone(),
                            affected_entities: 0,
                            avg_severity: 0.0,
                            max_severity: 0.0,
                            health_impact: 0.0,
                        });

                    summary.affected_entities += 1;
                    summary.max_severity = summary.max_severity.max(issue.severity);
                    summary.avg_severity += issue.severity;
                    summary.health_impact += issue.severity * 0.1; // Simple calculation
                }
            }

            // Finalize averages
            for summary in issue_categories.values_mut() {
                if summary.affected_entities > 0 {
                    summary.avg_severity /= summary.affected_entities as f64;
                }
            }

            // Create directory health score
            let dir_health = DirectoryHealthScore {
                path: dir.clone(),
                health_score,
                file_count: dir_candidates.len(),
                entity_count: dir_candidates.len(),
                refactoring_needed: dir_candidates.len(),
                critical_issues: dir_candidates
                    .iter()
                    .flat_map(|c| &c.issues)
                    .filter(|issue| issue.severity >= 2.0)
                    .count(),
                high_priority_issues: dir_candidates
                    .iter()
                    .flat_map(|c| &c.issues)
                    .filter(|issue| issue.severity >= 1.5)
                    .count(),
                avg_refactoring_score: if dir_candidates.is_empty() {
                    0.0
                } else {
                    dir_candidates.iter().map(|c| c.score).sum::<f64>()
                        / dir_candidates.len() as f64
                },
                weight: 1.0,
                children: vec![], // Will be populated below
                parent: dir.parent().map(|p| p.to_path_buf()),
                issue_categories,
            };

            directories.insert(dir.clone(), dir_health);
        }

        // Finalize depth statistics
        for depth_stat in depth_stats.values_mut() {
            depth_stat.avg_health_score /= depth_stat.directory_count as f64;
        }

        // Set up parent-child relationships
        let mut directories = directories;
        for dir in &all_directories {
            let children: Vec<PathBuf> = all_directories
                .iter()
                .filter(|other_dir| other_dir.parent() == Some(dir.as_path()))
                .cloned()
                .collect();

            if let Some(dir_score) = directories.get_mut(dir) {
                dir_score.children = children;
            }
        }

        // Find root directory
        let root_path = all_directories
            .iter()
            .min_by_key(|p| p.components().count())
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));

        let root = directories
            .get(&root_path)
            .cloned()
            .unwrap_or_else(|| DirectoryHealthScore {
                path: root_path,
                health_score: 1.0,
                file_count: 0,
                entity_count: 0,
                refactoring_needed: 0,
                critical_issues: 0,
                high_priority_issues: 0,
                avg_refactoring_score: 0.0,
                weight: 1.0,
                children: directories.keys().cloned().collect(),
                parent: None,
                issue_categories: HashMap::new(),
            });

        let tree_statistics = TreeStatistics {
            total_directories: directories.len(),
            max_depth: 1,
            avg_health_score: if directories.is_empty() {
                1.0
            } else {
                directories.values().map(|d| d.health_score).sum::<f64>() / directories.len() as f64
            },
            health_score_std_dev: 0.1,
            hotspot_directories: vec![],
            health_by_depth: depth_stats,
        };

        DirectoryHealthTree {
            root,
            directories,
            tree_statistics,
        }
    }

    /// Get the health score for a directory path, traversing up the hierarchy if not found
    pub fn get_health_score(&self, path: &Path) -> f64 {
        if let Some(dir) = self.directories.get(path) {
            return dir.health_score;
        }

        // Try parent directories
        let mut current = path.parent();
        while let Some(parent) = current {
            if let Some(dir) = self.directories.get(parent) {
                return dir.health_score;
            }
            current = parent.parent();
        }

        // Default to root health score
        self.root.health_score
    }

    /// Get all children directories for a given path
    pub fn get_children(&self, path: &Path) -> Vec<&DirectoryHealthScore> {
        let path_buf = path.to_path_buf();
        self.directories
            .values()
            .filter(|dir| dir.parent.as_ref() == Some(&path_buf))
            .collect()
    }

    /// Generate a tree representation as text
    pub fn to_tree_string(&self) -> String {
        let mut result = String::new();
        self.append_directory_tree(&mut result, &self.root, 0);
        result
    }

    fn append_directory_tree(&self, result: &mut String, dir: &DirectoryHealthScore, depth: usize) {
        let indent = "  ".repeat(depth);
        let health_indicator = if dir.health_score >= 0.8 {
            "✓"
        } else if dir.health_score >= 0.6 {
            "!"
        } else {
            "⚠"
        };

        result.push_str(&format!(
            "{}{} {} (health: {:.1}%)\n",
            indent,
            health_indicator,
            dir.path.display(),
            dir.health_score * 100.0
        ));

        // Add children
        let mut children: Vec<_> = dir
            .children
            .iter()
            .filter_map(|child_path| self.directories.get(child_path))
            .collect();
        children.sort_by(|a, b| a.path.cmp(&b.path));

        for child in children {
            self.append_directory_tree(result, child, depth + 1);
        }
    }
}
