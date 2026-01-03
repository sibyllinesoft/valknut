//! Feature normalization and scoring system.
//!
//! This module provides comprehensive scoring and normalization capabilities
//! for code analysis features, with support for various normalization schemes
//! including Bayesian approaches for handling challenging statistical cases.

use std::collections::HashMap;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::core::bayesian::BayesianNormalizer;
use crate::core::config::{NormalizationScheme, ScoringConfig, WeightsConfig};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::FeatureVector;

/// Main feature normalization engine that supports multiple schemes
#[derive(Debug)]
pub struct FeatureNormalizer {
    /// Configuration for this normalizer
    config: ScoringConfig,

    /// Statistical measures for each feature (non-Bayesian schemes)
    statistics: HashMap<String, NormalizationStatistics>,

    /// Bayesian normalizer (if using Bayesian schemes)
    bayesian_normalizer: Option<BayesianNormalizer>,
}

/// Statistical measures used for normalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationStatistics {
    /// Sample mean
    pub mean: f64,
    /// Sample variance
    pub variance: f64,
    /// Sample standard deviation
    pub std_dev: f64,
    /// Minimum value observed
    pub min: f64,
    /// Maximum value observed
    pub max: f64,
    /// Number of samples
    pub n_samples: usize,
    /// Median (for robust normalization)
    pub median: f64,
    /// Median Absolute Deviation (for robust normalization)
    pub mad: f64,
    /// 25th percentile
    pub q1: f64,
    /// 75th percentile
    pub q3: f64,
    /// Interquartile range
    pub iqr: f64,
}

/// Factory and calculation methods for [`NormalizationStatistics`].
impl NormalizationStatistics {
    /// Calculate statistics from a vector of values
    pub fn from_values(mut values: Vec<f64>) -> Self {
        let n = values.len();

        if n == 0 {
            return Self::empty();
        }

        // Sort for percentile calculations
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Basic statistics
        let sum: f64 = values.iter().sum();
        let mean = sum / n as f64;
        let variance = if n > 1 {
            values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64
        } else {
            0.0
        };
        let std_dev = variance.sqrt();
        let min = values[0];
        let max = values[n - 1];

        // Percentiles
        let median = Self::percentile(&values, 0.5);
        let q1 = Self::percentile(&values, 0.25);
        let q3 = Self::percentile(&values, 0.75);
        let iqr = q3 - q1;

        // Median Absolute Deviation
        let deviations: Vec<f64> = values.iter().map(|x| (x - median).abs()).collect();
        let median_abs_deviation = Self::median_of_slice(&deviations);

        Self {
            mean,
            variance,
            std_dev,
            min,
            max,
            n_samples: n,
            median,
            mad: median_abs_deviation,
            q1,
            q3,
            iqr,
        }
    }

    /// Create empty statistics
    pub fn empty() -> Self {
        Self {
            mean: 0.0,
            variance: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 0.0,
            n_samples: 0,
            median: 0.0,
            mad: 0.0,
            q1: 0.0,
            q3: 0.0,
            iqr: 0.0,
        }
    }

    /// Calculate percentile of sorted values
    fn percentile(sorted_values: &[f64], p: f64) -> f64 {
        if sorted_values.is_empty() {
            return 0.0;
        }

        let n = sorted_values.len();
        let index = p * (n - 1) as f64;
        let lower_index = index.floor() as usize;
        let upper_index = index.ceil() as usize;

        if lower_index == upper_index || upper_index >= n {
            sorted_values[lower_index.min(n - 1)]
        } else {
            let weight = index - lower_index as f64;
            sorted_values[lower_index] * (1.0 - weight) + sorted_values[upper_index] * weight
        }
    }

    /// Calculate median of a slice
    fn median_of_slice(values: &[f64]) -> f64 {
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        Self::percentile(&sorted, 0.5)
    }
}

/// Factory, fitting, and normalization methods for [`FeatureNormalizer`].
impl FeatureNormalizer {
    /// Create a new feature normalizer with the given configuration
    pub fn new(config: ScoringConfig) -> Self {
        let bayesian_normalizer = if config
            .normalization_scheme
            .to_string()
            .ends_with("_bayesian")
            || config.use_bayesian_fallbacks
        {
            Some(BayesianNormalizer::new(
                config.normalization_scheme.to_string(),
            ))
        } else {
            None
        };

        Self {
            config,
            statistics: HashMap::new(),
            bayesian_normalizer,
        }
    }

