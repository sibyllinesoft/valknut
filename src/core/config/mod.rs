//! Configuration types and management for valknut-rs.
//!
//! This module provides comprehensive configuration structures that mirror
//! the Python implementation while adding Rust-specific optimizations and
//! type safety guarantees.

pub mod dedupe;
pub mod live_reach;
pub mod validation;

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::errors::{Result, ValknutError};
use crate::detectors::cohesion::CohesionConfig;
use crate::detectors::structure::StructureConfig;

// Re-export types from submodules
pub use dedupe::{
    AdaptiveDenoiseConfig, AutoCalibrationConfig, DedupeConfig, DedupeWeights, DenoiseConfig,
    DenoiseWeights, RankingBy, RankingConfig, RankingCriteria, StopMotifsConfig,
};
pub use live_reach::{BuildConfig, IngestConfig, IslandConfig, LiveReachConfig};
pub use validation::{
    validate_non_negative, validate_positive_f64, validate_positive_i64, validate_positive_u32,
    validate_positive_usize, validate_unit_range,
};

/// Documentation health thresholds and penalties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocHealthConfig {
    /// Minimum AST nodes for a function before doc is required
    #[serde(default = "DocHealthConfig::default_min_fn_nodes")]
    pub min_fn_nodes: usize,
    /// Minimum AST nodes for a file before doc is required
    #[serde(default = "DocHealthConfig::default_min_file_nodes")]
    pub min_file_nodes: usize,
    /// Minimum files per directory before directory-level doc penalty applies
    #[serde(default = "DocHealthConfig::default_min_files_per_dir")]
    pub min_files_per_dir: usize,
}

impl Default for DocHealthConfig {
    fn default() -> Self {
        Self {
            min_fn_nodes: Self::default_min_fn_nodes(),
            min_file_nodes: Self::default_min_file_nodes(),
            min_files_per_dir: Self::default_min_files_per_dir(),
        }
    }
}

impl DocHealthConfig {
    const fn default_min_fn_nodes() -> usize {
        5
    }

    const fn default_min_file_nodes() -> usize {
        50
    }

    const fn default_min_files_per_dir() -> usize {
        5
    }
}

/// Main configuration for valknut analysis engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValknutConfig {
    /// Analysis pipeline configuration
    pub analysis: AnalysisConfig,

    /// Scoring and normalization settings
    pub scoring: ScoringConfig,

    /// Graph analysis configuration
    pub graph: GraphConfig,

    /// LSH and similarity detection settings
    pub lsh: LshConfig,

    /// Enhanced duplicate detection configuration
    #[serde(default)]
    pub dedupe: DedupeConfig,

    /// Clone denoising configuration
    #[serde(default)]
    pub denoise: DenoiseConfig,

    /// Language-specific settings
    pub languages: HashMap<String, LanguageConfig>,

    /// I/O and persistence settings
    pub io: IoConfig,

    /// Performance and resource limits
    pub performance: PerformanceConfig,

    /// Structure analysis configuration
    pub structure: StructureConfig,

    /// Coverage analysis and file discovery configuration
    #[serde(default)]
    pub coverage: CoverageConfig,

    /// Documentation health configuration
    #[serde(default)]
    pub docs: DocHealthConfig,

    /// Semantic cohesion analysis configuration
    #[serde(default)]
    pub cohesion: CohesionConfig,

    /// Live reachability analysis configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_reach: Option<LiveReachConfig>,

    /// Code quality analysis configuration (simple pattern-based analysis)
    // pub names: NamesConfig,
    /// Placeholder to maintain serialization compatibility
    #[serde(skip)]
    pub _names_placeholder: Option<()>,
}

impl Default for ValknutConfig {
    fn default() -> Self {
        Self::new_with_defaults()
    }
}

impl ValknutConfig {
    /// Construct a configuration using the canonical default values used across
    /// the CLI and public API layers. Keeping this in one place prevents the
    /// various configuration surfaces from drifting apart.
    pub(crate) fn new_with_defaults() -> Self {
        Self {
            analysis: AnalysisConfig::default(),
            scoring: ScoringConfig::default(),
            graph: GraphConfig::default(),
            lsh: LshConfig::default(),
            dedupe: DedupeConfig::default(),
            denoise: DenoiseConfig::default(),
            languages: Self::default_languages(),
            io: IoConfig::default(),
            performance: PerformanceConfig::default(),
            structure: StructureConfig::default(),
            coverage: CoverageConfig::default(),
            docs: DocHealthConfig::default(),
            cohesion: CohesionConfig::default(),
            live_reach: None,
            _names_placeholder: None,
        }
    }

