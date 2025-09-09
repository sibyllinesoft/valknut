//! Analysis results and reporting structures.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::scoring::{ScoringResult, Priority};
use crate::core::featureset::FeatureVector;
use crate::core::pipeline::{PipelineResults, ResultSummary};
// use crate::detectors::names::{RenamePack, ContractMismatchPack, ConsistencyIssue};

/// High-level analysis results for public API consumption
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisResults {
    /// Summary of the analysis
    pub summary: AnalysisSummary,
    
    /// Detailed results for entities that need refactoring
    pub refactoring_candidates: Vec<RefactoringCandidate>,
    
    /// Analysis statistics
    pub statistics: AnalysisStatistics,
    
    /// Semantic naming analysis results (temporarily disabled)
    // pub naming_results: Option<NamingAnalysisResults>,
    
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
#[derive(Debug, Serialize, Deserialize)]
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
}

/// A specific refactoring issue within an entity
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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

impl AnalysisResults {
    /// Create analysis results from pipeline results
    pub fn from_pipeline_results(pipeline_results: PipelineResults) -> Self {
        let summary_stats = pipeline_results.summary();
        
        // Convert scoring results to refactoring candidates
        let refactoring_candidates: Vec<RefactoringCandidate> = pipeline_results
            .scoring_results
            .files
            .iter()
            .filter(|result| result.needs_refactoring())
            .map(|result| RefactoringCandidate::from_scoring_result(result, &pipeline_results.feature_vectors))
            .collect();
        
        // Calculate priority distribution
        let mut priority_distribution = HashMap::new();
        for result in &pipeline_results.scoring_results.files {
            let priority_name = format!("{:?}", result.priority);
            *priority_distribution.entry(priority_name).or_insert(0) += 1;
        }
        
        // Count critical and high priority
        let critical_count = pipeline_results.scoring_results
            .files
            .iter()
            .filter(|r| matches!(r.priority, Priority::Critical))
            .count();
        
        let high_priority_count = pipeline_results.scoring_results
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
                pipeline_results.statistics.total_duration_ms / pipeline_results.statistics.files_processed.max(1) as u64
            ),
            avg_entity_processing_time: Duration::from_millis(
                pipeline_results.statistics.total_duration_ms / summary_stats.total_entities.max(1) as u64
            ),
            features_per_entity: HashMap::new(), // TODO: Calculate from feature vectors
            priority_distribution,
            issue_distribution: HashMap::new(), // TODO: Calculate from issues
            memory_stats: MemoryStats {
                peak_memory_bytes: pipeline_results.statistics.memory_stats.peak_memory_bytes as usize,
                final_memory_bytes: pipeline_results.statistics.memory_stats.current_memory_bytes as usize,
                efficiency_score: 0.85, // Placeholder
            },
        };
        
        let warnings = pipeline_results.errors
            .iter()
            .map(|e| e.to_string())
            .collect();
        
        Self {
            summary,
            refactoring_candidates,
            statistics,
            // naming_results: None, // Will be populated by naming analysis
            warnings,
        }
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
}

impl RefactoringCandidate {
    /// Create a refactoring candidate from a scoring result
    fn from_scoring_result(result: &ScoringResult, feature_vectors: &[FeatureVector]) -> Self {
        // Find the corresponding feature vector
        let feature_vector = feature_vectors
            .iter()
            .find(|v| v.entity_id == result.entity_id);
        
        // Extract entity information
        let (name, file_path, line_range) = if let Some(vector) = feature_vector {
            // Extract from metadata if available
            let name = vector.metadata.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&result.entity_id)
                .to_string();
            
            let file_path = vector.metadata.get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            
            let line_range = vector.metadata.get("line_range")
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
            
            (name, file_path, line_range)
        } else {
            (result.entity_id.clone(), "unknown".to_string(), None)
        };
        
        // Create issues from category scores
        let mut issues = Vec::new();
        for (category, &score) in &result.category_scores {
            if score > 0.5 {  // Only include significant issues
                let contributing_features: Vec<FeatureContribution> = result.feature_contributions
                    .iter()
                    .filter(|(feature_name, _)| Self::feature_belongs_to_category(feature_name, category))
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
            issues,
            suggestions,
        }
    }
    
