//! Configuration Layer Management
//!
//! This module provides layered configuration management for the CLI, allowing
//! seamless merging of default configurations, configuration files, and CLI overrides.

use anyhow;

use crate::cli::args::AnalyzeArgs;
use valknut_rs::core::config::{
    AnalysisConfig, CoverageConfig, DenoiseConfig, ReportFormat, ValknutConfig,
};

/// Trait for merging configuration layers
pub trait ConfigMerge<T> {
    /// Merge another configuration into this one, with the other taking priority
    fn merge_with(&mut self, other: T);
}

/// Convert CLI arguments to partial configuration overrides
pub trait FromCliArgs<T> {
    /// Create a partial configuration from CLI arguments
    fn from_cli_args(args: &T) -> Self;
}

/// Enhanced configuration loading with layered approach
pub fn build_layered_valknut_config(args: &AnalyzeArgs) -> anyhow::Result<ValknutConfig> {
    // Layer 1: Start with defaults
    let mut config = ValknutConfig::default();

    // Layer 2: Apply configuration file if provided
    if let Some(config_path) = &args.config {
        let file_config = ValknutConfig::from_yaml_file(config_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to load configuration from {}: {}",
                config_path.display(),
                e
            )
        })?;

        // Merge file config (overrides defaults)
        config.merge_with(file_config);
    }

    // Layer 3: Apply CLI overrides (highest priority)
    let cli_overrides = ValknutConfig::from_cli_args(args);
    config.merge_with(cli_overrides);

    // Validate final configuration
    config
        .validate()
        .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;

    Ok(config)
}

impl ConfigMerge<ValknutConfig> for ValknutConfig {
    fn merge_with(&mut self, other: ValknutConfig) {
        // Merge analysis config
        self.analysis.merge_with(other.analysis);

        // Merge coverage config
        self.coverage.merge_with(other.coverage);

        // Merge denoise config
        self.denoise.merge_with(other.denoise);

        // Merge I/O config - for paths and format, take the other if it's set
        if other.io.report_dir.is_some() {
            self.io.report_dir = other.io.report_dir;
        }
        // ReportFormat doesn't implement PartialEq, so we check by converting to string or using pattern matching
        match other.io.report_format {
            ReportFormat::Html => {} // No change needed
            _ => self.io.report_format = other.io.report_format,
        }
    }
}

impl ConfigMerge<AnalysisConfig> for AnalysisConfig {
    fn merge_with(&mut self, other: AnalysisConfig) {
        // For boolean flags, take the other value if it differs from default
        if !other.enable_scoring {
            self.enable_scoring = false;
        }
        if !other.enable_graph_analysis {
            self.enable_graph_analysis = false;
        }
        if !other.enable_lsh_analysis {
            self.enable_lsh_analysis = false;
        }
        if !other.enable_refactoring_analysis {
            self.enable_refactoring_analysis = false;
        }
        if !other.enable_coverage_analysis {
            self.enable_coverage_analysis = false;
        }
        if !other.enable_structure_analysis {
            self.enable_structure_analysis = false;
        }
        if !other.enable_names_analysis {
            self.enable_names_analysis = false;
        }

        // For positive enables (features disabled by default that CLI can enable)
        if other.enable_lsh_analysis && !self.enable_lsh_analysis {
            self.enable_lsh_analysis = true;
        }
    }
}

impl ConfigMerge<CoverageConfig> for CoverageConfig {
    fn merge_with(&mut self, other: CoverageConfig) {
        if other.coverage_file.is_some() {
            self.coverage_file = other.coverage_file;
        }
        if !other.auto_discover {
            self.auto_discover = false;
        }
        if other.max_age_days != 7 {
            // 7 is the default
            self.max_age_days = other.max_age_days;
        }
    }
}

