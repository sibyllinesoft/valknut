//! Live reachability analysis configuration types.

use serde::{Deserialize, Serialize};

use crate::core::errors::{Result, ValknutError};

use super::validation::{
    validate_non_negative, validate_positive_f64, validate_positive_u32, validate_positive_usize,
    validate_unit_range,
};

/// Configuration for live reachability analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveReachConfig {
    /// Ingestion configuration
    pub ingest: IngestConfig,

    /// Build/analysis configuration
    pub build: BuildConfig,
}

/// Configuration for stack ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestConfig {
    /// Namespace allow-list (prefixes to include)
    #[serde(default)]
    pub ns_allow: Vec<String>,

    /// Language for symbol normalization (auto|jvm|py|go|node|native)
    #[serde(default = "default_language")]
    pub lang: String,

    /// Input file glob pattern
    #[serde(default = "default_input_glob")]
    pub input_glob: String,

    /// Output directory for processed data
    #[serde(default = "default_out_dir")]
    pub out_dir: String,

    /// Upload URI for cloud storage (S3/GCS/Azure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_uri: Option<String>,
}

/// Configuration for build/analysis phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Analysis window in days
    #[serde(default = "default_since_days")]
    pub since_days: u32,

    /// Services to include in analysis
    #[serde(default = "default_services")]
    pub services: Vec<String>,

    /// Weight for static edges relative to runtime edges
    #[serde(default = "default_weight_static")]
    pub weight_static: f64,

    /// Island detection configuration
    pub island: IslandConfig,
}

/// Configuration for shadow island detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IslandConfig {
    /// Minimum community size to consider
    #[serde(default = "default_min_size")]
    pub min_size: usize,

    /// Minimum score threshold for shadow islands
    #[serde(default = "default_min_score")]
    pub min_score: f64,

    /// Louvain resolution parameter for community detection
    #[serde(default = "default_resolution")]
    pub resolution: f64,
}

// Default value functions

/// Returns the default language setting ("auto").
fn default_language() -> String {
    "auto".to_string()
}

/// Returns the default input glob pattern ("stacks/*.txt").
fn default_input_glob() -> String {
    "stacks/*.txt".to_string()
}

/// Returns the default output directory (".valknut/live/out").
fn default_out_dir() -> String {
    ".valknut/live/out".to_string()
}

/// Returns the default analysis window in days (30).
fn default_since_days() -> u32 {
    30
}

/// Returns the default services list (["api"]).
fn default_services() -> Vec<String> {
    vec!["api".to_string()]
}

/// Returns the default static edge weight (0.1).
fn default_weight_static() -> f64 {
    0.1
}

/// Returns the default minimum community size (5).
fn default_min_size() -> usize {
    5
}

/// Returns the default minimum score threshold (0.6).
fn default_min_score() -> f64 {
    0.6
}

/// Returns the default Louvain resolution parameter (0.8).
fn default_resolution() -> f64 {
    0.8
}

/// Default implementation for [`LiveReachConfig`].
impl Default for LiveReachConfig {
    /// Returns the default live reachability configuration.
    fn default() -> Self {
        Self {
            ingest: IngestConfig::default(),
            build: BuildConfig::default(),
        }
    }
}

/// Default implementation for [`IngestConfig`].
impl Default for IngestConfig {
    /// Returns the default ingestion configuration.
    fn default() -> Self {
        Self {
            ns_allow: vec!["myco.".to_string(), "github.com/myco/".to_string()],
            lang: default_language(),
            input_glob: default_input_glob(),
            out_dir: default_out_dir(),
            upload_uri: Some("s3://company-valknut/live".to_string()),
        }
    }
}

/// Default implementation for [`BuildConfig`].
impl Default for BuildConfig {
    /// Returns the default build configuration.
    fn default() -> Self {
        Self {
            since_days: default_since_days(),
            services: default_services(),
            weight_static: default_weight_static(),
            island: IslandConfig::default(),
        }
    }
}

/// Default implementation for [`IslandConfig`].
impl Default for IslandConfig {
    /// Returns the default island detection configuration.
    fn default() -> Self {
        Self {
            min_size: default_min_size(),
            min_score: default_min_score(),
            resolution: default_resolution(),
        }
    }
}

/// Valid language identifiers for live reachability analysis.
const VALID_LANGUAGES: &[&str] = &["auto", "jvm", "py", "go", "node", "native"];

/// Validation methods for [`LiveReachConfig`].
impl LiveReachConfig {
    /// Validate the live reachability configuration
    pub fn validate(&self) -> Result<()> {
        if !VALID_LANGUAGES.contains(&self.ingest.lang.as_str()) {
            return Err(ValknutError::validation(format!(
                "Invalid language: {}",
                self.ingest.lang
            )));
        }

        validate_positive_u32(self.build.since_days, "since_days")?;
        validate_non_negative(self.build.weight_static, "weight_static")?;
        validate_positive_usize(self.build.island.min_size, "min_size")?;
        validate_unit_range(self.build.island.min_score, "min_score")?;
        validate_positive_f64(self.build.island.resolution, "resolution")?;

        Ok(())
    }
}
