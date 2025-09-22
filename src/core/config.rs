//! Configuration types and management for valknut-rs.
//!
//! This module provides comprehensive configuration structures that mirror
//! the Python implementation while adding Rust-specific optimizations and
//! type safety guarantees.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
// Removed unused regex import

use crate::core::errors::{Result, ValknutError};
use crate::detectors::structure::StructureConfig;
// use crate::detectors::names::NamesConfig;

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
            live_reach: None,
            // names: NamesConfig::default(),
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
        // Structure config has built-in validation through Default implementation

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
    pub enable_scoring: bool,

    /// Enable graph analysis
    pub enable_graph_analysis: bool,

    /// Enable LSH-based similarity detection
    pub enable_lsh_analysis: bool,

    /// Enable refactoring analysis
    pub enable_refactoring_analysis: bool,

    /// Enable coverage analysis
    pub enable_coverage_analysis: bool,

    /// Enable structure analysis
    pub enable_structure_analysis: bool,

    /// Enable code quality analysis
    pub enable_names_analysis: bool,

    /// Minimum confidence threshold for results
    pub confidence_threshold: f64,

    /// Maximum number of files to process (0 = unlimited)
    pub max_files: usize,

    /// File patterns to exclude from analysis
    pub exclude_patterns: Vec<String>,

    /// File patterns to include in analysis
    pub include_patterns: Vec<String>,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        let module_defaults = crate::api::config_types::AnalysisModules::default();

        Self {
            enable_scoring: module_defaults.complexity,
            enable_graph_analysis: module_defaults.dependencies,
            enable_lsh_analysis: module_defaults.duplicates,
            enable_refactoring_analysis: module_defaults.refactoring,
            enable_coverage_analysis: module_defaults.coverage,
            enable_structure_analysis: module_defaults.structure,
            enable_names_analysis: true,
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
        }
    }
}

impl AnalysisConfig {
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
    pub normalization_scheme: NormalizationScheme,

    /// Enable Bayesian normalization fallbacks
    pub use_bayesian_fallbacks: bool,

    /// Enable confidence reporting
    pub confidence_reporting: bool,

    /// Feature weights configuration
    pub weights: WeightsConfig,

    /// Statistical parameters
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationScheme {
    /// Z-score normalization (standardization)
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
    pub complexity: f64,

    /// Graph-based feature weights
    pub graph: f64,

    /// Structure-based feature weights
    pub structure: f64,

    /// Style-based feature weights
    pub style: f64,

    /// Coverage-based feature weights
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
    pub confidence_level: f64,

    /// Minimum sample size for statistical analysis
    pub min_sample_size: usize,

    /// Outlier detection threshold (in standard deviations)
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
    pub enable_betweenness: bool,

    /// Enable closeness centrality calculation
    pub enable_closeness: bool,

    /// Enable cycle detection
    pub enable_cycle_detection: bool,

    /// Maximum graph size for exact algorithms
    pub max_exact_size: usize,

    /// Use approximation algorithms for large graphs
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
    pub num_hashes: usize,

    /// Number of LSH bands
    pub num_bands: usize,

    /// Shingle size for text similarity
    pub shingle_size: usize,

    /// Minimum Jaccard similarity threshold
    pub similarity_threshold: f64,

    /// Maximum candidates to consider per query
    pub max_candidates: usize,

    /// Use advanced similarity algorithms
    pub use_semantic_similarity: bool,
}

impl Default for LshConfig {
    fn default() -> Self {
        Self {
            num_hashes: 128,
            num_bands: 8,  // Reduced from 16 -> 8 for faster candidate filtering (16 rows per band)
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 100,
            use_semantic_similarity: false, // Keep name for backward compatibility
        }
    }
}

impl LshConfig {
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
    pub additional_settings: HashMap<String, serde_json::Value>,
}

impl LanguageConfig {
    /// Validate language configuration
    pub fn validate(&self) -> Result<()> {
        if self.file_extensions.is_empty() {
            return Err(ValknutError::validation("file_extensions cannot be empty"));
        }

        if self.max_file_size_mb <= 0.0 {
            return Err(ValknutError::validation(
                "max_file_size_mb must be positive",
            ));
        }

        if self.complexity_threshold <= 0.0 {
            return Err(ValknutError::validation(
                "complexity_threshold must be positive",
            ));
        }

        Ok(())
    }
}

/// I/O and persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoConfig {
    /// Cache directory path
    pub cache_dir: Option<PathBuf>,

