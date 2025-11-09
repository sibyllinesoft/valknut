//! Configuration Layer Management
//!
//! This module provides layered configuration management for the CLI, allowing
//! seamless merging of default configurations, configuration files, and CLI overrides.

use anyhow;

use crate::cli::args::AnalyzeArgs;
use valknut_rs::api::config_types as api_config;
use valknut_rs::core::config::{CoverageConfig, DenoiseConfig, LshConfig, ValknutConfig};

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

fn merge_language_settings(
    target: &mut ValknutConfig,
    source: &ValknutConfig,
    api_config: &api_config::AnalysisConfig,
) {
    for (language, source_config) in &source.languages {
        let entry = target
            .languages
            .entry(language.clone())
            .or_insert_with(|| source_config.clone());

        if api_config.languages.enabled.contains(language) {
            entry.enabled = true;
        } else {
            entry.enabled = source_config.enabled;
        }

        if api_config.languages.max_file_size_mb.is_none() {
            entry.max_file_size_mb = source_config.max_file_size_mb;
        }

        if !api_config
            .languages
            .complexity_thresholds
            .contains_key(language)
        {
            entry.complexity_threshold = source_config.complexity_threshold;
        }

        entry.file_extensions = source_config.file_extensions.clone();
        entry.tree_sitter_language = source_config.tree_sitter_language.clone();
        entry.additional_settings = source_config.additional_settings.clone();
    }
}

fn apply_advanced_sections_from_file(target: &mut ValknutConfig, source: &ValknutConfig) {
    target.scoring = source.scoring.clone();
    target.graph = source.graph.clone();
    target.lsh = source.lsh.clone();
    target.dedupe = source.dedupe.clone();
    target.denoise = source.denoise.clone();
    target.io = source.io.clone();
    target.performance = source.performance.clone();
    target.structure = source.structure.clone();
    target.live_reach = source.live_reach.clone();
    target.analysis.enable_names_analysis = source.analysis.enable_names_analysis;
}

/// Enhanced configuration loading with layered approach
pub fn build_layered_valknut_config(args: &AnalyzeArgs) -> anyhow::Result<ValknutConfig> {
    let mut api_config = api_config::AnalysisConfig::default();
    let mut file_config: Option<ValknutConfig> = None;

    if let Some(config_path) = &args.config {
        let loaded_config = ValknutConfig::from_yaml_file(config_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to load configuration from {}: {}",
                config_path.display(),
                e
            )
        })?;

        let api_from_file = api_config::AnalysisConfig::from_valknut_config(loaded_config.clone())
            .map_err(|e| anyhow::anyhow!("Failed to normalize configuration: {}", e))?;

        api_config.merge_with(api_from_file);
        file_config = Some(loaded_config);
    }

    let cli_api_overrides = api_config::AnalysisConfig::from_cli_args(args);
    api_config.merge_with(cli_api_overrides);

    let mut config = api_config.clone().to_valknut_config();

    if let Some(file_cfg) = file_config {
        apply_advanced_sections_from_file(&mut config, &file_cfg);
        merge_language_settings(&mut config, &file_cfg, &api_config);
    }

    let cli_overrides = ValknutConfig::from_cli_args(args);
    config.merge_with(cli_overrides);

    config
        .validate()
        .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;

    Ok(config)
}

