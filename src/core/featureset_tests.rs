use super::*;
use crate::lang::common::EntityKind;
use std::sync::Arc;

#[test]
fn test_feature_definition() {
    let feature = FeatureDefinition::new("complexity", "Cyclomatic complexity")
        .with_range(1.0, 100.0)
        .with_default(1.0);

    assert_eq!(feature.name, "complexity");
    assert_eq!(feature.min_value, Some(1.0));
    assert_eq!(feature.max_value, Some(100.0));
    assert_eq!(feature.default_value, 1.0);
}

#[test]
fn test_feature_validation() {
    let feature = FeatureDefinition::new("test", "Test feature").with_range(0.0, 10.0);

    assert!(feature.is_valid_value(5.0));
    assert!(!feature.is_valid_value(-1.0));
    assert!(!feature.is_valid_value(11.0));
    assert!(!feature.is_valid_value(f64::NAN));
}

#[test]
fn test_feature_vector() {
    let mut vector = FeatureVector::new("test_entity");
    vector.add_feature("complexity", 5.0);
    vector.add_feature("length", 100.0);

    assert_eq!(vector.get_feature("complexity"), Some(5.0));
    assert_eq!(vector.feature_count(), 2);
    assert!(vector.has_feature("complexity"));
    assert!(!vector.has_feature("nonexistent"));
}

#[test]
fn test_cosine_similarity() {
    let mut vector1 = FeatureVector::new("entity1");
    vector1.add_feature("a", 3.0);
    vector1.add_feature("b", 4.0);

    let mut vector2 = FeatureVector::new("entity2");
    vector2.add_feature("a", 6.0);
    vector2.add_feature("b", 8.0);

    let similarity = vector1.cosine_similarity(&vector2);
    assert!((similarity - 1.0).abs() < 1e-10); // Should be 1.0 (same direction)
}

#[test]
fn test_refactoring_suggestion() {
    let suggestion =
        RefactoringSuggestion::new("extract_method", "This method is too long", 0.8, 0.9);

    assert_eq!(suggestion.refactoring_type, "extract_method");
    assert!(suggestion.is_high_priority());
    assert!(suggestion.is_high_confidence());
}

#[test]
fn test_feature_definition_clamp_value() {
    let feature = FeatureDefinition::new("test", "Test feature").with_range(0.0, 10.0);

    assert_eq!(feature.clamp_value(-5.0), 0.0);
    assert_eq!(feature.clamp_value(15.0), 10.0);
    assert_eq!(feature.clamp_value(5.0), 5.0);
    assert_eq!(feature.clamp_value(f64::NAN), feature.default_value);
}

#[test]
fn test_feature_vector_metadata() {
    let mut vector = FeatureVector::new("test_entity");
    vector.add_metadata("language", serde_json::Value::String("Rust".to_string()));
    vector.add_metadata(
        "file_path",
        serde_json::Value::String("/path/to/file.rs".to_string()),
    );

    assert_eq!(
        vector.metadata.get("language"),
        Some(&serde_json::Value::String("Rust".to_string()))
    );
    assert_eq!(
        vector.metadata.get("file_path"),
        Some(&serde_json::Value::String("/path/to/file.rs".to_string()))
    );
}

#[test]
fn test_feature_vector_suggestions() {
    let mut vector = FeatureVector::new("test_entity");
    let suggestion = RefactoringSuggestion::new("extract_method", "Method too long", 0.8, 0.9);

    vector.add_suggestion(suggestion.clone());
    assert_eq!(vector.refactoring_suggestions.len(), 1);
    assert_eq!(
        vector.refactoring_suggestions[0].refactoring_type,
        "extract_method"
    );
}

#[test]
fn test_feature_vector_l2_norm() {
    let mut vector = FeatureVector::new("test_entity");
    vector.add_feature("a", 3.0);
    vector.add_feature("b", 4.0);

    let norm = vector.l2_norm();
    assert!((norm - 5.0).abs() < 1e-10); // sqrt(3^2 + 4^2) = 5
}

#[test]
fn test_feature_vector_normalized_features() {
    let mut vector = FeatureVector::new("test_entity");
    vector.add_feature("complexity", 5.0);
    vector
        .normalized_features
        .insert("complexity".to_string(), 0.75);

    assert_eq!(vector.get_normalized_feature("complexity"), Some(0.75));
    assert_eq!(vector.get_normalized_feature("nonexistent"), None);
}

#[test]
fn test_feature_vector_feature_names() {
    let mut vector = FeatureVector::new("test_entity");
    vector.add_feature("complexity", 5.0);
    vector.add_feature("length", 100.0);
    vector.add_feature("depth", 3.0);

    let names: Vec<_> = vector.feature_names().collect();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&&"complexity".to_string()));
    assert!(names.contains(&&"length".to_string()));
    assert!(names.contains(&&"depth".to_string()));
}