    /// Enable result caching
    pub enable_caching: bool,

    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,

    /// Report output directory
    pub report_dir: Option<PathBuf>,

    /// Report format
    pub report_format: ReportFormat,

    /// Enable database persistence
    #[cfg(feature = "database")]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    /// JSON format
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
    pub file_timeout_seconds: u64,

    /// Timeout for entire analysis (seconds)
    pub total_timeout_seconds: Option<u64>,

    /// Enable SIMD optimizations
    pub enable_simd: bool,

    /// Batch size for parallel processing
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
    pub search_paths: Vec<String>,

    /// File patterns to search for
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
fn default_language() -> String {
    "auto".to_string()
}
fn default_input_glob() -> String {
    "stacks/*.txt".to_string()
}
fn default_out_dir() -> String {
    ".valknut/live/out".to_string()
}
fn default_since_days() -> u32 {
    30
}
fn default_services() -> Vec<String> {
    vec!["api".to_string()]
}
fn default_weight_static() -> f64 {
    0.1
}
fn default_min_size() -> usize {
    5
}
fn default_min_score() -> f64 {
    0.6
}
fn default_resolution() -> f64 {
    0.8
}

impl Default for LiveReachConfig {
    fn default() -> Self {
        Self {
            ingest: IngestConfig::default(),
            build: BuildConfig::default(),
        }
    }
}

impl Default for IngestConfig {
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

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            since_days: default_since_days(),
            services: default_services(),
            weight_static: default_weight_static(),
            island: IslandConfig::default(),
        }
    }
}

impl Default for IslandConfig {
    fn default() -> Self {
        Self {
            min_size: default_min_size(),
            min_score: default_min_score(),
            resolution: default_resolution(),
        }
    }
}

impl LiveReachConfig {
    /// Validate the live reachability configuration
    pub fn validate(&self) -> Result<()> {
        // Validate language
        if !["auto", "jvm", "py", "go", "node", "native"].contains(&self.ingest.lang.as_str()) {
            return Err(ValknutError::validation(format!(
                "Invalid language: {}",
                self.ingest.lang
            )));
        }

        // Validate build config
        if self.build.since_days == 0 {
            return Err(ValknutError::validation(
                "since_days must be greater than 0",
            ));
        }

        if self.build.weight_static < 0.0 {
            return Err(ValknutError::validation(
                "weight_static must be non-negative",
            ));
        }

        if self.build.island.min_size == 0 {
            return Err(ValknutError::validation("min_size must be greater than 0"));
        }

        if self.build.island.min_score < 0.0 || self.build.island.min_score > 1.0 {
            return Err(ValknutError::validation(
                "min_score must be between 0.0 and 1.0",
            ));
        }

        if self.build.island.resolution <= 0.0 {
            return Err(ValknutError::validation("resolution must be positive"));
        }

        Ok(())
    }
}

