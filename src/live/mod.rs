//! Live Reachability Analysis
//! 
//! This module implements a production-safe system for sampling runtime call edges,
//! aggregating them into versioned call graphs, and detecting "shadow islands" -
//! tightly coupled code communities with low live reach.

pub mod types;
pub mod collectors;
pub mod storage;
pub mod graph;
pub mod community;
pub mod scoring;
pub mod reports;
pub mod cli;
pub mod stacks;

pub use types::*;

use crate::core::errors::{Result, ValknutError};
use std::path::Path;

/// Main configuration for live reachability analysis
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LiveReachConfig {
    /// Whether live reachability analysis is enabled
    pub enabled: bool,
    
    /// Services to include in analysis
    pub services: Vec<String>,
    
    /// Sampling rate for runtime collection (0.0 to 1.0)
    pub sample_rate: f64,
    
    /// Weight for static edges relative to runtime edges
    pub weight_static: f64,
    
    /// Analysis window in days
    pub window_days: u32,
    
    /// Island detection configuration
    pub island: IslandConfig,
    
    /// CI integration configuration
    pub ci: CiConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IslandConfig {
    /// Minimum community size to consider
    pub min_size: usize,
    
    /// Minimum score threshold for shadow islands
    pub min_score: f64,
    
    /// Louvain resolution parameter
    pub resolution: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CiConfig {
    /// Whether to warn about new code in shadow islands
    pub warn: bool,
    
    /// Whether to fail builds for shadow islands (not implemented yet)
    pub hard_fail: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageConfig {
    /// Storage bucket or path
    pub bucket: String,
    
    /// Path layout template
    pub layout: String,
}

impl Default for LiveReachConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            services: vec!["api".to_string(), "worker".to_string()],
            sample_rate: 0.02, // 2%
            weight_static: 0.1,
            window_days: 30,
            island: IslandConfig {
                min_size: 5,
                min_score: 0.6,
                resolution: 0.8,
            },
            ci: CiConfig {
                warn: true,
                hard_fail: false,
            },
            storage: StorageConfig {
                bucket: "s3://company-valknut/live".to_string(),
                layout: "edges/date={date}/svc={svc}/ver={ver}/part-*.parquet".to_string(),
            },
        }
    }
}

impl LiveReachConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.sample_rate < 0.0 || self.sample_rate > 1.0 {
            return Err(ValknutError::validation(
                "Sample rate must be between 0.0 and 1.0"
            ));
        }
        
        if self.weight_static < 0.0 {
            return Err(ValknutError::validation(
                "Static weight must be non-negative"
            ));
        }
        
        if self.window_days == 0 {
            return Err(ValknutError::validation(
                "Window days must be greater than 0"
            ));
        }
        
        if self.island.min_size == 0 {
            return Err(ValknutError::validation(
                "Minimum island size must be greater than 0"
            ));
        }
        
        if self.island.min_score < 0.0 || self.island.min_score > 1.0 {
            return Err(ValknutError::validation(
                "Island score threshold must be between 0.0 and 1.0"
            ));
        }
        
        if self.island.resolution <= 0.0 {
            return Err(ValknutError::validation(
                "Louvain resolution must be positive"
            ));
        }
        
        Ok(())
    }
}

// Re-export config types for external use
pub use self::LiveReachConfig;
pub use self::IslandConfig; 
pub use self::CiConfig;
pub use self::StorageConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LiveReachConfig::default();
        assert!(config.enabled);
        assert_eq!(config.sample_rate, 0.02);
        assert_eq!(config.weight_static, 0.1);
        assert_eq!(config.window_days, 30);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_sample_rate() {
        let mut config = LiveReachConfig::default();
        config.sample_rate = 1.5;
        assert!(config.validate().is_err());
        
        config.sample_rate = -0.1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_weight_static() {
        let mut config = LiveReachConfig::default();
        config.weight_static = -0.1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_window_days() {
        let mut config = LiveReachConfig::default();
        config.window_days = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_island_config() {
        let mut config = LiveReachConfig::default();
        
        config.island.min_size = 0;
        assert!(config.validate().is_err());
        
        config.island.min_size = 5;
        config.island.min_score = 1.5;
        assert!(config.validate().is_err());
        
        config.island.min_score = 0.6;
        config.island.resolution = 0.0;
        assert!(config.validate().is_err());
    }
}