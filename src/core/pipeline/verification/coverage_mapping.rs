//! Coverage mapping utilities for annotating refactoring candidates.
//!
//! This module provides functions for converting coverage analysis results
//! into a format suitable for annotating refactoring candidates with
//! coverage percentages.

use std::collections::HashMap;

use crate::core::pipeline::CoverageAnalysisResults;
use crate::detectors::coverage::CoveragePack;

use crate::core::pipeline::results::result_types::RefactoringCandidate;

/// Convert pipeline coverage results to coverage packs for API output.
pub fn convert_coverage_to_packs(coverage_results: &CoverageAnalysisResults) -> Vec<CoveragePack> {
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
///
/// Returns a tuple of:
/// - file_coverage: map of file_path -> coverage_percentage
/// - entity_coverage: map of (file_path, line_start, line_end) -> coverage_percentage
pub fn build_coverage_map(
    coverage_packs: &[CoveragePack],
) -> (HashMap<String, f64>, HashMap<(String, usize, usize), f64>) {
    let mut file_coverage: HashMap<String, f64> = HashMap::new();
    let mut entity_coverage: HashMap<(String, usize, usize), f64> = HashMap::new();

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

/// Check if two line ranges have significant overlap (at least 50% of candidate).
fn has_significant_overlap(
    candidate_start: usize,
    candidate_end: usize,
    sym_start: usize,
    sym_end: usize,
) -> bool {
    let overlap_start = candidate_start.max(sym_start);
    let overlap_end = candidate_end.min(sym_end);
    if overlap_end < overlap_start {
        return false;
    }
    let overlap = overlap_end - overlap_start + 1;
    let candidate_len = candidate_end - candidate_start + 1;
    overlap * 2 >= candidate_len
}

/// Find fuzzy coverage match by looking for significant overlap with any symbol.
fn find_fuzzy_coverage(
    file_path: &str,
    start: usize,
    end: usize,
    entity_coverage: &HashMap<(String, usize, usize), f64>,
) -> Option<f64> {
    entity_coverage
        .iter()
        .find(|((file, sym_start, sym_end), _)| {
            file == file_path && has_significant_overlap(start, end, *sym_start, *sym_end)
        })
        .map(|(_, &cov_pct)| cov_pct)
}

/// Annotate existing candidates with coverage data from coverage packs.
///
/// This modifies candidates in-place to add coverage_percentage field.
pub fn annotate_candidates_with_coverage(
    candidates: &mut [RefactoringCandidate],
    coverage_packs: &[CoveragePack],
) {
    let (file_coverage, entity_coverage) = build_coverage_map(coverage_packs);

    for candidate in candidates.iter_mut() {
        if let Some((start, end)) = candidate.line_range {
            // Try exact match first
            let key = (candidate.file_path.clone(), start, end);
            if let Some(&cov_pct) = entity_coverage.get(&key) {
                candidate.coverage_percentage = Some(cov_pct);
                continue;
            }

            // Try fuzzy match
            if let Some(cov_pct) = find_fuzzy_coverage(&candidate.file_path, start, end, &entity_coverage) {
                candidate.coverage_percentage = Some(cov_pct);
                continue;
            }
        }

        // Fall back to file-level coverage
        if let Some(&file_cov) = file_coverage.get(&candidate.file_path) {
            candidate.coverage_percentage = Some(file_cov);
        }
    }
}
