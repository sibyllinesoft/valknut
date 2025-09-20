use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{self, json};

use crate::core::featureset::FeatureVector;
use crate::core::pipeline::{PipelineResults, ResultSummary};
use crate::core::scoring::{Priority, ScoringResult};

use super::result_types::*;

impl AnalysisResults {
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
            },
            refactoring_candidates: Vec::new(),
            refactoring_candidates_by_file: Vec::new(),
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
            directory_health_tree: None,
            clone_analysis: None,
            coverage_packs: Vec::new(),
            unified_hierarchy: Vec::new(),
            warnings: Vec::new(),
            health_metrics: None,
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

        // Build directory health tree from pipeline results
        let directory_health_tree =
            Self::build_directory_health_tree(&pipeline_results, &refactoring_candidates);

        // Convert LSH results to clone analysis results
        let clone_analysis = Self::convert_lsh_to_clone_analysis(&pipeline_results);

        // Extract coverage packs from pipeline results
        let coverage_packs = Self::convert_coverage_to_packs(&pipeline_results.results.coverage);

        // Build unified hierarchy from refactoring candidates
        let unified_hierarchy = Self::build_unified_hierarchy(&refactoring_candidates);

        let health_metrics = Some(pipeline_results.results.health_metrics.clone());

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
            health_metrics,
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
            phase_filtering_stats: None,
            performance_metrics: None,
            notes,
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
            total_files: 10,
            total_entities: 50,
            total_lines_of_code: 1_000,
            languages: vec!["Rust".to_string()],
            total_issues: 3,
            high_priority_issues: 2,
            critical_issues: 1,
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
}