    /// Check if a feature belongs to a category
    fn feature_belongs_to_category(feature_name: &str, category: &str) -> bool {
        match category {
            "complexity" => feature_name.contains("cyclomatic") || feature_name.contains("cognitive"),
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
                    if issue.severity >= 2.0 {
                        suggestions.push(RefactoringSuggestion {
                            refactoring_type: "extract_method".to_string(),
                            description: "Consider breaking this large method into smaller, more focused methods".to_string(),
                            priority: 0.9,
                            effort: 0.6,
                            impact: 0.8,
                        });
                    }
                    
                    if issue.severity >= 1.5 {
                        suggestions.push(RefactoringSuggestion {
                            refactoring_type: "simplify_conditionals".to_string(),
                            description: "Simplify complex conditional logic".to_string(),
                            priority: 0.7,
                            effort: 0.4,
                            impact: 0.6,
                        });
                    }
                }
                "structure" => {
                    suggestions.push(RefactoringSuggestion {
                        refactoring_type: "improve_structure".to_string(),
                        description: "Improve the structural organization of this code".to_string(),
                        priority: 0.6,
                        effort: 0.7,
                        impact: 0.7,
                    });
                }
                _ => {}
            }
        }
        
        // Remove duplicates and sort by priority
        suggestions.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
        suggestions.dedup_by(|a, b| a.refactoring_type == b.refactoring_type);
        
        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scoring::{ScoringResult, Priority};
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
        
        scoring_result.category_scores.insert("complexity".to_string(), 1.5);
        scoring_result.feature_contributions.insert("cyclomatic".to_string(), 1.2);
        
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
        assert_eq!(issue.contributing_features[0].feature_name, "cyclomatic_complexity");
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
        assert_eq!(suggestion.description, "Extract complex logic into separate methods");
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
            warnings: vec![],
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
            warnings: vec![],
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
            warnings: vec![],
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
            warnings: vec![],
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
            warnings: vec![],
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
        assert!(RefactoringCandidate::feature_belongs_to_category("cyclomatic_complexity", "complexity"));
        assert!(RefactoringCandidate::feature_belongs_to_category("cognitive_load", "complexity"));
        assert!(RefactoringCandidate::feature_belongs_to_category("class_structure", "structure"));
        assert!(RefactoringCandidate::feature_belongs_to_category("fan_in", "graph"));
        assert!(RefactoringCandidate::feature_belongs_to_category("centrality_score", "graph"));
        assert!(RefactoringCandidate::feature_belongs_to_category("random_feature", "unknown")); // Catch-all
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

        let issues = vec![high_complexity_issue, moderate_complexity_issue, structure_issue];
        let suggestions = RefactoringCandidate::generate_suggestions(&issues);

        // Should generate multiple suggestions for different issues
        assert!(!suggestions.is_empty());

        // Should have extract_method for high complexity
        let extract_method = suggestions.iter().find(|s| s.refactoring_type == "extract_method");
        assert!(extract_method.is_some());

        // Should have simplify_conditionals for moderate complexity
        let simplify_conditionals = suggestions.iter().find(|s| s.refactoring_type == "simplify_conditionals");
        assert!(simplify_conditionals.is_some());

        // Should have improve_structure for structure issue
        let improve_structure = suggestions.iter().find(|s| s.refactoring_type == "improve_structure");
        assert!(improve_structure.is_some());

        // Should be sorted by priority (highest first)
        if suggestions.len() > 1 {
            for i in 0..suggestions.len()-1 {
                assert!(suggestions[i].priority >= suggestions[i+1].priority);
            }
        }
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
            warnings: vec!["Test warning".to_string()],
        };

        // Test JSON serialization
        let json = serde_json::to_string(&results).expect("Should serialize to JSON");
        let deserialized: AnalysisResults = serde_json::from_str(&json).expect("Should deserialize from JSON");
        
        assert_eq!(deserialized.summary.files_processed, 5);
        assert_eq!(deserialized.summary.entities_analyzed, 25);
        assert_eq!(deserialized.warnings.len(), 1);
        assert_eq!(deserialized.warnings[0], "Test warning");
    }
}

/*
/// Semantic naming analysis results for API consumption (temporarily disabled)
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
/// Summary statistics for semantic naming analysis (temporarily disabled)
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