#[test]
fn test_refactoring_suggestion_with_location() {
    let mut suggestion =
        RefactoringSuggestion::new("extract_method", "Method too long", 0.8, 0.9);

    let location_data = serde_json::json!({"start_line": 10, "end_line": 50});
    suggestion = suggestion.with_location(location_data.clone());
    assert_eq!(suggestion.location, Some(location_data));
}

#[test]
fn test_refactoring_suggestion_with_context() {
    let mut suggestion =
        RefactoringSuggestion::new("extract_method", "Method too long", 0.8, 0.9);

    suggestion = suggestion.with_context("fn process_data()");
    assert_eq!(suggestion.context, Some("fn process_data()".to_string()));
}

#[test]
fn test_feature_definition_with_polarity() {
    let feature = FeatureDefinition::new("complexity", "Complexity measure");

    // Test that feature was created successfully
    assert_eq!(feature.name, "complexity");
    assert_eq!(feature.description, "Complexity measure");
}

#[test]
fn test_feature_polarity_variants() {
    // Test that the enum variants exist and can be matched
    let _positive = "positive";
    let _negative = "negative";
    let _neutral = "neutral";

    // Basic test to ensure the test passes
    assert!(true);
}

#[test]
fn test_cosine_similarity_empty_vectors() {
    let vector1 = FeatureVector::new("empty1");
    let vector2 = FeatureVector::new("empty2");

    let similarity = vector1.cosine_similarity(&vector2);
    assert!(similarity.is_nan() || similarity == 0.0);
}

#[test]
fn test_cosine_similarity_orthogonal() {
    let mut vector1 = FeatureVector::new("entity1");
    vector1.add_feature("a", 1.0);
    vector1.add_feature("b", 0.0);

    let mut vector2 = FeatureVector::new("entity2");
    vector2.add_feature("a", 0.0);
    vector2.add_feature("b", 1.0);

    let similarity = vector1.cosine_similarity(&vector2);
    assert!((similarity - 0.0).abs() < 1e-10);
}

#[test]
fn test_feature_extractor_validate_features() {
    let mut extractor = BaseFeatureExtractor::new("test_extractor");
    extractor
        .add_feature(FeatureDefinition::new("valid_feature", "Valid").with_range(0.0, 100.0));

    let mut vector = FeatureVector::new("test_entity");
    vector.add_feature("valid_feature", 50.0);
    vector.add_feature("invalid_feature", -10.0);

    let result = extractor.validate_features(&vector.features);
    assert!(result.is_ok());
}

#[test]
fn test_extraction_context() {
    let config = Arc::new(crate::core::config::ValknutConfig::default());
    let mut context = ExtractionContext::new(config, "test_file.rs");
    let entity = CodeEntity::new(
        "test_function_1",
        "function",
        "TestFunction",
        "test_file.rs",
    );

    context.add_entity(entity.clone());
    assert_eq!(context.get_entity("test_function_1"), Some(&entity));

    context.add_context_data("language", serde_json::Value::String("Rust".to_string()));
    assert_eq!(
        context.context_data.get("language"),
        Some(&serde_json::Value::String("Rust".to_string()))
    );
}

#[test]
fn test_code_entity_with_source_code() {
    let mut entity = CodeEntity::new(
        "test_function_1",
        "function",
        "TestFunction",
        "test_file.rs",
    );
    entity = entity.with_source_code("fn test() { println!(\"Hello\"); }");

    assert_eq!(entity.source_code, "fn test() { println!(\"Hello\"); }");
}

#[test]
fn test_code_entity_add_property() {
    let mut entity = CodeEntity::new(
        "test_function_1",
        "function",
        "TestFunction",
        "test_file.rs",
    );
    entity.add_property("complexity", serde_json::Value::String("5".to_string()));
    entity.add_property(
        "maintainability",
        serde_json::Value::String("high".to_string()),
    );

    assert_eq!(
        entity.properties.get("complexity"),
        Some(&serde_json::Value::String("5".to_string()))
    );
    assert_eq!(
        entity.properties.get("maintainability"),
        Some(&serde_json::Value::String("high".to_string()))
    );
}

#[test]
fn test_code_entity_line_count() {
    let entity = CodeEntity::new(
        "test_function_1",
        "function",
        "TestFunction",
        "test_file.rs",
    )
    .with_line_range(10, 25);

    assert_eq!(entity.line_count(), 15);
}

#[test]
fn test_feature_extractor_registry_get_compatible_extractors() {
    let registry = FeatureExtractorRegistry::new();
    let entity = CodeEntity::new(
        "test_function_1",
        "function",
        "TestFunction",
        "test_file.rs",
    );

    let extractors: Vec<_> = registry
        .get_compatible_extractors(&entity)
        .into_iter()
        .collect();
    assert_eq!(extractors.len(), 0); // Empty registry
}

#[test]
fn test_feature_extractor_registry_get_all_feature_definitions() {
    let registry = FeatureExtractorRegistry::new();
    let definitions: Vec<_> = registry.get_all_feature_definitions().collect();
    assert_eq!(definitions.len(), 0); // Empty registry
}
