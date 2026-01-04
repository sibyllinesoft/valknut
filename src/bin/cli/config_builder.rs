//! Configuration building logic for analysis settings.

use std::env;
use std::path::Path;

use tracing::info;

use valknut_rs::core::config::ValknutConfig;
use valknut_rs::detectors::structure::StructureConfig;

use crate::cli::args::{
    AdvancedCloneArgs, AnalyzeArgs, CohesionArgs, CoverageArgs, PerformanceProfile,
};
use crate::cli::config_layer::build_layered_valknut_config;

/// Build comprehensive ValknutConfig from CLI arguments.
pub async fn build_valknut_config(args: &AnalyzeArgs) -> anyhow::Result<ValknutConfig> {
    // Use the new layered configuration approach
    let mut config = build_layered_valknut_config(args)?;

    // Apply performance profile optimizations
    apply_performance_profile(&mut config, &args.profile);

    Ok(config)
}

/// Apply performance profile optimizations to the configuration.
pub fn apply_performance_profile(config: &mut ValknutConfig, profile: &PerformanceProfile) {
    match profile {
        PerformanceProfile::Fast => {
            // Fast mode - reduced LSH precision for speed
            config.lsh.num_bands = 10;
            config.lsh.num_hashes = 50;
            info!("🚀 Performance profile: Fast mode - optimized for speed");
        }
        PerformanceProfile::Balanced => {
            // Balanced mode - good default (no changes needed)
            info!("⚖️  Performance profile: Balanced mode - default settings");
        }
        PerformanceProfile::Thorough => {
            // Thorough mode - higher LSH precision
            config.lsh.num_bands = 20;
            config.lsh.num_hashes = 160; // Must be divisible by num_bands
            config.denoise.enabled = true;
            info!("🔍 Performance profile: Thorough mode - comprehensive analysis");
        }
        PerformanceProfile::Extreme => {
            // Extreme mode - maximum LSH precision
            config.lsh.num_bands = 50;
            config.lsh.num_hashes = 200;
            config.denoise.enabled = true;
            info!("🔥 Performance profile: Extreme mode - maximum analysis depth");
        }
    }

    // Optional dev/demo preset to ensure UI clone pairs appear with low thresholds.
    if env::var("VALKNUT_DEV_DEMO").is_ok() || cfg!(debug_assertions) {
        apply_dev_clone_presets(config);
    }
}

/// Lower clone thresholds and enable semantic similarity for demos/UI snapshots.
pub fn apply_dev_clone_presets(config: &mut ValknutConfig) {
    config.analysis.enable_lsh_analysis = true;
    config.denoise.enabled = true;
    config.denoise.min_function_tokens = config.denoise.min_function_tokens.min(8).max(1);
    config.denoise.min_match_tokens = config.denoise.min_match_tokens.min(6).max(1);
    config.denoise.require_blocks = 1;
    config.denoise.similarity = config.denoise.similarity.min(0.7);
    config.denoise.threshold_s = config.denoise.similarity;
    config.dedupe.min_ast_nodes = config.dedupe.min_ast_nodes.min(8).max(1);
    config.dedupe.min_match_tokens = config.dedupe.min_match_tokens.min(8).max(1);
    config.lsh.use_semantic_similarity = true;
    config.lsh.similarity_threshold = config.lsh.similarity_threshold.min(0.7);
}

/// Build ValknutConfig from CLI args for analysis.
/// Shared by both progress and non-progress analysis functions.
#[allow(dead_code)]
pub async fn build_analysis_config(args: &AnalyzeArgs) -> anyhow::Result<ValknutConfig> {
    use valknut_rs::core::config::DenoiseConfig;

    let mut config = ValknutConfig::default();
    config.analysis.enable_lsh_analysis = true;
    config.analysis.enable_coverage_analysis = true;

    // Configure APTED verification
    apply_apted_config(&mut config, &args.advanced_clone);

    // Configure denoise settings
    let denoise_enabled = true;
    let auto_enabled = !args.advanced_clone.no_auto;

    log_denoise_status(denoise_enabled);

    config.denoise = build_denoise_config(args, denoise_enabled, auto_enabled);

    // Apply denoise-specific settings
    if denoise_enabled {
        apply_denoise_settings(&mut config, args, auto_enabled).await?;
    }

    // Apply analysis control flags
    apply_analysis_control_flags(&mut config, args);

    // Configure coverage
    config.coverage = build_coverage_config(&args.coverage);

    Ok(config)
}

