use serde::{Deserialize, Serialize};

use crate::core::errors::{Result, ValknutError};

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

impl Default for GraphConfig {
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

impl GraphConfig {
    /// Validate graph configuration
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=1.0).contains(&self.approximation_sample_rate) {
            return Err(ValknutError::validation(format!(
                "approximation_sample_rate must be between 0.0 and 1.0, got {}",
                self.approximation_sample_rate
            )));
        }
        Ok(())
    }
}
