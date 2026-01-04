//! Configuration types for LSH-based clone detection and denoising.
//!
//! This module provides configuration structures for the locality-sensitive
//! hashing (LSH) clone detection system, including parameters for shingle-based
//! fingerprinting, similarity thresholds, and advanced denoising options.

use serde::{Deserialize, Serialize};

use crate::core::config::validate_unit_range;
use crate::core::errors::{Result, ValknutError};

// Re-export shared types from core config
pub use crate::core::config::{DedupeWeights, DenoiseWeights, SimilarityWeights};

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

/// Default implementation for [`LshConfig`].
impl Default for LshConfig {
    /// Returns the default LSH configuration.
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

/// Conversion from core config [`LshConfig`](crate::core::config::LshConfig).
impl From<crate::core::config::LshConfig> for LshConfig {
    /// Converts from the core config LSH settings.
    fn from(value: crate::core::config::LshConfig) -> Self {
        Self {
            num_hashes: value.num_hashes,
            num_bands: value.num_bands,
            shingle_size: value.shingle_size,
            similarity_threshold: value.similarity_threshold,
            max_candidates: value.max_candidates,
            use_semantic_similarity: value.use_semantic_similarity,
        }
    }
}

/// Validation and utility methods for [`LshConfig`].
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

        validate_unit_range(self.similarity_threshold, "similarity_threshold")?;

        Ok(())
    }

    /// Returns the number of hashes per band (rows per band in LSH parlance).
    ///
    /// Higher values reduce false positives but may miss some similar pairs.
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

/// Default implementation for [`StopMotifsConfig`].
impl Default for StopMotifsConfig {
    /// Returns the default stop motifs configuration.
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

/// Default implementation for [`AutoCalibrationConfig`].
impl Default for AutoCalibrationConfig {
    /// Returns the default auto-calibration configuration.
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

/// Default implementation for [`RankingConfig`].
impl Default for RankingConfig {
    /// Returns the default ranking configuration.
    fn default() -> Self {
        Self {
            by: RankingBy::SavedTokens,
            min_saved_tokens: 100,
            min_rarity_gain: 1.2,
        }
    }
}

/// Default implementation for [`DenoiseConfig`].
impl Default for DenoiseConfig {
    /// Returns the default clone denoising configuration.
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

/// Validation for [`DenoiseConfig`].
impl DenoiseConfig {
    /// Validate denoise configuration
    pub fn validate(&self) -> Result<()> {
        self.validate_basic_params()?;
        self.validate_thresholds()?;
        validate_denoise_weights(&self.weights)?;
        self.stop_motifs.validate()?;
        self.auto_calibration.validate()?;
        Ok(())
    }

    fn validate_basic_params(&self) -> Result<()> {
        validate_positive(self.min_function_tokens, "min_function_tokens")?;
        validate_positive(self.min_match_tokens, "min_match_tokens")?;
        validate_positive(self.require_blocks, "require_blocks")?;
        Ok(())
    }

