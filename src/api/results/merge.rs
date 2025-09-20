use std::collections::HashMap;
use std::time::Duration;

use super::models::{
    AnalysisResults, CloneAnalysisPerformance, CloneAnalysisResults, DirectoryHealthTree,
    MemoryStats, PhaseFilteringStats,
};
use crate::core::pipeline::HealthMetrics;

impl AnalysisResults {
    pub fn merge(mut self, other: AnalysisResults) -> Self {
        self.merge_in_place(other);
        self
    }

    pub fn merge_in_place(&mut self, other: AnalysisResults) {
        let base_files = self.summary.files_processed;
        let base_entities = self.summary.entities_analyzed;
        let other_files = other.summary.files_processed;
        let other_entities = other.summary.entities_analyzed;

        self.summary.files_processed += other_files;
        self.summary.entities_analyzed += other_entities;
        self.summary.refactoring_needed += other.summary.refactoring_needed;
        self.summary.high_priority += other.summary.high_priority;
        self.summary.critical += other.summary.critical;

        self.summary.avg_refactoring_score = weighted_average(
            self.summary.avg_refactoring_score,
            base_entities,
            other.summary.avg_refactoring_score,
            other_entities,
        );
        self.summary.code_health_score = weighted_average(
            self.summary.code_health_score,
            base_files,
            other.summary.code_health_score,
            other_files,
        );

        self.health_metrics = merge_health_metrics(
            self.health_metrics.clone(),
            base_files,
            other.health_metrics.clone(),
            other_files,
        );

        self.refactoring_candidates
            .extend(other.refactoring_candidates.into_iter());

        self.statistics.total_duration += other.statistics.total_duration;
        self.statistics.avg_file_processing_time = weighted_duration(
            self.statistics.avg_file_processing_time,
            base_files,
            other.statistics.avg_file_processing_time,
            other_files,
        );
        self.statistics.avg_entity_processing_time = weighted_duration(
            self.statistics.avg_entity_processing_time,
            base_entities,
            other.statistics.avg_entity_processing_time,
            other_entities,
        );

        merge_maps(
            &mut self.statistics.features_per_entity,
            other.statistics.features_per_entity,
        );
        merge_count_maps(
            &mut self.statistics.priority_distribution,
            other.statistics.priority_distribution,
        );
        merge_count_maps(
            &mut self.statistics.issue_distribution,
            other.statistics.issue_distribution,
        );

        self.statistics
            .memory_stats
            .merge(other.statistics.memory_stats);

        match (&mut self.clone_analysis, other.clone_analysis) {
            (Some(current), Some(extra)) => current.merge(extra),
            (None, Some(extra)) => self.clone_analysis = Some(extra),
            _ => {}
        }

        if let Some(current_tree) = &mut self.directory_health_tree {
            if let Some(mut new_tree) = other.directory_health_tree {
                merge_directory_health(current_tree, &mut new_tree);
            }
        } else {
            self.directory_health_tree = other.directory_health_tree;
        }

        self.coverage_packs.extend(other.coverage_packs.into_iter());
        // NOTE: Do NOT extend unified_hierarchy here - it flattens the tree structure
        // We rebuild it properly after all merging is complete
        self.warnings.extend(other.warnings.into_iter());

        self.refactoring_candidates_by_file =
            AnalysisResults::group_candidates_by_file(&self.refactoring_candidates);
            
        // Rebuild unified hierarchy after merge to restore proper hierarchical structure
        if let Some(ref directory_health_tree) = self.directory_health_tree {
            self.unified_hierarchy = Self::build_unified_hierarchy_with_fallback(
                &self.refactoring_candidates,
                directory_health_tree,
            );
        }
    }
}

impl CloneAnalysisResults {
    pub fn merge(&mut self, other: CloneAnalysisResults) {
        self.denoising_enabled |= other.denoising_enabled;
        self.auto_calibration_applied = merge_optional_bool(
            self.auto_calibration_applied,
            other.auto_calibration_applied,
        );

        self.candidates_before_denoising = merge_optional_sum(
            self.candidates_before_denoising,
            other.candidates_before_denoising,
        );

        let base_after = self.candidates_after_denoising;
        let other_after = other.candidates_after_denoising;
        self.candidates_after_denoising += other_after;

        self.calibrated_threshold = merge_optional_average(
            self.calibrated_threshold,
            base_after,
            other.calibrated_threshold,
            other_after,
        );

        self.quality_score = merge_optional_average(
            self.quality_score,
            base_after,
            other.quality_score,
            other_after,
        );

        self.avg_similarity = merge_optional_average(
            self.avg_similarity,
            base_after,
            other.avg_similarity,
            other_after,
        );

        self.max_similarity = merge_optional_average(
            self.max_similarity,
            base_after,
            other.max_similarity,
            other_after,
        );

        match (&mut self.phase_filtering_stats, other.phase_filtering_stats) {
            (Some(current), Some(extra)) => current.merge(extra),
            (None, Some(extra)) => self.phase_filtering_stats = Some(extra),
            _ => {}
        }

        match (&mut self.performance_metrics, other.performance_metrics) {
            (Some(current), Some(extra)) => current.merge(extra),
            (None, Some(extra)) => self.performance_metrics = Some(extra),
            _ => {}
        }

        if !other.notes.is_empty() {
            self.notes.extend(other.notes);
            self.notes.sort();
            self.notes.dedup();
        }
    }
}

impl PhaseFilteringStats {
    pub fn merge(&mut self, other: PhaseFilteringStats) {
        self.phase1_weighted_signature += other.phase1_weighted_signature;
        self.phase2_structural_gates += other.phase2_structural_gates;
        self.phase3_stop_motifs_filter += other.phase3_stop_motifs_filter;
        self.phase4_payoff_ranking += other.phase4_payoff_ranking;
    }
}