    /// Load configuration from a YAML file
    pub fn from_yaml_file(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let content = std::fs::read_to_string(&path).map_err(|e| {
            ValknutError::io(format!("Failed to read config file: {}", path.display()), e)
        })?;

        serde_yaml::from_str(&content).map_err(Into::into)
    }

    /// Save configuration to a YAML file
    pub fn to_yaml_file(&self, path: impl Into<PathBuf>) -> Result<()> {
        let path = path.into();
        let content = serde_yaml::to_string(self)?;
        std::fs::write(&path, content).map_err(|e| {
            ValknutError::io(
                format!("Failed to write config file: {}", path.display()),
                e,
            )
        })
    }

    /// Get default language configurations
    fn default_languages() -> HashMap<String, LanguageConfig> {
        let mut languages = HashMap::new();

        languages.insert(
            "python".to_string(),
            LanguageConfig {
                enabled: true,
                file_extensions: vec![".py".to_string(), ".pyi".to_string()],
                tree_sitter_language: "python".to_string(),
                max_file_size_mb: 10.0,
                complexity_threshold: 10.0,
                additional_settings: HashMap::new(),
            },
        );

        languages.insert(
            "javascript".to_string(),
            LanguageConfig {
                enabled: true,
                file_extensions: vec![".js".to_string(), ".mjs".to_string(), ".jsx".to_string()],
                tree_sitter_language: "javascript".to_string(),
                max_file_size_mb: 5.0,
                complexity_threshold: 10.0,
                additional_settings: HashMap::new(),
            },
        );

        languages.insert(
            "typescript".to_string(),
            LanguageConfig {
                enabled: true,
                file_extensions: vec![".ts".to_string(), ".tsx".to_string(), ".d.ts".to_string()],
                tree_sitter_language: "typescript".to_string(),
                max_file_size_mb: 5.0,
                complexity_threshold: 10.0,
                additional_settings: HashMap::new(),
            },
        );

        languages.insert(
            "rust".to_string(),
            LanguageConfig {
                enabled: true,
                file_extensions: vec![".rs".to_string()],
                tree_sitter_language: "rust".to_string(),
                max_file_size_mb: 10.0,
                complexity_threshold: 15.0,
                additional_settings: HashMap::new(),
            },
        );

        languages.insert(
            "go".to_string(),
            LanguageConfig {
                enabled: true,
                file_extensions: vec![".go".to_string()],
                tree_sitter_language: "go".to_string(),
                max_file_size_mb: 8.0,
                complexity_threshold: 12.0,
                additional_settings: HashMap::new(),
            },
        );

        languages
    }

    /// Validate configuration settings
    pub fn validate(&self) -> Result<()> {
        self.analysis.validate()?;
        self.scoring.validate()?;
        self.graph.validate()?;
        self.lsh.validate()?;
        self.performance.validate()?;

        // Validate language configurations
        for (lang, config) in &self.languages {
            config.validate().map_err(|e| {
                ValknutError::config_field(
                    format!("Invalid language configuration: {e}"),
                    format!("languages.{lang}"),
                )
            })?;
        }

        // Validate dedupe configuration
        self.dedupe.validate()?;

        // Validate denoise configuration
        self.denoise.validate()?;

        // Validate coverage configuration
        self.coverage.validate()?;

        Ok(())
    }
}

