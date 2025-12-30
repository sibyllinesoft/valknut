use super::*;

#[test]
fn test_unified_config_default() {
    let config = AnalysisConfig::default();

    // Check module defaults
    assert!(config.modules.complexity);
    assert!(config.modules.dependencies);
    assert!(!config.modules.duplicates); // Should be false by default
    assert!(config.modules.refactoring);
    assert!(config.modules.structure);
    assert!(config.modules.coverage);

    // Check language defaults
    assert_eq!(
        config.languages.enabled,
        vec!["python", "javascript", "typescript"]
    );
    assert_eq!(config.languages.max_file_size_mb, Some(10.0));

    // Check quality defaults
    assert_eq!(config.quality.confidence_threshold, 0.7);
    assert!(!config.quality.strict_mode);

    // Check file defaults
    assert!(config
        .files
        .exclude_patterns
        .contains(&"*/node_modules/*".to_string()));
    assert_eq!(config.files.include_patterns, vec!["**/*"]);
}

#[test]
fn test_fluent_interface() {
    let config = AnalysisConfig::new()
        .modules(|_| AnalysisModules::code_quality())
        .languages(|l| {
            l.add_language("rust")
                .with_complexity_threshold("rust", 15.0)
        })
        .files(|f| {
            f.with_max_files(1000)
                .exclude_patterns(vec!["*/target/*".to_string()])
        })
        .quality(|q| q.strict().with_timeout(60))
        .coverage(|c| c.with_search_paths(vec!["./coverage/".to_string()]));

    // Verify modules
    assert!(config.modules.complexity);
    assert!(config.modules.duplicates);
    assert!(config.modules.refactoring);
    assert!(!config.modules.dependencies);

    // Verify languages
    assert!(config.languages.enabled.contains(&"rust".to_string()));
    assert_eq!(
        config.languages.complexity_thresholds.get("rust"),
        Some(&15.0)
    );

    // Verify files
    assert_eq!(config.files.max_files, Some(1000));
    assert!(config
        .files
        .exclude_patterns
        .contains(&"*/target/*".to_string()));

    // Verify quality
    assert!(config.quality.strict_mode);
    assert_eq!(config.quality.max_analysis_time_per_file, Some(60));

    // Verify coverage
    assert!(config
        .coverage
        .search_paths
        .contains(&"./coverage/".to_string()));
}

#[test]
fn test_convenience_methods() {
    let config = AnalysisConfig::new()
        .with_languages(vec!["rust".to_string(), "go".to_string()])
        .with_confidence_threshold(0.85)
        .with_max_files(500)
        .exclude_pattern("*/tests/*")
        .include_pattern("src/**/*.rs");

    assert_eq!(config.languages.enabled, vec!["rust", "go"]);
    assert_eq!(config.quality.confidence_threshold, 0.85);
    assert_eq!(config.files.max_files, Some(500));
    assert!(config
        .files
        .exclude_patterns
        .contains(&"*/tests/*".to_string()));
    assert!(config
        .files
        .include_patterns
        .contains(&"src/**/*.rs".to_string()));
}

#[test]
fn test_module_presets() {
    let essential = AnalysisModules::essential();
    assert!(essential.complexity);
    assert!(!essential.dependencies);
    assert!(!essential.duplicates);

    let all = AnalysisModules::all();
    assert!(all.complexity);
    assert!(all.dependencies);
    assert!(all.duplicates);
    assert!(all.refactoring);
    assert!(all.structure);
    assert!(all.coverage);

    let code_quality = AnalysisModules::code_quality();
    assert!(code_quality.complexity);
    assert!(code_quality.duplicates);
    assert!(code_quality.refactoring);
    assert!(!code_quality.dependencies);
}

#[test]
fn test_validation() {
    // Valid config should pass
    let valid_config = AnalysisConfig::default();
    assert!(valid_config.validate().is_ok());

    // Invalid confidence threshold
    let invalid_config = AnalysisConfig::new().with_confidence_threshold(1.5);
    assert!(invalid_config.validate().is_err());

    // No modules enabled should fail
    let no_modules_config = AnalysisConfig::new().disable_all_modules();
    assert!(no_modules_config.validate().is_err());

    // Zero max files should fail
    let zero_files_config = AnalysisConfig::new().files(|f| f.with_max_files(0));
    assert!(zero_files_config.validate().is_err());
}

#[test]
fn test_config_conversion() {
    let original_config = AnalysisConfig::new()
        .with_languages(vec!["python".to_string(), "rust".to_string()])
        .modules(|_| AnalysisModules::code_quality())
        .with_confidence_threshold(0.8)
        .with_max_files(200);

    // Convert to ValknutConfig and back
    let valknut_config = original_config.clone().to_valknut_config();
    let converted_back = AnalysisConfig::from_valknut_config(valknut_config).unwrap();

    // Check that key settings are preserved
    assert_eq!(converted_back.quality.confidence_threshold, 0.8);
    assert_eq!(converted_back.files.max_files, Some(200));
    assert!(converted_back
        .languages
        .enabled
        .contains(&"python".to_string()));
    assert!(converted_back
        .languages
        .enabled
        .contains(&"rust".to_string()));
    assert!(converted_back.modules.complexity);
    assert!(converted_back.modules.duplicates);
    assert!(converted_back.modules.refactoring);
}

#[test]
fn test_serialization() {
    let config = AnalysisConfig::new()
        .with_language("rust")
        .with_confidence_threshold(0.75);

    // Test that it can be serialized and deserialized
    let json = serde_json::to_string(&config).expect("Should serialize");
    let deserialized: AnalysisConfig = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(
        config.quality.confidence_threshold,
        deserialized.quality.confidence_threshold
    );
    assert!(deserialized.languages.enabled.contains(&"rust".to_string()));
}

#[test]
fn test_builder_pattern_immutability() {
    let original = AnalysisConfig::new();
    let modified = original.clone().with_confidence_threshold(0.9);

    // Original should remain unchanged
    assert_eq!(original.quality.confidence_threshold, 0.7);
    assert_eq!(modified.quality.confidence_threshold, 0.9);
}

#[test]
fn test_backward_compatibility() {
    // Test that old-style method calls still work
    let config = AnalysisConfig::new()
        .with_languages(vec!["rust".to_string()])
        .with_confidence_threshold(0.9)
        .with_max_files(500)
        .exclude_pattern("*/tests/*")
        .include_pattern("src/**/*.rs");

    assert_eq!(config.languages.enabled, vec!["rust"]);
    assert_eq!(config.quality.confidence_threshold, 0.9);
    assert_eq!(config.files.max_files, Some(500));
    assert!(config
        .files
        .exclude_patterns
        .contains(&"*/tests/*".to_string()));
    assert!(config
        .files
        .include_patterns
        .contains(&"src/**/*.rs".to_string()));
}

#[test]
fn test_module_convenience_methods() {
    let config = AnalysisConfig::new()
        .enable_all_modules()
        .disable_all_modules()
        .essential_modules_only();

    assert!(config.modules.complexity);
    assert!(!config.modules.dependencies);
    assert!(!config.modules.duplicates);
    assert!(!config.modules.refactoring);
}