/// Enhanced duplicate detection configuration with adaptive features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupeConfig {
    /// File patterns to include in dedupe analysis
    pub include: Vec<String>,

    /// File patterns to exclude from dedupe analysis
    pub exclude: Vec<String>,

    /// Minimum number of function tokens to consider
    pub min_function_tokens: usize,

    /// Minimum number of AST nodes to consider
    pub min_ast_nodes: usize,

    /// Minimum number of matching tokens for a duplicate
    pub min_match_tokens: usize,

    /// Minimum coverage ratio for matches
    pub min_match_coverage: f64,

    /// Shingle size for k-shingles (8-10 for TF-IDF analysis)
    pub shingle_k: usize,

    /// Require distinct blocks for meaningful matches (≥2 basic blocks)
    pub require_distinct_blocks: usize,

    /// Feature weights for multi-dimensional similarity
    pub weights: DedupeWeights,

    /// I/O signature mismatch penalty
    pub io_mismatch_penalty: f64,

    /// Final similarity threshold
    pub threshold_s: f64,

    /// String patterns for boilerplate detection (used with tree-sitter AST analysis)
    pub stop_phrases: Vec<String>,

    /// Ranking criteria for duplicates
    pub rank_by: RankingCriteria,

    /// Minimum saved tokens to report
    pub min_saved_tokens: usize,

    /// Keep top N duplicates per file
    pub keep_top_per_file: usize,

    /// Adaptive denoising configuration
    #[serde(default)]
    pub adaptive: AdaptiveDenoiseConfig,
}

/// Clone denoising configuration for reducing noise in clone detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenoiseConfig {
    /// Enable clone denoising system (default: true)
    pub enabled: bool,

    /// Enable automatic threshold calibration and denoising (default: true)
    pub auto: bool,

    /// Core thresholds (user-configurable)
    /// Minimum number of function tokens to consider (40+ recommended)
    pub min_function_tokens: usize,

    /// Minimum number of matching tokens for a duplicate (24+ recommended)
    pub min_match_tokens: usize,

    /// Require minimum distinct blocks for meaningful matches (≥2 basic blocks)
    pub require_blocks: usize,

    /// Final similarity threshold for clone detection (0.0-1.0)
    pub similarity: f64,

    /// Advanced settings
    /// Feature weights for multi-dimensional similarity
    pub weights: DenoiseWeights,

    /// I/O signature mismatch penalty
    pub io_mismatch_penalty: f64,

    /// Final similarity threshold (alias for similarity)
    pub threshold_s: f64,

    /// Stop motifs configuration (AST-based boilerplate filtering)
    pub stop_motifs: StopMotifsConfig,

    /// Auto-calibration configuration
    pub auto_calibration: AutoCalibrationConfig,

    /// Payoff ranking configuration
    pub ranking: RankingConfig,

    /// Enable dry-run mode (analyze but don't change behavior)
    pub dry_run: bool,
}

/// Feature weights for denoising multi-dimensional similarity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenoiseWeights {
    /// AST similarity weight
    pub ast: f64,

    /// Program dependence graph weight  
    pub pdg: f64,

    /// Embedding similarity weight
    pub emb: f64,
}

impl Default for DenoiseWeights {
    fn default() -> Self {
        Self {
            ast: 0.35,
            pdg: 0.45,
            emb: 0.20,
        }
    }
}

/// Stop motifs configuration for AST-based boilerplate filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopMotifsConfig {
    /// Enable stop motifs filtering
    pub enabled: bool,

    /// Top percentile of patterns marked as boilerplate (0.0-1.0)
    pub percentile: f64,

    /// Cache refresh interval in days
    pub refresh_days: i64,
}

impl Default for StopMotifsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            percentile: 0.5, // Top 0.5% patterns marked as boilerplate
            refresh_days: 7,
        }
    }
}

/// Auto-calibration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCalibrationConfig {
    /// Enable auto-calibration
    pub enabled: bool,

    /// Quality target (percentage of candidates that must meet quality)
    pub quality_target: f64,

    /// Sample size for calibration (top N candidates)
    pub sample_size: usize,

    /// Maximum binary search iterations
    pub max_iterations: usize,
}

impl Default for AutoCalibrationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            quality_target: 0.8, // 80% of candidates must meet quality
            sample_size: 200,    // Top 200 candidates for calibration
            max_iterations: 50,  // Binary search limit
        }
    }
}

/// Payoff ranking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingConfig {
    /// Ranking criteria
    pub by: RankingBy,

    /// Minimum saved tokens to report
    pub min_saved_tokens: usize,

    /// Minimum rarity gain threshold
    pub min_rarity_gain: f64,

    /// Use live reachability data if available
    pub live_reach_boost: bool,
}