/// Analysis pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Enable scoring analysis
    #[serde(default)]
    pub enable_scoring: bool,

    /// Enable graph analysis
    #[serde(default)]
    pub enable_graph_analysis: bool,

    /// Enable LSH-based similarity detection
    #[serde(default)]
    pub enable_lsh_analysis: bool,

    /// Enable refactoring analysis
    #[serde(default)]
    pub enable_refactoring_analysis: bool,

    /// Enable coverage analysis
    #[serde(default)]
    pub enable_coverage_analysis: bool,

    /// Enable structure analysis
    #[serde(default)]
    pub enable_structure_analysis: bool,

    /// Enable code quality analysis
    #[serde(default)]
    pub enable_names_analysis: bool,

    /// Enable semantic cohesion analysis (experimental - uses local embeddings)
    #[serde(default)]
    pub enable_cohesion_analysis: bool,

    /// Minimum confidence threshold for results
    #[serde(default)]
    pub confidence_threshold: f64,

    /// Maximum number of files to process (0 = unlimited)
    #[serde(default)]
    pub max_files: usize,

    /// File patterns to exclude from analysis
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// File patterns to include in analysis
    #[serde(default)]
    pub include_patterns: Vec<String>,

    /// Additional ignore patterns applied after include/exclude
    #[serde(default)]
    pub ignore_patterns: Vec<String>,

    /// Maximum file size in bytes to analyze (0 = unlimited, default = 500KB)
    /// Files larger than this are skipped during file discovery
    #[serde(default = "AnalysisConfig::default_max_file_size_bytes")]
    pub max_file_size_bytes: u64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            enable_scoring: true,
            enable_graph_analysis: true,
            enable_lsh_analysis: false,
            enable_refactoring_analysis: true,
            enable_coverage_analysis: true,
            enable_structure_analysis: true,
            enable_names_analysis: true,
            enable_cohesion_analysis: false, // Disabled by default - experimental
            confidence_threshold: 0.7,
            max_files: 0,
            exclude_patterns: vec![
                "*/node_modules/*".to_string(),
                "*/venv/*".to_string(),
                "*/target/*".to_string(),
                "*/__pycache__/*".to_string(),
                "*.min.js".to_string(),
            ],
            include_patterns: vec!["**/*".to_string()],
            ignore_patterns: Vec::new(),
            max_file_size_bytes: Self::default_max_file_size_bytes(),
        }
    }
}

impl AnalysisConfig {
    /// Default maximum file size: 500KB
    pub const fn default_max_file_size_bytes() -> u64 {
        500 * 1024
    }

    /// Validate analysis configuration
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=1.0).contains(&self.confidence_threshold) {
            return Err(ValknutError::validation(format!(
                "confidence_threshold must be between 0.0 and 1.0, got {}",
                self.confidence_threshold
            )));
        }
        Ok(())
    }
}

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

impl Default for ScoringConfig {
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

impl Default for WeightsConfig {
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

impl Default for StatisticalParams {
    fn default() -> Self {
        Self {
            confidence_level: 0.95,
            min_sample_size: 10,
            outlier_threshold: 3.0,
        }
    }
}

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

/// Graph analysis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    /// Enable betweenness centrality calculation
    #[serde(default)]
    pub enable_betweenness: bool,

    /// Enable closeness centrality calculation
    #[serde(default)]
    pub enable_closeness: bool,

    /// Enable cycle detection
    #[serde(default)]
    pub enable_cycle_detection: bool,

    /// Maximum graph size for exact algorithms
    #[serde(default)]
    pub max_exact_size: usize,

    /// Use approximation algorithms for large graphs
    #[serde(default)]
    pub use_approximation: bool,

    /// Sampling rate for approximation algorithms
    #[serde(default)]
    pub approximation_sample_rate: f64,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            enable_betweenness: true,
            enable_closeness: false,
            enable_cycle_detection: true,
            max_exact_size: 10000,
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

/// LSH and similarity detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LshConfig {
    /// Number of hash functions per band
    #[serde(default)]
    pub num_hashes: usize,

    /// Number of LSH bands
    #[serde(default)]
    pub num_bands: usize,

    /// Shingle size for text similarity
    #[serde(default)]
    pub shingle_size: usize,

    /// Minimum Jaccard similarity threshold
    #[serde(default)]
    pub similarity_threshold: f64,

    /// Maximum candidates to consider per query
    #[serde(default)]
    pub max_candidates: usize,

    /// Use advanced similarity algorithms
    #[serde(default)]
    pub use_semantic_similarity: bool,

    /// Verify candidate clone pairs using tree edit distance (APTED)
    #[serde(default)]
    pub verify_with_apted: bool,

    /// Maximum AST nodes allowed when building APTED trees per entity
    #[serde(default = "LshConfig::default_apted_max_nodes")]
    pub apted_max_nodes: usize,

