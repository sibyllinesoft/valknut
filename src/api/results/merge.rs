use std::collections::HashMap;
use std::time::Duration;

use super::models::{
    AnalysisResults, CloneAnalysisPerformance, CloneAnalysisResults, MemoryStats,
    PhaseFilteringStats, StageResultsBundle,
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

        self.passes = merge_stage_results(self.passes.clone(), other.passes);

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

        self.coverage_packs.extend(other.coverage_packs.into_iter());
        self.warnings.extend(other.warnings.into_iter());
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

        self.verification = match (self.verification.take(), other.verification) {
            (Some(mut left), Some(right)) => {
                let left_scored = left.pairs_scored;
                let right_scored = right.pairs_scored;
                let combined_scored = left_scored + right_scored;

                left.pairs_considered += right.pairs_considered;
                left.pairs_evaluated += right.pairs_evaluated;
                left.pairs_scored = combined_scored;

                left.avg_similarity = match (left.avg_similarity, right.avg_similarity) {
                    (Some(a), Some(b)) if combined_scored > 0 => Some(
                        ((a * left_scored as f64) + (b * right_scored as f64))
                            / combined_scored as f64,
                    ),
                    (Some(a), _) => Some(a),
                    (_, Some(b)) => Some(b),
                    _ => None,
                };

                Some(left)
            }
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right),
            (None, None) => None,
        };

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