/// Apply APTED verification settings to config.
pub fn apply_apted_config(config: &mut ValknutConfig, args: &AdvancedCloneArgs) {
    if args.no_apted_verify {
        config.lsh.verify_with_apted = false;
    } else if args.apted_verify {
        config.lsh.verify_with_apted = true;
    }
    if let Some(max_nodes) = args.apted_max_nodes {
        config.lsh.apted_max_nodes = max_nodes;
    }
    if let Some(max_pairs) = args.apted_max_pairs {
        config.lsh.apted_max_pairs_per_entity = max_pairs;
    }
}

/// Log denoise status.
pub fn log_denoise_status(enabled: bool) {
    if enabled {
        info!("Clone denoising enabled (advanced analysis mode)");
    } else {
        info!("Clone denoising disabled via --no-denoise flag");
    }
}

/// Build denoise configuration from CLI args.
pub fn build_denoise_config(
    args: &AnalyzeArgs,
    denoise_enabled: bool,
    auto_enabled: bool,
) -> valknut_rs::core::config::DenoiseConfig {
    use valknut_rs::core::config::{
        AutoCalibrationConfig, DenoiseConfig, DenoiseWeights, RankingConfig, StopMotifsConfig,
    };

    let min_function_tokens = args.clone_detection.min_function_tokens.unwrap_or(40);
    let min_match_tokens = args.clone_detection.min_match_tokens.unwrap_or(24);
    let require_blocks = args.clone_detection.require_blocks.unwrap_or(2);
    let similarity = args.clone_detection.similarity.unwrap_or(0.82);

    let weights = build_denoise_weights(&args.advanced_clone);
    let auto_calibration = build_auto_calibration_config(&args.advanced_clone, auto_enabled);
    let ranking = build_ranking_config(&args.advanced_clone);

    DenoiseConfig {
        enabled: denoise_enabled,
        auto: auto_enabled,
        min_function_tokens,
        min_match_tokens,
        require_blocks,
        similarity,
        weights,
        io_mismatch_penalty: args.advanced_clone.io_mismatch_penalty.unwrap_or(0.25),
        threshold_s: similarity,
        stop_motifs: StopMotifsConfig::default(),
        auto_calibration,
        ranking,
        dry_run: args.clone_detection.denoise_dry_run,
    }
}

/// Build denoise weights from CLI args.
pub fn build_denoise_weights(args: &AdvancedCloneArgs) -> valknut_rs::core::config::DenoiseWeights {
    let mut weights = valknut_rs::core::config::DenoiseWeights::default();
    if let Some(ast_weight) = args.ast_weight {
        weights.ast = ast_weight;
    }
    if let Some(pdg_weight) = args.pdg_weight {
        weights.pdg = pdg_weight;
    }
    if let Some(emb_weight) = args.emb_weight {
        weights.emb = emb_weight;
    }
    weights
}

/// Build auto-calibration config from CLI args.
pub fn build_auto_calibration_config(
    args: &AdvancedCloneArgs,
    auto_enabled: bool,
) -> valknut_rs::core::config::AutoCalibrationConfig {
    let mut config = valknut_rs::core::config::AutoCalibrationConfig {
        enabled: auto_enabled,
        ..Default::default()
    };
    if let Some(quality_target) = args.quality_target {
        config.quality_target = quality_target;
    }
    if let Some(sample_size) = args.sample_size {
        config.sample_size = sample_size;
    }
    config
}

/// Build ranking config from CLI args.
pub fn build_ranking_config(args: &AdvancedCloneArgs) -> valknut_rs::core::config::RankingConfig {
    let mut ranking = valknut_rs::core::config::RankingConfig::default();
    if let Some(min_saved_tokens) = args.min_saved_tokens {
        ranking.min_saved_tokens = min_saved_tokens;
    }
    if let Some(min_rarity_gain) = args.min_rarity_gain {
        ranking.min_rarity_gain = min_rarity_gain;
    }
    ranking
}

/// Apply denoise-specific settings when denoise is enabled.
pub async fn apply_denoise_settings(
    config: &mut ValknutConfig,
    args: &AnalyzeArgs,
    auto_enabled: bool,
) -> anyhow::Result<()> {
    use owo_colors::OwoColorize;

    config.dedupe.adaptive.rarity_weighting = true;
    config.lsh.shingle_size = 9;

    info!(
        "Denoise config - min_function_tokens: {}, min_match_tokens: {}, require_blocks: {}, similarity: {:.2}",
        config.denoise.min_function_tokens,
        config.denoise.min_match_tokens,
        config.denoise.require_blocks,
        config.denoise.similarity
    );

    create_denoise_cache_directories().await?;

    if auto_enabled {
        info!("Auto-calibration enabled (default)");
    } else {
        info!("Auto-calibration disabled via --no-auto flag");
    }

    if args.clone_detection.denoise_dry_run {
        info!("DRY-RUN mode enabled");
        println!("{}", "denoise: DRY-RUN (no changes).".yellow());
    }

    Ok(())
}

