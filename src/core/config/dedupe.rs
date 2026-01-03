//! Duplicate detection and denoising configuration types.

use serde::{Deserialize, Serialize};

use crate::core::errors::{Result, ValknutError};

use super::validation::{
    validate_bounded_usize, validate_non_negative, validate_positive_f64, validate_positive_i64,
    validate_positive_usize, validate_unit_range, validate_weights_sum,
};

/// Enhanced duplicate detection configuration with adaptive features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupeConfig {
    /// File patterns to include in dedupe analysis
    #[serde(default)]
    pub include: Vec<String>,

    /// File patterns to exclude from dedupe analysis
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Minimum number of function tokens to consider
    #[serde(default)]
    pub min_function_tokens: usize,

    /// Minimum number of AST nodes to consider
    #[serde(default)]
    pub min_ast_nodes: usize,

    /// Minimum number of matching tokens for a duplicate
    #[serde(default)]
    pub min_match_tokens: usize,

    /// Minimum coverage ratio for matches
    #[serde(default)]
    pub min_match_coverage: f64,

    /// Shingle size for k-shingles (8-10 for TF-IDF analysis)
    #[serde(default)]
    pub shingle_k: usize,

    /// Require distinct blocks for meaningful matches (≥2 basic blocks)
    #[serde(default)]
    pub require_distinct_blocks: usize,

    /// Feature weights for multi-dimensional similarity
    #[serde(default)]
    pub weights: DedupeWeights,

    /// I/O signature mismatch penalty
    #[serde(default)]
    pub io_mismatch_penalty: f64,

    /// Final similarity threshold
    #[serde(default)]
    pub threshold_s: f64,

    /// String patterns for boilerplate detection (used with tree-sitter AST analysis)
    #[serde(default)]
    pub stop_phrases: Vec<String>,

    /// Ranking criteria for duplicates
    #[serde(default)]
    pub rank_by: RankingCriteria,

    /// Minimum saved tokens to report
    #[serde(default)]
    pub min_saved_tokens: usize,

    /// Keep top N duplicates per file
    #[serde(default)]
    pub keep_top_per_file: usize,

    /// Adaptive denoising configuration
    #[serde(default)]
    pub adaptive: AdaptiveDenoiseConfig,
}

/// Clone denoising configuration for reducing noise in clone detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenoiseConfig {
    /// Enable clone denoising system (default: true)
    #[serde(default)]
    pub enabled: bool,

    /// Enable automatic threshold calibration and denoising (default: true)
    #[serde(default)]
    pub auto: bool,

    /// Core thresholds (user-configurable)
    /// Minimum number of function tokens to consider (40+ recommended)
    #[serde(default)]
    pub min_function_tokens: usize,

    /// Minimum number of matching tokens for a duplicate (24+ recommended)
    #[serde(default)]
    pub min_match_tokens: usize,

    /// Require minimum distinct blocks for meaningful matches (≥2 basic blocks)
    #[serde(default)]
    pub require_blocks: usize,

    /// Final similarity threshold for clone detection (0.0-1.0)
    #[serde(default)]
    pub similarity: f64,

    /// Advanced settings
    /// Feature weights for multi-dimensional similarity
    #[serde(default)]
    pub weights: DenoiseWeights,

    /// I/O signature mismatch penalty
    #[serde(default)]
    pub io_mismatch_penalty: f64,

    /// Final similarity threshold (alias for similarity)
    #[serde(default)]
    pub threshold_s: f64,

    /// Stop motifs configuration (AST-based boilerplate filtering)
    #[serde(default)]
    pub stop_motifs: StopMotifsConfig,

    /// Auto-calibration configuration
    #[serde(default)]
    pub auto_calibration: AutoCalibrationConfig,

    /// Payoff ranking configuration
    #[serde(default)]
    pub ranking: RankingConfig,

    /// Enable dry-run mode (analyze but don't change behavior)
    #[serde(default)]
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

/// Default implementation for [`DenoiseWeights`].
impl Default for DenoiseWeights {
    /// Returns the default weight distribution.
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
    #[serde(default)]
    pub enabled: bool,

    /// Top percentile of patterns marked as boilerplate (0.0-1.0)
    #[serde(default)]
    pub percentile: f64,

    /// Cache refresh interval in days
    #[serde(default)]
    pub refresh_days: i64,
}

