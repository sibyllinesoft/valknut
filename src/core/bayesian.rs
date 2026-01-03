//! Bayesian normalization with intelligent fallback strategies.
//!
//! This module provides sophisticated feature normalization using Bayesian priors
//! to handle challenging cases like zero-variance features and small sample sizes.
//! The implementation emphasizes numerical stability and performance while maintaining
//! statistical rigor.

use std::collections::HashMap;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "simd")]
use wide::f64x4;

use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::FeatureVector;

/// Confidence levels for variance estimation based on sample characteristics
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VarianceConfidence {
    /// >50 samples with good variance (high statistical power)
    High,
    /// 10-50 samples with some variance (moderate statistical power)
    Medium,
    /// 5-10 samples with minimal variance (low statistical power)
    Low,
    /// 2-5 samples (very low statistical power)
    VeryLow,
    /// <2 samples or zero variance (insufficient for inference)
    Insufficient,
}

/// Score and classification methods for [`VarianceConfidence`].
impl VarianceConfidence {
    /// Get the numeric confidence score (0.0-1.0)
    pub fn score(self) -> f64 {
        match self {
            Self::High => 0.9,
            Self::Medium => 0.7,
            Self::Low => 0.5,
            Self::VeryLow => 0.3,
            Self::Insufficient => 0.1,
        }
    }

    /// Determine confidence from sample size and variance
    pub fn from_samples(n_samples: usize, variance: f64, threshold: f64) -> Self {
        if n_samples < 2 || variance < f64::EPSILON {
            Self::Insufficient
        } else if n_samples >= 50 && variance > threshold {
            Self::High
        } else if n_samples >= 10 && variance > threshold * 0.5 {
            Self::Medium
        } else if n_samples >= 5 && variance > threshold * 0.1 {
            Self::Low
        } else {
            Self::VeryLow
        }
    }
}

/// Bayesian prior knowledge for a feature based on domain expertise
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturePrior {
    /// Feature name
    pub name: String,

    /// Beta distribution parameters for the prior
    pub alpha: f64, // Success count + 1 (shape parameter)
    pub beta: f64, // Failure count + 1 (shape parameter)

    /// Expected range based on domain knowledge
    pub expected_min: f64,
    pub expected_max: f64,
    pub expected_mean: f64,

    /// Variance confidence parameters
    pub min_samples_for_confidence: usize,
    pub variance_threshold: f64,

    /// Feature metadata
    pub feature_type: String,
    pub higher_is_worse: bool,
    pub typical_distribution: String,
}

/// Factory and builder methods for [`FeaturePrior`].
impl FeaturePrior {
    /// Create a new feature prior with reasonable defaults
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            alpha: 1.0,
            beta: 1.0,
            expected_min: 0.0,
            expected_max: 1.0,
            expected_mean: 0.5,
            min_samples_for_confidence: 10,
            variance_threshold: 0.01,
            feature_type: "generic".to_string(),
            higher_is_worse: true,
            typical_distribution: "normal".to_string(),
        }
    }

    /// Set Beta distribution parameters
    pub fn with_beta_params(mut self, alpha: f64, beta: f64) -> Self {
        self.alpha = alpha;
        self.beta = beta;
        self
    }

    /// Set expected value range
    pub fn with_range(mut self, min: f64, max: f64, mean: f64) -> Self {
        self.expected_min = min;
        self.expected_max = max;
        self.expected_mean = mean;
        self
    }

    /// Set feature type and characteristics
    pub fn with_type(
        mut self,
        feature_type: impl Into<String>,
        distribution: impl Into<String>,
    ) -> Self {
        self.feature_type = feature_type.into();
        self.typical_distribution = distribution.into();
        self
    }

    /// Calculate the prior mean using Beta distribution
    pub fn prior_mean(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    /// Calculate the prior variance using Beta distribution
    pub fn prior_variance(&self) -> f64 {
        let ab = self.alpha + self.beta;
        (self.alpha * self.beta) / (ab * ab * (ab + 1.0))
    }

    /// Get the effective sample size of the prior
    pub fn effective_sample_size(&self) -> f64 {
        self.alpha + self.beta
    }
}