/// Apply analysis control flags from CLI args.
pub fn apply_analysis_control_flags(config: &mut ValknutConfig, args: &AnalyzeArgs) {
    if args.coverage.no_coverage {
        config.analysis.enable_coverage_analysis = false;
    }
    if args.analysis_control.no_complexity {
        config.analysis.enable_scoring = false;
    }
    if args.analysis_control.no_structure {
        config.analysis.enable_structure_analysis = false;
    }
    if args.analysis_control.no_refactoring {
        config.analysis.enable_refactoring_analysis = false;
    }
    if args.analysis_control.no_impact {
        config.analysis.enable_graph_analysis = false;
    }
    if args.analysis_control.no_lsh {
        config.analysis.enable_lsh_analysis = false;
    }
    if args.analysis_control.cohesion {
        config.analysis.enable_cohesion_analysis = true;
        config.cohesion.enabled = true;
        apply_cohesion_args(config, &args.cohesion);
    }
}

/// Apply cohesion-specific CLI args.
pub fn apply_cohesion_args(config: &mut ValknutConfig, args: &CohesionArgs) {
    if let Some(min_score) = args.cohesion_min_score {
        config.cohesion.thresholds.min_cohesion = min_score;
    }
    if let Some(min_doc_alignment) = args.cohesion_min_doc_alignment {
        config.cohesion.thresholds.min_doc_alignment = min_doc_alignment;
    }
    if let Some(outlier_percentile) = args.cohesion_outlier_percentile {
        config.cohesion.thresholds.outlier_percentile = outlier_percentile;
    }
}

/// Build coverage config from CLI args.
pub fn build_coverage_config(args: &CoverageArgs) -> valknut_rs::core::config::CoverageConfig {
    let mut config = valknut_rs::core::config::CoverageConfig::default();
    if let Some(coverage_file) = &args.coverage_file {
        config.coverage_file = Some(coverage_file.clone());
        config.auto_discover = false;
    }
    if args.no_coverage_auto_discover {
        config.auto_discover = false;
    }
    if let Some(max_age_days) = args.coverage_max_age_days {
        config.max_age_days = max_age_days;
    }
    config
}

/// Create denoise cache directories if they don't exist.
pub async fn create_denoise_cache_directories() -> anyhow::Result<()> {
    let cache_base = std::path::Path::new(".valknut/cache/denoise");

    // Create the denoise cache directory
    tokio::fs::create_dir_all(&cache_base).await?;

    // Create cache files if they don't exist
    let stop_motifs_path = cache_base.join("stop_motifs.v1.json");
    let auto_calibration_path = cache_base.join("auto_calibration.v1.json");

    if !stop_motifs_path.exists() {
        let empty_motifs = serde_json::json!({
            "version": 1,
            "created": chrono::Utc::now().to_rfc3339(),
            "stop_motifs": []
        });
        tokio::fs::write(
            &stop_motifs_path,
            serde_json::to_string_pretty(&empty_motifs)?,
        )
        .await?;
        info!("Created denoise cache file: {}", stop_motifs_path.display());
    }

    if !auto_calibration_path.exists() {
        let empty_calibration = serde_json::json!({
            "version": 1,
            "created": chrono::Utc::now().to_rfc3339(),
            "calibration_data": {}
        });
        tokio::fs::write(
            &auto_calibration_path,
            serde_json::to_string_pretty(&empty_calibration)?,
        )
        .await?;
        info!(
            "Created denoise cache file: {}",
            auto_calibration_path.display()
        );
    }

    Ok(())
}

/// Load configuration from file or use defaults.
pub async fn load_configuration(config_path: Option<&Path>) -> anyhow::Result<StructureConfig> {
    let config = match config_path {
        Some(path) => {
            let content = tokio::fs::read_to_string(path).await?;
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("yaml" | "yml") => serde_yaml::from_str(&content)?,
                Some("json") => serde_json::from_str(&content)?,
                _ => serde_yaml::from_str(&content)?,
            }
        }
        None => StructureConfig::default(),
    };

    Ok(config)
}
