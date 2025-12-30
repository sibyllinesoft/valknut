//! Statistical utility functions for directory analysis.

use rayon::prelude::*;

/// Calculate a distribution-based optimality score.
///
/// Returns a score in [0, 1] where 1.0 means the value equals the optimal (mean),
/// and the score decreases as the value deviates from optimal. The score is
/// computed as the ratio of the normal distribution density at the given value
/// to the density at the mean (which is the maximum density).
///
/// This simplifies to: `score = exp(-0.5 * ((value - optimal) / stddev)²)`
pub fn calculate_distribution_score(value: usize, optimal: usize, stddev: f64) -> f64 {
    if stddev <= 0.0 {
        // If stddev is zero or negative, return 1.0 only if value equals optimal
        return if value == optimal { 1.0 } else { 0.0 };
    }

    let z = (value as f64 - optimal as f64) / stddev;
    (-0.5 * z * z).exp()
}

/// Calculate Gini coefficient for LOC distribution with O(n log n) optimization
pub fn calculate_gini_coefficient(values: &[usize]) -> f64 {
    if values.len() <= 1 {
        return 0.0;
    }

    let n = values.len() as f64;
    let sum: f64 = values.iter().map(|&v| v as f64).sum();

    if sum == 0.0 {
        return 0.0;
    }

    // O(n log n) algorithm using the standard Gini formula
    // Sort the values first (O(n log n))
    let mut sorted_values = values.to_vec();
    sorted_values.sort_unstable();

    // Calculate using the efficient formula: Gini = (2 * sum(i * y_i) / (n * sum(y_i))) - (n + 1) / n
    // where i is the rank (1-indexed) and y_i is the sorted value
    let mut weighted_sum = 0.0;
    for (i, &val) in sorted_values.iter().enumerate() {
        weighted_sum += (i as f64 + 1.0) * val as f64;
    }

    let gini = (2.0 * weighted_sum) / (n * sum) - (n + 1.0) / n;
    gini.max(0.0) // Ensure non-negative result
}

/// Calculate entropy for LOC distribution with parallel optimization
pub fn calculate_entropy(values: &[usize]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let total: usize = values.iter().sum();
    if total == 0 {
        return 0.0;
    }

    // For small arrays, use sequential computation
    if values.len() < 100 {
        return values
            .iter()
            .filter(|&&x| x > 0)
            .map(|&x| {
                let p = x as f64 / total as f64;
                -p * p.log2()
            })
            .sum();
    }

    // For larger arrays, use parallel computation
    let total_f64 = total as f64;
    values
        .par_iter()
        .filter(|&&x| x > 0)
        .map(|&x| {
            let p = x as f64 / total_f64;
            -p * p.log2()
        })
        .sum()
}

/// Calculate size normalization factor for directory metrics.
///
/// Prevents small codebases from being over-penalized
/// and large ones from being under-penalized.
pub fn calculate_size_normalization_factor(files: usize, total_loc: usize) -> f64 {
    let base_files = 10.0;
    let base_loc = 1000.0;

    let file_factor = (files as f64 / base_files).ln_1p() / base_files.ln();
    let loc_factor = (total_loc as f64 / base_loc).ln_1p() / base_loc.ln();

    // Combine factors and normalize to [0.5, 1.5] range
    let combined = (file_factor + loc_factor) * 0.5;
    1.0 + combined.tanh() * 0.5
}
