//! Coverage gap scoring and ranking utilities.
//!
//! This module provides scoring algorithms for prioritizing coverage gaps
//! based on various metrics like complexity, fan-in, and file centrality.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::errors::Result;

use super::config::CoverageConfig;
use super::types::CoverageGap;

/// Metrics computed for a single file's coverage gaps.
#[derive(Debug, Clone)]
pub struct FileMetrics {
    /// Total lines of code in all gaps for this file.
    pub total_gap_loc: usize,
    /// Average complexity across all gaps.
    pub avg_complexity: f64,
    /// Estimated centrality of this file in the codebase.
    pub centrality: f64,
    /// Number of gaps in this file.
    pub gap_count: usize,
}

/// Score and rank coverage gaps based on configured weights.
pub fn score_gaps(config: &CoverageConfig, gaps: &mut [CoverageGap]) -> Result<()> {
    let weights = &config.weights;
    let file_metrics = calculate_file_metrics(gaps)?;

    for gap in gaps.iter_mut() {
        if let Some(metrics) = file_metrics.get(&gap.path) {
            gap.features.dependency_centrality_file = metrics.centrality;
            gap.file_loc = gap.file_loc.max(metrics.total_gap_loc);
        }

        let size_score = normalize_size_score(gap.features.gap_loc);
        let complexity_score = normalize_complexity_score(
            gap.features.cyclomatic_in_gap + gap.features.cognitive_in_gap,
        );
        let fan_in_score = normalize_fan_in_score(gap.features.fan_in_gap);
        let exports_score = if gap.features.exports_touched {
            1.0
        } else {
            0.0
        };
        let centrality_score = gap.features.dependency_centrality_file;
        let docs_score = if gap.features.docstring_or_comment_present {
            0.0
        } else {
            1.0
        };

        gap.score = (size_score * weights.size)
            + (complexity_score * weights.complexity)
            + (fan_in_score * weights.fan_in)
            + (exports_score * weights.exports)
            + (centrality_score * weights.centrality)
            + (docs_score * weights.docs);

        gap.score = gap.score.clamp(0.0, 1.0);
    }

    gaps.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(())
}

/// Calculate aggregate metrics for each file's coverage gaps.
pub fn calculate_file_metrics(gaps: &[CoverageGap]) -> Result<HashMap<PathBuf, FileMetrics>> {
    let mut metrics = HashMap::new();
    let mut grouped: HashMap<PathBuf, Vec<&CoverageGap>> = HashMap::new();
    for gap in gaps {
        grouped.entry(gap.path.clone()).or_default().push(gap);
    }

    for (path, file_gaps) in grouped {
        let total_gap_loc: usize = file_gaps.iter().map(|g| g.features.gap_loc).sum();
        let avg_complexity = if file_gaps.is_empty() {
            0.0
        } else {
            file_gaps
                .iter()
                .map(|g| g.features.cyclomatic_in_gap + g.features.cognitive_in_gap)
                .sum::<f64>()
                / file_gaps.len() as f64
        };

        let centrality = estimate_file_centrality(&path);

        metrics.insert(
            path,
            FileMetrics {
                total_gap_loc,
                avg_complexity,
                centrality,
                gap_count: file_gaps.len(),
            },
        );
    }

    Ok(metrics)
}

/// Estimate a file's centrality based on naming conventions.
///
/// Files like `lib.rs`, `main.rs`, `__init__.py`, and `index.js` are
/// considered more central to the codebase.
pub fn estimate_file_centrality(file_path: &PathBuf) -> f64 {
    let path_str = file_path.to_string_lossy().to_lowercase();
    if path_str.contains("lib.rs")
        || path_str.contains("main.rs")
        || path_str.contains("__init__.py")
        || path_str.contains("index.")
    {
        return 0.9;
    }
    if path_str.contains("core")
        || path_str.contains("base")
        || path_str.contains("common")
        || path_str.contains("util")
    {
        return 0.7;
    }
    if path_str.contains("test") || path_str.contains("example") {
        return 0.2;
    }
    0.5
}

/// Normalize gap size to a 0-1 score using exponential decay.
///
/// Larger gaps get higher scores, approaching 1.0 asymptotically.
pub fn normalize_size_score(gap_loc: usize) -> f64 {
    let x = gap_loc as f64;
    1.0 - (-x / 20.0).exp()
}

/// Normalize complexity to a 0-1 score using exponential decay.
///
/// Higher complexity gets higher scores, approaching 1.0 asymptotically.
pub fn normalize_complexity_score(complexity: f64) -> f64 {
    1.0 - (-complexity / 10.0).exp()
}

/// Normalize fan-in to a 0-1 score using a saturation curve.
///
/// Higher fan-in gets higher scores, clamped between 0.0 and 1.0.
pub fn normalize_fan_in_score(fan_in: usize) -> f64 {
    let x = fan_in as f64;
    (x / (x + 5.0)).clamp(0.0, 1.0)
}
