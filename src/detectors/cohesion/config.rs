//! Configuration for semantic cohesion analysis.

use serde::{Deserialize, Serialize};

/// Main configuration for cohesion analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohesionConfig {
    /// Enable cohesion analysis
    pub enabled: bool,
    /// Embedding model configuration
    pub embedding: EmbeddingConfig,
    /// Symbol extraction configuration
    pub symbols: SymbolConfig,
    /// Threshold configuration
    pub thresholds: CohesionThresholds,
    /// Rollup configuration for hierarchical aggregation
    pub rollup: RollupConfig,
    /// Issue reporting configuration
    pub issues: IssueConfig,
}

impl Default for CohesionConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Opt-in for now
            embedding: EmbeddingConfig::default(),
            symbols: SymbolConfig::default(),
            thresholds: CohesionThresholds::default(),
            rollup: RollupConfig::default(),
            issues: IssueConfig::default(),
        }
    }
}

/// Configuration for embedding generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Embedding model to use
    pub model: EmbeddingModel,
    /// Cache directory for model files (relative to project root or absolute)
    pub cache_dir: Option<String>,
    /// Whether to show download progress for model files
    pub show_download_progress: bool,
    /// Maximum batch size for embedding generation
    pub batch_size: usize,
    /// Embedding dimension (model-specific, used for validation)
    pub dimension: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: EmbeddingModel::EmbeddingGemma300M,
            cache_dir: None, // Uses fastembed default
            show_download_progress: false,
            batch_size: 32,
            dimension: 768, // EmbeddingGemma default
        }
    }
}

/// Available embedding models (subset of fastembed models)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingModel {
    /// Google EmbeddingGemma 300M - 768 dim, good quality/speed balance (default)
    EmbeddingGemma300M,
    /// BGE Small EN v1.5 - 384 dim, very fast
    BGESmallENV15,
    /// BGE Small EN v1.5 Quantized - 384 dim, fastest
    BGESmallENV15Q,
    /// All-MiniLM-L6-v2 - 384 dim, fast and lightweight
    AllMiniLML6V2,
    /// All-MiniLM-L6-v2 Quantized - 384 dim, very fast
    AllMiniLML6V2Q,
    /// Nomic Embed Text v1.5 - 768 dim, good for code
    NomicEmbedTextV15,
    /// Jina Embeddings v2 Base Code - 768 dim, optimized for code
    JinaEmbeddingsV2BaseCode,
}

impl EmbeddingModel {
    /// Get the embedding dimension for this model
    pub fn dimension(&self) -> usize {
        match self {
            EmbeddingModel::EmbeddingGemma300M => 768,
            EmbeddingModel::BGESmallENV15 | EmbeddingModel::BGESmallENV15Q => 384,
            EmbeddingModel::AllMiniLML6V2 | EmbeddingModel::AllMiniLML6V2Q => 384,
            EmbeddingModel::NomicEmbedTextV15 => 768,
            EmbeddingModel::JinaEmbeddingsV2BaseCode => 768,
        }
    }

    /// Get a human-readable name for this model
    pub fn display_name(&self) -> &'static str {
        match self {
            EmbeddingModel::EmbeddingGemma300M => "EmbeddingGemma-300M",
            EmbeddingModel::BGESmallENV15 => "BGE-small-en-v1.5",
            EmbeddingModel::BGESmallENV15Q => "BGE-small-en-v1.5 (quantized)",
            EmbeddingModel::AllMiniLML6V2 => "all-MiniLM-L6-v2",
            EmbeddingModel::AllMiniLML6V2Q => "all-MiniLM-L6-v2 (quantized)",
            EmbeddingModel::NomicEmbedTextV15 => "nomic-embed-text-v1.5",
            EmbeddingModel::JinaEmbeddingsV2BaseCode => "jina-embeddings-v2-base-code",
        }
    }
}

/// Configuration for symbol extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolConfig {
    /// Minimum cumulative TF-IDF mass to select (0.0 to 1.0)
    /// Default 0.80 means select symbols until 80% of total weight is captured
    pub tfidf_mass_threshold: f64,
    /// Minimum number of symbols to select per entity
    pub min_symbols: usize,
    /// Maximum number of symbols to select per entity
    pub max_symbols: usize,
    /// Coefficient for sublinear cap: K_cap = ceil(a * sqrt(m))
    pub sublinear_coefficient: f64,
    /// Include signature tokens (parameter names, return types)
    pub include_signature: bool,
    /// Include short doc summary in code text
    pub include_doc_summary: bool,
    /// Maximum tokens from doc summary to include
    pub max_doc_summary_tokens: usize,
}

impl Default for SymbolConfig {
    fn default() -> Self {
        Self {
            tfidf_mass_threshold: 0.80,
            min_symbols: 5,
            max_symbols: 40,
            sublinear_coefficient: 3.0,
            include_signature: true,
            include_doc_summary: false, // Start without for speed
            max_doc_summary_tokens: 20,
        }
    }
}

