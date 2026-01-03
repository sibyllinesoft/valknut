//! Statistical scoring and normalization algorithms.
//!
//! This module provides:
//! - Bayesian normalization for feature statistics
//! - Feature scoring and prioritization
//! - Variance confidence calculations

pub mod bayesian;
pub mod features;

// Re-export main types
pub use bayesian::{BayesianNormalizer, FeaturePrior, FeatureStatistics, VarianceConfidence};
pub use features::{FeatureNormalizer, FeatureScorer, NormalizationStatistics, Priority, ScoringResult};
