use serde::{Deserialize, Serialize};

use crate::core::errors::{Result, ValknutError};

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
            num_bands: 8, // Reduced from 16 -> 8 for faster candidate filtering (16 rows per band)
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 100,
            use_semantic_similarity: false,
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
    pub min_function_tokens: usize,

    /// Minimum number of matching tokens for a duplicate (24+ recommended)
    pub min_match_tokens: usize,

    /// Require minimum distinct blocks for meaningful matches (≥2 basic blocks)
    pub require_blocks: usize,

    /// Final similarity threshold for clone detection (0.0-1.0)
    pub similarity: f64,

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
            percentile: 0.5,
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
            quality_target: 0.8,
            sample_size: 200,
            max_iterations: 50,
        }
    }
}

/// Payoff ranking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingConfig {
    /// Ranking criteria
    pub by: RankingBy,

    /// Minimum saved tokens threshold
    pub min_saved_tokens: usize,

    /// Minimum rarity gain threshold
    pub min_rarity_gain: f64,
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
        }
    }
}

impl Default for DenoiseConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Changed to opt-in for better default performance
            auto: true,
            min_function_tokens: 60, // Increased from 40 -> 60 to filter smaller functions
            min_match_tokens: 32,    // Increased from 24 -> 32 to reduce comparison workload
            require_blocks: 2,
            similarity: 0.80, // Lowered from 0.82 -> 0.80 for faster threshold checks
            weights: DenoiseWeights::default(),
            io_mismatch_penalty: 0.25,
            threshold_s: 0.80, // Updated to match similarity field
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

        let weight_sum = self.weights.ast + self.weights.pdg + self.weights.emb;
        if (weight_sum - 1.0).abs() > 0.1 {
            return Err(ValknutError::validation(
                "denoise weights should sum to approximately 1.0",
            ));
        }

        if self.weights.ast < 0.0 || self.weights.pdg < 0.0 || self.weights.emb < 0.0 {
            return Err(ValknutError::validation(
                "denoise weights must be non-negative",
            ));
        }

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

        Ok(())
    }
}

/// Feature weights for dedupe multi-dimensional similarity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupeWeights {
    /// AST similarity weight
    pub ast: f64,

    /// Program dependence graph weight
    pub pdg: f64,

    /// Embedding similarity weight
    pub emb: f64,
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

        let weight_sum = self.weights.ast + self.weights.pdg + self.weights.emb;
        if (weight_sum - 1.0).abs() > 0.1 {
            return Err(ValknutError::validation(
                "weights should sum to approximately 1.0",
            ));
        }

        for pattern in &self.stop_phrases {
            if pattern.is_empty() {
                return Err(ValknutError::validation(
                    "Empty pattern in stop_phrases".to_string(),
                ));
            }
        }

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