    /// Fit the normalizer to a collection of feature vectors
    pub fn fit(&mut self, feature_vectors: &[FeatureVector]) -> Result<()> {
        if feature_vectors.is_empty() {
            return Err(ValknutError::validation(
                "No feature vectors provided for normalization fitting",
            ));
        }

        // If using Bayesian normalizer, delegate fitting
        if let Some(ref mut bayesian) = self.bayesian_normalizer {
            bayesian.fit(feature_vectors)?;

            // Optionally report confidence diagnostics
            if self.config.confidence_reporting {
                self.report_bayesian_diagnostics();
            }
            return Ok(());
        }

        // Collect feature values for classical statistics
        let mut feature_values: HashMap<String, Vec<f64>> = HashMap::new();
        for vector in feature_vectors {
            for (feature_name, &value) in &vector.features {
                feature_values
                    .entry(feature_name.clone())
                    .or_default()
                    .push(value);
            }
        }

        // Calculate classical statistics for each feature
        self.statistics = feature_values
            .into_par_iter()
            .map(|(feature_name, values)| {
                let stats = NormalizationStatistics::from_values(values);
                (feature_name, stats)
            })
            .collect();

        Ok(())
    }

    /// Normalize feature vectors using the fitted statistics
    pub fn normalize(&self, feature_vectors: &mut [FeatureVector]) -> Result<()> {
        // If using Bayesian normalizer, delegate normalization
        if let Some(ref bayesian) = self.bayesian_normalizer {
            return bayesian.normalize(feature_vectors);
        }

        // Classical normalization
        feature_vectors.par_iter_mut().try_for_each(|vector| {
            for (feature_name, &value) in vector.features.clone().iter() {
                if let Some(stats) = self.statistics.get(feature_name) {
                    let normalized_value = self.normalize_value(value, stats)?;
                    vector
                        .normalized_features
                        .insert(feature_name.clone(), normalized_value);
                } else {
                    // No statistics available - use identity
                    vector
                        .normalized_features
                        .insert(feature_name.clone(), value);
                }
            }
            Ok::<(), ValknutError>(())
        })?;

        Ok(())
    }

    /// Normalize a single value using the specified scheme and statistics
    fn normalize_value(&self, value: f64, stats: &NormalizationStatistics) -> Result<f64> {
        if value.is_nan() || value.is_infinite() {
            return Ok(0.0);
        }

        let normalized = match self.config.normalization_scheme {
            NormalizationScheme::ZScore => self.zscore_normalize(value, stats),
            NormalizationScheme::MinMax => self.minmax_normalize(value, stats),
            NormalizationScheme::Robust => self.robust_normalize(value, stats),
            NormalizationScheme::ZScoreBayesian
            | NormalizationScheme::MinMaxBayesian
            | NormalizationScheme::RobustBayesian => {
                return Err(ValknutError::internal(
                    "Bayesian normalization should be handled by BayesianNormalizer",
                ));
            }
        };

        Ok(normalized.clamp(-10.0, 10.0))
    }

    /// Z-score normalization with zero variance handling
    fn zscore_normalize(&self, value: f64, stats: &NormalizationStatistics) -> f64 {
        if stats.variance < f64::EPSILON {
            return self.fallback_or_default(value, stats, 0.0);
        }
        (value - stats.mean) / stats.std_dev
    }

    /// Min-max normalization with zero range handling
    fn minmax_normalize(&self, value: f64, stats: &NormalizationStatistics) -> f64 {
        let range = stats.max - stats.min;
        if range < f64::EPSILON {
            return self.fallback_or_default(value, stats, 0.5);
        }
        (value - stats.min) / range
    }

    /// Robust normalization using median and MAD/IQR
    fn robust_normalize(&self, value: f64, stats: &NormalizationStatistics) -> f64 {
        // Try MAD-based normalization first
        if stats.mad >= f64::EPSILON {
            return (value - stats.median) / (1.4826 * stats.mad);
        }
        // Fallback to IQR-based normalization
        if stats.iqr >= f64::EPSILON {
            return (value - stats.median) / stats.iqr;
        }
        // All divisors are zero, use fallback
        self.fallback_or_default(value, stats, 0.0)
    }

    /// Return Bayesian fallback value if enabled, otherwise return default
    fn fallback_or_default(&self, value: f64, stats: &NormalizationStatistics, default: f64) -> f64 {
        if self.config.use_bayesian_fallbacks {
            self.bayesian_fallback_normalize(value, stats)
        } else {
            default
        }
    }

    /// Bayesian fallback for degenerate cases
    fn bayesian_fallback_normalize(&self, _value: f64, _stats: &NormalizationStatistics) -> f64 {
        0.0
    }

    /// Report Bayesian diagnostics if enabled
    fn report_bayesian_diagnostics(&self) {
        if let Some(ref bayesian) = self.bayesian_normalizer {
            let diagnostics = bayesian.get_diagnostics();
            tracing::info!("Bayesian normalization diagnostics: {:#?}", diagnostics);
        }
    }