    /// Maximum number of clone candidates per entity to verify via APTED (0 = use max_candidates)
    #[serde(default)]
    pub apted_max_pairs_per_entity: usize,
}

impl Default for LshConfig {
    fn default() -> Self {
        Self {
            num_hashes: 128,
            num_bands: 8, // Reduced from 16 -> 8 for faster candidate filtering (16 rows per band)
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 100,
            use_semantic_similarity: false, // Keep name for backward compatibility
            verify_with_apted: true,
            apted_max_nodes: LshConfig::default_apted_max_nodes(),
            apted_max_pairs_per_entity: 25,
        }
    }
}

impl LshConfig {
    /// Default maximum number of AST nodes considered when building APTED trees
    pub const fn default_apted_max_nodes() -> usize {
        4000
    }

    /// Validate LSH configuration
    pub fn validate(&self) -> Result<()> {
        if self.num_hashes == 0 {
            return Err(ValknutError::validation(
                "num_hashes must be greater than 0",
            ));
        }

        if self.num_bands == 0 {
            return Err(ValknutError::validation("num_bands must be greater than 0"));
        }

        if self.num_hashes % self.num_bands != 0 {
            return Err(ValknutError::validation(
                "num_hashes must be divisible by num_bands",
            ));
        }

        if !(0.0..=1.0).contains(&self.similarity_threshold) {
            return Err(ValknutError::validation(format!(
                "similarity_threshold must be between 0.0 and 1.0, got {}",
                self.similarity_threshold
            )));
        }

        if self.verify_with_apted && self.apted_max_nodes == 0 {
            return Err(ValknutError::validation(
                "apted_max_nodes must be greater than 0 when APTED verification is enabled"
                    .to_string(),
            ));
        }

        Ok(())
    }

    /// Get the number of hashes per band
    pub fn hashes_per_band(&self) -> usize {
        self.num_hashes / self.num_bands
    }
}

/// Language-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    /// Enable analysis for this language
    pub enabled: bool,

    /// File extensions to process
    pub file_extensions: Vec<String>,

    /// Tree-sitter language identifier
    pub tree_sitter_language: String,

    /// Maximum file size to process (in MB)
    pub max_file_size_mb: f64,

    /// Complexity threshold for this language
    pub complexity_threshold: f64,

    /// Additional language-specific settings
    #[serde(default)]
    pub additional_settings: HashMap<String, serde_json::Value>,
}

impl LanguageConfig {
    /// Validate language configuration
    pub fn validate(&self) -> Result<()> {
        if self.file_extensions.is_empty() {
            return Err(ValknutError::validation("file_extensions cannot be empty"));
        }
        validate_positive_f64(self.max_file_size_mb, "max_file_size_mb")?;
        validate_positive_f64(self.complexity_threshold, "complexity_threshold")?;
        Ok(())
    }
}

/// I/O and persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoConfig {
    /// Cache directory path
    pub cache_dir: Option<PathBuf>,

    /// Enable result caching
    #[serde(default)]
    pub enable_caching: bool,

    /// Cache TTL in seconds
    #[serde(default)]
    pub cache_ttl_seconds: u64,

    /// Report output directory
    pub report_dir: Option<PathBuf>,

    /// Report format
    #[serde(default)]
    pub report_format: ReportFormat,

    /// Enable database persistence
    #[cfg(feature = "database")]
    #[serde(default)]
    pub enable_database: bool,

    /// Database connection string
    #[cfg(feature = "database")]
    pub database_url: Option<String>,
}

impl Default for IoConfig {
    fn default() -> Self {
        Self {
            cache_dir: None,
            enable_caching: true,
            cache_ttl_seconds: 3600, // 1 hour
            report_dir: None,
            report_format: ReportFormat::Json,
            #[cfg(feature = "database")]
            enable_database: false,
            #[cfg(feature = "database")]
            database_url: None,
        }
    }
}

/// Available report formats
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    /// JSON format
    #[default]
    Json,
    /// YAML format
    Yaml,
    /// HTML format
    Html,
    /// CSV format (for tabular data)
    Csv,
}