/// Default implementation for [`StopMotifsConfig`].
impl Default for StopMotifsConfig {
    /// Returns the default stop motifs configuration.
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
    #[serde(default)]
    pub enabled: bool,

    /// Quality target (percentage of candidates that must meet quality)
    #[serde(default)]
    pub quality_target: f64,

    /// Sample size for calibration (top N candidates)
    #[serde(default)]
    pub sample_size: usize,

    /// Maximum binary search iterations
    #[serde(default)]
    pub max_iterations: usize,
}

/// Default implementation for [`AutoCalibrationConfig`].
impl Default for AutoCalibrationConfig {
    /// Returns the default auto-calibration configuration.
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
    #[serde(default)]
    pub by: RankingBy,

    /// Minimum saved tokens to report
    #[serde(default)]
    pub min_saved_tokens: usize,

    /// Minimum rarity gain threshold
    #[serde(default)]
    pub min_rarity_gain: f64,

    /// Use live reachability data if available
    #[serde(default)]
    pub live_reach_boost: bool,
}

/// Ranking criteria options
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RankingBy {
    /// Rank by potential token savings
    #[default]
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
            live_reach_boost: true,
        }
    }
}

/// Default implementation for [`DenoiseConfig`].
impl Default for DenoiseConfig {
    /// Returns the default clone denoising configuration.
    fn default() -> Self {
        Self {
            enabled: false,          // Changed to opt-in for better default performance
            auto: true,              // Default auto-calibration enabled
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
        if !self.enabled {
            return Ok(());
        }

        validate_positive_usize(self.min_function_tokens, "min_function_tokens")?;
        validate_positive_usize(self.min_match_tokens, "min_match_tokens")?;
        validate_positive_usize(self.require_blocks, "require_blocks")?;
        validate_unit_range(self.similarity, "similarity")?;
        validate_unit_range(self.threshold_s, "threshold_s")?;
        validate_unit_range(self.io_mismatch_penalty, "io_mismatch_penalty")?;

        // Validate weights sum to approximately 1.0
        let weight_sum = self.weights.ast + self.weights.pdg + self.weights.emb;
        if (weight_sum - 1.0).abs() > 0.1 {
            return Err(ValknutError::validation(
                "denoise weights should sum to approximately 1.0",
            ));
        }

        validate_non_negative(self.weights.ast, "weights.ast")?;
        validate_non_negative(self.weights.pdg, "weights.pdg")?;
        validate_non_negative(self.weights.emb, "weights.emb")?;
        validate_unit_range(self.stop_motifs.percentile, "stop_motifs.percentile")?;
        validate_positive_i64(self.stop_motifs.refresh_days, "stop_motifs.refresh_days")?;
        validate_unit_range(
            self.auto_calibration.quality_target,
            "auto_calibration.quality_target",
        )?;
        validate_positive_usize(self.auto_calibration.sample_size, "auto_calibration.sample_size")?;
        validate_positive_usize(
            self.auto_calibration.max_iterations,
            "auto_calibration.max_iterations",
        )?;
        validate_positive_usize(self.ranking.min_saved_tokens, "ranking.min_saved_tokens")?;
        validate_positive_f64(self.ranking.min_rarity_gain, "ranking.min_rarity_gain")?;

        Ok(())
    }
}

/// Feature weights for multi-dimensional duplicate detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupeWeights {
    /// AST similarity weight
    #[serde(default)]
    pub ast: f64,

    /// Program dependence graph weight
    #[serde(default)]
    pub pdg: f64,

    /// Embedding similarity weight
    #[serde(default)]
    pub emb: f64,
}