/// Threshold configuration for cohesion analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohesionThresholds {
    /// Minimum cohesion score before flagging (0.0 to 1.0)
    /// Default uses percentile-based thresholding per bucket
    pub min_cohesion: f64,
    /// Minimum doc-code alignment before flagging as DOC_MISMATCH
    pub min_doc_alignment: f64,
    /// Minimum doc specificity before flagging as DOC_GENERIC
    pub min_doc_specificity: f64,
    /// Minimum tokens in doc before flagging as DOC_TOO_SHORT
    pub min_doc_tokens: usize,
    /// Minimum similarity to centroid before flagging as outlier
    pub min_outlier_similarity: f64,
    /// Percentile threshold for outlier detection (bottom X% are outliers)
    pub outlier_percentile: f64,
    /// Whether to use percentile-based thresholds (per language/level bucket)
    pub use_percentile_thresholds: bool,
}

impl Default for CohesionThresholds {
    fn default() -> Self {
        Self {
            min_cohesion: 0.3,           // Below 0.3 is concerning
            min_doc_alignment: 0.4,      // Below 0.4 suggests mismatch
            min_doc_specificity: 0.3,    // Below 0.3 is too generic
            min_doc_tokens: 6,           // Minimum meaningful doc length
            min_outlier_similarity: 0.2, // Very low similarity = outlier
            outlier_percentile: 0.10,    // Bottom 10% within container
            use_percentile_thresholds: true,
        }
    }
}

/// Configuration for hierarchical rollup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollupConfig {
    /// Minimum entities in a file to compute cohesion
    pub min_file_entities: usize,
    /// Minimum files in a folder to compute cohesion
    pub min_folder_files: usize,
    /// Weight function for file rollup: "linear", "log", "sqrt"
    pub file_weight_function: WeightFunction,
    /// Trim percentage for robust centroid (bottom X% removed)
    pub centroid_trim_percent: f64,
    /// Whether to use MAD-based trimming instead of fixed percentile
    pub use_mad_trimming: bool,
    /// MAD multiplier for outlier detection (e.g., 1.5)
    pub mad_multiplier: f64,
}

impl Default for RollupConfig {
    fn default() -> Self {
        Self {
            min_file_entities: 5,
            min_folder_files: 2,
            file_weight_function: WeightFunction::Log,
            centroid_trim_percent: 0.15, // Trim bottom 15%
            use_mad_trimming: false,     // Start with simple trimming
            mad_multiplier: 1.5,
        }
    }
}

/// Weight function for rollup aggregation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeightFunction {
    /// Linear weighting: w = n
    Linear,
    /// Logarithmic weighting: w = log(1 + n)
    Log,
    /// Square root weighting: w = sqrt(n)
    Sqrt,
}

impl WeightFunction {
    /// Calculate weight for a count
    pub fn weight(&self, n: usize) -> f64 {
        let n = n as f64;
        match self {
            WeightFunction::Linear => n,
            WeightFunction::Log => (1.0 + n).ln(),
            WeightFunction::Sqrt => n.sqrt(),
        }
    }
}

/// Configuration for issue reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueConfig {
    /// Maximum issues to report per category
    pub max_issues_per_category: usize,
    /// Skip generated/vendor/build/test paths
    pub skip_generated_paths: bool,
    /// Patterns to exclude from analysis
    pub exclude_patterns: Vec<String>,
    /// Include patterns (if non-empty, only these are analyzed)
    pub include_patterns: Vec<String>,
}

impl Default for IssueConfig {
    fn default() -> Self {
        Self {
            max_issues_per_category: 50,
            skip_generated_paths: true,
            exclude_patterns: vec![
                "**/node_modules/**".to_string(),
                "**/vendor/**".to_string(),
                "**/build/**".to_string(),
                "**/dist/**".to_string(),
                "**/target/**".to_string(),
                "**/.git/**".to_string(),
                "**/generated/**".to_string(),
                "**/*.generated.*".to_string(),
                "**/*.min.js".to_string(),
                "**/*.bundle.js".to_string(),
            ],
            include_patterns: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled() {
        let config = CohesionConfig::default();
        assert!(!config.enabled);
    }

    #[test]
    fn embedding_model_dimensions() {
        assert_eq!(EmbeddingModel::EmbeddingGemma300M.dimension(), 768);
        assert_eq!(EmbeddingModel::BGESmallENV15.dimension(), 384);
        assert_eq!(EmbeddingModel::AllMiniLML6V2.dimension(), 384);
    }

    #[test]
    fn weight_function_calculations() {
        assert_eq!(WeightFunction::Linear.weight(10), 10.0);
        assert!((WeightFunction::Log.weight(10) - 2.398).abs() < 0.01);
        assert!((WeightFunction::Sqrt.weight(100) - 10.0).abs() < 0.001);
    }

    #[test]
    fn symbol_config_defaults_are_sane() {
        let config = SymbolConfig::default();
        assert!(config.min_symbols < config.max_symbols);
        assert!(config.tfidf_mass_threshold > 0.0 && config.tfidf_mass_threshold <= 1.0);
    }
}
