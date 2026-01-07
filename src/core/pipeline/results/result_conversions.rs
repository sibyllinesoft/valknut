use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{self, json};

use crate::core::featureset::FeatureVector;
use super::pipeline_results::DocumentationAnalysisResults;
use crate::core::pipeline::{PipelineResults, ResultSummary, StageResultsBundle};
use crate::core::scoring::{Priority, ScoringResult};

use crate::core::pipeline::discovery::code_dictionary::{
    issue_code_for_category, issue_definition_for_category, suggestion_code_for_kind,
    suggestion_definition_for_kind,
};
use super::result_types::*;
use crate::core::pipeline::health::suggestion_generator::generate_suggestions;

/// Hierarchy building and conversion methods for [`AnalysisResults`].
impl AnalysisResults {
    /// Build a minimal unified hierarchy; falls back to candidate-based grouping when directory data is empty.
    pub fn build_unified_hierarchy_with_fallback(
        candidates: &[RefactoringCandidate],
        directory_tree: &DirectoryHealthTree,
    ) -> Vec<serde_json::Value> {
        // Prefer directory tree if present
        if !directory_tree.directories.is_empty() || directory_tree.root.file_count > 0 {
            let root_name = directory_tree.root.path.display().to_string();
            return vec![serde_json::json!({
                "name": root_name,
                "type": "folder",
                "healthScore": directory_tree.root.health_score,
                "children": Vec::<serde_json::Value>::new(),
            })];
        }

        // Fallback: group candidates by file
        let mut grouped = AnalysisResults::group_candidates_by_file(candidates)
            .into_iter()
            .map(|group| {
                serde_json::json!({
                    "name": group.file_name,
                    "path": group.file_path,
                    "type": "file",
                    "entityCount": group.entity_count,
                    "avgScore": group.avg_score,
                })
            })
            .collect::<Vec<_>>();

        if grouped.is_empty() {
            grouped.push(serde_json::json!({"name": "root", "type": "folder", "children": Vec::<serde_json::Value>::new()}));
        }

        grouped
    }
    /// Create an empty analysis result placeholder
    pub fn empty() -> Self {
        AnalysisResults {
            project_root: PathBuf::new(),
            summary: AnalysisSummary {
                files_processed: 0,
                entities_analyzed: 0,
                refactoring_needed: 0,
                high_priority: 0,
                critical: 0,
                avg_refactoring_score: 0.0,
                code_health_score: 1.0,
                total_files: 0,
                total_entities: 0,
                total_lines_of_code: 0,
                languages: Vec::new(),
                total_issues: 0,
                high_priority_issues: 0,
                critical_issues: 0,
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
            normalized: None,
            passes: StageResultsBundle::disabled(),
            refactoring_candidates: Vec::new(),
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(0),
                avg_file_processing_time: Duration::from_secs(0),
                avg_entity_processing_time: Duration::from_secs(0),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 0,
                    final_memory_bytes: 0,
                    efficiency_score: 1.0,
                },
            },
            clone_analysis: None,
            coverage_packs: Vec::new(),
            warnings: Vec::new(),
            health_metrics: None,
            code_dictionary: CodeDictionary::default(),
            documentation: None,
        }
    }

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
    ///
    /// The `project_root` is the base directory for all analyzed files.
    /// All file paths in the results will be stored relative to this root.
    pub fn from_pipeline_results(pipeline_results: PipelineResults, project_root: PathBuf) -> Self {
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
                RefactoringCandidate::from_scoring_result(result, &pipeline_results.feature_vectors, &project_root)
            })
            .collect();
        // Created refactoring candidates

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

        let base_summary = &pipeline_results.results.summary;

        let summary = AnalysisSummary {
            files_processed: pipeline_results.statistics.files_processed,
            entities_analyzed: summary_stats.total_entities,
            refactoring_needed: summary_stats.refactoring_needed,
            high_priority: high_priority_count,
            critical: critical_count,
            avg_refactoring_score: summary_stats.avg_score,
            code_health_score,
            total_files: base_summary.total_files,
            total_entities: base_summary.total_entities,
            total_lines_of_code: base_summary.total_lines_of_code,
            languages: base_summary.languages.clone(),
            total_issues: base_summary.total_issues,
            high_priority_issues: base_summary.high_priority_issues,
            critical_issues: base_summary.critical_issues,
            doc_health_score: base_summary.doc_health_score,
            doc_issue_count: base_summary.doc_issue_count,
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
                peak_memory_bytes: pipeline_results.statistics.memory_stats.peak_memory_bytes,
                final_memory_bytes: pipeline_results.statistics.memory_stats.final_memory_bytes,
                efficiency_score: pipeline_results.statistics.memory_stats.efficiency_score,
            },
        };

        let warnings = pipeline_results
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect();

        // Convert LSH results to clone analysis results
        let clone_analysis = Self::convert_lsh_to_clone_analysis(&pipeline_results);

        // Extract coverage packs from pipeline results
        let coverage_packs =
            crate::core::pipeline::verification::coverage_mapping::convert_coverage_to_packs(&pipeline_results.results.coverage);

        // Annotate existing candidates with coverage percentages (instead of creating fake entities)
        let mut refactoring_candidates = refactoring_candidates;
        crate::core::pipeline::verification::coverage_mapping::annotate_candidates_with_coverage(
            &mut refactoring_candidates,
            &coverage_packs,
        );

        let health_metrics = Some(pipeline_results.results.health_metrics.clone());

        let mut code_dictionary = CodeDictionary::default();
        for candidate in &refactoring_candidates {
            for issue in &candidate.issues {
                code_dictionary
                    .issues
                    .entry(issue.code.clone())
                    .or_insert_with(|| issue_definition_for_category(&issue.category));
            }
            for suggestion in &candidate.suggestions {
                code_dictionary
                    .suggestions
                    .entry(suggestion.code.clone())
                    .or_insert_with(|| {
                        suggestion_definition_for_kind(&suggestion.refactoring_type)
                    });
            }
        }

        // Add ADDTEST suggestion definition to dictionary if coverage candidates exist
        if !coverage_packs.is_empty() {
            code_dictionary
                .suggestions
                .entry("ADDTEST".to_string())
                .or_insert_with(|| CodeDefinition {
                    code: "ADDTEST".to_string(),
                    title: "Add Test Coverage".to_string(),
                    summary: "Write tests to cover this untested code path and improve safety."
                        .to_string(),
                    category: Some("coverage".to_string()),
                });
        }

        let passes = StageResultsBundle {
            structure: pipeline_results.results.structure.clone(),
            coverage: pipeline_results.results.coverage.clone(),
            complexity: pipeline_results.results.complexity.clone(),
            refactoring: pipeline_results.results.refactoring.clone(),
            impact: pipeline_results.results.impact.clone(),
            lsh: pipeline_results.results.lsh.clone(),
            cohesion: pipeline_results.results.cohesion.clone(),
        };

        let documentation =
            pipeline_results
                .results
                .documentation
                .enabled
                .then(|| DocumentationResults {
                    issues_count: pipeline_results.results.documentation.issues_count,
                    doc_health_score: pipeline_results.results.documentation.doc_health_score,
                    file_doc_health: pipeline_results
                        .results
                        .documentation
                        .file_doc_health
                        .clone(),
                    file_doc_issues: pipeline_results
                        .results
                        .documentation
                        .file_doc_issues
                        .clone(),
                    directory_doc_health: pipeline_results
                        .results
                        .documentation
                        .directory_doc_health
                        .clone(),
                    directory_doc_issues: pipeline_results
                        .results
                        .documentation
                        .directory_doc_issues
                        .clone(),
                });

        Self {
            project_root,
            summary,
            normalized: None,
            passes,
            refactoring_candidates,
            statistics,
            // naming_results: None, // Will be populated by naming analysis
            clone_analysis,
            warnings,
            coverage_packs,
            health_metrics,
            code_dictionary,
            documentation,
        }
    }

    /// Calculate overall code health score from pipeline summary
    pub(crate) fn calculate_code_health_score(summary: &ResultSummary) -> f64 {
        if summary.total_entities == 0 {
            return 1.0; // No entities = perfect health (or no data)
        }

        let refactoring_ratio = summary.refactoring_needed as f64 / summary.total_entities as f64;
        let health_score = 1.0 - refactoring_ratio;

        // Adjust based on average score magnitude
        let score_penalty = (summary.avg_score.abs() / 2.0).min(0.3);

        (health_score - score_penalty).clamp(0.0, 1.0)
    }

    /// Convert LSH results to CloneAnalysisResults
    pub(crate) fn convert_lsh_to_clone_analysis(
        pipeline_results: &PipelineResults,
    ) -> Option<CloneAnalysisResults> {
        let lsh_results = &pipeline_results.results.lsh;

        if !lsh_results.enabled {
            return None;
        }

        let mut notes = Vec::new();

        if lsh_results.clone_pairs.is_empty() {
            notes.push("Clone detector did not report any duplicate candidates.".to_string());
        }

        if !lsh_results.denoising_enabled {
            notes.push(
                "Clone denoising disabled; pre-denoise candidate counts and filtering telemetry are unavailable.".to_string(),
            );
        } else {
            notes.push(
                "Denoising telemetry does not expose pre-filter candidate counts; upgrade detector instrumentation to populate them.".to_string(),
            );
        }

        if lsh_results.tfidf_stats.is_none() {
            notes.push(
                "TF-IDF statistics were not captured; phase filtering breakdown is omitted."
                    .to_string(),
            );
        }

        let avg_similarity = Some(lsh_results.avg_similarity);
        let max_similarity = Some(lsh_results.max_similarity);

        Some(CloneAnalysisResults {
            denoising_enabled: lsh_results.denoising_enabled,
            auto_calibration_applied: None,
            candidates_before_denoising: None,
            candidates_after_denoising: lsh_results.duplicate_count,
            calibrated_threshold: None,
            quality_score: avg_similarity,
            avg_similarity,
            max_similarity,
            verification: lsh_results.verification.clone(),
            phase_filtering_stats: None,
            performance_metrics: None,
            notes,
            clone_pairs: lsh_results.clone_pairs.clone(),
        })
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

/// Factory and conversion methods for [`RefactoringCandidate`].
impl RefactoringCandidate {
    /// Create a refactoring candidate from a scoring result
    ///
    /// The `project_root` is used to convert absolute file paths to relative paths.
    pub(crate) fn from_scoring_result(
        result: &ScoringResult,
        feature_vectors: &[FeatureVector],
        project_root: &std::path::Path,
    ) -> Self {
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

            // Convert to relative path by stripping project root
            let path = std::path::Path::new(&raw_path);
            if let Ok(relative) = path.strip_prefix(project_root) {
                relative.to_string_lossy().to_string()
            } else if raw_path.starts_with("./") {
                // Fallback: clean "./" prefix
                raw_path[2..].to_string()
            } else {
                raw_path
            }
        };

        // Extract entity information
        let (name, line_range) = if let Some(vector) = feature_vector {
            // Extract from metadata if available, falling back to parsing entity_id
            let name = vector
                .metadata
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| Self::extract_name_from_entity_id(&result.entity_id));

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
            (Self::extract_name_from_entity_id(&result.entity_id), None)
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
                    code: issue_code_for_category(category),
                    category: category.clone(),
                    severity: score,
                    contributing_features,
                };

                issues.push(issue);
            }
        }

        // Generate suggestions based on issues
        let suggestions = generate_suggestions(&issues, &name, line_range);

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
            coverage_percentage: None,
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

    /// Extract the display name from an entity_id.
    /// Entity ID formats vary but commonly follow patterns like:
    /// - "file_path:name:line_number" (complexity detector)
    /// - "file_path:type_number:counter" (language adapters)
    /// This function extracts just the name portion for display.
    fn extract_name_from_entity_id(entity_id: &str) -> String {
        let parts: Vec<&str> = entity_id.split(':').collect();
        if parts.len() >= 2 {
            // Try to find the name portion - usually the second-to-last part
            // that isn't a pure number (which would be line number or counter)
            for i in (1..parts.len()).rev() {
                let part = parts[i];
                // Skip pure numeric parts (likely line numbers or counters)
                if part.parse::<u64>().is_err() {
                    return part.to_string();
                }
            }
            // If all parts after the first are numbers, use the second part
            if parts.len() > 1 {
                return parts[1].to_string();
            }
        }
        // Fallback to full entity_id
        entity_id.to_string()
    }

}


#[cfg(test)]
#[path = "result_conversions_tests.rs"]
mod tests;