/// Statistical measures for feature normalization
#[derive(Debug, Clone)]
pub struct FeatureStatistics {
    /// Sample mean
    pub mean: f64,
    /// Sample variance
    pub variance: f64,
    /// Sample standard deviation
    pub std_dev: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// Number of samples
    pub n_samples: usize,
    /// Variance confidence level
    pub confidence: VarianceConfidence,
    /// Weight given to prior vs empirical data
    pub prior_weight: f64,
    /// Posterior mean (Bayesian estimate)
    pub posterior_mean: f64,
    /// Posterior variance (Bayesian estimate)
    pub posterior_variance: f64,
}

/// Factory methods for [`FeatureStatistics`].
impl FeatureStatistics {
    /// Create new statistics from raw values
    pub fn from_values(values: &[f64]) -> Self {
        let n = values.len();
        if n == 0 {
            return Self {
                mean: 0.0,
                variance: 0.0,
                std_dev: 0.0,
                min: 0.0,
                max: 0.0,
                n_samples: 0,
                confidence: VarianceConfidence::Insufficient,
                prior_weight: 0.0,
                posterior_mean: 0.0,
                posterior_variance: 0.0,
            };
        }

        let mean = values.iter().sum::<f64>() / n as f64;
        let variance = if n > 1 {
            values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64
        } else {
            0.0
        };
        let std_dev = variance.sqrt();
        let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        Self {
            mean,
            variance,
            std_dev,
            min,
            max,
            n_samples: n,
            confidence: VarianceConfidence::Insufficient,
            prior_weight: 0.0,
            posterior_mean: mean,
            posterior_variance: variance,
        }
    }
}

/// Enhanced normalizer with Bayesian priors for intelligent fallbacks
#[derive(Debug)]
pub struct BayesianNormalizer {
    /// Normalization scheme to use
    pub scheme: String,

    /// Statistical measures for each feature
    statistics: HashMap<String, FeatureStatistics>,

    /// Domain-specific priors for features
    priors: HashMap<String, FeaturePrior>,

    /// Variance confidence for each feature
    variance_confidence: HashMap<String, VarianceConfidence>,
}

/// Factory, fitting, and normalization methods for [`BayesianNormalizer`].
impl BayesianNormalizer {
    /// Create a new Bayesian normalizer
    pub fn new(scheme: impl Into<String>) -> Self {
        let mut normalizer = Self {
            scheme: scheme.into(),
            statistics: HashMap::new(),
            priors: HashMap::new(),
            variance_confidence: HashMap::new(),
        };

        // Initialize domain-specific priors
        normalizer.initialize_feature_priors();
        normalizer
    }

    /// Initialize domain-specific priors for common features
    fn initialize_feature_priors(&mut self) {
        // Define all feature categories with their beta params and feature definitions
        let feature_categories: &[(&str, (f64, f64), &[(&str, f64, f64, f64, &str)])] = &[
            // Complexity features - typically right-skewed, most functions are simple
            (
                "complexity",
                (2.0, 5.0), // Preference for lower complexity
                &[
                    ("cyclomatic", 1.0, 20.0, 3.0, "right_skewed"),
                    ("cognitive", 0.0, 50.0, 5.0, "right_skewed"),
                    ("max_nesting", 0.0, 10.0, 2.0, "right_skewed"),
                    ("param_count", 0.0, 15.0, 3.0, "right_skewed"),
                    ("branch_fanout", 0.0, 10.0, 2.0, "right_skewed"),
                ],
            ),
            // Graph centrality features - often zero with occasional spikes
            (
                "centrality",
                (1.0, 10.0), // Strong preference for low centrality
                &[
                    ("betweenness_approx", 0.0, 1.0, 0.1, "highly_skewed"),
                    ("fan_in", 0.0, 50.0, 2.0, "right_skewed"),
                    ("fan_out", 0.0, 20.0, 3.0, "right_skewed"),
                    ("closeness", 0.0, 1.0, 0.3, "bimodal"),
                    ("eigenvector", 0.0, 1.0, 0.2, "highly_skewed"),
                ],
            ),
            // Cycle features - binary or small integers
            (
                "cycles",
                (1.0, 4.0), // Most code is not in cycles
                &[
                    ("in_cycle", 0.0, 1.0, 0.2, "bernoulli"),
                    ("cycle_size", 0.0, 20.0, 0.5, "right_skewed"),
                ],
            ),
            // Clone/duplication features
            (
                "clones",
                (1.0, 8.0), // Most code has low duplication
                &[
                    ("clone_mass", 0.0, 1.0, 0.1, "right_skewed"),
                    ("similarity", 0.0, 1.0, 0.3, "bimodal"),
                ],
            ),
        ];

        for (feature_type, (alpha, beta), features) in feature_categories {
            for &(name, min_val, max_val, mean_val, dist) in *features {
                let prior = FeaturePrior::new(name)
                    .with_beta_params(*alpha, *beta)
                    .with_range(min_val, max_val, mean_val)
                    .with_type(*feature_type, dist);
                self.priors.insert(name.to_string(), prior);
            }
        }
    }

