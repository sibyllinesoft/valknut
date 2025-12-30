use super::*;

fn expect_denoise_error<F: FnOnce(&mut DenoiseConfig)>(modifier: F, needle: &str) {
    let mut cfg = DenoiseConfig::default();
    modifier(&mut cfg);
    let message = cfg
        .validate()
        .expect_err("expected validation failure")
        .to_string();
    assert!(
        message.contains(needle),
        "expected message containing '{needle}', got '{message}'"
    );
}

fn expect_dedupe_error<F: FnOnce(&mut DedupeConfig)>(modifier: F, needle: &str) {
    let mut cfg = DedupeConfig::default();
    modifier(&mut cfg);
    let message = cfg
        .validate()
        .expect_err("expected validation failure")
        .to_string();
    assert!(
        message.contains(needle),
        "expected message containing '{needle}', got '{message}'"
    );
}

#[test]
fn lsh_config_validation_and_conversion() {
    let core_cfg = crate::core::config::LshConfig::default();
    let mut cfg: LshConfig = core_cfg.clone().into();
    assert!(cfg.validate().is_ok());
    assert_eq!(
        cfg.hashes_per_band(),
        core_cfg.num_hashes / core_cfg.num_bands
    );

    cfg.num_hashes = 0;
    assert!(cfg
        .validate()
        .unwrap_err()
        .to_string()
        .contains("num_hashes"));
    cfg.num_hashes = 64;
    cfg.num_bands = 0;
    assert!(cfg
        .validate()
        .unwrap_err()
        .to_string()
        .contains("num_bands"));
    cfg.num_bands = 6;
    cfg.num_hashes = 63;
    assert!(cfg
        .validate()
        .unwrap_err()
        .to_string()
        .contains("divisible"));
    cfg.num_hashes = 60;
    cfg.similarity_threshold = 1.5;
    assert!(cfg
        .validate()
        .unwrap_err()
        .to_string()
        .contains("similarity_threshold"));
}

#[test]
fn denoise_config_validation_rules() {
    let valid = DenoiseConfig::default();
    assert!(valid.validate().is_ok());

    expect_denoise_error(|cfg| cfg.min_function_tokens = 0, "min_function_tokens");
    expect_denoise_error(|cfg| cfg.min_match_tokens = 0, "min_match_tokens");
    expect_denoise_error(|cfg| cfg.require_blocks = 0, "require_blocks");
    expect_denoise_error(|cfg| cfg.similarity = 1.5, "similarity");
    expect_denoise_error(|cfg| cfg.threshold_s = -0.1, "threshold_s");
    expect_denoise_error(|cfg| cfg.io_mismatch_penalty = 1.5, "io_mismatch_penalty");
    expect_denoise_error(
        |cfg| {
            cfg.weights.ast = 0.9;
            cfg.weights.pdg = 0.9;
            cfg.weights.emb = 0.9;
        },
        "weights",
    );
    expect_denoise_error(
        |cfg| {
            cfg.weights.ast = -0.1;
            cfg.weights.pdg = 0.55;
            cfg.weights.emb = 0.55;
        },
        "non-negative",
    );
    expect_denoise_error(|cfg| cfg.stop_motifs.percentile = 1.5, "percentile");
    expect_denoise_error(|cfg| cfg.stop_motifs.refresh_days = 0, "refresh_days");
    expect_denoise_error(
        |cfg| cfg.auto_calibration.quality_target = 1.5,
        "quality_target",
    );
    expect_denoise_error(|cfg| cfg.auto_calibration.sample_size = 0, "sample_size");
    expect_denoise_error(
        |cfg| cfg.auto_calibration.max_iterations = 0,
        "max_iterations",
    );
}

#[test]
fn dedupe_config_validation_rules() {
    let valid = DedupeConfig::default();
    assert!(valid.validate().is_ok());

    expect_dedupe_error(|cfg| cfg.min_function_tokens = 0, "min_function_tokens");
    expect_dedupe_error(|cfg| cfg.min_ast_nodes = 0, "min_ast_nodes");
    expect_dedupe_error(|cfg| cfg.min_match_tokens = 0, "min_match_tokens");
    expect_dedupe_error(|cfg| cfg.min_match_coverage = 1.5, "min_match_coverage");
    expect_dedupe_error(|cfg| cfg.shingle_k = 0, "shingle_k");
    expect_dedupe_error(|cfg| cfg.io_mismatch_penalty = -0.1, "io_mismatch_penalty");
    expect_dedupe_error(|cfg| cfg.threshold_s = 2.0, "threshold_s");
    expect_dedupe_error(
        |cfg| {
            cfg.weights.ast = 0.8;
            cfg.weights.pdg = 0.8;
            cfg.weights.emb = 0.8;
        },
        "weights",
    );
    expect_dedupe_error(|cfg| cfg.stop_phrases.push(String::new()), "Empty pattern");
    expect_dedupe_error(
        |cfg| cfg.adaptive.stop_motif_percentile = 1.5,
        "stop_motif_percentile",
    );
    expect_dedupe_error(
        |cfg| cfg.adaptive.hub_suppression_threshold = -0.1,
        "hub_suppression_threshold",
    );
    expect_dedupe_error(
        |cfg| cfg.adaptive.quality_gate_percentage = 2.0,
        "quality_gate_percentage",
    );
    expect_dedupe_error(|cfg| cfg.adaptive.tfidf_kgram_size = 0, "tfidf_kgram_size");
    expect_dedupe_error(|cfg| cfg.adaptive.wl_iterations = 0, "wl_iterations");
    expect_dedupe_error(|cfg| cfg.adaptive.min_rarity_gain = 0.0, "min_rarity_gain");
    expect_dedupe_error(
        |cfg| cfg.adaptive.external_call_jaccard_threshold = 2.0,
        "external_call_jaccard_threshold",
    );
    expect_dedupe_error(
        |cfg| cfg.adaptive.cache_refresh_days = 0,
        "cache_refresh_days",
    );
}
