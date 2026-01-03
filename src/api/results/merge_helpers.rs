//! Helper functions for merging analysis results.
//!
//! This module contains utility functions for combining and averaging
//! values during the merge of two AnalysisResults.

use std::collections::HashMap;
use std::time::Duration;

use crate::core::pipeline::HealthMetrics;

use super::models::StageResultsBundle;

/// Merge two HashMaps by summing f64 values for matching keys.
pub fn merge_maps(map: &mut HashMap<String, f64>, other: HashMap<String, f64>) {
    for (key, value) in other {
        *map.entry(key).or_insert(0.0) += value;
    }
}

/// Merge two HashMaps by summing usize values for matching keys.
pub fn merge_count_maps(map: &mut HashMap<String, usize>, other: HashMap<String, usize>) {
    for (key, value) in other {
        *map.entry(key).or_insert(0) += value;
    }
}

/// Merge stage results, preferring enabled stages from the incoming bundle.
pub fn merge_stage_results(
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
        cohesion: if incoming.cohesion.enabled {
            incoming.cohesion
        } else {
            current.cohesion
        },
    }
}

/// Calculate a weighted average of two f64 values.
pub fn weighted_average(
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

/// Calculate a weighted average of two Durations.
pub fn weighted_duration(
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

/// Merge two optional bools using OR logic.
pub fn merge_optional_bool(current: Option<bool>, incoming: Option<bool>) -> Option<bool> {
    match (current, incoming) {
        (Some(a), Some(b)) => Some(a || b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Merge two optional usize values by summing.
pub fn merge_optional_sum(current: Option<usize>, incoming: Option<usize>) -> Option<usize> {
    match (current, incoming) {
        (Some(a), Some(b)) => Some(a + b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Merge two optional u64 values by summing.
pub fn merge_optional_sum_u64(current: Option<u64>, incoming: Option<u64>) -> Option<u64> {
    match (current, incoming) {
        (Some(a), Some(b)) => Some(a + b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Merge two optional u64 values by taking the maximum.
pub fn merge_optional_max_u64(current: Option<u64>, incoming: Option<u64>) -> Option<u64> {
    match (current, incoming) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Merge two optional f64 values using weighted average.
pub fn merge_optional_average(
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

/// Merge two optional HealthMetrics using weighted average for each field.
pub fn merge_health_metrics(
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
                    doc_health_score: (a.doc_health_score + b.doc_health_score) / 2.0,
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
                doc_health_score: weighted_average(
                    a.doc_health_score,
                    current_weight,
                    b.doc_health_score,
                    incoming_weight,
                ),
            })
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}
