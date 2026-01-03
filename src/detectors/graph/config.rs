use serde::{Deserialize, Serialize};

use crate::core::config::validate_unit_range;
use crate::core::errors::Result;

/// Graph analysis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    /// Enable betweenness centrality calculation
    pub enable_betweenness: bool,

    /// Enable closeness centrality calculation
    pub enable_closeness: bool,

    /// Enable dependency cycle detection
    pub enable_cycle_detection: bool,

    /// Maximum graph size for exact algorithms
    pub max_exact_size: usize,

    /// Enable approximation algorithms for large graphs
    pub use_approximation: bool,

    /// Sampling rate for approximation algorithms
    pub approximation_sample_rate: f64,
}

/// Default implementation for [`GraphConfig`].
impl Default for GraphConfig {
    /// Returns the default graph analysis configuration.
    fn default() -> Self {
        Self {
            enable_betweenness: true,
            enable_closeness: false,
            enable_cycle_detection: true,
            max_exact_size: 10_000,
            use_approximation: true,
            approximation_sample_rate: 0.1,
        }
    }
}

/// Validation methods for [`GraphConfig`].
impl GraphConfig {
    /// Validate graph configuration
    pub fn validate(&self) -> Result<()> {
        validate_unit_range(self.approximation_sample_rate, "approximation_sample_rate")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_configuration_is_valid() {
        let config = GraphConfig::default();
        assert!(config.validate().is_ok());
        assert!(config.enable_betweenness);
        assert!(config.enable_cycle_detection);
        assert!(config.use_approximation);
        assert!((0.0..=1.0).contains(&config.approximation_sample_rate));
    }

    #[test]
    fn validate_rejects_out_of_range_sampling_rate() {
        let mut config = GraphConfig::default();
        config.approximation_sample_rate = 1.5;
        let err = config
            .validate()
            .expect_err("sampling rate must be in range");
        let message = format!("{}", err);
        assert!(
            message.contains("approximation_sample_rate"),
            "unexpected error message: {message}"
        );
    }
}
