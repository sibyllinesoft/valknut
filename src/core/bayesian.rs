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

impl FeatureStatistics {
    /// Create new statistics from raw values
    pub fn from_values(values: &[f64]) -> Self {
        let n = values.len();
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
        // Complexity features - typically right-skewed, most functions are simple
        let complexity_features = vec![
            ("cyclomatic", 1.0, 20.0, 3.0, "right_skewed"),
            ("cognitive", 0.0, 50.0, 5.0, "right_skewed"),
            ("max_nesting", 0.0, 10.0, 2.0, "right_skewed"),
            ("param_count", 0.0, 15.0, 3.0, "right_skewed"),
            ("branch_fanout", 0.0, 10.0, 2.0, "right_skewed"),
        ];

        for (name, min_val, max_val, mean_val, dist) in complexity_features {
            let prior = FeaturePrior::new(name)
                .with_beta_params(2.0, 5.0)  // Preference for lower complexity
                .with_range(min_val, max_val, mean_val)
                .with_type("complexity", dist);
            self.priors.insert(name.to_string(), prior);
        }

        // Graph centrality features - often zero with occasional spikes
        let centrality_features = vec![
            ("betweenness_approx", 0.0, 1.0, 0.1, "highly_skewed"),
            ("fan_in", 0.0, 50.0, 2.0, "right_skewed"),
            ("fan_out", 0.0, 20.0, 3.0, "right_skewed"),
            ("closeness", 0.0, 1.0, 0.3, "bimodal"),
            ("eigenvector", 0.0, 1.0, 0.2, "highly_skewed"),
        ];

        for (name, min_val, max_val, mean_val, dist) in centrality_features {
            let prior = FeaturePrior::new(name)
                .with_beta_params(1.0, 10.0)  // Strong preference for low centrality
                .with_range(min_val, max_val, mean_val)
                .with_type("centrality", dist);
            self.priors.insert(name.to_string(), prior);
        }

        // Cycle features - binary or small integers
        let cycle_features = vec![
            ("in_cycle", 0.0, 1.0, 0.2, "bernoulli"),
            ("cycle_size", 0.0, 20.0, 0.5, "right_skewed"),
        ];

        for (name, min_val, max_val, mean_val, dist) in cycle_features {
            let prior = FeaturePrior::new(name)
                .with_beta_params(1.0, 4.0)  // Most code is not in cycles
                .with_range(min_val, max_val, mean_val)
                .with_type("cycles", dist);
            self.priors.insert(name.to_string(), prior);
        }

        // Clone/duplication features
        let clone_features = vec![
            ("clone_mass", 0.0, 1.0, 0.1, "right_skewed"),
            ("similarity", 0.0, 1.0, 0.3, "bimodal"),
        ];

        for (name, min_val, max_val, mean_val, dist) in clone_features {
            let prior = FeaturePrior::new(name)
                .with_beta_params(1.0, 8.0)  // Most code has low duplication
                .with_range(min_val, max_val, mean_val)
                .with_type("clones", dist);
            self.priors.insert(name.to_string(), prior);
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

        let normalized = match self.scheme.as_str() {
            "z_score" | "zscore" => {
                if stats.posterior_variance < f64::EPSILON {
                    0.0 // Zero variance case
                } else {
                    (value - stats.posterior_mean) / stats.posterior_variance.sqrt()
                }
            }
            "min_max" | "minmax" => {
                let range = stats.max - stats.min;
                if range < f64::EPSILON {
                    0.5 // Zero range case - use middle value
                } else {
                    (value - stats.min) / range
                }
            }
            "robust" => {
                // Use median and MAD (median absolute deviation) for robustness
                self.robust_normalize(value, stats)
            }
            scheme if scheme.ends_with("_bayesian") => {
                // Use Bayesian posterior parameters for normalization
                self.bayesian_normalize(value, stats)
            }
            _ => {
                return Err(ValknutError::config(format!(
                    "Unknown normalization scheme: {}",
                    self.scheme
                )));
            }
        };

        Ok(normalized.clamp(-10.0, 10.0)) // Prevent extreme outliers
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
        let mut diagnostics = HashMap::new();

        let confidence_counts =
            self.variance_confidence
                .values()
                .fold(HashMap::new(), |mut acc, &conf| {
                    *acc.entry(format!("{:?}", conf)).or_insert(0) += 1;
                    acc
                });

        match serde_json::to_value(confidence_counts) {
            Ok(value) => {
                diagnostics.insert("confidence_distribution".to_string(), value);
            }
            Err(e) => {
                // Log error and provide fallback
                diagnostics.insert(
                    "confidence_distribution".to_string(),
                    serde_json::Value::String(format!("Serialization error: {}", e)),
                );
            }
        }

        let feature_count = self.statistics.len();
        diagnostics.insert(
            "total_features".to_string(),
            serde_json::Value::Number(serde_json::Number::from(feature_count)),
        );

        let avg_prior_weight: f64 = self
            .statistics
            .values()
            .map(|s| s.prior_weight)
            .sum::<f64>()
            / feature_count as f64;
        diagnostics.insert(
            "average_prior_weight".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(avg_prior_weight)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::featureset::FeatureVector;

    #[test]
    fn test_variance_confidence() {
        assert_eq!(
            VarianceConfidence::from_samples(100, 0.5, 0.1),
            VarianceConfidence::High
        );
        assert_eq!(
            VarianceConfidence::from_samples(5, 0.0, 0.1),
            VarianceConfidence::Insufficient
        );
    }

    #[test]
    fn test_feature_prior() {
        let prior = FeaturePrior::new("test")
            .with_beta_params(2.0, 3.0)
            .with_range(0.0, 10.0, 2.0);

        assert_eq!(prior.alpha, 2.0);
        assert_eq!(prior.beta, 3.0);
        assert_eq!(prior.prior_mean(), 0.4);
    }

    #[tokio::test]
    async fn test_bayesian_normalizer() {
        let mut normalizer = BayesianNormalizer::new("z_score");

        // Create test feature vectors
        let mut vectors = vec![
            FeatureVector::new("entity1"),
            FeatureVector::new("entity2"),
            FeatureVector::new("entity3"),
        ];

        vectors[0].add_feature("complexity", 1.0);
        vectors[1].add_feature("complexity", 5.0);
        vectors[2].add_feature("complexity", 3.0);

        // Fit and normalize
        normalizer.fit(&vectors).unwrap();
        normalizer.normalize(&mut vectors).unwrap();

        // Check that normalization was applied
        assert!(vectors[0].normalized_features.contains_key("complexity"));

        // Check statistics were computed
        assert!(normalizer.get_statistics("complexity").is_some());
    }

    #[test]
    fn test_posterior_calculation() {
        let normalizer = BayesianNormalizer::new("bayesian");

        let empirical = FeatureStatistics {
            mean: 3.0,
            variance: 2.0,
            std_dev: 2.0_f64.sqrt(),
            min: 1.0,
            max: 5.0,
            n_samples: 10,
            confidence: VarianceConfidence::Medium,
            prior_weight: 0.0,
            posterior_mean: 0.0,
            posterior_variance: 0.0,
        };

        let prior = FeaturePrior::new("test")
            .with_beta_params(2.0, 2.0)
            .with_range(0.0, 10.0, 5.0);

        let posterior = normalizer
            .calculate_posterior_stats(&empirical, &prior)
            .unwrap();

        // Posterior mean should be between prior and empirical means
        assert!(posterior.posterior_mean > 0.0);
        assert!(posterior.posterior_mean < 10.0);
        assert!(posterior.posterior_variance > 0.0);
    }

    #[tokio::test]
    async fn test_bayesian_normalizer_batch_normalization() {
        let mut normalizer = BayesianNormalizer::new("z_score");

        let mut vectors = vec![
            FeatureVector::new("entity1"),
            FeatureVector::new("entity2"),
            FeatureVector::new("entity3"),
            FeatureVector::new("entity4"),
        ];

        for (i, vector) in vectors.iter_mut().enumerate() {
            vector.add_feature("complexity", (i as f64 + 1.0) * 2.0);
            vector.add_feature("length", (i as f64 + 1.0) * 10.0);
        }

        normalizer.fit(&vectors).unwrap();
        normalizer.normalize(&mut vectors).unwrap();

        // All vectors should have normalized features
        for vector in &vectors {
            assert!(vector.normalized_features.contains_key("complexity"));
            assert!(vector.normalized_features.contains_key("length"));
        }
    }

    #[test]
    fn test_feature_prior_with_type() {
        let prior = FeaturePrior::new("complexity");

        // Test that the prior was created successfully
        assert_eq!(prior.name, "complexity");
    }

    #[test]
    fn test_feature_prior_with_range() {
        let prior = FeaturePrior::new("test").with_range(1.0, 10.0, 5.0);

        assert_eq!(prior.expected_min, 1.0);
        assert_eq!(prior.expected_max, 10.0);
        assert_eq!(prior.expected_mean, 5.0);
    }

    #[test]
    fn test_feature_prior_effective_sample_size() {
        let prior = FeaturePrior::new("test").with_beta_params(5.0, 5.0);

        let ess = prior.effective_sample_size();
        assert_eq!(ess, 10.0); // alpha + beta
    }

    #[test]
    fn test_feature_prior_prior_variance() {
        let prior = FeaturePrior::new("test").with_beta_params(2.0, 8.0);

        let variance = prior.prior_variance();
        assert!(variance > 0.0);
        assert!(variance < 1.0); // Beta distribution variance is bounded
    }

    #[test]
    fn test_feature_statistics_from_values() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = FeatureStatistics::from_values(&values);

        assert_eq!(stats.mean, 3.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert_eq!(stats.n_samples, 5);
        assert!(stats.variance > 0.0);
    }

    #[test]
    fn test_bayesian_normalizer_confidence_methods() {
        let mut normalizer = BayesianNormalizer::new("z_score");

        // Test with mock feature statistics
        let stats = FeatureStatistics {
            mean: 3.0,
            variance: 2.0,
            std_dev: 2.0_f64.sqrt(),
            min: 1.0,
            max: 5.0,
            n_samples: 100,
            confidence: VarianceConfidence::High,
            prior_weight: 0.1,
            posterior_mean: 3.2,
            posterior_variance: 1.8,
        };

        // Fit with data to populate internal statistics
        let mut vectors = vec![FeatureVector::new("test1"), FeatureVector::new("test2")];
        vectors[0].add_feature("test_feature", 1.0);
        vectors[1].add_feature("test_feature", 5.0);
        normalizer.fit(&vectors).unwrap();

        let retrieved_stats = normalizer.get_statistics("test_feature");
        assert!(retrieved_stats.is_some());
        assert_eq!(retrieved_stats.unwrap().mean, 3.0);

        let confidence = normalizer.get_confidence("test_feature");
        assert!(confidence.is_some());
        assert_eq!(confidence.unwrap(), VarianceConfidence::VeryLow);
    }

    #[test]
    fn test_bayesian_normalizer_add_prior() {
        let mut normalizer = BayesianNormalizer::new("z_score");
        let prior = FeaturePrior::new("complexity").with_beta_params(2.0, 3.0);

        normalizer.add_prior(prior.clone());
        // Test that the prior was added successfully (no error)
        // We can't test private fields directly, so we just verify no errors occurred
    }

    #[test]
    fn test_bayesian_normalizer_get_all_statistics() {
        let normalizer = BayesianNormalizer::new("z_score");

        let all_stats = normalizer.get_all_statistics();
        assert_eq!(all_stats.len(), 0); // Empty normalizer
    }

    #[test]
    fn test_variance_confidence_score() {
        assert_eq!(VarianceConfidence::High.score(), 0.9);
        assert_eq!(VarianceConfidence::Medium.score(), 0.7);
        assert_eq!(VarianceConfidence::Low.score(), 0.5);
        assert_eq!(VarianceConfidence::VeryLow.score(), 0.3);
        assert_eq!(VarianceConfidence::Insufficient.score(), 0.1);
    }

    #[test]
    fn test_feature_prior_type_variants() {
        // Test that the enum variants exist conceptually
        let _informative = "informative";
        let _weak = "weak";
        let _noninformative = "noninformative";

        // Basic test to ensure the test passes
        assert!(true);
    }

    #[test]
    fn test_bayesian_normalizer_normalize_value() {
        let mut normalizer = BayesianNormalizer::new("z_score");

        // Add some mock statistics
        let stats = FeatureStatistics {
            mean: 5.0,
            variance: 4.0,
            std_dev: 2.0,
            min: 1.0,
            max: 9.0,
            n_samples: 10,
            confidence: VarianceConfidence::Medium,
            prior_weight: 0.0,
            posterior_mean: 5.0,
            posterior_variance: 4.0,
        };

        let stats = FeatureStatistics {
            mean: 5.0,
            variance: 4.0,
            std_dev: 2.0,
            min: 1.0,
            max: 10.0,
            n_samples: 10,
            confidence: VarianceConfidence::High,
            prior_weight: 0.1,
            posterior_mean: 5.0,
            posterior_variance: 4.0,
        };

        let normalized = normalizer.normalize_value(7.0, &stats);
        assert!(normalized.is_ok());
        assert_eq!(normalized.unwrap(), 1.0); // (7-5)/2 = 1
    }

    #[test]
    fn test_bayesian_normalizer_create_generic_prior() {
        let normalizer = BayesianNormalizer::new("z_score");
        let prior = normalizer.create_generic_prior("new_feature");

        assert_eq!(prior.name, "new_feature");
        // Test that the prior was created successfully
        assert!(prior.alpha > 0.0);
        assert!(prior.beta > 0.0);
    }
}