/// Ranking criteria options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankingBy {
    /// Rank by potential token savings
    SavedTokens,

    /// Rank by frequency/occurrence count
    Frequency,
}

impl Default for RankingConfig {
    fn default() -> Self {
        Self {
            by: RankingBy::SavedTokens,
            min_saved_tokens: 100,
            min_rarity_gain: 1.2,
            live_reach_boost: true,
        }
    }
}

impl Default for DenoiseConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Changed to opt-in for better default performance
            auto: true,    // Default auto-calibration enabled
            min_function_tokens: 60,  // Increased from 40 -> 60 to filter smaller functions
            min_match_tokens: 32,     // Increased from 24 -> 32 to reduce comparison workload
            require_blocks: 2,
            similarity: 0.80,         // Lowered from 0.82 -> 0.80 for faster threshold checks
            weights: DenoiseWeights::default(),
            io_mismatch_penalty: 0.25,
            threshold_s: 0.80,        // Updated to match similarity field
            stop_motifs: StopMotifsConfig::default(),
            auto_calibration: AutoCalibrationConfig::default(),
            ranking: RankingConfig::default(),
            dry_run: false,
        }
    }
}

impl DenoiseConfig {
    /// Validate denoise configuration
    pub fn validate(&self) -> Result<()> {
        if self.min_function_tokens == 0 {
            return Err(ValknutError::validation(
                "min_function_tokens must be greater than 0",
            ));
        }

        if self.min_match_tokens == 0 {
            return Err(ValknutError::validation(
                "min_match_tokens must be greater than 0",
            ));
        }

        if self.require_blocks == 0 {
            return Err(ValknutError::validation(
                "require_blocks must be greater than 0",
            ));
        }

        if !(0.0..=1.0).contains(&self.similarity) {
            return Err(ValknutError::validation(
                "similarity must be between 0.0 and 1.0",
            ));
        }

        if !(0.0..=1.0).contains(&self.threshold_s) {
            return Err(ValknutError::validation(
                "threshold_s must be between 0.0 and 1.0",
            ));
        }

        if !(0.0..=1.0).contains(&self.io_mismatch_penalty) {
            return Err(ValknutError::validation(
                "io_mismatch_penalty must be between 0.0 and 1.0",
            ));
        }

        // Validate weights sum to approximately 1.0
        let weight_sum = self.weights.ast + self.weights.pdg + self.weights.emb;
        if (weight_sum - 1.0).abs() > 0.1 {
            return Err(ValknutError::validation(
                "denoise weights should sum to approximately 1.0",
            ));
        }

        // Validate individual weights are non-negative
        if self.weights.ast < 0.0 || self.weights.pdg < 0.0 || self.weights.emb < 0.0 {
            return Err(ValknutError::validation(
                "denoise weights must be non-negative",
            ));
        }

        // Validate stop motifs config
        if !(0.0..=1.0).contains(&self.stop_motifs.percentile) {
            return Err(ValknutError::validation(
                "stop_motifs.percentile must be between 0.0 and 1.0",
            ));
        }

        if self.stop_motifs.refresh_days <= 0 {
            return Err(ValknutError::validation(
                "stop_motifs.refresh_days must be greater than 0",
            ));
        }

        // Validate auto-calibration config
        if !(0.0..=1.0).contains(&self.auto_calibration.quality_target) {
            return Err(ValknutError::validation(
                "auto_calibration.quality_target must be between 0.0 and 1.0",
            ));
        }

        if self.auto_calibration.sample_size == 0 {
            return Err(ValknutError::validation(
                "auto_calibration.sample_size must be greater than 0",
            ));
        }

        if self.auto_calibration.max_iterations == 0 {
            return Err(ValknutError::validation(
                "auto_calibration.max_iterations must be greater than 0",
            ));
        }

        // Validate ranking config
        if self.ranking.min_saved_tokens == 0 {
            return Err(ValknutError::validation(
                "ranking.min_saved_tokens must be greater than 0",
            ));
        }

        if self.ranking.min_rarity_gain <= 0.0 {
            return Err(ValknutError::validation(
                "ranking.min_rarity_gain must be greater than 0.0",
            ));
        }

        Ok(())
    }
}