    /// Get statistics for a specific feature
    pub fn get_statistics(&self, feature_name: &str) -> Option<&NormalizationStatistics> {
        self.statistics.get(feature_name)
    }

    /// Get all normalization statistics
    pub fn get_all_statistics(&self) -> &HashMap<String, NormalizationStatistics> {
        &self.statistics
    }

    /// Get the Bayesian normalizer if available
    pub fn get_bayesian_normalizer(&self) -> Option<&BayesianNormalizer> {
        self.bayesian_normalizer.as_ref()
    }

    /// Get a mutable reference to the Bayesian normalizer if available
    pub fn get_bayesian_normalizer_mut(&mut self) -> Option<&mut BayesianNormalizer> {
        self.bayesian_normalizer.as_mut()
    }
}

/// Feature scoring engine that combines normalization with weighted scoring
#[derive(Debug)]
pub struct FeatureScorer {
    /// Normalizer for feature preprocessing
    normalizer: FeatureNormalizer,

    /// Feature weights configuration
    weights: WeightsConfig,
}

/// Scoring and weighting methods for [`FeatureScorer`].
impl FeatureScorer {
    /// Create a new feature scorer
    pub fn new(config: ScoringConfig) -> Self {
        Self {
            normalizer: FeatureNormalizer::new(config.clone()),
            weights: config.weights,
        }
    }

    /// Fit the scorer to training data
    pub fn fit(&mut self, feature_vectors: &[FeatureVector]) -> Result<()> {
        self.normalizer.fit(feature_vectors)
    }

    /// Get a mutable reference to the underlying normalizer
    pub fn normalizer(&mut self) -> &mut FeatureNormalizer {
        &mut self.normalizer
    }

    /// Score feature vectors, returning normalized and weighted scores
    pub fn score(&self, feature_vectors: &mut [FeatureVector]) -> Result<Vec<ScoringResult>> {
        // First normalize all features
        self.normalizer.normalize(feature_vectors)?;

        // Then compute weighted scores
        let results: Result<Vec<ScoringResult>> = feature_vectors
            .par_iter()
            .map(|vector| self.compute_scores(vector))
            .collect();

        results
    }

    /// Score a single feature vector (optimized for parallel processing)
    pub fn score_single(&self, vector: &FeatureVector) -> Result<ScoringResult> {
        // Create a mutable copy for normalization
        let mut normalized_vector = vector.clone();

        // Normalize this single vector
        self.normalizer
            .normalize(std::slice::from_mut(&mut normalized_vector))?;

        // Compute scores
        self.compute_scores(&normalized_vector)
    }

    /// Compute scoring results for a single feature vector
    fn compute_scores(&self, vector: &FeatureVector) -> Result<ScoringResult> {
        let mut category_scores = HashMap::new();
        let mut feature_contributions = HashMap::new();

        // Calculate category scores based on feature weights
        let mut total_weighted_score = 0.0;
        let mut total_weight = 0.0;

        for (feature_name, &normalized_value) in &vector.normalized_features {
            let (category, weight) = self.get_feature_category_and_weight(feature_name);

            let contribution = normalized_value * weight;
            feature_contributions.insert(feature_name.clone(), contribution);

            // Accumulate category score
            *category_scores.entry(category.clone()).or_insert(0.0) += contribution;

            // Accumulate total
            total_weighted_score += contribution;
            total_weight += weight;
        }

        // Normalize category scores by their total weight
        for (category, score) in &mut category_scores {
            let category_weight = self.get_category_weight(category);
            if category_weight > 0.0 {
                *score /= category_weight;
            }
        }

        // Calculate overall refactoring priority score
        let overall_score = if total_weight > 0.0 {
            total_weighted_score / total_weight
        } else {
            0.0
        };

        // Determine priority level
        let priority = Self::calculate_priority(overall_score);

        Ok(ScoringResult {
            entity_id: vector.entity_id.clone(),
            overall_score,
            priority,
            category_scores,
            feature_contributions,
            normalized_feature_count: vector.normalized_features.len(),
            confidence: self.calculate_confidence(vector),
        })
    }

    /// Get the category and weight for a feature
    fn get_feature_category_and_weight(&self, feature_name: &str) -> (String, f64) {
        // Category patterns: (keywords, category_name, weight_getter)
        const CATEGORY_PATTERNS: &[(&[&str], &str)] = &[
            (&["cyclomatic", "cognitive", "complexity"], "complexity"),
            (&["betweenness", "centrality", "fan_"], "graph"),
            (&["structure", "class", "method", "function", "directory", "lines_of_code"], "structure"),
            (&["style", "naming", "format"], "style"),
            (&["coverage", "test"], "coverage"),
        ];

        for (keywords, category) in CATEGORY_PATTERNS {
            if keywords.iter().any(|kw| feature_name.contains(kw)) {
                let weight = self.get_category_weight(category);
                return (category.to_string(), weight);
            }
        }

        ("other".to_string(), 1.0)
    }