impl CloneAnalysisPerformance {
    pub fn merge(&mut self, other: CloneAnalysisPerformance) {
        let base_time = self.total_time_ms;
        let base_entities_per_second = self.entities_per_second;
        let incoming_time = other.total_time_ms;

        self.total_time_ms = merge_optional_sum_u64(base_time, incoming_time);
        self.memory_usage_bytes =
            merge_optional_max_u64(self.memory_usage_bytes, other.memory_usage_bytes);
        self.entities_per_second = merge_optional_average(
            base_entities_per_second,
            base_time.unwrap_or(0) as usize,
            other.entities_per_second,
            incoming_time.unwrap_or(0) as usize,
        );
    }
}

fn merge_maps(map: &mut HashMap<String, f64>, other: HashMap<String, f64>) {
    for (key, value) in other {
        *map.entry(key).or_insert(0.0) += value;
    }
}

fn merge_count_maps(map: &mut HashMap<String, usize>, other: HashMap<String, usize>) {
    for (key, value) in other {
        *map.entry(key).or_insert(0) += value;
    }
}

fn merge_directory_health(current: &mut DirectoryHealthTree, incoming: &mut DirectoryHealthTree) {
    current.directories.extend(incoming.directories.drain());

    let mut combined_hotspots = current.tree_statistics.hotspot_directories.clone();
    combined_hotspots.extend(
        incoming
            .tree_statistics
            .hotspot_directories
            .clone()
            .into_iter(),
    );

    combined_hotspots.sort_by(|a, b| {
        a.health_score
            .partial_cmp(&b.health_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    combined_hotspots.dedup_by(|a, b| a.path == b.path);
    current.tree_statistics.hotspot_directories = combined_hotspots;
}

fn weighted_average(
    current: f64,
    current_weight: usize,
    additional: f64,
    additional_weight: usize,
) -> f64 {
    let total_weight = current_weight + additional_weight;
    if total_weight == 0 {
        return (current + additional) / 2.0;
    }

    ((current * current_weight as f64) + (additional * additional_weight as f64))
        / total_weight as f64
}

fn weighted_duration(
    current: Duration,
    current_weight: usize,
    additional: Duration,
    additional_weight: usize,
) -> Duration {
    let total_weight = current_weight + additional_weight;
    if total_weight == 0 {
        return Duration::from_secs(0);
    }

    let current_secs = current.as_secs_f64();
    let additional_secs = additional.as_secs_f64();
    Duration::from_secs_f64(
        ((current_secs * current_weight as f64) + (additional_secs * additional_weight as f64))
            / total_weight as f64,
    )
}

fn merge_optional_bool(current: Option<bool>, incoming: Option<bool>) -> Option<bool> {
    match (current, incoming) {
        (Some(a), Some(b)) => Some(a || b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn merge_optional_sum(current: Option<usize>, incoming: Option<usize>) -> Option<usize> {
    match (current, incoming) {
        (Some(a), Some(b)) => Some(a + b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn merge_optional_sum_u64(current: Option<u64>, incoming: Option<u64>) -> Option<u64> {
    match (current, incoming) {
        (Some(a), Some(b)) => Some(a + b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn merge_optional_max_u64(current: Option<u64>, incoming: Option<u64>) -> Option<u64> {
    match (current, incoming) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn merge_optional_average(
    current: Option<f64>,
    current_weight: usize,
    incoming: Option<f64>,
    incoming_weight: usize,
) -> Option<f64> {
    match (current, incoming) {
        (Some(a), Some(b)) => {
            let total_weight = current_weight + incoming_weight;
            if total_weight == 0 {
                Some((a + b) / 2.0)
            } else {
                Some(
                    ((a * current_weight as f64) + (b * incoming_weight as f64))
                        / total_weight as f64,
                )
            }
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn merge_health_metrics(
    current: Option<HealthMetrics>,
    current_weight: usize,
    incoming: Option<HealthMetrics>,
    incoming_weight: usize,
) -> Option<HealthMetrics> {
    match (current, incoming) {
        (Some(a), Some(b)) => {
            let total_weight = current_weight + incoming_weight;
            if total_weight == 0 {
                return Some(HealthMetrics {
                    overall_health_score: (a.overall_health_score + b.overall_health_score) / 2.0,
                    maintainability_score: (a.maintainability_score + b.maintainability_score)
                        / 2.0,
                    technical_debt_ratio: (a.technical_debt_ratio + b.technical_debt_ratio) / 2.0,
                    complexity_score: (a.complexity_score + b.complexity_score) / 2.0,
                    structure_quality_score: (a.structure_quality_score
                        + b.structure_quality_score)
                        / 2.0,
                });
            }

            Some(HealthMetrics {
                overall_health_score: weighted_average(
                    a.overall_health_score,
                    current_weight,
                    b.overall_health_score,
                    incoming_weight,
                ),
                maintainability_score: weighted_average(
                    a.maintainability_score,
                    current_weight,
                    b.maintainability_score,
                    incoming_weight,
                ),
                technical_debt_ratio: weighted_average(
                    a.technical_debt_ratio,
                    current_weight,
                    b.technical_debt_ratio,
                    incoming_weight,
                ),
                complexity_score: weighted_average(
                    a.complexity_score,
                    current_weight,
                    b.complexity_score,
                    incoming_weight,
                ),
                structure_quality_score: weighted_average(
                    a.structure_quality_score,
                    current_weight,
                    b.structure_quality_score,
                    incoming_weight,
                ),
            })
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}