/// Feature weights for multi-dimensional duplicate detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupeWeights {
    /// AST similarity weight
    pub ast: f64,

    /// Program dependence graph weight  
    pub pdg: f64,

    /// Embedding similarity weight
    pub emb: f64,
}

/// Ranking criteria for duplicates
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankingCriteria {
    /// Rank by potential token savings
    SavedTokens,

    /// Rank by similarity score
    Similarity,

    /// Rank by both similarity and savings
    Combined,
}

/// Adaptive denoising configuration for intelligent clone detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveDenoiseConfig {
    /// Enable automatic denoising with threshold tuning
    pub auto_denoise: bool,

    /// Enable adaptive learning of boilerplate patterns
    pub adaptive_learning: bool,

    /// Enable TF-IDF rarity weighting for structural analysis
    pub rarity_weighting: bool,

    /// Enable structural validation (PDG motifs, basic blocks)
    pub structural_validation: bool,

    /// Enable live reachability boost integration
    pub live_reach_integration: bool,

    /// Stop motif percentile threshold (0.0-1.0, e.g., 0.75 = top 0.75%)
    pub stop_motif_percentile: f64,

    /// Hub suppression threshold (0.0-1.0, patterns in >60% of files)
    pub hub_suppression_threshold: f64,

    /// Quality gate percentage (0.0-1.0, 80% of candidates must meet quality)
    pub quality_gate_percentage: f64,

    /// TF-IDF k-gram size for structural analysis
    pub tfidf_kgram_size: usize,

    /// Weisfeiler-Lehman hash iterations for PDG motifs
    pub wl_iterations: usize,

    /// Minimum rarity gain threshold
    pub min_rarity_gain: f64,

    /// External call Jaccard similarity penalty threshold
    pub external_call_jaccard_threshold: f64,

    /// Cache refresh interval in days
    pub cache_refresh_days: i64,

    /// Enable automatic cache refresh
    pub auto_refresh_cache: bool,
}

impl Default for AdaptiveDenoiseConfig {
    fn default() -> Self {
        Self {
            auto_denoise: true,
            adaptive_learning: true,
            rarity_weighting: true,
            structural_validation: true,
            live_reach_integration: true,
            stop_motif_percentile: 0.75,
            hub_suppression_threshold: 0.6,
            quality_gate_percentage: 0.8,
            tfidf_kgram_size: 8,
            wl_iterations: 3,
            min_rarity_gain: 1.2,
            external_call_jaccard_threshold: 0.2,
            cache_refresh_days: 7,
            auto_refresh_cache: true,
        }
    }
}

impl Default for DedupeConfig {
    fn default() -> Self {
        Self {
            include: vec!["src/**".to_string()],
            exclude: vec![
                "benchmarks/**".to_string(),
                "examples/**".to_string(),
                "datasets/**".to_string(),
                "**/generated/**".to_string(),
                "**/*.pb.rs".to_string(),
            ],
            min_function_tokens: 40,
            min_ast_nodes: 35,
            min_match_tokens: 24,
            min_match_coverage: 0.40,
            shingle_k: 9,
            require_distinct_blocks: 2,
            weights: DedupeWeights::default(),
            io_mismatch_penalty: 0.25,
            threshold_s: 0.82,
            stop_phrases: vec![
                r"^\s*@staticmethod\b".to_string(),
                r"group\.bench_with_input\s*\(".to_string(),
                r"\bb\.iter\s*\(\|\|".to_string(),
                r"\bgroup\.finish\s*\(\)\s*;?".to_string(),
                r"\blet\s+config\s*=\s*AnalysisConfig::(new|default)\s*\(\)\s*;?".to_string(),
                r"\bchecks\.push\s*\(\s*HealthCheck\s*\{".to_string(),
            ],
            rank_by: RankingCriteria::SavedTokens,
            min_saved_tokens: 100,
            keep_top_per_file: 3,
            adaptive: AdaptiveDenoiseConfig::default(),
        }
    }
}