    fn validate_thresholds(&self) -> Result<()> {
        validate_unit_range(self.similarity, "similarity")?;
        validate_unit_range(self.threshold_s, "threshold_s")?;
        validate_unit_range(self.io_mismatch_penalty, "io_mismatch_penalty")?;
        Ok(())
    }
}

/// Validate that denoise weights sum to approximately 1.0 and are non-negative.
fn validate_denoise_weights(weights: &DenoiseWeights) -> Result<()> {
    let weight_sum = weights.ast + weights.pdg + weights.emb;
    if (weight_sum - 1.0).abs() > 0.1 {
        return Err(ValknutError::validation(
            "denoise weights should sum to approximately 1.0",
        ));
    }
    if weights.ast < 0.0 || weights.pdg < 0.0 || weights.emb < 0.0 {
        return Err(ValknutError::validation(
            "denoise weights must be non-negative",
        ));
    }
    Ok(())
}

/// Validation for [`StopMotifsConfig`].
impl StopMotifsConfig {
    fn validate(&self) -> Result<()> {
        validate_unit_range(self.percentile, "stop_motifs.percentile")?;
        if self.refresh_days <= 0 {
            return Err(ValknutError::validation(
                "stop_motifs.refresh_days must be greater than 0",
            ));
        }
        Ok(())
    }
}

/// Validation for [`AutoCalibrationConfig`].
impl AutoCalibrationConfig {
    fn validate(&self) -> Result<()> {
        validate_unit_range(self.quality_target, "auto_calibration.quality_target")?;
        validate_positive(self.sample_size, "auto_calibration.sample_size")?;
        validate_positive(self.max_iterations, "auto_calibration.max_iterations")?;
        Ok(())
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

/// Default implementation for [`AdaptiveDenoiseConfig`].
impl Default for AdaptiveDenoiseConfig {
    /// Returns the default adaptive denoising configuration.
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

/// Default implementation for [`DedupeConfig`].
impl Default for DedupeConfig {
    /// Returns the default dedupe configuration.
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
            min_ast_nodes: 20,
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

/// Validation for [`DedupeConfig`].
impl DedupeConfig {
    /// Validate dedupe configuration
    pub fn validate(&self) -> Result<()> {
        self.validate_basic_params()?;
        self.validate_thresholds()?;
        validate_dedupe_weights(&self.weights)?;
        self.validate_stop_phrases()?;
        self.adaptive.validate()?;
        Ok(())
    }

    fn validate_basic_params(&self) -> Result<()> {
        validate_positive(self.min_function_tokens, "min_function_tokens")?;
        validate_positive(self.min_ast_nodes, "min_ast_nodes")?;
        validate_positive(self.min_match_tokens, "min_match_tokens")?;
        validate_positive(self.shingle_k, "shingle_k")?;
        Ok(())
    }

    fn validate_thresholds(&self) -> Result<()> {
        validate_unit_range(self.min_match_coverage, "min_match_coverage")?;
        validate_unit_range(self.io_mismatch_penalty, "io_mismatch_penalty")?;
        validate_unit_range(self.threshold_s, "threshold_s")?;
        Ok(())
    }

    fn validate_stop_phrases(&self) -> Result<()> {
        for pattern in &self.stop_phrases {
            if pattern.is_empty() {
                return Err(ValknutError::validation("Empty pattern in stop_phrases"));
            }
        }
        Ok(())
    }
}

/// Validate that dedupe weights sum to approximately 1.0.
fn validate_dedupe_weights(weights: &DedupeWeights) -> Result<()> {
    let weight_sum = weights.ast + weights.pdg + weights.emb;
    if (weight_sum - 1.0).abs() > 0.1 {
        return Err(ValknutError::validation(
            "weights should sum to approximately 1.0",
        ));
    }
    Ok(())
}

/// Validate that a value is positive (greater than 0).
fn validate_positive(value: usize, name: &str) -> Result<()> {
    if value == 0 {
        return Err(ValknutError::validation(format!(
            "{} must be greater than 0",
            name
        )));
    }
    Ok(())
}

/// Validation for [`AdaptiveDenoiseConfig`].
impl AdaptiveDenoiseConfig {
    fn validate(&self) -> Result<()> {
        validate_unit_range(self.stop_motif_percentile, "adaptive.stop_motif_percentile")?;
        validate_unit_range(self.hub_suppression_threshold, "adaptive.hub_suppression_threshold")?;
        validate_unit_range(self.quality_gate_percentage, "adaptive.quality_gate_percentage")?;
        validate_unit_range(self.external_call_jaccard_threshold, "adaptive.external_call_jaccard_threshold")?;
        self.validate_bounded_params()?;
        Ok(())
    }

    fn validate_bounded_params(&self) -> Result<()> {
        if self.tfidf_kgram_size == 0 || self.tfidf_kgram_size > 20 {
            return Err(ValknutError::validation(
                "adaptive.tfidf_kgram_size must be between 1 and 20",
            ));
        }
        if self.wl_iterations == 0 || self.wl_iterations > 10 {
            return Err(ValknutError::validation(
                "adaptive.wl_iterations must be between 1 and 10",
            ));
        }
        if self.min_rarity_gain <= 0.0 {
            return Err(ValknutError::validation(
                "adaptive.min_rarity_gain must be greater than 0.0",
            ));
        }
        if self.cache_refresh_days <= 0 {
            return Err(ValknutError::validation(
                "adaptive.cache_refresh_days must be greater than 0",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
