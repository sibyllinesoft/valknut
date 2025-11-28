use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{self, json};

use crate::core::featureset::FeatureVector;
use crate::core::pipeline::pipeline_results::DocumentationAnalysisResults;
use crate::core::pipeline::{PipelineResults, ResultSummary, StageResultsBundle};
use crate::core::scoring::{Priority, ScoringResult};

use super::code_dictionary::{
    issue_code_for_category, issue_definition_for_category, suggestion_code_for_kind,
    suggestion_definition_for_kind,
};
use super::result_types::*;

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
        let coverage_packs = Self::convert_coverage_to_packs(&pipeline_results.results.coverage);

        // Annotate existing candidates with coverage percentages (instead of creating fake entities)
        let mut refactoring_candidates = refactoring_candidates;
        Self::annotate_candidates_with_coverage(&mut refactoring_candidates, &coverage_packs);

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
    fn calculate_code_health_score(summary: &ResultSummary) -> f64 {
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
    fn convert_lsh_to_clone_analysis(
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

    /// Build a coverage map from coverage packs for annotating existing candidates.
    /// Returns a map of (file_path, line_start, line_end) -> coverage_percentage
    /// Also returns file-level coverage percentages.
    fn build_coverage_map(
        coverage_packs: &[crate::detectors::coverage::CoveragePack],
    ) -> (
        std::collections::HashMap<String, f64>, // file -> coverage %
        std::collections::HashMap<(String, usize, usize), f64>, // (file, start, end) -> coverage %
    ) {
        let mut file_coverage: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();
        let mut entity_coverage: std::collections::HashMap<(String, usize, usize), f64> =
            std::collections::HashMap::new();

        for pack in coverage_packs {
            let file_path = pack.path.display().to_string();
            let clean_path = if file_path.starts_with("./") {
                file_path[2..].to_string()
            } else {
                file_path.clone()
            };

            // File-level coverage from pack.file_info
            let file_cov_pct = pack.file_info.coverage_before * 100.0;
            file_coverage.insert(clean_path.clone(), file_cov_pct);

            // For each gap, calculate coverage for symbols that overlap with it
            for gap in &pack.gaps {
                for symbol in &gap.symbols {
                    let symbol_loc = symbol.line_end.saturating_sub(symbol.line_start) + 1;
                    if symbol_loc == 0 {
                        continue;
                    }

                    // Calculate how much of this symbol is uncovered
                    // The gap may only partially overlap the symbol
                    let overlap_start = gap.span.start.max(symbol.line_start);
                    let overlap_end = gap.span.end.min(symbol.line_end);
                    let uncovered_lines = if overlap_end >= overlap_start {
                        overlap_end - overlap_start + 1
                    } else {
                        0
                    };

                    let coverage_pct = 100.0 * (1.0 - (uncovered_lines as f64 / symbol_loc as f64));

                    // Store by (file, start, end) - use symbol's range
                    let key = (clean_path.clone(), symbol.line_start, symbol.line_end);
                    // If we already have coverage for this symbol, take the minimum (worst case)
                    entity_coverage
                        .entry(key)
                        .and_modify(|existing| *existing = existing.min(coverage_pct))
                        .or_insert(coverage_pct);
                }
            }
        }

        (file_coverage, entity_coverage)
    }

    /// Annotate existing candidates with coverage data from coverage packs.
    /// This modifies candidates in-place to add coverage_percentage field.
    fn annotate_candidates_with_coverage(
        candidates: &mut [RefactoringCandidate],
        coverage_packs: &[crate::detectors::coverage::CoveragePack],
    ) {
        let (file_coverage, entity_coverage) = Self::build_coverage_map(coverage_packs);

        for candidate in candidates.iter_mut() {
            // Try to find exact entity coverage match by line range
            if let Some((start, end)) = candidate.line_range {
                let key = (candidate.file_path.clone(), start, end);
                if let Some(&cov_pct) = entity_coverage.get(&key) {
                    candidate.coverage_percentage = Some(cov_pct);
                    continue;
                }

                // Try fuzzy match - find any symbol range that overlaps significantly
                for ((file, sym_start, sym_end), &cov_pct) in &entity_coverage {
                    if file == &candidate.file_path {
                        // Check for significant overlap (at least 50%)
                        let overlap_start = start.max(*sym_start);
                        let overlap_end = end.min(*sym_end);
                        if overlap_end >= overlap_start {
                            let overlap = overlap_end - overlap_start + 1;
                            let candidate_len = end - start + 1;
                            if overlap * 2 >= candidate_len {
                                candidate.coverage_percentage = Some(cov_pct);
                                break;
                            }
                        }
                    }
                }
            }

            // Fall back to file-level coverage if no entity match
            if candidate.coverage_percentage.is_none() {
                if let Some(&file_cov) = file_coverage.get(&candidate.file_path) {
                    candidate.coverage_percentage = Some(file_cov);
                }
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
                    code: issue_code_for_category(category),
                    category: category.clone(),
                    severity: score,
                    contributing_features,
                };

                issues.push(issue);
            }
        }

        // Generate suggestions based on issues
        let suggestions = Self::generate_suggestions(&issues, &name, line_range);

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

    /// Generate refactoring suggestions based on issues
    /// Generate refactoring suggestions based on issues and entity context
    fn generate_suggestions(
        issues: &[RefactoringIssue],
        entity_name: &str,
        line_range: Option<(usize, usize)>,
    ) -> Vec<RefactoringSuggestion> {
        use std::collections::HashSet;

        let mut suggestions = Vec::new();
        if issues.is_empty() {
            return suggestions;
        }

        let severity_label = |score: f64| {
            if score >= 2.0 {
                "very high"
            } else if score >= 1.5 {
                "high"
            } else if score >= 1.0 {
                "moderate"
            } else {
                "low"
            }
        };

        let mut emitted_codes: HashSet<String> = HashSet::new();

        for issue in issues {
            let severity_factor = (issue.severity / 2.0).clamp(0.1, 1.0);
            let base_priority = (0.45 + severity_factor * 0.5).clamp(0.1, 1.0);
            let base_impact = (0.55 + severity_factor * 0.35).min(1.0);

            let mut category_emitted = false;

            let mut emit = |kind: &str,
                            effort: f64,
                            priority_override: Option<f64>,
                            impact_override: Option<f64>| {
                let code = suggestion_code_for_kind(kind);
                if emitted_codes.insert(code.clone()) {
                    suggestions.push(RefactoringSuggestion {
                        refactoring_type: kind.to_string(),
                        code,
                        priority: priority_override.unwrap_or(base_priority),
                        effort: effort.clamp(0.1, 1.0),
                        impact: impact_override.unwrap_or(base_impact),
                    });
                }
            };

            for feature in &issue.contributing_features {
                let name = feature.feature_name.to_lowercase();
                let raw_value = feature.value;

                if name.contains("duplicate_code_count") && raw_value > 0.0 {
                    let duplicates = raw_value.round().max(1.0) as usize;
                    let impact = (base_impact + (duplicates as f64 * 0.05)).min(1.0);
                    emit(
                        &format!("eliminate_duplication_{}_blocks", duplicates),
                        0.65,
                        None,
                        Some(impact),
                    );
                    category_emitted = true;
                } else if name.contains("extract_method_count") && raw_value > 0.0 {
                    let occurrences = raw_value.round().max(1.0) as usize;
                    emit(
                        &format!("extract_method_{}_helpers", occurrences),
                        0.55,
                        None,
                        None,
                    );
                    category_emitted = true;
                } else if name.contains("extract_class_count") && raw_value > 0.0 {
                    let occurrences = raw_value.round().max(1.0) as usize;
                    emit(
                        &format!("extract_class_{}_areas", occurrences),
                        0.7,
                        None,
                        Some((base_impact + 0.1).min(1.0)),
                    );
                    category_emitted = true;
                } else if name.contains("simplify_conditionals_count") && raw_value > 0.0 {
                    let occurrences = raw_value.round().max(1.0) as usize;
                    emit(
                        &format!("simplify_{}_conditionals", occurrences),
                        0.45,
                        None,
                        None,
                    );
                    category_emitted = true;
                } else if name.contains("cyclomatic") && raw_value > 0.0 {
                    let complexity_level = raw_value.round() as u32;
                    emit(
                        &format!("reduce_cyclomatic_complexity_{}", complexity_level),
                        0.5,
                        Some((base_priority + 0.1).min(1.0)),
                        None,
                    );
                    category_emitted = true;
                } else if name.contains("cognitive") && raw_value > 0.0 {
                    let complexity_level = raw_value.round() as u32;
                    emit(
                        &format!("reduce_cognitive_complexity_{}", complexity_level),
                        0.5,
                        Some((base_priority + 0.1).min(1.0)),
                        None,
                    );
                    category_emitted = true;
                } else if name.contains("fan_in") || name.contains("fan_out") {
                    let fan_level = raw_value.round() as u32;
                    let fan_type = if name.contains("fan_in") {
                        "fan_in"
                    } else {
                        "fan_out"
                    };
                    emit(
                        &format!("reduce_{}_{}", fan_type, fan_level),
                        0.6,
                        None,
                        Some((base_impact + 0.1).min(1.0)),
                    );
                    category_emitted = true;
                } else if name.contains("centrality") || name.contains("choke") {
                    let centrality_level = raw_value.round() as u32;
                    let centrality_type = if name.contains("centrality") {
                        "centrality"
                    } else {
                        "chokepoint"
                    };
                    emit(
                        &format!("reduce_{}_{}", centrality_type, centrality_level),
                        0.65,
                        None,
                        Some((base_impact + 0.15).min(1.0)),
                    );
                    category_emitted = true;
                }
            }

            if !category_emitted {
                let severity = severity_label(issue.severity);

                let kind = match issue.category.as_str() {
                    "complexity" => match severity {
                        "very high" | "critical" => "extract_method_high_complexity",
                        "high" => "extract_method_complex",
                        "medium" => "reduce_nested_branching",
                        _ => "simplify_logic",
                    },
                    "structure" => match severity {
                        "very high" | "critical" => "extract_class_large_module",
                        "high" => "split_responsibilities",
                        "medium" => "move_method_better_cohesion",
                        _ => "organize_imports",
                    },
                    "graph" => match severity {
                        "very high" | "critical" => "introduce_facade_decouple_deps",
                        "high" => "extract_interface_dependency_inversion",
                        "medium" => "move_method_reduce_coupling",
                        _ => "inline_temp_simplify_deps",
                    },
                    "maintainability" => match severity {
                        "very high" | "critical" => "rename_class_improve_clarity",
                        "high" => "rename_method_improve_intent",
                        "medium" => "extract_variable_clarify_logic",
                        _ => "add_comments_explain_purpose",
                    },
                    "readability" => match severity {
                        "very high" | "critical" => "extract_method_clarify_intent",
                        "high" => "rename_variable_descriptive",
                        "medium" => "replace_magic_number_constant",
                        _ => "format_code_consistent_style",
                    },
                    _ => "refactor_code_quality",
                };

                emit(kind, 0.4, None, None);
            }
        }

        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::featureset::FeatureVector;
    use crate::core::pipeline::pipeline_results::{
        LshAnalysisResults as PipelineLshAnalysisResult, MemoryStats as PipelineMemoryStats,
        TfIdfStats,
    };
    use crate::core::pipeline::{
        AnalysisConfig, CloneVerificationResults, ComplexityAnalysisResults,
        ComprehensiveAnalysisResult, CoverageAnalysisResults, HealthMetrics, ImpactAnalysisResults,
        PipelineResults, PipelineStatistics, RefactoringAnalysisResults, ScoringResults,
        StructureAnalysisResults,
    };
    use crate::core::scoring::{Priority, ScoringResult};
    use crate::detectors::coverage::{
        CoverageGap, CoveragePack, FileInfo, GapFeatures, GapMarkers, GapSymbol, PackEffort,
        PackValue, SnippetPreview, SymbolKind, UncoveredSpan,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    fn sample_candidate(
        file_path: &str,
        name: &str,
        priority: Priority,
        category: &str,
        score: f64,
    ) -> RefactoringCandidate {
        RefactoringCandidate {
            entity_id: format!("{}:{}", file_path, name),
            name: name.to_string(),
            file_path: file_path.to_string(),
            line_range: Some((1, 5)),
            priority,
            score,
            confidence: 0.85,
            issues: vec![RefactoringIssue {
                code: format!("{}_CODE", category.to_uppercase()),
                category: category.to_string(),
                severity: 1.2,
                contributing_features: Vec::new(),
            }],
            suggestions: Vec::new(),
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        }
    }

    fn pipeline_results_fixture() -> PipelineResults {
        let summary = AnalysisSummary {
            files_processed: 2,
            entities_analyzed: 3,
            refactoring_needed: 1,
            high_priority: 1,
            critical: 0,
            avg_refactoring_score: 0.75,
            code_health_score: 0.82,
            total_files: 2,
            total_entities: 3,
            total_lines_of_code: 200,
            languages: vec!["rust".to_string()],
            total_issues: 1,
            high_priority_issues: 1,
            critical_issues: 0,
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        let structure = StructureAnalysisResults {
            enabled: true,
            directory_recommendations: Vec::new(),
            file_splitting_recommendations: Vec::new(),
            issues_count: 0,
        };

        let complexity = ComplexityAnalysisResults {
            enabled: true,
            detailed_results: Vec::new(),
            average_cyclomatic_complexity: 10.0,
            average_cognitive_complexity: 8.0,
            average_technical_debt_score: 0.3,
            average_maintainability_index: 0.7,
            issues_count: 1,
        };

        let refactoring = RefactoringAnalysisResults {
            enabled: true,
            detailed_results: Vec::new(),
            opportunities_count: 1,
        };

        let impact = ImpactAnalysisResults {
            enabled: true,
            dependency_cycles: Vec::new(),
            chokepoints: Vec::new(),
            clone_groups: Vec::new(),
            issues_count: 0,
        };

        let lsh = PipelineLshAnalysisResult {
            enabled: false,
            clone_pairs: Vec::new(),
            max_similarity: 0.0,
            avg_similarity: 0.0,
            duplicate_count: 0,
            apted_verification_enabled: false,
            verification: None,
            denoising_enabled: false,
            tfidf_stats: None,
        };

        let coverage = CoverageAnalysisResults {
            enabled: false,
            coverage_files_used: Vec::new(),
            coverage_gaps: Vec::new(),
            gaps_count: 0,
            overall_coverage_percentage: None,
            analysis_method: "none".to_string(),
        };

        let documentation = DocumentationAnalysisResults {
            enabled: false,
            issues_count: 0,
            doc_health_score: 100.0,
            file_doc_health: HashMap::new(),
            file_doc_issues: HashMap::new(),
            directory_doc_health: HashMap::new(),
            directory_doc_issues: HashMap::new(),
        };

        let health_metrics = HealthMetrics {
            overall_health_score: 0.82,
            maintainability_score: 0.78,
            technical_debt_ratio: 0.22,
            complexity_score: 20.0,
            structure_quality_score: 0.7,
            doc_health_score: 1.0,
        };

        let comprehensive = ComprehensiveAnalysisResult {
            analysis_id: "analysis-123".to_string(),
            timestamp: Utc::now(),
            processing_time: 1.25,
            config: AnalysisConfig::default(),
            summary: summary.clone(),
            structure,
            complexity,
            refactoring,
            impact,
            lsh,
            coverage,
            documentation,
            health_metrics,
        };

        let pipeline_statistics = PipelineStatistics {
            memory_stats: PipelineMemoryStats {
                current_memory_bytes: 750_000,
                peak_memory_bytes: 1_500_000,
                final_memory_bytes: 900_000,
                efficiency_score: 0.8,
            },
            files_processed: summary.files_processed,
            total_duration_ms: 250,
        };

        let mut scoring_result = ScoringResult {
            entity_id: "src/lib.rs:function:process_data".to_string(),
            overall_score: 45.0,
            priority: Priority::High,
            category_scores: HashMap::new(),
            feature_contributions: HashMap::new(),
            normalized_feature_count: 3,
            confidence: 0.9,
        };
        scoring_result
            .category_scores
            .insert("complexity".to_string(), 1.6);
        scoring_result
            .feature_contributions
            .insert("cyclomatic_complexity".to_string(), 1.2);

        let scoring_results = ScoringResults {
            files: vec![scoring_result.clone()],
        };

        let mut vector = FeatureVector::new(&scoring_result.entity_id);
        vector.add_feature("cyclomatic_complexity", 13.0);
        vector.add_metadata("name", json!("process_data"));
        vector.add_metadata("line_range", json!([12, 36]));

        PipelineResults {
            analysis_id: comprehensive.analysis_id.clone(),
            timestamp: comprehensive.timestamp,
            results: comprehensive,
            statistics: pipeline_statistics,
            errors: vec!["engine warning".to_string()],
            scoring_results,
            feature_vectors: vec![vector],
        }
    }

    fn sample_coverage_pack_json() -> serde_json::Value {
        let gap = CoverageGap {
            path: PathBuf::from("src/lib.rs"),
            span: UncoveredSpan {
                path: PathBuf::from("src/lib.rs"),
                start: 10,
                end: 18,
                hits: Some(0),
            },
            file_loc: 200,
            language: "rust".to_string(),
            score: 0.78,
            features: GapFeatures {
                gap_loc: 8,
                cyclomatic_in_gap: 1.2,
                cognitive_in_gap: 1.0,
                fan_in_gap: 3,
                exports_touched: false,
                dependency_centrality_file: 0.4,
                interface_surface: 2,
                docstring_or_comment_present: false,
                exception_density_in_gap: 0.0,
            },
            symbols: vec![GapSymbol {
                kind: SymbolKind::Function,
                name: "process_data".to_string(),
                signature: "fn process_data()".to_string(),
                line_start: 10,
                line_end: 18,
            }],
            preview: SnippetPreview {
                language: "rust".to_string(),
                pre: vec!["fn helper() {}".to_string()],
                head: vec!["fn process_data() {".to_string()],
                tail: vec!["}".to_string()],
                post: vec!["// end".to_string()],
                markers: GapMarkers {
                    start_line: 10,
                    end_line: 18,
                },
                imports: Vec::new(),
            },
        };

        let pack = CoveragePack {
            kind: "hotspot".to_string(),
            pack_id: "pack-1".to_string(),
            path: PathBuf::from("src/lib.rs"),
            file_info: FileInfo {
                loc: 200,
                coverage_before: 42.0,
                coverage_after_if_filled: 64.0,
            },
            gaps: vec![gap],
            value: PackValue {
                file_cov_gain: 12.0,
                repo_cov_gain_est: 2.4,
            },
            effort: PackEffort {
                tests_to_write_est: 2,
                mocks_est: 0,
            },
        };

        serde_json::to_value(pack).expect("pack serializes")
    }

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
            total_files: 10,
            total_entities: 50,
            total_lines_of_code: 1_000,
            languages: vec!["Rust".to_string()],
            total_issues: 3,
            high_priority_issues: 2,
            critical_issues: 1,
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        assert_eq!(summary.files_processed, 10);
        assert_eq!(summary.entities_analyzed, 50);
        assert_eq!(summary.refactoring_needed, 5);
        assert_eq!(summary.high_priority, 2);
        assert_eq!(summary.critical, 1);
        assert!((summary.code_health_score - 0.85).abs() < f64::EPSILON);
        assert_eq!(summary.total_files, 10);
        assert_eq!(summary.total_entities, 50);
        assert_eq!(summary.total_lines_of_code, 1_000);
        assert_eq!(summary.languages, vec!["Rust".to_string()]);
        assert_eq!(summary.total_issues, 3);
        assert_eq!(summary.high_priority_issues, 2);
        assert_eq!(summary.critical_issues, 1);
    }

    #[test]
    fn group_candidates_by_file_sorts_by_priority_and_score() {
        let candidates = vec![
            sample_candidate("src/lib.rs", "High", Priority::High, "complexity", 2.0),
            sample_candidate("src/lib.rs", "Low", Priority::Low, "structure", 0.3),
            sample_candidate(
                "src/critical.rs",
                "CriticalOne",
                Priority::Critical,
                "architecture",
                4.0,
            ),
        ];

        let groups = AnalysisResults::group_candidates_by_file(&candidates);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].file_path, "src/critical.rs");
        assert_eq!(groups[0].highest_priority, Priority::Critical);
        assert_eq!(groups[1].file_path, "src/lib.rs");
        assert_eq!(groups[1].entity_count, 2);
        assert!(groups[1].avg_score > 1.0);
    }

    #[test]
    fn top_issues_returns_sorted_categories() {
        let mut results = AnalysisResults::empty();
        results.refactoring_candidates = vec![
            sample_candidate("src/lib.rs", "One", Priority::High, "complexity", 2.0),
            sample_candidate("src/lib.rs", "Two", Priority::High, "complexity", 2.5),
            sample_candidate("src/utils.rs", "Three", Priority::Low, "structure", 1.0),
        ];

        let issues = results.top_issues(2);
        assert_eq!(issues[0].0, "complexity");
        assert_eq!(issues[0].1, 2);
        assert_eq!(issues[1].0, "structure");
        assert_eq!(issues[1].1, 1);
    }

    #[test]
    fn from_pipeline_results_populates_dictionary_and_warnings() {
        let pipeline_results = pipeline_results_fixture();
        let analysis = AnalysisResults::from_pipeline_results(pipeline_results);

        assert_eq!(analysis.summary.total_files, 2);
        assert_eq!(analysis.refactoring_candidates.len(), 1);
        assert!(analysis.code_dictionary.issues.contains_key("CMPLX"));
        assert!(analysis
            .code_dictionary
            .suggestions
            .contains_key("RDCYCLEX"));
        assert_eq!(analysis.warnings, vec!["engine warning".to_string()]);
    }

    #[test]
    fn convert_coverage_to_packs_filters_invalid_entries() {
        let mut coverage = CoverageAnalysisResults {
            enabled: true,
            coverage_files_used: Vec::new(),
            coverage_gaps: vec![sample_coverage_pack_json(), json!({"invalid": true})],
            gaps_count: 1,
            overall_coverage_percentage: Some(42.0),
            analysis_method: "coverage-py".to_string(),
        };

        let packs = AnalysisResults::convert_coverage_to_packs(&coverage);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].pack_id, "pack-1");
        assert_eq!(packs[0].gaps.len(), 1);

        coverage.enabled = false;
        assert!(AnalysisResults::convert_coverage_to_packs(&coverage).is_empty());
    }

    #[test]
    fn convert_lsh_to_clone_analysis_returns_details() {
        let mut pipeline_results = pipeline_results_fixture();
        {
            let lsh = &mut pipeline_results.results.lsh;
            lsh.enabled = true;
            lsh.denoising_enabled = true;
            lsh.clone_pairs = vec![json!({"pair": 1})];
            lsh.duplicate_count = 3;
            lsh.avg_similarity = 0.83;
            lsh.max_similarity = 0.93;
            lsh.verification = Some(CloneVerificationResults {
                method: "apted".to_string(),
                pairs_considered: 3,
                pairs_evaluated: 2,
                pairs_scored: 2,
                avg_similarity: Some(0.9),
            });
            lsh.tfidf_stats = Some(TfIdfStats {
                total_grams: 120,
                unique_grams: 40,
                top1pct_contribution: 0.35,
            });
        }

        let clone_analysis = AnalysisResults::convert_lsh_to_clone_analysis(&pipeline_results)
            .expect("should convert");
        assert!(clone_analysis.denoising_enabled);
        assert_eq!(clone_analysis.candidates_after_denoising, 3);
        assert!(clone_analysis
            .notes
            .iter()
            .any(|note| note.to_lowercase().contains("denoising")));
    }
}