    /// Fit the normalizer to feature vectors with Bayesian enhancement
    pub fn fit(&mut self, feature_vectors: &[FeatureVector]) -> Result<()> {
        if feature_vectors.is_empty() {
            return Err(ValknutError::validation(
                "No feature vectors provided for Bayesian fitting",
            ));
        }

        // Collect feature values
        let mut feature_values: HashMap<String, Vec<f64>> = HashMap::new();
        for vector in feature_vectors {
            for (feature_name, &value) in &vector.features {
                feature_values
                    .entry(feature_name.clone())
                    .or_default()
                    .push(value);
            }
        }

        // Calculate statistics with Bayesian enhancement
        for (feature_name, values) in feature_values {
            if values.is_empty() {
                continue;
            }

            // Calculate empirical statistics
            let mut empirical_stats = FeatureStatistics::from_values(&values);

            // Get or create prior for this feature
            let prior = self
                .priors
                .get(&feature_name)
                .cloned()
                .unwrap_or_else(|| self.create_generic_prior(&feature_name));

            // Assess variance confidence
            let confidence = VarianceConfidence::from_samples(
                values.len(),
                empirical_stats.variance,
                prior.variance_threshold,
            );
            empirical_stats.confidence = confidence;

            // Calculate Bayesian posterior statistics
            let posterior_stats = self.calculate_posterior_stats(&empirical_stats, &prior)?;

            self.statistics
                .insert(feature_name.clone(), posterior_stats);
            self.variance_confidence.insert(feature_name, confidence);
        }

        Ok(())
    }

    /// Normalize feature vectors using Bayesian statistics
    pub fn normalize(&self, feature_vectors: &mut [FeatureVector]) -> Result<()> {
        for vector in feature_vectors {
            for (feature_name, &value) in vector.features.clone().iter() {
                if let Some(stats) = self.statistics.get(feature_name) {
                    let normalized_value = self.normalize_value(value, stats)?;
                    vector
                        .normalized_features
                        .insert(feature_name.clone(), normalized_value);
                } else {
                    // No statistics available, use identity normalization
                    vector
                        .normalized_features
                        .insert(feature_name.clone(), value);
                }
            }
        }
        Ok(())
    }

    /// Parallel normalize feature vectors using Rayon for bulk operations
    #[cfg(feature = "parallel")]
    pub fn normalize_parallel(&self, feature_vectors: &mut [FeatureVector]) -> Result<()> {
        feature_vectors
            .par_iter_mut()
            .try_for_each(|vector| -> Result<()> {
                for (feature_name, &value) in vector.features.clone().iter() {
                    if let Some(stats) = self.statistics.get(feature_name) {
                        let normalized_value = self.normalize_value(value, stats)?;
                        vector
                            .normalized_features
                            .insert(feature_name.clone(), normalized_value);
                    } else {
                        vector
                            .normalized_features
                            .insert(feature_name.clone(), value);
                    }
                }
                Ok(())
            })
    }