/// Performance and resource configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum number of parallel threads
    pub max_threads: Option<usize>,

    /// Memory limit in MB
    pub memory_limit_mb: Option<usize>,

    /// Timeout for individual file analysis (seconds)
    #[serde(default)]
    pub file_timeout_seconds: u64,

    /// Timeout for entire analysis (seconds)
    pub total_timeout_seconds: Option<u64>,

    /// Enable SIMD optimizations
    #[serde(default)]
    pub enable_simd: bool,

    /// Batch size for parallel processing
    #[serde(default)]
    pub batch_size: usize,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_threads: None,     // Use system default
            memory_limit_mb: None, // No limit
            file_timeout_seconds: 30,
            total_timeout_seconds: None, // No limit
            enable_simd: cfg!(feature = "simd"),
            batch_size: 100,
        }
    }
}

impl PerformanceConfig {
    /// Validate performance configuration
    pub fn validate(&self) -> Result<()> {
        if let Some(threads) = self.max_threads {
            if threads == 0 {
                return Err(ValknutError::validation(
                    "max_threads must be greater than 0",
                ));
            }
        }

        if let Some(memory) = self.memory_limit_mb {
            if memory == 0 {
                return Err(ValknutError::validation(
                    "memory_limit_mb must be greater than 0",
                ));
            }
        }

        if self.batch_size == 0 {
            return Err(ValknutError::validation(
                "batch_size must be greater than 0",
            ));
        }

        Ok(())
    }
}

/// Configuration for coverage analysis and automatic file discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageConfig {
    /// Enable automatic coverage file discovery
    pub auto_discover: bool,

    /// Search paths for coverage files (relative to analysis root)
    #[serde(default)]
    pub search_paths: Vec<String>,

    /// File patterns to search for
    #[serde(default)]
    pub file_patterns: Vec<String>,

    /// Maximum age of coverage files in days (0 = no age limit)
    pub max_age_days: u32,

    /// Specific coverage file path (overrides auto discovery)
    pub coverage_file: Option<PathBuf>,
}

impl Default for CoverageConfig {
    fn default() -> Self {
        Self {
            auto_discover: true,
            search_paths: vec![
                "./coverage/".to_string(),
                "./target/coverage/".to_string(),
                "./target/tarpaulin/".to_string(),
                "./target/".to_string(),
                "./.coverage/".to_string(),
                "./htmlcov/".to_string(),
                "./coverage-reports/".to_string(),
                "./reports/".to_string(),
                "./test-results/".to_string(),
                "./build/coverage/".to_string(),
                "./build/test-results/".to_string(),
                "./".to_string(), // Root directory last
            ],
            file_patterns: vec![
                // Primary coverage file patterns
                "coverage.xml".to_string(),
                "lcov.info".to_string(),
                "coverage.json".to_string(),
                "coverage.lcov".to_string(),
                "cobertura.xml".to_string(),
                // Coverage.py variations
                "coverage-final.json".to_string(),
                "coverage-summary.json".to_string(),
                ".coverage".to_string(),
                // Common framework patterns
                "junit.xml".to_string(),
                "jacoco.xml".to_string(),
                "clover.xml".to_string(),
                // Recursive patterns
                "**/coverage.xml".to_string(),
                "**/lcov.info".to_string(),
                "**/coverage.json".to_string(),
                "**/cobertura.xml".to_string(),
                "**/jacoco.xml".to_string(),
                "**/clover.xml".to_string(),
                // Language-specific patterns
                "target/coverage/*.xml".to_string(),
                "target/tarpaulin/coverage.xml".to_string(),
                "target/llvm-cov/coverage.lcov".to_string(),
                "build/coverage/*.xml".to_string(),
                "coverage/coverage-final.json".to_string(),
                "htmlcov/coverage.json".to_string(),
                // Build system patterns
                "**/build/jacoco/*.xml".to_string(),
                "**/build/reports/jacoco/test/*.xml".to_string(),
                "**/build/test-results/test/*.xml".to_string(),
            ],
            max_age_days: 7, // Only use coverage files newer than 7 days
            coverage_file: None,
        }
    }
}

impl CoverageConfig {
    /// Validate coverage configuration
    pub fn validate(&self) -> Result<()> {
        if self.file_patterns.is_empty() && self.auto_discover {
            return Err(ValknutError::validation(
                "file_patterns cannot be empty when auto_discover is enabled",
            ));
        }

        if self.search_paths.is_empty() && self.auto_discover {
            return Err(ValknutError::validation(
                "search_paths cannot be empty when auto_discover is enabled",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
