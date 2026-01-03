//! Scoring and normalization configuration types.
//!
//! This module contains configuration for scoring, normalization schemes,
//! feature weights, and statistical parameters.

use serde::{Deserialize, Serialize};

use crate::core::errors::{Result, ValknutError};

/// Scoring and normalization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
    /// Normalization scheme to use
    #[serde(default)]
    pub normalization_scheme: NormalizationScheme,

    /// Enable Bayesian normalization fallbacks
    #[serde(default)]
    pub use_bayesian_fallbacks: bool,

    /// Enable confidence reporting
    #[serde(default)]
    pub confidence_reporting: bool,

    /// Feature weights configuration
    #[serde(default)]
    pub weights: WeightsConfig,

    /// Statistical parameters
    #[serde(default)]
    pub statistical_params: StatisticalParams,
}

/// Default implementation for [`ScoringConfig`].
impl Default for ScoringConfig {
    /// Returns default scoring configuration with Z-score normalization.
    fn default() -> Self {
        Self {
            normalization_scheme: NormalizationScheme::ZScore,
            use_bayesian_fallbacks: true,
            confidence_reporting: false,
            weights: WeightsConfig::default(),
            statistical_params: StatisticalParams::default(),
        }
    }
}

/// Validation methods for [`ScoringConfig`].
impl ScoringConfig {
    /// Validate scoring configuration
    pub fn validate(&self) -> Result<()> {
        self.weights.validate()?;
        self.statistical_params.validate()?;
        Ok(())
    }
}

/// Available normalization schemes
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationScheme {
    /// Z-score normalization (standardization)
    #[default]
    ZScore,
    /// Min-max normalization to [0, 1] range
    MinMax,
    /// Robust normalization using median and IQR
    Robust,
    /// Z-score with Bayesian priors
    ZScoreBayesian,
    /// Min-max with Bayesian estimation
    MinMaxBayesian,
    /// Robust with Bayesian estimation
    RobustBayesian,
}

/// Feature weights configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightsConfig {
    /// Complexity feature weights
    #[serde(default)]
    pub complexity: f64,

    /// Graph-based feature weights
    #[serde(default)]
    pub graph: f64,

    /// Structure-based feature weights
    #[serde(default)]
    pub structure: f64,

    /// Style-based feature weights
    #[serde(default)]
    pub style: f64,

    /// Coverage-based feature weights
    #[serde(default)]
    pub coverage: f64,
}

/// Default implementation for [`WeightsConfig`].
impl Default for WeightsConfig {
    /// Returns default feature weights balanced for common use cases.
    fn default() -> Self {
        Self {
            complexity: 1.0,
            graph: 0.8,
            structure: 0.9,
            style: 0.5,
            coverage: 0.7,
        }
    }
}

/// Validation methods for [`WeightsConfig`].
impl WeightsConfig {
    /// Validate weights configuration
    pub fn validate(&self) -> Result<()> {
        let weights = [
            self.complexity,
            self.graph,
            self.structure,
            self.style,
            self.coverage,
        ];

        for (name, &weight) in ["complexity", "graph", "structure", "style", "coverage"]
            .iter()
            .zip(&weights)
        {
            if weight < 0.0 || weight > 10.0 {
                return Err(ValknutError::validation(format!(
                    "Weight for '{}' must be between 0.0 and 10.0, got {}",
                    name, weight
                )));
            }
        }

        Ok(())
    }
}

/// Statistical parameters for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalParams {
    /// Confidence interval level (0.95 = 95%)
    #[serde(default)]
    pub confidence_level: f64,

    /// Minimum sample size for statistical analysis
    #[serde(default)]
    pub min_sample_size: usize,

    /// Outlier detection threshold (in standard deviations)
    #[serde(default)]
    pub outlier_threshold: f64,
}

/// Default implementation for [`StatisticalParams`].
impl Default for StatisticalParams {
    /// Returns default statistical parameters for analysis.
    fn default() -> Self {
        Self {
            confidence_level: 0.95,
            min_sample_size: 10,
            outlier_threshold: 3.0,
        }
    }
}

/// Validation methods for [`StatisticalParams`].
impl StatisticalParams {
    /// Validate statistical parameters
    pub fn validate(&self) -> Result<()> {
        if !(0.0..1.0).contains(&self.confidence_level) {
            return Err(ValknutError::validation(format!(
                "confidence_level must be between 0.0 and 1.0, got {}",
                self.confidence_level
            )));
        }

        if self.min_sample_size == 0 {
            return Err(ValknutError::validation(
                "min_sample_size must be greater than 0",
            ));
        }

        if self.outlier_threshold <= 0.0 {
            return Err(ValknutError::validation(
                "outlier_threshold must be positive",
            ));
        }

        Ok(())
    }
}
