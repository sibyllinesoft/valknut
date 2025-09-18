//! Live Reachability Analysis System
//!
//! This module implements a production-safe runtime analysis system that samples
//! actual call patterns in deployed applications to identify "shadow islands" -
//! tightly coupled code communities with low live reachability.
//!
//! ## Key Features
//!
//! - **Non-intrusive Sampling**: Lightweight runtime collectors with minimal overhead
//! - **Versioned Call Graphs**: Track call pattern evolution across deployments
//! - **Community Detection**: Identify tightly coupled code clusters
//! - **Shadow Island Detection**: Find dead or rarely-used code communities
//! - **Production Safety**: Designed for zero-impact deployment monitoring
//!
//! ## Architecture Components
//!
//! - **collectors**: Runtime sampling infrastructure for call edge collection
//! - **storage**: Versioned storage system for call graph persistence
//! - **graph**: Call graph construction and analysis algorithms
//! - **community**: Code community detection using graph clustering
//! - **scoring**: Reachability scoring and shadow island ranking
//! - **reports**: Visualization and reporting for live analysis results
//! - **cli**: Command-line interface for live analysis operations
//! - **stacks**: Call stack analysis and pattern recognition
//!
//! ## Usage
//!
//! ```ignore
//! use valknut_rs::live::collectors::CallCollector;
//! use valknut_rs::live::graph::CallGraph;
//!
//! // Set up non-intrusive call collection
//! let collector = CallCollector::new()
//!     .with_sampling_rate(0.001) // 0.1% sampling
//!     .with_buffer_size(10000);
//!
//! // Analyze collected call patterns
//! let graph = CallGraph::from_samples(&collector.samples())?;
//! let communities = graph.detect_communities()?;
//! let shadow_islands = communities.find_shadow_islands()?;
//! ```
//!
//! ## Production Integration
//!
//! The live analysis system is designed for safe deployment in production environments:
//! - Configurable sampling rates to control overhead
//! - Async collection to avoid blocking application threads
//! - Graceful degradation on resource constraints
//! - Optional persistence for historical trend analysis

pub mod cli;
pub mod collectors;
pub mod community;
pub mod graph;
pub mod reports;
pub mod scoring;
pub mod stacks;
pub mod storage;
pub mod types;

pub use types::*;

pub use crate::core::config::IslandConfig;
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
            island: IslandConfig::default(),
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
                "Sample rate must be between 0.0 and 1.0",
            ));
        }

        if self.weight_static < 0.0 {
            return Err(ValknutError::validation(
                "Static weight must be non-negative",
            ));
        }

        if self.window_days == 0 {
            return Err(ValknutError::validation(
                "Window days must be greater than 0",
            ));
        }

        if self.island.min_size == 0 {
            return Err(ValknutError::validation(
                "Minimum island size must be greater than 0",
            ));
        }

        if self.island.min_score < 0.0 || self.island.min_score > 1.0 {
            return Err(ValknutError::validation(
                "Island score threshold must be between 0.0 and 1.0",
            ));
        }

        if self.island.resolution <= 0.0 {
            return Err(ValknutError::validation(
                "Louvain resolution must be positive",
            ));
        }

        Ok(())
    }
}

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
