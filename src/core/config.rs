//! Configuration types and management for valknut-rs.
//!
//! This module provides comprehensive configuration structures that mirror
//! the Python implementation while adding Rust-specific optimizations and
//! type safety guarantees.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
    
    /// Language-specific settings
    pub languages: HashMap<String, LanguageConfig>,
    
    /// I/O and persistence settings
    pub io: IoConfig,
    
    /// Performance and resource limits
    pub performance: PerformanceConfig,
    
    /// Structure analysis configuration
    pub structure: StructureConfig,
    
    /// Code quality analysis configuration (simple pattern-based analysis)
    // pub names: NamesConfig,
    /// Placeholder to maintain serialization compatibility
    #[serde(skip)]
    pub _names_placeholder: Option<()>,
}

impl Default for ValknutConfig {
    fn default() -> Self {
        Self {
            analysis: AnalysisConfig::default(),
            scoring: ScoringConfig::default(),
            graph: GraphConfig::default(),
            lsh: LshConfig::default(),
            languages: Self::default_languages(),
            io: IoConfig::default(),
            performance: PerformanceConfig::default(),
            structure: StructureConfig::default(),
            // names: NamesConfig::default(),
            _names_placeholder: None,
        }
    }
}

impl ValknutConfig {
    /// Load configuration from a YAML file
    pub fn from_yaml_file(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let content = std::fs::read_to_string(&path)
            .map_err(|e| ValknutError::io(format!("Failed to read config file: {}", path.display()), e))?;
        
        serde_yaml::from_str(&content).map_err(Into::into)
    }
    
    /// Save configuration to a YAML file
    pub fn to_yaml_file(&self, path: impl Into<PathBuf>) -> Result<()> {
        let path = path.into();
        let content = serde_yaml::to_string(self)?;
        std::fs::write(&path, content)
            .map_err(|e| ValknutError::io(format!("Failed to write config file: {}", path.display()), e))
    }
    
    /// Get default language configurations
    fn default_languages() -> HashMap<String, LanguageConfig> {
        let mut languages = HashMap::new();
        
        languages.insert("python".to_string(), LanguageConfig {
            enabled: true,
            file_extensions: vec![".py".to_string(), ".pyi".to_string()],
            tree_sitter_language: "python".to_string(),
            max_file_size_mb: 10.0,
            complexity_threshold: 10.0,
            additional_settings: HashMap::new(),
        });
        
        languages.insert("javascript".to_string(), LanguageConfig {
            enabled: true,
            file_extensions: vec![".js".to_string(), ".mjs".to_string(), ".jsx".to_string()],
            tree_sitter_language: "javascript".to_string(),
            max_file_size_mb: 5.0,
            complexity_threshold: 10.0,
            additional_settings: HashMap::new(),
        });
        
        languages.insert("typescript".to_string(), LanguageConfig {
            enabled: true,
            file_extensions: vec![".ts".to_string(), ".tsx".to_string(), ".d.ts".to_string()],
            tree_sitter_language: "typescript".to_string(),
            max_file_size_mb: 5.0,
            complexity_threshold: 10.0,
            additional_settings: HashMap::new(),
        });
        
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
            config.validate().map_err(|e| ValknutError::config_field(
                format!("Invalid language configuration: {e}"), 
                format!("languages.{lang}")
            ))?;
        }
        
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
        Self {
            enable_scoring: true,
            enable_graph_analysis: true,
            enable_lsh_analysis: true,
            enable_refactoring_analysis: true,
            enable_coverage_analysis: false,
            enable_structure_analysis: true,
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
            return Err(ValknutError::validation(
                format!("confidence_threshold must be between 0.0 and 1.0, got {}", self.confidence_threshold)
            ));
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
        let weights = [self.complexity, self.graph, self.structure, self.style, self.coverage];
        
        for (name, &weight) in ["complexity", "graph", "structure", "style", "coverage"].iter().zip(&weights) {
            if weight < 0.0 || weight > 10.0 {
                return Err(ValknutError::validation(
                    format!("Weight for '{}' must be between 0.0 and 10.0, got {}", name, weight)
                ));
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
            return Err(ValknutError::validation(
                format!("confidence_level must be between 0.0 and 1.0, got {}", self.confidence_level)
            ));
        }
        
        if self.min_sample_size == 0 {
            return Err(ValknutError::validation("min_sample_size must be greater than 0"));
        }
        
        if self.outlier_threshold <= 0.0 {
            return Err(ValknutError::validation("outlier_threshold must be positive"));
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
            return Err(ValknutError::validation(
                format!("approximation_sample_rate must be between 0.0 and 1.0, got {}", self.approximation_sample_rate)
            ));
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
            num_bands: 16,
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 100,
            use_semantic_similarity: false,  // Keep name for backward compatibility
        }
    }
}

impl LshConfig {
    /// Validate LSH configuration
    pub fn validate(&self) -> Result<()> {
        if self.num_hashes == 0 {
            return Err(ValknutError::validation("num_hashes must be greater than 0"));
        }
        
        if self.num_bands == 0 {
            return Err(ValknutError::validation("num_bands must be greater than 0"));
        }
        
        if self.num_hashes % self.num_bands != 0 {
            return Err(ValknutError::validation("num_hashes must be divisible by num_bands"));
        }
        
        if !(0.0..=1.0).contains(&self.similarity_threshold) {
            return Err(ValknutError::validation(
                format!("similarity_threshold must be between 0.0 and 1.0, got {}", self.similarity_threshold)
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
    pub additional_settings: HashMap<String, serde_json::Value>,
}

impl LanguageConfig {
    /// Validate language configuration
    pub fn validate(&self) -> Result<()> {
        if self.file_extensions.is_empty() {
            return Err(ValknutError::validation("file_extensions cannot be empty"));
        }
        
        if self.max_file_size_mb <= 0.0 {
            return Err(ValknutError::validation("max_file_size_mb must be positive"));
        }
        
        if self.complexity_threshold <= 0.0 {
            return Err(ValknutError::validation("complexity_threshold must be positive"));
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
            max_threads: None, // Use system default
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
                return Err(ValknutError::validation("max_threads must be greater than 0"));
            }
        }
        
        if let Some(memory) = self.memory_limit_mb {
            if memory == 0 {
                return Err(ValknutError::validation("memory_limit_mb must be greater than 0"));
            }
        }
        
        if self.batch_size == 0 {
            return Err(ValknutError::validation("batch_size must be greater than 0"));
        }
        
        Ok(())
    }
}