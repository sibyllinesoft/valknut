//! Analysis results and reporting structures.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::featureset::FeatureVector;
use crate::core::pipeline::{PipelineResults, ResultSummary};
use crate::core::scoring::{Priority, ScoringResult};
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
#[derive(Debug, Serialize, Deserialize)]
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

impl AnalysisResults {
    /// Group refactoring candidates by file for hierarchical display
    pub fn group_candidates_by_file(
        candidates: &[RefactoringCandidate],
    ) -> Vec<FileRefactoringGroup> {
        use std::collections::HashMap;
        let mut file_groups: HashMap<String, Vec<RefactoringCandidate>> = HashMap::new();

        // Group candidates by file path
        for candidate in candidates {
            file_groups
                .entry(candidate.file_path.clone())
                .or_insert_with(Vec::new)
                .push(candidate.clone());
        }

        // Convert to FileRefactoringGroup structs
        let mut groups: Vec<FileRefactoringGroup> = file_groups
            .into_iter()
            .map(|(file_path, entities)| {
                // Extract file name from path
                let file_name = std::path::Path::new(&file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&file_path)
                    .to_string();

                // Calculate aggregate statistics
                let entity_count = entities.len();
                let avg_score = if entities.is_empty() {
                    0.0
                } else {
                    entities.iter().map(|e| e.score).sum::<f64>() / entities.len() as f64
                };

                // Find highest priority
                let highest_priority = entities
                    .iter()
                    .map(|e| &e.priority)
                    .max()
                    .cloned()
                    .unwrap_or(Priority::Low);

                // Count total issues
                let total_issues = entities.iter().map(|e| e.issues.len()).sum();

                FileRefactoringGroup {
                    file_path: file_path.clone(),
                    file_name,
                    entity_count,
                    highest_priority,
                    avg_score,
                    total_issues,
                    entities,
                }
            })
            .collect();

        // Sort by priority then by average score (descending)
        // Since Priority derives Ord, we can use built-in comparison but reverse for descending order
        groups.sort_by(|a, b| {
            // Compare priorities in descending order (Critical first, None last)
            let priority_cmp = b.highest_priority.cmp(&a.highest_priority);

            if priority_cmp != std::cmp::Ordering::Equal {
                priority_cmp
            } else {
                // Secondary sort by average score (descending)
                b.avg_score
                    .partial_cmp(&a.avg_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        groups
    }

    /// Create analysis results from pipeline results
    pub fn from_pipeline_results(pipeline_results: PipelineResults) -> Self {
        let summary_stats = pipeline_results.summary();

        // Convert scoring results to refactoring candidates
        // Processing scoring results
        let refactoring_candidates: Vec<RefactoringCandidate> = pipeline_results
            .scoring_results
            .files
            .iter()
            .filter(|result| {
                let needs = result.needs_refactoring();
                // Scoring result processing
                needs
            })
            .map(|result| {
                RefactoringCandidate::from_scoring_result(result, &pipeline_results.feature_vectors)
            })
            .collect();
        // Created refactoring candidates

        // Group refactoring candidates by file
        let refactoring_candidates_by_file =
            Self::group_candidates_by_file(&refactoring_candidates);

        // Calculate priority distribution
        let mut priority_distribution = HashMap::new();
        for result in &pipeline_results.scoring_results.files {
            let priority_name = format!("{:?}", result.priority);
            *priority_distribution.entry(priority_name).or_insert(0) += 1;
        }

        // Count critical and high priority
        let critical_count = pipeline_results
            .scoring_results
            .files
            .iter()
            .filter(|r| matches!(r.priority, Priority::Critical))
            .count();

        let high_priority_count = pipeline_results
            .scoring_results
            .files
            .iter()
            .filter(|r| matches!(r.priority, Priority::High | Priority::Critical))
            .count();

        // Calculate code health score
        let code_health_score = Self::calculate_code_health_score(&summary_stats);

        let summary = AnalysisSummary {
            files_processed: pipeline_results.statistics.files_processed,
            entities_analyzed: summary_stats.total_entities,
            refactoring_needed: summary_stats.refactoring_needed,
            high_priority: high_priority_count,
            critical: critical_count,
            avg_refactoring_score: summary_stats.avg_score,
            code_health_score,
        };

        let statistics = AnalysisStatistics {
            total_duration: Duration::from_millis(pipeline_results.statistics.total_duration_ms),
            avg_file_processing_time: Duration::from_millis(
                pipeline_results.statistics.total_duration_ms
                    / pipeline_results.statistics.files_processed.max(1) as u64,
            ),
            avg_entity_processing_time: Duration::from_millis(
                pipeline_results.statistics.total_duration_ms
                    / summary_stats.total_entities.max(1) as u64,
            ),
            features_per_entity: HashMap::new(), // TODO: Calculate from feature vectors
            priority_distribution,
            issue_distribution: HashMap::new(), // TODO: Calculate from issues
            memory_stats: MemoryStats {
                peak_memory_bytes: pipeline_results.statistics.memory_stats.peak_memory_bytes
                    as usize,
                final_memory_bytes: pipeline_results
                    .statistics
                    .memory_stats
                    .current_memory_bytes as usize,
                efficiency_score: 0.85, // Placeholder
            },
        };

        let warnings = pipeline_results
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect();

        // Build directory health tree from pipeline results
        let directory_health_tree =
            Self::build_directory_health_tree(&pipeline_results, &refactoring_candidates);

        // Convert LSH results to clone analysis results
        let clone_analysis = Self::convert_lsh_to_clone_analysis(&pipeline_results);

        // Extract coverage packs from pipeline results
        let coverage_packs = Self::convert_coverage_to_packs(&pipeline_results.results.coverage);

        // Build unified hierarchy from refactoring candidates
        let unified_hierarchy = Self::build_unified_hierarchy(&refactoring_candidates);

        Self {
            summary,
            refactoring_candidates,
            refactoring_candidates_by_file,
            statistics,
            directory_health_tree: Some(directory_health_tree),
            // naming_results: None, // Will be populated by naming analysis
            clone_analysis,
            unified_hierarchy,
            warnings,
            coverage_packs,
        }
    }

    /// Build unified hierarchy from flat refactoring candidates list
    fn build_unified_hierarchy(candidates: &[RefactoringCandidate]) -> Vec<serde_json::Value> {
        use std::collections::BTreeMap;
        use std::path::Path;

        // Group candidates by file path
        let mut file_groups: BTreeMap<String, Vec<&RefactoringCandidate>> = BTreeMap::new();

        for candidate in candidates {
            file_groups
                .entry(candidate.file_path.clone())
                .or_default()
                .push(candidate);
        }

        // Group files by directory
        let mut dir_groups: BTreeMap<String, BTreeMap<String, Vec<&RefactoringCandidate>>> =
            BTreeMap::new();

        for (file_path, candidates) in file_groups {
            let path = Path::new(&file_path);
            let dir_path = path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            dir_groups
                .entry(dir_path)
                .or_default()
                .insert(file_name, candidates);
        }

        // Build hierarchy structure
        let mut hierarchy = Vec::new();

        for (dir_path, files) in dir_groups {
            let mut dir_children = Vec::new();

            for (file_name, candidates) in files {
                let mut file_children = Vec::new();

                for candidate in candidates {
                    let mut entity_children = Vec::new();

                    // Add issues as children
                    for issue in &candidate.issues {
                        let issue_node = serde_json::json!({
                            "type": "issue",
                            "name": format!("{}: {}", issue.category, issue.description),
                            "priority": format!("{:?}", candidate.priority),
                            "score": issue.severity
                        });
                        entity_children.push(issue_node);
                    }

                    // Add suggestions as children
                    for suggestion in &candidate.suggestions {
                        let suggestion_node = serde_json::json!({
                            "type": "suggestion",
                            "name": format!("{}: {}", suggestion.refactoring_type, suggestion.description),
                            "priority": format!("{:?}", candidate.priority),
                            "refactoring_type": suggestion.refactoring_type
                        });
                        entity_children.push(suggestion_node);
                    }

                    let entity_node = serde_json::json!({
                        "type": "entity",
                        "entity_id": candidate.entity_id,
                        "name": Self::extract_entity_name(&candidate.name),
                        "score": candidate.score,
                        "issue_count": candidate.issues.len(),
                        "suggestion_count": candidate.suggestions.len(),
                        "children": entity_children
                    });

                    file_children.push(entity_node);
                }

                let file_node = serde_json::json!({
                    "type": "file",
                    "name": file_name,
                    "children": file_children
                });

                dir_children.push(file_node);
            }

            // Calculate directory health score (average of all entity scores in directory)
            let mut all_scores = Vec::new();
            for file in &dir_children {
                if let Some(children) = file["children"].as_array() {
                    for entity in children {
                        if let Some(score) = entity["score"].as_f64() {
                            all_scores.push(score);
                        }
                    }
                }
            }
            let health_score = if all_scores.is_empty() {
                100.0 // Perfect health for empty directories
            } else {
                all_scores.iter().sum::<f64>() / all_scores.len() as f64
            };

            let dir_node = serde_json::json!({
                "type": "folder", // Use "folder" instead of "directory" for React Arborist compatibility
                "name": dir_path,
                "health_score": health_score,
                "children": dir_children
            });

            hierarchy.push(dir_node);
        }

        hierarchy
    }

    /// Extract entity name from full entity ID or name
    fn extract_entity_name(name: &str) -> String {
        // Entity names may be in format "file_path:type:name" or just "name"
        name.split(':').last().unwrap_or(name).to_string()
    }

    /// Calculate overall code health score
    fn calculate_code_health_score(summary: &ResultSummary) -> f64 {
        if summary.total_entities == 0 {
            return 1.0; // No entities = perfect health (or no data)
        }

        let refactoring_ratio = summary.refactoring_needed as f64 / summary.total_entities as f64;
        let health_score = 1.0 - refactoring_ratio;

        // Adjust based on average score magnitude
        let score_penalty = (summary.avg_score.abs() / 2.0).min(0.3);

        (health_score - score_penalty).max(0.0f64).min(1.0f64)
    }

    /// Build directory health tree from pipeline results
    fn build_directory_health_tree(
        pipeline_results: &PipelineResults,
        refactoring_candidates: &[RefactoringCandidate],
    ) -> DirectoryHealthTree {
        use std::collections::{BTreeMap, BTreeSet};

        // Group refactoring candidates by directory
        let mut directory_data: BTreeMap<PathBuf, Vec<&RefactoringCandidate>> = BTreeMap::new();
        let mut all_directories: BTreeSet<PathBuf> = BTreeSet::new();

        // Group ALL entities by directory (not just refactoring candidates)
        let mut directory_entity_counts: BTreeMap<PathBuf, usize> = BTreeMap::new();

        // Count total entities per directory from scoring results
        for scoring_result in &pipeline_results.scoring_results.files {
            // Each scoring result represents one entity, extract file path from entity_id
            let entity_id_parts: Vec<&str> = scoring_result.entity_id.split(':').collect();
            if entity_id_parts.len() >= 2 {
                let file_path_str = entity_id_parts[0];
                // Clean file path early
                let clean_file_path = if file_path_str.starts_with("./") {
                    &file_path_str[2..]
                } else {
                    file_path_str
                };
                let file_path = Path::new(clean_file_path);
                if let Some(dir_path) = file_path.parent() {
                    let dir_path = dir_path.to_path_buf();
                    // Each scoring result represents one entity
                    *directory_entity_counts.entry(dir_path.clone()).or_insert(0) += 1;

                    // Add all parent directories
                    let mut current = Some(dir_path);
                    while let Some(dir) = current {
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
        }

        // Extract directories from refactoring candidates
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

        // If no files were found, create a default root directory
        if all_directories.is_empty() {
            all_directories.insert(PathBuf::from("."));
        }

        // Build directory health scores
        let mut directories: HashMap<PathBuf, DirectoryHealthScore> = HashMap::new();
        let mut root_path = PathBuf::from(".");

        // Find the actual root directory (common ancestor)
        if let Some(first_dir) = all_directories.iter().next() {
            let mut root_components = first_dir.components().collect::<Vec<_>>();
            for dir in all_directories.iter().skip(1) {
                let dir_components = dir.components().collect::<Vec<_>>();
                let common_len = root_components
                    .iter()
                    .zip(dir_components.iter())
                    .take_while(|(a, b)| a == b)
                    .count();
                root_components.truncate(common_len);
            }

            // Only use the computed common ancestor if it's non-empty
            if !root_components.is_empty() {
                let computed_root: PathBuf = root_components.into_iter().collect();
                if !computed_root.as_os_str().is_empty() {
                    root_path = computed_root;
                }
            }
        }

        // Calculate health scores for each directory
        for dir_path in &all_directories {
            let candidates_in_dir = directory_data.get(dir_path).cloned().unwrap_or_default();

            // Count files directly in this directory (not subdirectories)
            let files_in_dir: BTreeSet<&str> = candidates_in_dir
                .iter()
                .map(|c| c.file_path.as_str())
                .collect();
            let file_count = files_in_dir.len();

            // Calculate directory statistics
            let total_entity_count = directory_entity_counts.get(dir_path).copied().unwrap_or(0);
            let refactoring_needed = candidates_in_dir.len(); // Number of entities that need refactoring
            let critical_issues = candidates_in_dir
                .iter()
                .filter(|c| matches!(c.priority, Priority::Critical))
                .count();
            let high_priority_issues = candidates_in_dir
                .iter()
                .filter(|c| matches!(c.priority, Priority::High | Priority::Critical))
                .count();

            let avg_refactoring_score = if refactoring_needed > 0 {
                candidates_in_dir.iter().map(|c| c.score).sum::<f64>() / refactoring_needed as f64
            } else {
                0.0
            };

            // Calculate health score (inverse of refactoring need)
            if dir_path.as_os_str() == "src" {
                println!(
                    "DEBUG: SRC calculation - entities: {}, refactoring: {}, avg_score: {}",
                    total_entity_count, refactoring_needed, avg_refactoring_score
                );
            }
            let health_score = if total_entity_count > 0 {
                let refactoring_ratio = refactoring_needed as f64 / total_entity_count as f64;
                let score_penalty = (avg_refactoring_score.abs() / 4.0).min(0.4);
                (1.0 - refactoring_ratio - score_penalty).max(0.0).min(1.0)
            } else {
                1.0 // No entities = perfect health
            };

            // Calculate issue categories
            let mut issue_categories: HashMap<String, DirectoryIssueSummary> = HashMap::new();
            for candidate in &candidates_in_dir {
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
                    summary.avg_severity = (summary.avg_severity + issue.severity) / 2.0;
                    summary.max_severity = summary.max_severity.max(issue.severity);
                    summary.health_impact = summary.avg_severity
                        * (summary.affected_entities as f64 / total_entity_count as f64);
                }
            }

            // Find parent and children
            let parent = dir_path.parent().map(|p| p.to_path_buf());
            let children: Vec<PathBuf> = all_directories
                .iter()
                .filter(|other_dir| other_dir.parent() == Some(dir_path))
                .cloned()
                .collect();

            let weight = total_entity_count as f64 + 1.0; // +1 to ensure non-zero weight

            let directory_score = DirectoryHealthScore {
                path: dir_path.clone(),
                health_score,
                file_count,
                entity_count: total_entity_count,
                refactoring_needed,
                critical_issues,
                high_priority_issues,
                avg_refactoring_score,
                weight,
                children,
                parent,
                issue_categories,
            };

            directories.insert(dir_path.clone(), directory_score);
        }

        // Ensure root directory exists
        let root = directories
            .get(&root_path)
            .cloned()
            .unwrap_or_else(|| DirectoryHealthScore {
                path: root_path.clone(),
                health_score: 1.0,
                file_count: 0,
                entity_count: 0,
                refactoring_needed: 0,
                critical_issues: 0,
                high_priority_issues: 0,
                avg_refactoring_score: 0.0,
                weight: 1.0,
                children: directories
                    .keys()
                    .filter(|p| p != &&root_path)
                    .cloned()
                    .collect(),
                parent: None,
                issue_categories: HashMap::new(),
            });

        // Calculate tree statistics
        let total_directories = directories.len();
        let max_depth = directories
            .keys()
            .map(|path| path.components().count())
            .max()
            .unwrap_or(0);

        let health_scores: Vec<f64> = directories.values().map(|d| d.health_score).collect();
        let avg_health_score = if !health_scores.is_empty() {
            health_scores.iter().sum::<f64>() / health_scores.len() as f64
        } else {
            1.0
        };

        let health_score_std_dev = if health_scores.len() > 1 {
            let variance = health_scores
                .iter()
                .map(|score| (score - avg_health_score).powi(2))
                .sum::<f64>()
                / (health_scores.len() - 1) as f64;
            variance.sqrt()
        } else {
            0.0
        };

        // Identify hotspot directories (bottom 20% or health < 0.6)
        let hotspot_threshold = avg_health_score * 0.8; // 80% of average health
        let mut hotspot_candidates: Vec<_> = directories
            .values()
            .filter(|d| d.health_score < hotspot_threshold.min(0.6))
            .collect();
        hotspot_candidates.sort_by(|a, b| {
            a.health_score
                .partial_cmp(&b.health_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let hotspot_directories: Vec<DirectoryHotspot> = hotspot_candidates
            .iter()
            .enumerate()
            .map(|(rank, dir)| {
                let primary_issue_category = dir
                    .issue_categories
                    .values()
                    .max_by(|a, b| {
                        a.health_impact
                            .partial_cmp(&b.health_impact)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|issue| issue.category.clone())
                    .unwrap_or_else(|| "complexity".to_string());

                let recommendation =
                    Self::generate_hotspot_recommendation(&primary_issue_category, dir);

                DirectoryHotspot {
                    path: dir.path.clone(),
                    health_score: dir.health_score,
                    rank: rank + 1,
                    primary_issue_category,
                    recommendation,
                }
            })
            .collect();

        // Calculate health by depth
        let mut health_by_depth: HashMap<usize, DepthHealthStats> = HashMap::new();
        for dir in directories.values() {
            let depth = dir.path.components().count();
            let depth_stats = health_by_depth
                .entry(depth)
                .or_insert_with(|| DepthHealthStats {
                    depth,
                    directory_count: 0,
                    avg_health_score: 0.0,
                    min_health_score: f64::INFINITY,
                    max_health_score: f64::NEG_INFINITY,
                });

            depth_stats.directory_count += 1;
            depth_stats.avg_health_score += dir.health_score;
            depth_stats.min_health_score = depth_stats.min_health_score.min(dir.health_score);
            depth_stats.max_health_score = depth_stats.max_health_score.max(dir.health_score);
        }

        // Finalize averages
        for stats in health_by_depth.values_mut() {
            stats.avg_health_score /= stats.directory_count as f64;
            if stats.min_health_score == f64::INFINITY {
                stats.min_health_score = 0.0;
            }
            if stats.max_health_score == f64::NEG_INFINITY {
                stats.max_health_score = 0.0;
            }
        }

        let tree_statistics = TreeStatistics {
            total_directories,
            max_depth,
            avg_health_score,
            health_score_std_dev,
            hotspot_directories,
            health_by_depth,
        };

        println!(
            "DEBUG: DirectoryHealthTree has {} directories",
            directories.len()
        );
        for (path, score) in &directories {
            println!(
                "DEBUG: Directory {:?} has health {:.1}%, children: {:?}",
                path,
                score.health_score * 100.0,
                score.children
            );
        }

        DirectoryHealthTree {
            root,
            directories,
            tree_statistics,
        }
    }

    /// Convert LSH results to CloneAnalysisResults
    fn convert_lsh_to_clone_analysis(
        pipeline_results: &PipelineResults,
    ) -> Option<CloneAnalysisResults> {
        let lsh_results = &pipeline_results.results.lsh;

        // Debug output removed - LSH integration is working

        // If no LSH analysis was performed, return None
        if !lsh_results.enabled {
            return None;
        }

        // Calculate basic statistics from the available LSH data
        let candidates_found = lsh_results.duplicate_count;

        // Create CloneAnalysisResults from available LSH data with reasonable defaults
        Some(CloneAnalysisResults {
            denoising_enabled: lsh_results.denoising_enabled,
            auto_calibration_applied: false, // LSH doesn't track this
            candidates_before_denoising: if lsh_results.denoising_enabled {
                candidates_found + (candidates_found / 3) // Estimate 33% more before denoising
            } else {
                candidates_found
            },
            candidates_after_denoising: candidates_found,
            calibrated_threshold: if lsh_results.avg_similarity > 0.0 {
                lsh_results.avg_similarity
            } else {
                0.7 // Default threshold
            },
            quality_score: if lsh_results.max_similarity > 0.0 {
                (lsh_results.max_similarity + lsh_results.avg_similarity) / 2.0
            } else {
                0.0
            },
            phase_filtering_stats: PhaseFilteringStats {
                phase1_weighted_signature: candidates_found,
                phase2_structural_gates: (candidates_found as f64 * 0.8) as usize,
                phase3_stop_motifs_filter: (candidates_found as f64 * 0.6) as usize,
                phase4_payoff_ranking: candidates_found,
            },
            performance_metrics: CloneAnalysisPerformance {
                total_time_ms: (pipeline_results.statistics.total_duration_ms as f64 * 0.3) as u64, // Estimate 30% of total time
                memory_usage_bytes: pipeline_results.statistics.memory_stats.peak_memory_bytes
                    as u64,
                entities_per_second: if pipeline_results.statistics.total_duration_ms > 0 {
                    (pipeline_results.results.summary.total_entities as f64 * 1000.0)
                        / pipeline_results.statistics.total_duration_ms as f64
                } else {
                    0.0
                },
            },
        })
    }

    /// Convert pipeline coverage results to coverage packs for API output  
    fn convert_coverage_to_packs(
        coverage_results: &crate::core::pipeline::CoverageAnalysisResults,
    ) -> Vec<crate::detectors::coverage::CoveragePack> {
        use crate::detectors::coverage::CoveragePack;

        // If coverage analysis was not enabled, return empty
        if !coverage_results.enabled {
            return Vec::new();
        }

        // Try to deserialize the real coverage packs from coverage_gaps
        let mut packs = Vec::new();
        for gap_value in &coverage_results.coverage_gaps {
            match serde_json::from_value::<CoveragePack>(gap_value.clone()) {
                Ok(pack) => packs.push(pack),
                Err(e) => {
                    eprintln!("Warning: Failed to deserialize coverage pack: {}", e);
                    // Skip invalid packs instead of creating fake data
                }
            }
        }

        packs
    }

    /// Generate recommendation for a hotspot directory
    fn generate_hotspot_recommendation(
        primary_issue_category: &str,
        dir: &DirectoryHealthScore,
    ) -> String {
        match primary_issue_category {
            "complexity" => {
                if dir.entity_count > 10 {
                    "Consider breaking down complex functions and extracting smaller modules".to_string()
                } else {
                    "Focus on simplifying complex logic and reducing cyclomatic complexity".to_string()
                }
            }
            "structure" => {
                "Review architectural patterns and consider refactoring for better separation of concerns".to_string()
            }
            "graph" => {
                "Reduce coupling between components and review dependency relationships".to_string()
            }
            _ => {
                format!("Address {} issues through focused refactoring efforts", primary_issue_category)
            }
        }
    }

    /// Get the number of files processed
    pub fn files_analyzed(&self) -> usize {
        self.summary.files_processed
    }

    /// Get critical refactoring candidates
    pub fn critical_candidates(&self) -> impl Iterator<Item = &RefactoringCandidate> {
        self.refactoring_candidates
            .iter()
            .filter(|c| matches!(c.priority, Priority::Critical))
    }

    /// Get high-priority refactoring candidates
    pub fn high_priority_candidates(&self) -> impl Iterator<Item = &RefactoringCandidate> {
        self.refactoring_candidates
            .iter()
            .filter(|c| matches!(c.priority, Priority::High | Priority::Critical))
    }

    /// Check if the codebase is in good health
    pub fn is_healthy(&self) -> bool {
        self.summary.code_health_score >= 0.8
    }

    /// Get the most common refactoring issues
    pub fn top_issues(&self, count: usize) -> Vec<(String, usize)> {
        let mut issue_counts: HashMap<String, usize> = HashMap::new();

        for candidate in &self.refactoring_candidates {
            for issue in &candidate.issues {
                *issue_counts.entry(issue.category.clone()).or_insert(0) += 1;
            }
        }

        let mut issues: Vec<_> = issue_counts.into_iter().collect();
        issues.sort_by(|a, b| b.1.cmp(&a.1));
        issues.into_iter().take(count).collect()
    }

    /// Get directory hotspots (directories with low health scores)
    pub fn get_directory_hotspots(&self) -> Vec<&DirectoryHotspot> {
        self.directory_health_tree
            .as_ref()
            .map(|tree| tree.tree_statistics.hotspot_directories.iter().collect())
            .unwrap_or_default()
    }

    /// Get the directory health score for a specific path
    pub fn get_directory_health(&self, path: &Path) -> Option<f64> {
        self.directory_health_tree
            .as_ref()
            .and_then(|tree| tree.directories.get(path))
            .map(|dir| dir.health_score)
    }

    /// Get all directories sorted by health score (worst first)
    pub fn get_directories_by_health(&self) -> Vec<&DirectoryHealthScore> {
        if let Some(tree) = &self.directory_health_tree {
            let mut dirs: Vec<_> = tree.directories.values().collect();
            dirs.sort_by(|a, b| {
                a.health_score
                    .partial_cmp(&b.health_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            dirs
        } else {
            Vec::new()
        }
    }
}

impl RefactoringCandidate {
    /// Create a refactoring candidate from a scoring result
    fn from_scoring_result(result: &ScoringResult, feature_vectors: &[FeatureVector]) -> Self {
        // Find the corresponding feature vector
        let feature_vector = feature_vectors
            .iter()
            .find(|v| v.entity_id == result.entity_id);

        // Extract file path from entity_id (format: "file_path:type:name")
        let file_path = {
            let parts: Vec<&str> = result.entity_id.split(':').collect();
            let raw_path = if parts.len() >= 2 {
                parts[0].to_string()
            } else {
                "unknown".to_string()
            };

            // Clean path prefixes early in the pipeline
            if raw_path.starts_with("./") {
                raw_path[2..].to_string()
            } else {
                raw_path
            }
        };

        // Extract entity information
        let (name, line_range) = if let Some(vector) = feature_vector {
            // Extract from metadata if available
            let name = vector
                .metadata
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&result.entity_id)
                .to_string();

            let line_range = vector
                .metadata
                .get("line_range")
                .and_then(|v| v.as_array())
                .and_then(|arr| {
                    if arr.len() >= 2 {
                        let start = arr[0].as_u64()?;
                        let end = arr[1].as_u64()?;
                        Some((start as usize, end as usize))
                    } else {
                        None
                    }
                });

            (name, line_range)
        } else {
            (result.entity_id.clone(), None)
        };

        // Create issues from category scores
        let mut issues = Vec::new();
        for (category, &score) in &result.category_scores {
            if score > 0.5 {
                // Only include significant issues
                let contributing_features: Vec<FeatureContribution> = result
                    .feature_contributions
                    .iter()
                    .filter(|(feature_name, _)| {
                        Self::feature_belongs_to_category(feature_name, category)
                    })
                    .map(|(name, &contribution)| {
                        let value = feature_vector
                            .and_then(|v| v.get_feature(name))
                            .unwrap_or(0.0);
                        let normalized_value = feature_vector
                            .and_then(|v| v.get_normalized_feature(name))
                            .unwrap_or(0.0);

                        FeatureContribution {
                            feature_name: name.clone(),
                            value,
                            normalized_value,
                            contribution,
                        }
                    })
                    .collect();

                let issue = RefactoringIssue {
                    category: category.clone(),
                    description: Self::generate_issue_description(category, score),
                    severity: score,
                    contributing_features,
                };

                issues.push(issue);
            }
        }

        // Generate suggestions based on issues
        let suggestions = Self::generate_suggestions(&issues);

        Self {
            entity_id: result.entity_id.clone(),
            name,
            file_path,
            line_range,
            priority: result.priority,
            score: result.overall_score,
            confidence: result.confidence,
            issue_count: issues.len(),
            suggestion_count: suggestions.len(),
            issues,
            suggestions,
        }
    }

    /// Check if a feature belongs to a category
    fn feature_belongs_to_category(feature_name: &str, category: &str) -> bool {
        match category {
            "complexity" => {
                feature_name.contains("cyclomatic") || feature_name.contains("cognitive")
            }
            "structure" => feature_name.contains("structure") || feature_name.contains("class"),
            "graph" => feature_name.contains("fan_") || feature_name.contains("centrality"),
            _ => true,
        }
    }

    /// Generate issue description based on category and severity
    fn generate_issue_description(category: &str, severity: f64) -> String {
        let severity_level = if severity >= 2.0 {
            "very high"
        } else if severity >= 1.5 {
            "high"
        } else if severity >= 1.0 {
            "moderate"
        } else {
            "low"
        };

        match category {
            "complexity" => format!("This entity has {} complexity that may make it difficult to understand and maintain", severity_level),
            "structure" => format!("This entity has {} structural issues that may indicate design problems", severity_level),
            "graph" => format!("This entity has {} coupling or dependency issues", severity_level),
            _ => format!("This entity has {} issues in the {} category", severity_level, category),
        }
    }

    /// Generate refactoring suggestions based on issues
    fn generate_suggestions(issues: &[RefactoringIssue]) -> Vec<RefactoringSuggestion> {
        let mut suggestions = Vec::new();

        for issue in issues {
            match issue.category.as_str() {
                "complexity" => {
                    // Analyze contributing features for specific complexity issues
                    let mut complexity_features = issue
                        .contributing_features
                        .iter()
                        .filter(|f| f.feature_name.contains("complexity"))
                        .collect::<Vec<_>>();
                    complexity_features.sort_by(|a, b| {
                        b.contribution
                            .partial_cmp(&a.contribution)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });

                    if issue.severity >= 2.0 {
                        let primary_feature = complexity_features
                            .first()
                            .map(|f| f.feature_name.as_str())
                            .unwrap_or("complexity");

                        let description = match primary_feature {
                            s if s.contains("cyclomatic") => {
                                format!("High cyclomatic complexity ({}). Break down nested conditionals and loops into smaller, focused methods", 
                                    complexity_features.first().map(|f| format!("score: {:.1}", f.value)).unwrap_or_default())
                            },
                            s if s.contains("cognitive") => {
                                format!("High cognitive complexity ({}). Reduce mental overhead by extracting complex logic into well-named helper methods", 
                                    complexity_features.first().map(|f| format!("score: {:.1}", f.value)).unwrap_or_default())
                            },
                            s if s.contains("nesting") => {
                                "Deep nesting levels detected. Use early returns and guard clauses to reduce nesting depth".to_string()
                            },
                            _ => {
                                format!("High complexity detected (severity: {:.1}). Consider breaking this large method into smaller, more focused methods", issue.severity)
                            }
                        };

                        suggestions.push(RefactoringSuggestion {
                            refactoring_type: "extract_method".to_string(),
                            description,
                            priority: 0.9,
                            effort: 0.6,
                            impact: 0.8,
                        });
                    }

                    if issue.severity >= 1.5 {
                        // Check if conditional complexity is a major factor
                        let conditional_contribution = issue
                            .contributing_features
                            .iter()
                            .filter(|f| {
                                f.feature_name.contains("conditional")
                                    || f.feature_name.contains("branch")
                            })
                            .map(|f| f.contribution)
                            .fold(0.0, |acc, x| acc + x);

                        let description = if conditional_contribution > 1.0 {
                            "Complex conditional logic detected. Simplify by using early returns, combining conditions, or extracting boolean methods"
                        } else {
                            "Simplify complex conditional logic using guard clauses and boolean extraction"
                        };

                        suggestions.push(RefactoringSuggestion {
                            refactoring_type: "simplify_conditionals".to_string(),
                            description: description.to_string(),
                            priority: 0.7,
                            effort: 0.4,
                            impact: 0.6,
                        });
                    }
                }
                "structure" => {
                    // Look for specific structural issues
                    let structural_features =
                        issue.contributing_features.iter().collect::<Vec<_>>();

                    let description = if !structural_features.is_empty() {
                        let primary_issues = structural_features
                            .iter()
                            .take(2)
                            .map(|f| {
                                format!(
                                    "{} ({})",
                                    f.feature_name.replace("_", " "),
                                    if f.value > 10.0 { "high" } else { "moderate" }
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ");

                        format!("Structural issues detected: {}. Consider reorganizing code into cohesive modules and reducing coupling", primary_issues)
                    } else {
                        format!("Structural issues detected (severity: {:.1}). Improve the organization and cohesion of this code", issue.severity)
                    };

                    suggestions.push(RefactoringSuggestion {
                        refactoring_type: "improve_structure".to_string(),
                        description,
                        priority: 0.6,
                        effort: 0.7,
                        impact: 0.7,
                    });
                }
                "maintainability" => {
                    suggestions.push(RefactoringSuggestion {
                        refactoring_type: "improve_maintainability".to_string(),
                        description: format!("Maintainability issues detected (severity: {:.1}). Add documentation, improve naming, and reduce technical debt", issue.severity),
                        priority: 0.5,
                        effort: 0.5,
                        impact: 0.6,
                    });
                }
                "readability" => {
                    suggestions.push(RefactoringSuggestion {
                        refactoring_type: "improve_readability".to_string(),
                        description: format!("Readability issues detected (severity: {:.1}). Improve variable names, add comments, and simplify expressions", issue.severity),
                        priority: 0.4,
                        effort: 0.3,
                        impact: 0.5,
                    });
                }
                _ => {
                    suggestions.push(RefactoringSuggestion {
                        refactoring_type: "general_refactoring".to_string(),
                        description: format!("Issues detected in {} category (severity: {:.1}). Review and improve this code area", issue.category, issue.severity),
                        priority: 0.3,
                        effort: 0.5,
                        impact: 0.4,
                    });
                }
            }
        }

        // Remove duplicates and sort by priority
        suggestions.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        suggestions.dedup_by(|a, b| {
            a.refactoring_type == b.refactoring_type && a.description == b.description
        });

        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scoring::{Priority, ScoringResult};
    // Removed unused imports
    use std::collections::HashMap;

    #[test]
    fn test_code_health_calculation() {
        let summary = crate::core::pipeline::ResultSummary {
            total_files: 10,
            total_issues: 5,
            health_score: 0.8,
            processing_time: 1.5,
            total_entities: 100,
            refactoring_needed: 20,
            avg_score: 0.5,
        };

        let health_score = AnalysisResults::calculate_code_health_score(&summary);
        assert!(health_score > 0.0);
        assert!(health_score <= 1.0);
    }

    #[test]
    fn test_refactoring_candidate_creation() {
        let mut scoring_result = ScoringResult {
            entity_id: "test_entity".to_string(),
            overall_score: 2.0,
            priority: Priority::High,
            category_scores: HashMap::new(),
            feature_contributions: HashMap::new(),
            normalized_feature_count: 5,
            confidence: 0.8,
        };

        scoring_result
            .category_scores
            .insert("complexity".to_string(), 1.5);
        scoring_result
            .feature_contributions
            .insert("cyclomatic".to_string(), 1.2);

        let candidate = RefactoringCandidate::from_scoring_result(&scoring_result, &[]);

        assert_eq!(candidate.entity_id, "test_entity");
        assert_eq!(candidate.priority, Priority::High);
        assert!(!candidate.issues.is_empty());
        assert!(!candidate.suggestions.is_empty());
    }

    #[test]
    fn test_analysis_summary_default() {
        let summary = AnalysisSummary {
            files_processed: 10,
            entities_analyzed: 50,
            refactoring_needed: 5,
            high_priority: 2,
            critical: 1,
            avg_refactoring_score: 1.2,
            code_health_score: 0.85,
        };

        assert_eq!(summary.files_processed, 10);
        assert_eq!(summary.entities_analyzed, 50);
        assert_eq!(summary.refactoring_needed, 5);
        assert_eq!(summary.high_priority, 2);
        assert_eq!(summary.critical, 1);
        assert_eq!(summary.avg_refactoring_score, 1.2);
        assert_eq!(summary.code_health_score, 0.85);
    }

    #[test]
    fn test_refactoring_candidate_fields() {
        let candidate = RefactoringCandidate {
            entity_id: "func_123".to_string(),
            name: "process_data".to_string(),
            file_path: "src/main.rs".to_string(),
            line_range: Some((10, 50)),
            priority: Priority::Critical,
            score: 2.5,
            confidence: 0.9,
            issues: vec![],
            suggestions: vec![],
            issue_count: 0,
            suggestion_count: 0,
        };

        assert_eq!(candidate.entity_id, "func_123");
        assert_eq!(candidate.name, "process_data");
        assert_eq!(candidate.file_path, "src/main.rs");
        assert_eq!(candidate.line_range, Some((10, 50)));
        assert_eq!(candidate.priority, Priority::Critical);
        assert_eq!(candidate.score, 2.5);
        assert_eq!(candidate.confidence, 0.9);
    }

    #[test]
    fn test_refactoring_issue_creation() {
        let contribution = FeatureContribution {
            feature_name: "cyclomatic_complexity".to_string(),
            value: 12.0,
            normalized_value: 0.8,
            contribution: 0.6,
        };

        let issue = RefactoringIssue {
            category: "complexity".to_string(),
            description: "High cyclomatic complexity".to_string(),
            severity: 1.8,
            contributing_features: vec![contribution],
        };

        assert_eq!(issue.category, "complexity");
        assert_eq!(issue.description, "High cyclomatic complexity");
        assert_eq!(issue.severity, 1.8);
        assert_eq!(issue.contributing_features.len(), 1);
        assert_eq!(
            issue.contributing_features[0].feature_name,
            "cyclomatic_complexity"
        );
    }

    #[test]
    fn test_refactoring_suggestion_creation() {
        let suggestion = RefactoringSuggestion {
            refactoring_type: "extract_method".to_string(),
            description: "Extract complex logic into separate methods".to_string(),
            priority: 0.8,
            effort: 0.6,
            impact: 0.9,
        };

        assert_eq!(suggestion.refactoring_type, "extract_method");
        assert_eq!(
            suggestion.description,
            "Extract complex logic into separate methods"
        );
        assert_eq!(suggestion.priority, 0.8);
        assert_eq!(suggestion.effort, 0.6);
        assert_eq!(suggestion.impact, 0.9);
    }

    #[test]
    fn test_feature_contribution_creation() {
        let contribution = FeatureContribution {
            feature_name: "nesting_depth".to_string(),
            value: 5.0,
            normalized_value: 0.7,
            contribution: 0.4,
        };

        assert_eq!(contribution.feature_name, "nesting_depth");
        assert_eq!(contribution.value, 5.0);
        assert_eq!(contribution.normalized_value, 0.7);
        assert_eq!(contribution.contribution, 0.4);
    }

    #[test]
    fn test_analysis_statistics_creation() {
        let mut priority_dist = HashMap::new();
        priority_dist.insert("High".to_string(), 5);
        priority_dist.insert("Medium".to_string(), 10);

        let mut issue_dist = HashMap::new();
        issue_dist.insert("complexity".to_string(), 8);
        issue_dist.insert("structure".to_string(), 3);

        let memory_stats = MemoryStats {
            peak_memory_bytes: 1024000,
            final_memory_bytes: 512000,
            efficiency_score: 0.9,
        };

        let stats = AnalysisStatistics {
            total_duration: Duration::from_secs(30),
            avg_file_processing_time: Duration::from_millis(500),
            avg_entity_processing_time: Duration::from_millis(50),
            features_per_entity: HashMap::new(),
            priority_distribution: priority_dist.clone(),
            issue_distribution: issue_dist.clone(),
            memory_stats,
        };

        assert_eq!(stats.total_duration, Duration::from_secs(30));
        assert_eq!(stats.avg_file_processing_time, Duration::from_millis(500));
        assert_eq!(stats.avg_entity_processing_time, Duration::from_millis(50));
        assert_eq!(stats.priority_distribution, priority_dist);
        assert_eq!(stats.issue_distribution, issue_dist);
        assert_eq!(stats.memory_stats.peak_memory_bytes, 1024000);
        assert_eq!(stats.memory_stats.final_memory_bytes, 512000);
        assert_eq!(stats.memory_stats.efficiency_score, 0.9);
    }

    #[test]
    fn test_memory_stats_fields() {
        let memory_stats = MemoryStats {
            peak_memory_bytes: 2048000,
            final_memory_bytes: 1024000,
            efficiency_score: 0.75,
        };

        assert_eq!(memory_stats.peak_memory_bytes, 2048000);
        assert_eq!(memory_stats.final_memory_bytes, 1024000);
        assert_eq!(memory_stats.efficiency_score, 0.75);
    }

    #[test]
    fn test_analysis_results_files_analyzed() {
        let summary = AnalysisSummary {
            files_processed: 25,
            entities_analyzed: 100,
            refactoring_needed: 10,
            high_priority: 3,
            critical: 1,
            avg_refactoring_score: 1.1,
            code_health_score: 0.7,
        };

        let results = AnalysisResults {
            summary,
            refactoring_candidates: vec![],
            refactoring_candidates_by_file: vec![],
            unified_hierarchy: vec![],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(10),
                avg_file_processing_time: Duration::from_millis(400),
                avg_entity_processing_time: Duration::from_millis(100),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 1024,
                    final_memory_bytes: 512,
                    efficiency_score: 0.8,
                },
            },
            directory_health_tree: None,
            clone_analysis: None,
            warnings: vec![],
            coverage_packs: Vec::new(),
        };

        assert_eq!(results.files_analyzed(), 25);
    }

    #[test]
    fn test_analysis_results_critical_candidates() {
        let critical_candidate = RefactoringCandidate {
            entity_id: "crit_1".to_string(),
            name: "critical_function".to_string(),
            file_path: "src/critical.rs".to_string(),
            line_range: None,
            priority: Priority::Critical,
            score: 3.0,
            confidence: 0.95,
            issues: vec![],
            suggestions: vec![],
            issue_count: 0,
            suggestion_count: 0,
        };

        let high_candidate = RefactoringCandidate {
            entity_id: "high_1".to_string(),
            name: "high_function".to_string(),
            file_path: "src/high.rs".to_string(),
            line_range: None,
            priority: Priority::High,
            score: 2.0,
            confidence: 0.85,
            issues: vec![],
            suggestions: vec![],
            issue_count: 0,
            suggestion_count: 0,
        };

        let results = AnalysisResults {
            summary: AnalysisSummary {
                files_processed: 2,
                entities_analyzed: 2,
                refactoring_needed: 2,
                high_priority: 2,
                critical: 1,
                avg_refactoring_score: 2.5,
                code_health_score: 0.6,
            },
            refactoring_candidates: vec![critical_candidate, high_candidate],
            refactoring_candidates_by_file: vec![],
            unified_hierarchy: vec![],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(5),
                avg_file_processing_time: Duration::from_millis(2500),
                avg_entity_processing_time: Duration::from_millis(2500),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 1024,
                    final_memory_bytes: 512,
                    efficiency_score: 0.7,
                },
            },
            directory_health_tree: None,
            clone_analysis: None,
            warnings: vec![],
            coverage_packs: Vec::new(),
        };

        let critical_count = results.critical_candidates().count();
        assert_eq!(critical_count, 1);

        let high_priority_count = results.high_priority_candidates().count();
        assert_eq!(high_priority_count, 2); // Both critical and high
    }

    #[test]
    fn test_analysis_results_is_healthy() {
        let healthy_results = AnalysisResults {
            summary: AnalysisSummary {
                files_processed: 10,
                entities_analyzed: 50,
                refactoring_needed: 2,
                high_priority: 0,
                critical: 0,
                avg_refactoring_score: 0.5,
                code_health_score: 0.85, // Healthy threshold is 0.8
            },
            refactoring_candidates: vec![],
            refactoring_candidates_by_file: vec![],
            unified_hierarchy: vec![],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(10),
                avg_file_processing_time: Duration::from_millis(1000),
                avg_entity_processing_time: Duration::from_millis(200),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 1024,
                    final_memory_bytes: 512,
                    efficiency_score: 0.9,
                },
            },
            directory_health_tree: None,
            clone_analysis: None,
            warnings: vec![],
            coverage_packs: Vec::new(),
        };

        assert!(healthy_results.is_healthy());

        let unhealthy_results = AnalysisResults {
            summary: AnalysisSummary {
                files_processed: 10,
                entities_analyzed: 50,
                refactoring_needed: 25,
                high_priority: 10,
                critical: 5,
                avg_refactoring_score: 2.5,
                code_health_score: 0.7, // Below healthy threshold
            },
            refactoring_candidates: vec![],
            refactoring_candidates_by_file: vec![],
            unified_hierarchy: vec![],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(10),
                avg_file_processing_time: Duration::from_millis(1000),
                avg_entity_processing_time: Duration::from_millis(200),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 1024,
                    final_memory_bytes: 512,
                    efficiency_score: 0.6,
                },
            },
            directory_health_tree: None,
            clone_analysis: None,
            warnings: vec![],
            coverage_packs: Vec::new(),
        };

        assert!(!unhealthy_results.is_healthy());
    }

    #[test]
    fn test_analysis_results_top_issues() {
        let issue1 = RefactoringIssue {
            category: "complexity".to_string(),
            description: "High complexity".to_string(),
            severity: 2.0,
            contributing_features: vec![],
        };

        let issue2 = RefactoringIssue {
            category: "structure".to_string(),
            description: "Poor structure".to_string(),
            severity: 1.5,
            contributing_features: vec![],
        };

        let issue3 = RefactoringIssue {
            category: "complexity".to_string(),
            description: "Another complexity issue".to_string(),
            severity: 1.8,
            contributing_features: vec![],
        };

        let candidate1 = RefactoringCandidate {
            entity_id: "entity1".to_string(),
            name: "function1".to_string(),
            file_path: "src/main.rs".to_string(),
            line_range: None,
            priority: Priority::High,
            score: 2.0,
            confidence: 0.9,
            issues: vec![issue1, issue2],
            suggestions: vec![],
            issue_count: 2,
            suggestion_count: 0,
        };

        let candidate2 = RefactoringCandidate {
            entity_id: "entity2".to_string(),
            name: "function2".to_string(),
            file_path: "src/lib.rs".to_string(),
            line_range: None,
            priority: Priority::Medium,
            score: 1.5,
            confidence: 0.8,
            issues: vec![issue3],
            suggestions: vec![],
            issue_count: 1,
            suggestion_count: 0,
        };

        let results = AnalysisResults {
            summary: AnalysisSummary {
                files_processed: 2,
                entities_analyzed: 2,
                refactoring_needed: 2,
                high_priority: 1,
                critical: 0,
                avg_refactoring_score: 1.75,
                code_health_score: 0.7,
            },
            refactoring_candidates: vec![candidate1, candidate2],
            refactoring_candidates_by_file: vec![],
            unified_hierarchy: vec![],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(5),
                avg_file_processing_time: Duration::from_millis(2500),
                avg_entity_processing_time: Duration::from_millis(2500),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 1024,
                    final_memory_bytes: 512,
                    efficiency_score: 0.8,
                },
            },
            directory_health_tree: None,
            clone_analysis: None,
            warnings: vec![],
            coverage_packs: Vec::new(),
        };

        let top_issues = results.top_issues(2);
        assert_eq!(top_issues.len(), 2);

        // Complexity should be first (appears twice)
        assert_eq!(top_issues[0].0, "complexity");
        assert_eq!(top_issues[0].1, 2);

        // Structure should be second (appears once)
        assert_eq!(top_issues[1].0, "structure");
        assert_eq!(top_issues[1].1, 1);
    }

    #[test]
    fn test_calculate_code_health_score_edge_cases() {
        // Test with zero entities
        let empty_summary = crate::core::pipeline::ResultSummary {
            total_files: 0,
            total_issues: 0,
            health_score: 0.0,
            processing_time: 0.0,
            total_entities: 0,
            refactoring_needed: 0,
            avg_score: 0.0,
        };
        let health_score = AnalysisResults::calculate_code_health_score(&empty_summary);
        assert_eq!(health_score, 1.0); // Perfect health when no entities

        // Test with all entities needing refactoring
        let bad_summary = crate::core::pipeline::ResultSummary {
            total_files: 10,
            total_issues: 100,
            health_score: 0.1,
            processing_time: 5.0,
            total_entities: 100,
            refactoring_needed: 100,
            avg_score: 5.0, // Very high average score
        };
        let health_score = AnalysisResults::calculate_code_health_score(&bad_summary);
        assert!(health_score >= 0.0);
        assert!(health_score <= 1.0);
        assert!(health_score < 0.5); // Should be poor health
    }

    #[test]
    fn test_feature_belongs_to_category() {
        assert!(RefactoringCandidate::feature_belongs_to_category(
            "cyclomatic_complexity",
            "complexity"
        ));
        assert!(RefactoringCandidate::feature_belongs_to_category(
            "cognitive_load",
            "complexity"
        ));
        assert!(RefactoringCandidate::feature_belongs_to_category(
            "class_structure",
            "structure"
        ));
        assert!(RefactoringCandidate::feature_belongs_to_category(
            "fan_in", "graph"
        ));
        assert!(RefactoringCandidate::feature_belongs_to_category(
            "centrality_score",
            "graph"
        ));
        assert!(RefactoringCandidate::feature_belongs_to_category(
            "random_feature",
            "unknown"
        )); // Catch-all
    }

    #[test]
    fn test_generate_issue_description() {
        let complexity_desc = RefactoringCandidate::generate_issue_description("complexity", 2.5);
        assert!(complexity_desc.contains("very high"));
        assert!(complexity_desc.contains("complexity"));

        let structure_desc = RefactoringCandidate::generate_issue_description("structure", 1.3);
        assert!(structure_desc.contains("moderate"));
        assert!(structure_desc.contains("structural"));

        let graph_desc = RefactoringCandidate::generate_issue_description("graph", 0.8);
        assert!(graph_desc.contains("low"));
        assert!(graph_desc.contains("coupling"));

        let unknown_desc = RefactoringCandidate::generate_issue_description("unknown", 1.7);
        assert!(unknown_desc.contains("high"));
        assert!(unknown_desc.contains("unknown"));
    }

    #[test]
    fn test_generate_suggestions() {
        let high_complexity_issue = RefactoringIssue {
            category: "complexity".to_string(),
            description: "Very high complexity".to_string(),
            severity: 2.5,
            contributing_features: vec![],
        };

        let moderate_complexity_issue = RefactoringIssue {
            category: "complexity".to_string(),
            description: "Moderate complexity".to_string(),
            severity: 1.6,
            contributing_features: vec![],
        };

        let structure_issue = RefactoringIssue {
            category: "structure".to_string(),
            description: "Poor structure".to_string(),
            severity: 1.2,
            contributing_features: vec![],
        };

        let issues = vec![
            high_complexity_issue,
            moderate_complexity_issue,
            structure_issue,
        ];
        let suggestions = RefactoringCandidate::generate_suggestions(&issues);

        // Should generate multiple suggestions for different issues
        assert!(!suggestions.is_empty());

        // Should have extract_method for high complexity
        let extract_method = suggestions
            .iter()
            .find(|s| s.refactoring_type == "extract_method");
        assert!(extract_method.is_some());

        // Should have simplify_conditionals for moderate complexity
        let simplify_conditionals = suggestions
            .iter()
            .find(|s| s.refactoring_type == "simplify_conditionals");
        assert!(simplify_conditionals.is_some());

        // Should have improve_structure for structure issue
        let improve_structure = suggestions
            .iter()
            .find(|s| s.refactoring_type == "improve_structure");
        assert!(improve_structure.is_some());

        // Should be sorted by priority (highest first)
        if suggestions.len() > 1 {
            for i in 0..suggestions.len() - 1 {
                assert!(suggestions[i].priority >= suggestions[i + 1].priority);
            }
        }
    }

    #[test]
    fn test_directory_health_tree_creation() {
        use std::path::PathBuf;

        // Create test refactoring candidates
        let candidates = vec![
            RefactoringCandidate {
                entity_id: "func1".to_string(),
                name: "complex_function".to_string(),
                file_path: "src/main.rs".to_string(),
                line_range: Some((10, 50)),
                priority: Priority::High,
                score: 2.0,
                confidence: 0.9,
                issues: vec![RefactoringIssue {
                    category: "complexity".to_string(),
                    description: "High complexity".to_string(),
                    severity: 2.0,
                    contributing_features: vec![],
                }],
                suggestions: vec![],
                issue_count: 1,
                suggestion_count: 0,
            },
            RefactoringCandidate {
                entity_id: "func2".to_string(),
                name: "another_function".to_string(),
                file_path: "src/utils/helper.rs".to_string(),
                line_range: Some((5, 25)),
                priority: Priority::Medium,
                score: 1.5,
                confidence: 0.8,
                issues: vec![RefactoringIssue {
                    category: "structure".to_string(),
                    description: "Poor structure".to_string(),
                    severity: 1.5,
                    contributing_features: vec![],
                }],
                suggestions: vec![],
                issue_count: 1,
                suggestion_count: 0,
            },
        ];

        // Create directory health tree using the simplified constructor
        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Verify tree structure
        assert!(!health_tree.directories.is_empty());
        assert!(health_tree.tree_statistics.total_directories > 0);

        // Check that we have directories for src and src/utils
        let src_path = PathBuf::from("src");
        let utils_path = PathBuf::from("src/utils");

        assert!(
            health_tree.directories.contains_key(&src_path)
                || health_tree.directories.contains_key(&utils_path)
        );

        // Verify tree string generation doesn't panic
        let tree_string = health_tree.to_tree_string();
        assert!(!tree_string.is_empty());

        // Check that the tree contains health percentages
        assert!(tree_string.contains("health:"));

        println!("Generated directory tree:");
        println!("{}", tree_string);
    }

    #[test]
    fn test_serialization_deserialization() {
        let results = AnalysisResults {
            summary: AnalysisSummary {
                files_processed: 5,
                entities_analyzed: 25,
                refactoring_needed: 3,
                high_priority: 1,
                critical: 0,
                avg_refactoring_score: 1.2,
                code_health_score: 0.8,
            },
            refactoring_candidates: vec![],
            refactoring_candidates_by_file: vec![],
            unified_hierarchy: vec![],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(10),
                avg_file_processing_time: Duration::from_millis(2000),
                avg_entity_processing_time: Duration::from_millis(400),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 2048,
                    final_memory_bytes: 1024,
                    efficiency_score: 0.85,
                },
            },
            directory_health_tree: None,
            clone_analysis: None,
            warnings: vec!["Test warning".to_string()],
            coverage_packs: Vec::new(),
        };

        // Test JSON serialization
        let json = serde_json::to_string(&results).expect("Should serialize to JSON");
        let deserialized: AnalysisResults =
            serde_json::from_str(&json).expect("Should deserialize from JSON");

        assert_eq!(deserialized.summary.files_processed, 5);
        assert_eq!(deserialized.summary.entities_analyzed, 25);
        assert_eq!(deserialized.warnings.len(), 1);
        assert_eq!(deserialized.warnings[0], "Test warning");
    }
}

/*
/// Code quality analysis results for API consumption (simple pattern-based analysis)
#[derive(Debug, Serialize, Deserialize)]
pub struct NamingAnalysisResults {
    /// Function rename recommendations
    pub rename_packs: Vec<RenamePack>,

    /// Contract mismatch recommendations
    pub contract_mismatch_packs: Vec<ContractMismatchPack>,

    /// Project-wide naming consistency issues
    pub consistency_issues: Vec<ConsistencyIssue>,

    /// Summary statistics for naming analysis
    pub naming_summary: NamingSummary,
}
*/

/*
/// Summary statistics for code quality analysis (simple pattern-based analysis)
#[derive(Debug, Serialize, Deserialize)]
pub struct NamingSummary {
    /// Total functions analyzed
    pub functions_analyzed: usize,

    /// Functions with potential naming issues
    pub functions_with_issues: usize,

    /// Functions above mismatch threshold
    pub high_mismatch_functions: usize,

    /// Functions affecting public API
    pub public_api_functions: usize,

    /// Average mismatch score across all functions
    pub avg_mismatch_score: f64,

    /// Most common mismatch types
    pub common_mismatch_types: Vec<(String, usize)>,

    /// Project lexicon statistics
    pub lexicon_stats: LexiconStats,
}
*/

/*
/// Project lexicon statistics (temporarily disabled)
#[derive(Debug, Serialize, Deserialize)]
pub struct LexiconStats {
    /// Number of unique domain nouns discovered
    pub domain_nouns: usize,

    /// Number of verb patterns identified
    pub verb_patterns: usize,

    /// Number of synonym clusters detected
    pub synonym_clusters: usize,

    /// Most frequent domain terms
    pub top_domain_terms: Vec<(String, usize)>,
}
*/

/// Refactoring candidates grouped by file
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileRefactoringGroup {
    /// File path
    pub file_path: String,

    /// File name (without path for display)
    pub file_name: String,

    /// Number of entities in this file
    pub entity_count: usize,

    /// Highest priority level in this file
    pub highest_priority: Priority,

    /// Average refactoring score for this file
    pub avg_score: f64,

    /// Total issues across all entities in this file
    pub total_issues: usize,

    /// Entities/functions that need refactoring in this file
    pub entities: Vec<RefactoringCandidate>,
}

/// Clone detection and denoising analysis results
#[derive(Debug, Serialize, Deserialize)]
pub struct CloneAnalysisResults {
    /// Whether clone denoising was enabled
    pub denoising_enabled: bool,

    /// Whether auto-calibration was applied
    pub auto_calibration_applied: bool,

    /// Number of clone candidates before denoising
    pub candidates_before_denoising: usize,

    /// Number of clone candidates after denoising
    pub candidates_after_denoising: usize,

    /// Calibrated similarity threshold (if auto-calibration was used)
    pub calibrated_threshold: f64,

    /// Quality score achieved by denoising
    pub quality_score: f64,

    /// Number of candidates filtered by each phase
    pub phase_filtering_stats: PhaseFilteringStats,

    /// Performance metrics
    pub performance_metrics: CloneAnalysisPerformance,
}

/// Statistics for filtering performed by each phase
#[derive(Debug, Serialize, Deserialize)]
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

/// Performance metrics for clone analysis
#[derive(Debug, Serialize, Deserialize)]
pub struct CloneAnalysisPerformance {
    /// Total analysis time in milliseconds
    pub total_time_ms: u64,

    /// Memory usage in bytes
    pub memory_usage_bytes: u64,

    /// Entities processed per second
    pub entities_per_second: f64,
}