/// Ranking criteria for duplicates
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RankingCriteria {
    /// Rank by potential token savings
    #[default]
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
    #[serde(default)]
    pub auto_denoise: bool,

    /// Enable adaptive learning of boilerplate patterns
    #[serde(default)]
    pub adaptive_learning: bool,

    /// Enable TF-IDF rarity weighting for structural analysis
    #[serde(default)]
    pub rarity_weighting: bool,

    /// Enable structural validation (PDG motifs, basic blocks)
    #[serde(default)]
    pub structural_validation: bool,

    /// Enable live reachability boost integration
    #[serde(default)]
    pub live_reach_integration: bool,

    /// Stop motif percentile threshold (0.0-1.0, e.g., 0.75 = top 0.75%)
    #[serde(default)]
    pub stop_motif_percentile: f64,

    /// Hub suppression threshold (0.0-1.0, patterns in >60% of files)
    #[serde(default)]
    pub hub_suppression_threshold: f64,

    /// Quality gate percentage (0.0-1.0, 80% of candidates must meet quality)
    #[serde(default)]
    pub quality_gate_percentage: f64,

    /// TF-IDF k-gram size for structural analysis
    #[serde(default)]
    pub tfidf_kgram_size: usize,

    /// Weisfeiler-Lehman hash iterations for PDG motifs
    #[serde(default)]
    pub wl_iterations: usize,

    /// Minimum rarity gain threshold
    #[serde(default)]
    pub min_rarity_gain: f64,

    /// External call Jaccard similarity penalty threshold
    #[serde(default)]
    pub external_call_jaccard_threshold: f64,

    /// Cache refresh interval in days
    #[serde(default)]
    pub cache_refresh_days: i64,

    /// Enable automatic cache refresh
    #[serde(default)]
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

/// Default implementation for [`DedupeWeights`].
impl Default for DedupeWeights {
    /// Returns the default weight distribution.
    fn default() -> Self {
        Self {
            ast: 0.35,
            pdg: 0.45,
            emb: 0.20,
        }
    }
}

/// Validation for [`DedupeWeights`].
impl DedupeWeights {
    /// Validate weights configuration.
    pub fn validate(&self) -> Result<()> {
        validate_non_negative(self.ast, "weights.ast")?;
        validate_non_negative(self.pdg, "weights.pdg")?;
        validate_non_negative(self.emb, "weights.emb")?;
        validate_weights_sum(&[self.ast, self.pdg, self.emb], 0.1, "weights")?;
        Ok(())
    }
}

/// Validation for [`AdaptiveDenoiseConfig`].
impl AdaptiveDenoiseConfig {
    /// Validate adaptive denoising configuration.
    pub fn validate(&self) -> Result<()> {
        validate_unit_range(self.stop_motif_percentile, "adaptive.stop_motif_percentile")?;
        validate_unit_range(
            self.hub_suppression_threshold,
            "adaptive.hub_suppression_threshold",
        )?;
        validate_unit_range(
            self.quality_gate_percentage,
            "adaptive.quality_gate_percentage",
        )?;
        validate_bounded_usize(self.tfidf_kgram_size, 1, 20, "adaptive.tfidf_kgram_size")?;
        validate_bounded_usize(self.wl_iterations, 1, 10, "adaptive.wl_iterations")?;
        validate_positive_f64(self.min_rarity_gain, "adaptive.min_rarity_gain")?;
        validate_unit_range(
            self.external_call_jaccard_threshold,
            "adaptive.external_call_jaccard_threshold",
        )?;
        validate_positive_i64(self.cache_refresh_days, "adaptive.cache_refresh_days")?;
        Ok(())
    }
}

/// Validation for [`DedupeConfig`].
impl DedupeConfig {
    /// Validate dedupe configuration.
    pub fn validate(&self) -> Result<()> {
        // Core size thresholds
        validate_positive_usize(self.min_function_tokens, "min_function_tokens")?;
        validate_positive_usize(self.min_ast_nodes, "min_ast_nodes")?;
        validate_positive_usize(self.min_match_tokens, "min_match_tokens")?;
        validate_positive_usize(self.shingle_k, "shingle_k")?;

        // Unit range validations
        validate_unit_range(self.min_match_coverage, "min_match_coverage")?;
        validate_unit_range(self.io_mismatch_penalty, "io_mismatch_penalty")?;
        validate_unit_range(self.threshold_s, "threshold_s")?;

        // Weights validation
        self.weights.validate()?;

        // Stop phrases validation
        self.validate_stop_phrases()?;

        // Adaptive config validation
        self.adaptive.validate()?;

        Ok(())
    }

    /// Validate stop phrases are non-empty.
    fn validate_stop_phrases(&self) -> Result<()> {
        for pattern in &self.stop_phrases {
            if pattern.is_empty() {
                return Err(ValknutError::validation("Empty pattern in stop_phrases"));
            }
        }
        Ok(())
    }
}