impl ConfigMerge<ValknutConfig> for ValknutConfig {
    fn merge_with(&mut self, other: ValknutConfig) {
        self.coverage.merge_with(other.coverage);
        self.denoise.merge_with(other.denoise);

        if other.io.cache_dir.is_some() {
            self.io.cache_dir = other.io.cache_dir;
        }
        if other.io.report_dir.is_some() {
            self.io.report_dir = other.io.report_dir;
        }
        if other.io.cache_ttl_seconds != self.io.cache_ttl_seconds {
            self.io.cache_ttl_seconds = other.io.cache_ttl_seconds;
        }
        if other.lsh.verify_with_apted != self.lsh.verify_with_apted {
            self.lsh.verify_with_apted = other.lsh.verify_with_apted;
        }
        let default_lsh = LshConfig::default();
        if other.lsh.apted_max_nodes != default_lsh.apted_max_nodes {
            self.lsh.apted_max_nodes = other.lsh.apted_max_nodes;
        }
        if other.lsh.apted_max_pairs_per_entity != default_lsh.apted_max_pairs_per_entity {
            self.lsh.apted_max_pairs_per_entity = other.lsh.apted_max_pairs_per_entity;
        }
        if other.io.enable_caching != self.io.enable_caching {
            self.io.enable_caching = other.io.enable_caching;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::{Cli, Commands};
    use clap::Parser;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    #[test]
    fn layered_config_honors_file_and_cli_priorities() {
        let temp = tempdir().expect("temp dir");
        let config_path = temp.path().join("valknut.yml");
        let coverage_file_path = temp.path().join("coverage.lcov");
        fs::write(&coverage_file_path, "TN:\n").expect("coverage file");

        let mut file_config = ValknutConfig::default();
        file_config.coverage.auto_discover = false;
        file_config.coverage.max_age_days = 14;
        file_config
            .languages
            .entry("python".into())
            .and_modify(|lang| {
                lang.enabled = false;
                lang.max_file_size_mb = 4.0;
                lang.additional_settings
                    .insert("source".into(), "file".into());
            });
        file_config.io.cache_dir = Some(PathBuf::from("file-cache"));
        file_config.lsh.verify_with_apted = false;
        file_config
            .to_yaml_file(&config_path)
            .expect("write config");

        let cli = Cli::parse_from([
            "valknut",
            "analyze",
            "--config",
            config_path.to_str().unwrap(),
            "--no-coverage",
            "--no-structure",
            "--no-impact",
            "--coverage-file",
            coverage_file_path.to_str().unwrap(),
            "--no-coverage-auto-discover",
            "--denoise",
            "--denoise-dry-run",
            "--min-function-tokens",
            "50",
            "--min-match-tokens",
            "30",
            "--require-blocks",
            "3",
            "--similarity",
            "0.9",
            "--ast-weight",
            "0.4",
            "--pdg-weight",
            "0.4",
            "--emb-weight",
            "0.2",
            "--io-mismatch-penalty",
            "0.3",
            "--quality-target",
            "0.9",
            "--sample-size",
            "300",
            "--min-saved-tokens",
            "150",
            "--min-rarity-gain",
            "1.4",
            "--apted-max-nodes",
            "512",
            "--apted-max-pairs",
            "10",
            "--apted-verify",
        ]);
        let Commands::Analyze(args_box) = cli.command else {
            panic!("expected analyze command");
        };
        let args = *args_box;

        let config = build_layered_valknut_config(&args).expect("build config");

        // File-driven advanced sections retained
        assert_eq!(
            config.io.cache_dir.as_deref(),
            Some(Path::new("file-cache"))
        );

        // CLI overrides applied
        assert!(!config.coverage.auto_discover);
        assert_eq!(
            config.coverage.coverage_file.as_deref(),
            Some(coverage_file_path.as_path())
        );
        assert_eq!(config.coverage.max_age_days, 14);

        assert!(config.denoise.dry_run);
        assert_eq!(config.denoise.min_function_tokens, 50);
        assert_eq!(config.denoise.min_match_tokens, 30);
        assert_eq!(config.denoise.require_blocks, 3);
        assert!((config.denoise.similarity - 0.9).abs() < f64::EPSILON);
        assert!((config.denoise.weights.ast - 0.4).abs() < f64::EPSILON);
        assert!((config.denoise.weights.pdg - 0.4).abs() < f64::EPSILON);
        assert!((config.denoise.weights.emb - 0.2).abs() < f64::EPSILON);
        assert!((config.denoise.io_mismatch_penalty - 0.3).abs() < f64::EPSILON);
        assert!((config.denoise.auto_calibration.quality_target - 0.9).abs() < f64::EPSILON);
        assert_eq!(config.denoise.auto_calibration.sample_size, 300);
        assert_eq!(config.denoise.ranking.min_saved_tokens, 150);
        assert!((config.denoise.ranking.min_rarity_gain - 1.4).abs() < f64::EPSILON);

        // LSH overrides
        assert!(config.lsh.verify_with_apted);
        assert_eq!(config.lsh.apted_max_nodes, 512);
        assert_eq!(config.lsh.apted_max_pairs_per_entity, 10);

        // Language merge retains file-specified metadata and re-enables via CLI defaults
        let python = config.languages.get("python").expect("python config");
        assert!(!python.enabled, "file-level disablement should persist");
        assert_eq!(python.max_file_size_mb, 4.0);
        assert_eq!(
            python
                .additional_settings
                .get("source")
                .and_then(|value| value.as_str()),
            Some("file")
        );
    }
}

impl ConfigMerge<api_config::AnalysisConfig> for api_config::AnalysisConfig {
    fn merge_with(&mut self, other: api_config::AnalysisConfig) {
        let default_modules = api_config::AnalysisModules::default();

        if other.modules.complexity != default_modules.complexity {
            self.modules.complexity = other.modules.complexity;
        }
        if other.modules.dependencies != default_modules.dependencies {
            self.modules.dependencies = other.modules.dependencies;
        }
        if other.modules.duplicates != default_modules.duplicates {
            self.modules.duplicates = other.modules.duplicates;
        }
        if other.modules.refactoring != default_modules.refactoring {
            self.modules.refactoring = other.modules.refactoring;
        }
        if other.modules.structure != default_modules.structure {
            self.modules.structure = other.modules.structure;
        }
        if other.modules.coverage != default_modules.coverage {
            self.modules.coverage = other.modules.coverage;
        }

        if !other.languages.enabled.is_empty() {
            self.languages.enabled = other.languages.enabled;
        }

        let default_language = api_config::LanguageSettings::default();
        if other.languages.max_file_size_mb != default_language.max_file_size_mb {
            self.languages.max_file_size_mb = other.languages.max_file_size_mb;
        }
        if !other.languages.complexity_thresholds.is_empty()
            && other.languages.complexity_thresholds != default_language.complexity_thresholds
        {
            for (language, threshold) in other.languages.complexity_thresholds {
                self.languages
                    .complexity_thresholds
                    .insert(language, threshold);
            }
        }

        let default_files = api_config::FileSettings::default();
        if other.files.include_patterns != default_files.include_patterns {
            self.files.include_patterns = other.files.include_patterns;
        }
        if other.files.exclude_patterns != default_files.exclude_patterns {
            self.files.exclude_patterns = other.files.exclude_patterns;
        }
        if other.files.max_files.is_some() {
            self.files.max_files = other.files.max_files;
        }
        if other.files.follow_symlinks {
            self.files.follow_symlinks = true;
        }

        let default_quality = api_config::QualitySettings::default();
        if (other.quality.confidence_threshold - default_quality.confidence_threshold).abs()
            > f64::EPSILON
        {
            self.quality.confidence_threshold = other.quality.confidence_threshold;
        }
        if other.quality.max_analysis_time_per_file != default_quality.max_analysis_time_per_file {
            self.quality.max_analysis_time_per_file = other.quality.max_analysis_time_per_file;
        }
        if other.quality.strict_mode {
            self.quality.strict_mode = true;
        }

        let default_coverage = api_config::CoverageSettings::default();
        if other.coverage.enabled != default_coverage.enabled {
            self.coverage.enabled = other.coverage.enabled;
        }
        if other.coverage.file_path.is_some() {
            self.coverage.file_path = other.coverage.file_path;
        }
        if other.coverage.auto_discover != default_coverage.auto_discover {
            self.coverage.auto_discover = other.coverage.auto_discover;
        }
        if other.coverage.max_age_days != default_coverage.max_age_days {
            self.coverage.max_age_days = other.coverage.max_age_days;
        }
        if other.coverage.search_paths != default_coverage.search_paths
            && !other.coverage.search_paths.is_empty()
        {
            self.coverage.search_paths = other.coverage.search_paths;
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
        let mut config = ValknutConfig::default();
        config.coverage = CoverageConfig::from_cli_args(args);
        config.denoise = DenoiseConfig::from_cli_args(args);
        if args.advanced_clone.no_apted_verify {
            config.lsh.verify_with_apted = false;
        } else if args.advanced_clone.apted_verify {
            config.lsh.verify_with_apted = true;
        }
        if let Some(max_nodes) = args.advanced_clone.apted_max_nodes {
            config.lsh.apted_max_nodes = max_nodes;
        }
        if let Some(max_pairs) = args.advanced_clone.apted_max_pairs {
            config.lsh.apted_max_pairs_per_entity = max_pairs;
        }
        config
    }
}

impl FromCliArgs<AnalyzeArgs> for api_config::AnalysisConfig {
    fn from_cli_args(args: &AnalyzeArgs) -> Self {
        let mut config = api_config::AnalysisConfig::default();

        config.modules.structure = !args.analysis_control.no_structure;
        config.modules.refactoring = !args.analysis_control.no_refactoring;
        config.modules.dependencies = !args.analysis_control.no_impact;
        // config.modules.duplicates = !args.analysis_control.no_lsh; // Clone analysis (LSH) disabled by default for performance
        if args.analysis_control.no_lsh {
            config.modules.duplicates = false;
        } else {
            config.modules.duplicates = true;
            if args.clone_detection.semantic_clones
                || args.clone_detection.denoise
                || args.advanced_clone.no_apted_verify
                || args.advanced_clone.apted_verify
            {
                config.modules.duplicates = true;
            }
        }
        config.modules.coverage = !args.coverage.no_coverage;
        config.modules.complexity = !args.analysis_control.no_complexity;

        config.languages.enabled.clear();
        config.languages.complexity_thresholds.clear();
        config.languages.max_file_size_mb = None;

        if args.coverage.no_coverage {
            config.coverage.enabled = false;
        }
        if let Some(path) = &args.coverage.coverage_file {
            config.coverage.file_path = Some(path.clone());
        }
        if args.coverage.no_coverage_auto_discover {
            config.coverage.auto_discover = false;
        }
        if let Some(max_age) = args.coverage.coverage_max_age_days {
            config.coverage.max_age_days = max_age;
        }

        config
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
            enabled: args.clone_detection.denoise,
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