fn merge_stage_results(
    current: StageResultsBundle,
    incoming: StageResultsBundle,
) -> StageResultsBundle {
    StageResultsBundle {
        structure: if incoming.structure.enabled {
            incoming.structure
        } else {
            current.structure
        },
        coverage: if incoming.coverage.enabled {
            incoming.coverage
        } else {
            current.coverage
        },
        complexity: if incoming.complexity.enabled {
            incoming.complexity
        } else {
            current.complexity
        },
        refactoring: if incoming.refactoring.enabled {
            incoming.refactoring
        } else {
            current.refactoring
        },
        impact: if incoming.impact.enabled {
            incoming.impact
        } else {
            current.impact
        },
        lsh: if incoming.lsh.enabled {
            incoming.lsh
        } else {
            current.lsh
        },
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::results::{
        FeatureContribution, RefactoringCandidate, RefactoringIssue, RefactoringSuggestion,
    };
    use crate::core::pipeline::CloneVerificationResults;
    use crate::core::scoring::Priority;
    use std::collections::HashMap;

    fn sample_candidate(file_path: &str, priority: Priority, score: f64) -> RefactoringCandidate {
        RefactoringCandidate {
            entity_id: format!("{}:entity", file_path),
            name: "entity".into(),
            file_path: file_path.into(),
            line_range: Some((1, 10)),
            priority,
            score,
            confidence: 0.9,
            issues: vec![RefactoringIssue {
                code: "complexity".into(),
                category: "complexity".into(),
                severity: 0.8,
                contributing_features: vec![FeatureContribution {
                    feature_name: "cyclomatic".into(),
                    value: 12.0,
                    normalized_value: 0.6,
                    contribution: 0.3,
                }],
            }],
            suggestions: vec![RefactoringSuggestion {
                refactoring_type: "extract_method".into(),
                code: "extract_method".into(),
                priority: 0.7,
                effort: 0.4,
                impact: 0.6,
            }],
            issue_count: 1,
            suggestion_count: 1,
        }
    }

    fn sample_clone_analysis(
        after: usize,
        quality: f64,
        max_similarity: f64,
    ) -> CloneAnalysisResults {
        CloneAnalysisResults {
            denoising_enabled: true,
            auto_calibration_applied: Some(true),
            candidates_before_denoising: Some(after * 2),
            candidates_after_denoising: after,
            calibrated_threshold: Some(0.7),
            quality_score: Some(quality),
            avg_similarity: Some(0.5),
            max_similarity: Some(max_similarity),
            verification: Some(CloneVerificationResults {
                method: "apted".into(),
                pairs_considered: after * 3,
                pairs_evaluated: after * 2,
                pairs_scored: after,
                avg_similarity: Some(0.6),
            }),
            phase_filtering_stats: Some(PhaseFilteringStats {
                phase1_weighted_signature: after,
                phase2_structural_gates: after + 1,
                phase3_stop_motifs_filter: after + 2,
                phase4_payoff_ranking: after + 3,
            }),
            performance_metrics: Some(CloneAnalysisPerformance {
                total_time_ms: Some((after as u64) * 100),
                memory_usage_bytes: Some(1024 * after as u64),
                entities_per_second: Some(10.0),
            }),
            notes: vec!["left".into()],
        }
    }

    fn sample_health(score: f64) -> HealthMetrics {
        HealthMetrics {
            overall_health_score: score,
            maintainability_score: score - 0.1,
            technical_debt_ratio: 1.0 - score,
            complexity_score: score - 0.2,
            structure_quality_score: score + 0.1,
        }
    }

    #[test]
    fn weighted_average_respects_weights() {
        let result = weighted_average(0.6, 4, 0.2, 1);
        assert!((result - 0.52).abs() < 1e-6);

        let equal_weight = weighted_average(0.5, 0, 0.7, 0);
        assert!((equal_weight - 0.6).abs() < 1e-6);
    }

    #[test]
    fn weighted_duration_handles_zero_weight() {
        let result = weighted_duration(Duration::from_secs(3), 2, Duration::from_secs(9), 3);
        assert!((result.as_secs_f64() - 6.6).abs() < 1e-6);

        let zero = weighted_duration(Duration::from_secs(10), 0, Duration::from_secs(5), 0);
        assert_eq!(zero.as_secs(), 0);
    }

    #[test]
    fn optional_merges_behave() {
        assert_eq!(merge_optional_bool(Some(true), Some(false)), Some(true));
        assert_eq!(merge_optional_sum(Some(2_usize), Some(3)), Some(5));
        assert_eq!(merge_optional_sum_u64(Some(2), Some(3)), Some(5));
        assert_eq!(merge_optional_max_u64(Some(4), Some(6)), Some(6));
        let averaged = merge_optional_average(Some(0.6), 2, Some(0.2), 1).unwrap();
        assert!((averaged - 0.4666666666666667).abs() < 1e-12);
    }

    #[test]
    fn health_metrics_merge_weighted() {
        let merged = merge_health_metrics(Some(sample_health(0.7)), 2, Some(sample_health(0.3)), 3)
            .expect("merged metrics");

        assert!((merged.overall_health_score - 0.46).abs() < 1e-6);
        assert!((merged.structure_quality_score - 0.56).abs() < 1e-6);
    }

    #[test]
    fn clone_analysis_performance_merge_accumulates() {
        let mut left = CloneAnalysisPerformance {
            total_time_ms: Some(1000),
            memory_usage_bytes: Some(2048),
            entities_per_second: Some(15.0),
        };
        let right = CloneAnalysisPerformance {
            total_time_ms: Some(400),
            memory_usage_bytes: Some(4096),
            entities_per_second: Some(5.0),
        };

        left.merge(right);

        assert_eq!(left.total_time_ms, Some(1400));
        assert_eq!(left.memory_usage_bytes, Some(4096));
        assert!(left.entities_per_second.unwrap() > 10.0);
    }

    #[test]
    fn clone_analysis_results_merge_combines() {
        let mut left = sample_clone_analysis(4, 0.8, 0.9);
        let right = CloneAnalysisResults {
            denoising_enabled: false,
            auto_calibration_applied: Some(false),
            candidates_before_denoising: Some(2),
            candidates_after_denoising: 2,
            calibrated_threshold: Some(0.5),
            quality_score: Some(0.2),
            avg_similarity: Some(0.4),
            max_similarity: Some(0.5),
            verification: Some(CloneVerificationResults {
                method: "apted".into(),
                pairs_considered: 4,
                pairs_evaluated: 3,
                pairs_scored: 2,
                avg_similarity: Some(0.5),
            }),
            phase_filtering_stats: Some(PhaseFilteringStats {
                phase1_weighted_signature: 1,
                phase2_structural_gates: 2,
                phase3_stop_motifs_filter: 3,
                phase4_payoff_ranking: 4,
            }),
            performance_metrics: Some(CloneAnalysisPerformance {
                total_time_ms: Some(200),
                memory_usage_bytes: Some(512),
                entities_per_second: Some(12.0),
            }),
            notes: vec!["right".into()],
        };

        left.merge(right);

        assert_eq!(left.candidates_after_denoising, 6);
        assert_eq!(left.candidates_before_denoising, Some(10));
        assert_eq!(left.notes.len(), 2);
        assert!(left.denoising_enabled);
        assert_eq!(
            left.phase_filtering_stats
                .as_ref()
                .unwrap()
                .phase1_weighted_signature,
            5
        );
        assert_eq!(
            left.performance_metrics.as_ref().unwrap().total_time_ms,
            Some(600)
        );
        assert!((left.quality_score.unwrap() - 0.6).abs() < 1e-6);
        assert!((left.max_similarity.unwrap() - 0.7666666667).abs() < 1e-6);
    }

    #[test]
    fn analysis_results_merge_in_place_combines_everything() {
        let mut left = AnalysisResults::empty();
        left.summary.files_processed = 2;
        left.summary.entities_analyzed = 4;
        left.summary.refactoring_needed = 1;
        left.summary.high_priority = 1;
        left.summary.avg_refactoring_score = 0.6;
        left.summary.code_health_score = 0.8;
        left.health_metrics = Some(sample_health(0.7));
        left.statistics.total_duration = Duration::from_secs(10);
        left.statistics.avg_file_processing_time = Duration::from_secs(3);
        left.statistics.avg_entity_processing_time = Duration::from_secs(2);
        left.statistics
            .features_per_entity
            .insert("cyclomatic".into(), 2.0);
        left.statistics
            .priority_distribution
            .insert("High".into(), 1);
        left.statistics
            .issue_distribution
            .insert("complexity".into(), 1);
        left.statistics.memory_stats = MemoryStats {
            peak_memory_bytes: 100,
            final_memory_bytes: 80,
            efficiency_score: 0.5,
        };
        left.clone_analysis = Some(sample_clone_analysis(4, 0.8, 0.9));
        left.refactoring_candidates = vec![sample_candidate("src/lib.rs", Priority::High, 0.6)];
        left.coverage_packs = Vec::new();
        left.warnings.push("left warning".into());
        left.passes.structure.enabled = true;
        left.passes.structure.issues_count = 5;

        let mut right = AnalysisResults::empty();
        right.summary.files_processed = 3;
        right.summary.entities_analyzed = 6;
        right.summary.refactoring_needed = 2;
        right.summary.high_priority = 0;
        right.summary.avg_refactoring_score = 0.3;
        right.summary.code_health_score = 0.4;
        right.health_metrics = Some(sample_health(0.3));
        right.statistics.total_duration = Duration::from_secs(20);
        right.statistics.avg_file_processing_time = Duration::from_secs(9);
        right.statistics.avg_entity_processing_time = Duration::from_secs(4);
        right
            .statistics
            .features_per_entity
            .insert("nesting".into(), 1.5);
        right
            .statistics
            .priority_distribution
            .insert("Medium".into(), 3);
        right
            .statistics
            .issue_distribution
            .insert("structure".into(), 2);
        right.statistics.memory_stats = MemoryStats {
            peak_memory_bytes: 120,
            final_memory_bytes: 90,
            efficiency_score: 0.9,
        };
        right.clone_analysis = Some(sample_clone_analysis(2, 0.2, 0.4));
        if let Some(clone) = right.clone_analysis.as_mut() {
            clone.notes = vec!["right".into()];
        }
        right.refactoring_candidates =
            vec![sample_candidate("tests/main.rs", Priority::Medium, 0.3)];
        right.warnings.push("right warning".into());
        right.passes.coverage.enabled = true;
        right.passes.coverage.gaps_count = 2;

        left.merge_in_place(right);

        assert_eq!(left.summary.files_processed, 5);
        assert_eq!(left.summary.entities_analyzed, 10);
        assert_eq!(left.summary.refactoring_needed, 3);
        assert!((left.summary.avg_refactoring_score - 0.42).abs() < 1e-6);
        assert!((left.summary.code_health_score - 0.56).abs() < 1e-6);
        assert!(left.statistics.priority_distribution.contains_key("Medium"));
        assert!(left.statistics.issue_distribution.contains_key("structure"));
        assert!(left.statistics.memory_stats.peak_memory_bytes >= 120);
        assert_eq!(left.clone_analysis.as_ref().unwrap().notes.len(), 2);
        assert!(left
            .warnings
            .iter()
            .any(|warning| warning == "left warning"));
        assert!(left
            .warnings
            .iter()
            .any(|warning| warning == "right warning"));
        assert!(left.passes.structure.enabled);
        assert!(left.passes.coverage.enabled);

        let health = left.health_metrics.unwrap();
        assert!((health.overall_health_score - 0.46).abs() < 1e-6);
    }
}
