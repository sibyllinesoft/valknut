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

    /// Generate refactoring suggestions based on issues and entity context
    fn generate_suggestions(
        issues: &[RefactoringIssue],
        _entity_name: &str,
        _line_range: Option<(usize, usize)>,
    ) -> Vec<RefactoringSuggestion> {
        use std::collections::HashSet;

        if issues.is_empty() {
            return Vec::new();
        }

        let mut suggestions = Vec::new();
        let mut emitted_codes: HashSet<String> = HashSet::new();

        for issue in issues {
            let base = SuggestionBase::from_severity(issue.severity);
            let mut category_emitted = false;

            for feature in &issue.contributing_features {
                if let Some(suggestion) = Self::suggestion_for_feature(feature, &base) {
                    if emitted_codes.insert(suggestion.code.clone()) {
                        suggestions.push(suggestion);
                    }
                    category_emitted = true;
                }
            }

            if !category_emitted {
                let kind = Self::fallback_suggestion_kind(&issue.category, issue.severity);
                let code = suggestion_code_for_kind(kind);
                if emitted_codes.insert(code.clone()) {
                    suggestions.push(RefactoringSuggestion {
                        refactoring_type: kind.to_string(),
                        code,
                        priority: base.priority,
                        effort: 0.4,
                        impact: base.impact,
                    });
                }
            }
        }

        suggestions
    }

    fn suggestion_for_feature(
        feature: &FeatureContribution,
        base: &SuggestionBase,
    ) -> Option<RefactoringSuggestion> {
        let name = feature.feature_name.to_lowercase();
        let value = feature.value;

        if value <= 0.0 {
            return None;
        }

        let count = value.round().max(1.0) as usize;

        // Feature pattern matching with associated suggestion params
        let (kind_fmt, effort, priority_boost, impact_boost): (&str, f64, f64, f64) =
            if name.contains("duplicate_code_count") {
                ("eliminate_duplication_{}_blocks", 0.65, 0.0, count as f64 * 0.05)
            } else if name.contains("extract_method_count") {
                ("extract_method_{}_helpers", 0.55, 0.0, 0.0)
            } else if name.contains("extract_class_count") {
                ("extract_class_{}_areas", 0.7, 0.0, 0.1)
            } else if name.contains("simplify_conditionals_count") {
                ("simplify_{}_conditionals", 0.45, 0.0, 0.0)
            } else if name.contains("cyclomatic") {
                ("reduce_cyclomatic_complexity_{}", 0.5, 0.1, 0.0)
            } else if name.contains("cognitive") {
                ("reduce_cognitive_complexity_{}", 0.5, 0.1, 0.0)
            } else if name.contains("fan_in") {
                ("reduce_fan_in_{}", 0.6, 0.0, 0.1)
            } else if name.contains("fan_out") {
                ("reduce_fan_out_{}", 0.6, 0.0, 0.1)
            } else if name.contains("centrality") {
                ("reduce_centrality_{}", 0.65, 0.0, 0.15)
            } else if name.contains("choke") {
                ("reduce_chokepoint_{}", 0.65, 0.0, 0.15)
            } else {
                return None;
            };

        let kind = kind_fmt.replace("{}", &count.to_string());
        let code = suggestion_code_for_kind(&kind);

        Some(RefactoringSuggestion {
            refactoring_type: kind,
            code,
            priority: (base.priority + priority_boost).min(1.0),
            effort: effort.clamp(0.1, 1.0),
            impact: (base.impact + impact_boost).min(1.0),
        })
    }

    fn fallback_suggestion_kind(category: &str, severity: f64) -> &'static str {
        let severity_level = if severity >= 2.0 {
            3 // very high / critical
        } else if severity >= 1.5 {
            2 // high
        } else if severity >= 1.0 {
            1 // moderate
        } else {
            0 // low
        };

        match (category, severity_level) {
            ("complexity", 3 | 2) => "extract_method_high_complexity",
            ("complexity", 1) => "reduce_nested_branching",
            ("complexity", _) => "simplify_logic",

            ("structure", 3 | 2) => "extract_class_large_module",
            ("structure", 1) => "move_method_better_cohesion",
            ("structure", _) => "organize_imports",

            ("graph", 3 | 2) => "introduce_facade_decouple_deps",
            ("graph", 1) => "move_method_reduce_coupling",
            ("graph", _) => "inline_temp_simplify_deps",

            ("maintainability", 3 | 2) => "rename_class_improve_clarity",
            ("maintainability", 1) => "extract_variable_clarify_logic",
            ("maintainability", _) => "add_comments_explain_purpose",

            ("readability", 3 | 2) => "extract_method_clarify_intent",
            ("readability", 1) => "replace_magic_number_constant",
            ("readability", _) => "format_code_consistent_style",

            _ => "refactor_code_quality",
        }
    }
}

/// Helper struct for suggestion base values computed from issue severity
struct SuggestionBase {
    priority: f64,
    impact: f64,
}

impl SuggestionBase {
    fn from_severity(severity: f64) -> Self {
        let severity_factor = (severity / 2.0).clamp(0.1, 1.0);
        Self {
            priority: (0.45 + severity_factor * 0.5).clamp(0.1, 1.0),
            impact: (0.55 + severity_factor * 0.35).min(1.0),
        }
    }
}


#[cfg(test)]
#[path = "result_conversions_tests.rs"]
mod tests;