impl ConfigMerge<DenoiseConfig> for DenoiseConfig {
    fn merge_with(&mut self, other: DenoiseConfig) {
        if !other.enabled {
            self.enabled = false;
        }
        if !other.auto {
            self.auto = false;
        }
        if other.dry_run {
            self.dry_run = true;
        }

        // Merge numerical parameters if they differ from defaults
        if other.min_function_tokens != 40 {
            self.min_function_tokens = other.min_function_tokens;
        }
        if other.min_match_tokens != 24 {
            self.min_match_tokens = other.min_match_tokens;
        }
        if other.require_blocks != 2 {
            self.require_blocks = other.require_blocks;
        }
        if other.similarity != 0.82 {
            self.similarity = other.similarity;
            self.threshold_s = other.similarity;
        }

        // Merge weights if they differ from defaults
        if other.weights.ast != 0.35 {
            self.weights.ast = other.weights.ast;
        }
        if other.weights.pdg != 0.45 {
            self.weights.pdg = other.weights.pdg;
        }
        if other.weights.emb != 0.20 {
            self.weights.emb = other.weights.emb;
        }

        if other.io_mismatch_penalty != 0.25 {
            self.io_mismatch_penalty = other.io_mismatch_penalty;
        }

        // Merge auto-calibration settings
        if other.auto_calibration.quality_target != 0.8 {
            self.auto_calibration.quality_target = other.auto_calibration.quality_target;
        }
        if other.auto_calibration.sample_size != 200 {
            self.auto_calibration.sample_size = other.auto_calibration.sample_size;
        }

        // Merge ranking settings
        if other.ranking.min_saved_tokens != 100 {
            self.ranking.min_saved_tokens = other.ranking.min_saved_tokens;
        }
        if other.ranking.min_rarity_gain != 1.2 {
            self.ranking.min_rarity_gain = other.ranking.min_rarity_gain;
        }

        // Note: loose_sweep, rarity_weighting, structural_validation
        // and live_reach_boost are not in the DenoiseConfig struct
    }
}

impl FromCliArgs<AnalyzeArgs> for ValknutConfig {
    fn from_cli_args(args: &AnalyzeArgs) -> Self {
        ValknutConfig {
            analysis: AnalysisConfig::from_cli_args(args),
            coverage: CoverageConfig::from_cli_args(args),
            denoise: DenoiseConfig::from_cli_args(args),
            ..Default::default()
        }
    }
}

impl FromCliArgs<AnalyzeArgs> for AnalysisConfig {
    fn from_cli_args(args: &AnalyzeArgs) -> Self {
        AnalysisConfig {
            enable_structure_analysis: !args.analysis_control.no_structure,
            enable_refactoring_analysis: !args.analysis_control.no_refactoring,
            enable_graph_analysis: !args.analysis_control.no_impact, // Impact includes graph
            enable_lsh_analysis: !args.analysis_control.no_lsh,
            enable_coverage_analysis: !args.coverage.no_coverage,
            enable_scoring: !args.analysis_control.no_complexity, // Map complexity to scoring
            enable_names_analysis: true,                          // Always enabled
            ..Default::default()
        }
    }
}

impl FromCliArgs<AnalyzeArgs> for CoverageConfig {
    fn from_cli_args(args: &AnalyzeArgs) -> Self {
        CoverageConfig {
            coverage_file: args.coverage.coverage_file.clone(),
            auto_discover: !args.coverage.no_coverage_auto_discover,
            max_age_days: args.coverage.coverage_max_age_days.unwrap_or(7),
            ..Default::default()
        }
    }
}

impl FromCliArgs<AnalyzeArgs> for DenoiseConfig {
    fn from_cli_args(args: &AnalyzeArgs) -> Self {
        DenoiseConfig {
            enabled: !args.clone_detection.no_denoise,
            auto: !args.advanced_clone.no_auto,
            dry_run: args.clone_detection.denoise_dry_run,
            min_function_tokens: args.clone_detection.min_function_tokens.unwrap_or(40),
            min_match_tokens: args.clone_detection.min_match_tokens.unwrap_or(24),
            require_blocks: args.clone_detection.require_blocks.unwrap_or(2),
            similarity: args.clone_detection.similarity.unwrap_or(0.82),
            threshold_s: args.clone_detection.similarity.unwrap_or(0.82),

            weights: valknut_rs::core::config::DenoiseWeights {
                ast: args.advanced_clone.ast_weight.unwrap_or(0.35),
                pdg: args.advanced_clone.pdg_weight.unwrap_or(0.45),
                emb: args.advanced_clone.emb_weight.unwrap_or(0.20),
            },

            io_mismatch_penalty: args.advanced_clone.io_mismatch_penalty.unwrap_or(0.25),

            auto_calibration: valknut_rs::core::config::AutoCalibrationConfig {
                enabled: !args.advanced_clone.no_auto,
                quality_target: args.advanced_clone.quality_target.unwrap_or(0.8),
                sample_size: args.advanced_clone.sample_size.unwrap_or(200),
                max_iterations: 10, // Default from config.rs
            },

            ranking: valknut_rs::core::config::RankingConfig {
                by: valknut_rs::core::config::RankingBy::SavedTokens, // Default from config.rs
                min_saved_tokens: args.advanced_clone.min_saved_tokens.unwrap_or(100),
                min_rarity_gain: args.advanced_clone.min_rarity_gain.unwrap_or(1.2),
                live_reach_boost: args.advanced_clone.live_reach_boost,
            },

            // Note: loose_sweep, rarity_weighting, structural_validation
            // are not in the DenoiseConfig struct
            ..Default::default()
        }
    }
}