impl Default for DedupeWeights {
    fn default() -> Self {
        Self {
            ast: 0.35,
            pdg: 0.45,
            emb: 0.20,
        }
    }
}

impl DedupeConfig {
    /// Validate dedupe configuration
    pub fn validate(&self) -> Result<()> {
        if self.min_function_tokens == 0 {
            return Err(ValknutError::validation(
                "min_function_tokens must be greater than 0",
            ));
        }

        if self.min_ast_nodes == 0 {
            return Err(ValknutError::validation(
                "min_ast_nodes must be greater than 0",
            ));
        }

        if self.min_match_tokens == 0 {
            return Err(ValknutError::validation(
                "min_match_tokens must be greater than 0",
            ));
        }

        if !(0.0..=1.0).contains(&self.min_match_coverage) {
            return Err(ValknutError::validation(
                "min_match_coverage must be between 0.0 and 1.0",
            ));
        }

        if self.shingle_k == 0 {
            return Err(ValknutError::validation("shingle_k must be greater than 0"));
        }

        if !(0.0..=1.0).contains(&self.io_mismatch_penalty) {
            return Err(ValknutError::validation(
                "io_mismatch_penalty must be between 0.0 and 1.0",
            ));
        }

        if !(0.0..=1.0).contains(&self.threshold_s) {
            return Err(ValknutError::validation(
                "threshold_s must be between 0.0 and 1.0",
            ));
        }

        // Validate weights sum to reasonable values
        let weight_sum = self.weights.ast + self.weights.pdg + self.weights.emb;
        if (weight_sum - 1.0).abs() > 0.1 {
            return Err(ValknutError::validation(
                "weights should sum to approximately 1.0",
            ));
        }

        // Validate patterns (simplified - no regex validation)
        for pattern in &self.stop_phrases {
            if pattern.is_empty() {
                return Err(ValknutError::validation(
                    "Empty pattern in stop_phrases".to_string(),
                ));
            }
        }

        // Validate adaptive denoising configuration
        if !(0.0..=1.0).contains(&self.adaptive.stop_motif_percentile) {
            return Err(ValknutError::validation(
                "adaptive.stop_motif_percentile must be between 0.0 and 1.0",
            ));
        }

        if !(0.0..=1.0).contains(&self.adaptive.hub_suppression_threshold) {
            return Err(ValknutError::validation(
                "adaptive.hub_suppression_threshold must be between 0.0 and 1.0",
            ));
        }

        if !(0.0..=1.0).contains(&self.adaptive.quality_gate_percentage) {
            return Err(ValknutError::validation(
                "adaptive.quality_gate_percentage must be between 0.0 and 1.0",
            ));
        }

        if self.adaptive.tfidf_kgram_size == 0 || self.adaptive.tfidf_kgram_size > 20 {
            return Err(ValknutError::validation(
                "adaptive.tfidf_kgram_size must be between 1 and 20",
            ));
        }

        if self.adaptive.wl_iterations == 0 || self.adaptive.wl_iterations > 10 {
            return Err(ValknutError::validation(
                "adaptive.wl_iterations must be between 1 and 10",
            ));
        }

        if self.adaptive.min_rarity_gain <= 0.0 {
            return Err(ValknutError::validation(
                "adaptive.min_rarity_gain must be greater than 0.0",
            ));
        }

        if !(0.0..=1.0).contains(&self.adaptive.external_call_jaccard_threshold) {
            return Err(ValknutError::validation(
                "adaptive.external_call_jaccard_threshold must be between 0.0 and 1.0",
            ));
        }

        if self.adaptive.cache_refresh_days <= 0 {
            return Err(ValknutError::validation(
                "adaptive.cache_refresh_days must be greater than 0",
            ));
        }

        Ok(())
    }
}