    /// SIMD-accelerated batch normalization for arrays of values
    #[cfg(feature = "simd")]
    pub fn normalize_batch_simd(&self, values: &mut [f64], feature_name: &str) -> Result<()> {
        let Some(stats) = self.statistics.get(feature_name) else {
            return Ok(()); // No statistics available
        };

        match self.scheme.as_str() {
            "z_score" | "zscore" => {
                if stats.posterior_variance < f64::EPSILON {
                    // Zero variance - set all to zero
                    values.fill(0.0);
                } else {
                    let mean_vec = f64x4::splat(stats.posterior_mean);
                    let inv_std_vec = f64x4::splat(1.0 / stats.posterior_variance.sqrt());

                    // Process chunks of 4
                    let (chunks, remainder) =
                        values.split_at_mut(values.len() - (values.len() % 4));
                    for chunk in chunks.chunks_exact_mut(4) {
                        let vals = f64x4::from([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        let normalized = (vals - mean_vec) * inv_std_vec;
                        chunk.copy_from_slice(&normalized.to_array());
                    }

                    // Handle remainder
                    let inv_std = 1.0 / stats.posterior_variance.sqrt();
                    for val in remainder {
                        *val = (*val - stats.posterior_mean) * inv_std;
                    }
                }
            }
            "min_max" | "minmax" => {
                let range = stats.max - stats.min;
                if range < f64::EPSILON {
                    values.fill(0.5);
                } else {
                    let min_vec = f64x4::splat(stats.min);
                    let inv_range_vec = f64x4::splat(1.0 / range);

                    // Process chunks of 4
                    let (chunks, remainder) =
                        values.split_at_mut(values.len() - (values.len() % 4));
                    for chunk in chunks.chunks_exact_mut(4) {
                        let vals = f64x4::from([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        let normalized = (vals - min_vec) * inv_range_vec;
                        chunk.copy_from_slice(&normalized.to_array());
                    }

                    // Handle remainder
                    let inv_range = 1.0 / range;
                    for val in remainder {
                        *val = (*val - stats.min) * inv_range;
                    }
                }
            }
            _ => {
                // Fallback to scalar implementation
                for val in values {
                    *val = self.normalize_value(*val, stats)?;
                }
            }
        }

        Ok(())
    }

    /// Normalize a single value using the given statistics
    fn normalize_value(&self, value: f64, stats: &FeatureStatistics) -> Result<f64> {
        if value.is_nan() || value.is_infinite() {
            return Ok(0.0);
        }

        let normalized = self.apply_normalization_scheme(value, stats)?;
        Ok(normalized.clamp(-10.0, 10.0))
    }

    /// Apply the configured normalization scheme to a value.
    fn apply_normalization_scheme(&self, value: f64, stats: &FeatureStatistics) -> Result<f64> {
        match self.scheme.as_str() {
            "z_score" | "zscore" => Ok(self.z_score_normalize(value, stats)),
            "min_max" | "minmax" => Ok(self.min_max_normalize(value, stats)),
            "robust" => Ok(self.robust_normalize(value, stats)),
            scheme if scheme.ends_with("_bayesian") => Ok(self.bayesian_normalize(value, stats)),
            _ => Err(ValknutError::config(format!(
                "Unknown normalization scheme: {}",
                self.scheme
            ))),
        }
    }

    /// Z-score normalization using posterior statistics.
    fn z_score_normalize(&self, value: f64, stats: &FeatureStatistics) -> f64 {
        if stats.posterior_variance < f64::EPSILON {
            0.0
        } else {
            (value - stats.posterior_mean) / stats.posterior_variance.sqrt()
        }
    }

    /// Min-max normalization to [0, 1] range.
    fn min_max_normalize(&self, value: f64, stats: &FeatureStatistics) -> f64 {
        let range = stats.max - stats.min;
        if range < f64::EPSILON {
            0.5
        } else {
            (value - stats.min) / range
        }
    }

    /// Robust normalization using median and MAD
    fn robust_normalize(&self, value: f64, stats: &FeatureStatistics) -> f64 {
        // For now, fallback to posterior mean and sqrt(variance)
        // TODO: Implement proper median and MAD calculation when needed
        if stats.posterior_variance < f64::EPSILON {
            0.0
        } else {
            (value - stats.posterior_mean) / stats.posterior_variance.sqrt()
        }
    }

    /// Bayesian normalization using posterior parameters
    fn bayesian_normalize(&self, value: f64, stats: &FeatureStatistics) -> f64 {
        if stats.posterior_variance < f64::EPSILON {
            // Use prior information to generate plausible normalized values
            if stats.confidence == VarianceConfidence::Insufficient {
                // Very low confidence, use prior-based random sampling
                self.sample_from_prior_normalized(stats.posterior_mean)
            } else {
                0.0
            }
        } else {
            // Standard Bayesian normalization
            (value - stats.posterior_mean) / stats.posterior_variance.sqrt()
        }
    }

    /// Sample a normalized value from prior knowledge
    fn sample_from_prior_normalized(&self, prior_mean: f64) -> f64 {
        // Use a simple transformation based on prior mean
        // This provides some variability while maintaining order
        if prior_mean < 0.5 {
            -0.5 // Slightly negative for low prior mean
        } else {
            0.5 // Slightly positive for high prior mean
        }
    }

    /// Calculate Bayesian posterior statistics combining empirical data with priors
    fn calculate_posterior_stats(
        &self,
        empirical: &FeatureStatistics,
        prior: &FeaturePrior,
    ) -> Result<FeatureStatistics> {
        let prior_weight = self.calculate_prior_weight(empirical.n_samples, empirical.confidence);
        let _empirical_weight = 1.0 - prior_weight;

        // Bayesian conjugate update for Normal-Normal model
        let prior_mean = prior.prior_mean();
        let prior_var = prior.prior_variance().max(f64::EPSILON);
        let empirical_var = empirical.variance.max(f64::EPSILON);

        // Posterior parameters
        let posterior_precision = 1.0 / prior_var + (empirical.n_samples as f64) / empirical_var;
        let posterior_variance = 1.0 / posterior_precision;

        let posterior_mean = posterior_variance
            * (prior_mean / prior_var
                + (empirical.n_samples as f64) * empirical.mean / empirical_var);

        let mut stats = empirical.clone();
        stats.prior_weight = prior_weight;
        stats.posterior_mean = posterior_mean;
        stats.posterior_variance = posterior_variance;

        Ok(stats)
    }

    /// Calculate the weight to give to prior vs empirical data
    fn calculate_prior_weight(&self, n_samples: usize, confidence: VarianceConfidence) -> f64 {
        let base_weight = match confidence {
            VarianceConfidence::High => 0.1,
            VarianceConfidence::Medium => 0.3,
            VarianceConfidence::Low => 0.5,
            VarianceConfidence::VeryLow => 0.7,
            VarianceConfidence::Insufficient => 0.9,
        };

        // Adjust based on sample size
        let sample_factor = 1.0 / (1.0 + (n_samples as f64).ln());
        (base_weight * sample_factor).clamp(0.05, 0.95)
    }

    /// Create a generic prior for unknown features
    fn create_generic_prior(&self, feature_name: &str) -> FeaturePrior {
        FeaturePrior::new(feature_name)
            .with_beta_params(1.0, 1.0)  // Uninformative prior
            .with_range(0.0, 1.0, 0.5)
            .with_type("generic", "normal")
    }

    /// Get statistics for a specific feature
    pub fn get_statistics(&self, feature_name: &str) -> Option<&FeatureStatistics> {
        self.statistics.get(feature_name)
    }

    /// Get all feature statistics
    pub fn get_all_statistics(&self) -> &HashMap<String, FeatureStatistics> {
        &self.statistics
    }

    /// Get confidence level for a feature
    pub fn get_confidence(&self, feature_name: &str) -> Option<VarianceConfidence> {
        self.variance_confidence.get(feature_name).copied()
    }

    /// Add a custom prior for a feature
    pub fn add_prior(&mut self, prior: FeaturePrior) {
        self.priors.insert(prior.name.clone(), prior);
    }

    /// Generate diagnostic information about the normalization
    pub fn get_diagnostics(&self) -> HashMap<String, serde_json::Value> {
        let feature_count = self.statistics.len();

        HashMap::from([
            ("confidence_distribution".to_string(), self.confidence_distribution_value()),
            ("total_features".to_string(), serde_json::json!(feature_count)),
            ("average_prior_weight".to_string(), self.average_prior_weight_value(feature_count)),
        ])
    }

    /// Calculate confidence distribution as a JSON value.
    fn confidence_distribution_value(&self) -> serde_json::Value {
        let counts = self.count_confidence_levels();
        serde_json::to_value(counts).unwrap_or_else(|e| {
            serde_json::Value::String(format!("Serialization error: {}", e))
        })
    }

    /// Count occurrences of each confidence level.
    fn count_confidence_levels(&self) -> HashMap<String, usize> {
        self.variance_confidence
            .values()
            .fold(HashMap::new(), |mut acc, &conf| {
                *acc.entry(format!("{:?}", conf)).or_insert(0) += 1;
                acc
            })
    }

    /// Calculate average prior weight as a JSON value.
    fn average_prior_weight_value(&self, feature_count: usize) -> serde_json::Value {
        let avg = self.average_prior_weight(feature_count);
        serde_json::Number::from_f64(avg)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::json!(0))
    }

    /// Calculate the average prior weight across all features.
    fn average_prior_weight(&self, feature_count: usize) -> f64 {
        if feature_count == 0 {
            return 0.0;
        }
        self.statistics.values().map(|s| s.prior_weight).sum::<f64>() / feature_count as f64
    }
}


#[cfg(test)]
#[path = "bayesian_tests.rs"]
mod tests;
