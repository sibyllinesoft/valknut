use super::*;
use crate::core::errors::ValknutError;
use std::collections::HashMap;

fn expect_validation_error<T: std::fmt::Debug>(result: Result<T>) -> ValknutError {
    result.expect_err("expected validation failure")
}

#[test]
fn default_configs_validate_successfully() {
    ValknutConfig::default()
        .validate()
        .expect("valknut default");
    AnalysisConfig::default()
        .validate()
        .expect("analysis default");
    ScoringConfig::default()
        .validate()
        .expect("scoring default");
    CoverageConfig::default()
        .validate()
        .expect("coverage default");
    PerformanceConfig::default()
        .validate()
        .expect("performance default");
    DedupeConfig::default().validate().expect("dedupe default");
    DenoiseConfig::default()
        .validate()
        .expect("denoise default");
}

#[test]
fn analysis_config_confidence_threshold_bounds() {
    let mut config = AnalysisConfig::default();
    config.confidence_threshold = 1.5;
    let err = expect_validation_error(config.validate());
    assert!(matches!(err, ValknutError::Validation { .. }));
}

#[test]
fn coverage_config_requires_patterns_when_auto_discovering() {
    let mut config = CoverageConfig::default();
    config.file_patterns.clear();
    let err = expect_validation_error(config.validate());
    assert!(
        format!("{err}").contains("file_patterns"),
        "unexpected error message: {err}"
    );

    config.file_patterns = vec!["coverage.xml".into()];
    config.search_paths.clear();
    let err = expect_validation_error(config.validate());
    assert!(
        format!("{err}").contains("search_paths"),
        "unexpected error message: {err}"
    );
}

#[test]
fn performance_config_rejects_zero_limits() {
    let mut config = PerformanceConfig::default();
    config.max_threads = Some(0);
    let err = expect_validation_error(config.validate());
    assert!(format!("{err}").contains("max_threads"));

    config.max_threads = Some(4);
    config.batch_size = 0;
    let err = expect_validation_error(config.validate());
    assert!(format!("{err}").contains("batch_size"));
}

#[test]
fn language_config_requires_extensions_and_thresholds() {
    let config = LanguageConfig {
        enabled: true,
        file_extensions: Vec::new(),
        tree_sitter_language: "rust".into(),
        max_file_size_mb: 10.0,
        complexity_threshold: 5.0,
        additional_settings: HashMap::new(),
    };
    let err = expect_validation_error(config.validate());
    assert!(format!("{err}").contains("file_extensions"));

    let config = LanguageConfig {
        enabled: true,
        file_extensions: vec![".rs".into()],
        tree_sitter_language: "rust".into(),
        max_file_size_mb: -1.0,
        complexity_threshold: 5.0,
        additional_settings: HashMap::new(),
    };
    let err = expect_validation_error(config.validate());
    assert!(format!("{err}").contains("max_file_size_mb"));
}

#[test]
fn denoise_config_validates_weight_sum() {
    let mut config = DenoiseConfig::default();
    config.enabled = true;
    config.weights.ast = -0.1;
    let err = expect_validation_error(config.validate());
    assert!(format!("{err}").contains("weights"), "{err}");
}

#[test]
fn dedupe_config_enforces_positive_thresholds() {
    let mut config = DedupeConfig::default();
    config.min_match_tokens = 0;
    let err = expect_validation_error(config.validate());
    assert!(format!("{err}").contains("min_match_tokens"), "{err}");

    let mut config = DedupeConfig::default();
    config.adaptive.hub_suppression_threshold = 1.5;
    let err = expect_validation_error(config.validate());
    assert!(
        format!("{err}").contains("hub_suppression_threshold"),
        "{err}"
    );
}