    /// Get the total weight for a category
    fn get_category_weight(&self, category: &str) -> f64 {
        match category {
            "complexity" => self.weights.complexity,
            "graph" => self.weights.graph,
            "structure" => self.weights.structure,
            "style" => self.weights.style,
            "coverage" => self.weights.coverage,
            _ => 1.0,
        }
    }

    /// Calculate priority level from overall score
    fn calculate_priority(score: f64) -> Priority {
        let abs_score = score.abs();

        if abs_score >= 2.0 {
            Priority::Critical
        } else if abs_score >= 1.5 {
            Priority::High
        } else if abs_score >= 1.0 {
            Priority::Medium
        } else if abs_score >= 0.5 {
            Priority::Low
        } else {
            Priority::None
        }
    }

    /// Calculate confidence in the scoring result
    fn calculate_confidence(&self, vector: &FeatureVector) -> f64 {
        let feature_count = vector.normalized_features.len() as f64;
        let base_confidence = (feature_count / 10.0).min(1.0); // More features = higher confidence

        // Adjust based on Bayesian confidence if available
        if let Some(bayesian) = self.normalizer.get_bayesian_normalizer() {
            let mut confidence_sum = 0.0;
            let mut confidence_count = 0;

            for feature_name in vector.normalized_features.keys() {
                if let Some(confidence) = bayesian.get_confidence(feature_name) {
                    confidence_sum += confidence.score();
                    confidence_count += 1;
                }
            }

            if confidence_count > 0 {
                let avg_bayesian_confidence = confidence_sum / confidence_count as f64;
                base_confidence * avg_bayesian_confidence
            } else {
                base_confidence
            }
        } else {
            base_confidence
        }
    }

    /// Get the underlying normalizer
    pub fn get_normalizer(&self) -> &FeatureNormalizer {
        &self.normalizer
    }
}

/// Priority levels for refactoring suggestions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// No refactoring needed
    None,
    /// Low priority refactoring
    Low,
    /// Medium priority refactoring
    Medium,
    /// High priority refactoring
    High,
    /// Critical refactoring required
    Critical,
}

/// Conversion methods for [`Priority`].
impl Priority {
    /// Get numeric priority value
    pub fn value(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Low => 0.25,
            Self::Medium => 0.5,
            Self::High => 0.75,
            Self::Critical => 1.0,
        }
    }
}

/// Result of feature scoring for an entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringResult {
    /// Entity identifier
    pub entity_id: String,

    /// Overall refactoring priority score
    pub overall_score: f64,

    /// Priority level
    pub priority: Priority,

    /// Scores broken down by feature category
    pub category_scores: HashMap<String, f64>,

    /// Individual feature contributions to the score
    pub feature_contributions: HashMap<String, f64>,

    /// Number of normalized features used in scoring
    pub normalized_feature_count: usize,

    /// Confidence in the scoring result (0.0-1.0)
    pub confidence: f64,
}

/// Query and analysis methods for [`ScoringResult`].
impl ScoringResult {
    /// Check if this entity needs refactoring
    pub fn needs_refactoring(&self) -> bool {
        self.priority != Priority::None
    }

    /// Check if this is a high-priority refactoring candidate
    pub fn is_high_priority(&self) -> bool {
        matches!(self.priority, Priority::High | Priority::Critical)
    }

    /// Get the dominant feature category (highest scoring)
    pub fn dominant_category(&self) -> Option<(String, f64)> {
        self.category_scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(k, v)| (k.clone(), *v))
    }

    /// Get the top contributing features
    pub fn top_contributing_features(&self, count: usize) -> Vec<(String, f64)> {
        let mut contributions: Vec<_> = self
            .feature_contributions
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        contributions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        contributions.into_iter().take(count).collect()
    }
}

/// Extension trait for NormalizationScheme to convert to string.
trait NormalizationSchemeExt {
    /// Converts the scheme to its string representation.
    fn to_string(&self) -> String;
}

/// [`NormalizationSchemeExt`] implementation for [`NormalizationScheme`].
impl NormalizationSchemeExt for NormalizationScheme {
    /// Converts the normalization scheme to its string representation.
    fn to_string(&self) -> String {
        match self {
            Self::ZScore => "z_score".to_string(),
            Self::MinMax => "min_max".to_string(),
            Self::Robust => "robust".to_string(),
            Self::ZScoreBayesian => "z_score_bayesian".to_string(),
            Self::MinMaxBayesian => "min_max_bayesian".to_string(),
            Self::RobustBayesian => "robust_bayesian".to_string(),
        }
    }
}

#[cfg(test)]
#[path = "features_tests.rs"]
mod tests;
