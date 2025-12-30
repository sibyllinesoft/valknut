//! Feature extraction framework and data structures.
//!
//! This module provides the core abstractions for feature extraction in valknut-rs,
//! including feature definitions, extractors, and feature vectors. The design emphasizes
//! performance and type safety while maintaining compatibility with the Python implementation.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::core::errors::{Result, ValknutError};

#[cfg(test)]
#[path = "featureset_tests.rs"]
mod tests;

/// Unique identifier for entities in the system
pub type EntityId = String;

/// Definition of a feature that can be extracted from code entities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeatureDefinition {
    /// Unique name of the feature
    pub name: String,

    /// Human-readable description of what this feature measures
    pub description: String,

    /// Data type of the feature value (for serialization metadata)
    pub data_type: String,

    /// Minimum expected value (for normalization)
    pub min_value: Option<f64>,

    /// Maximum expected value (for normalization)
    pub max_value: Option<f64>,

    /// Default value when feature cannot be computed
    pub default_value: f64,

    /// True if higher values indicate more refactoring need
    pub higher_is_worse: bool,
}

impl FeatureDefinition {
    /// Create a new feature definition
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            data_type: "f64".to_string(),
            min_value: None,
            max_value: None,
            default_value: 0.0,
            higher_is_worse: true,
        }
    }

    /// Set the value range for this feature
    pub fn with_range(mut self, min_value: f64, max_value: f64) -> Self {
        self.min_value = Some(min_value);
        self.max_value = Some(max_value);
        self
    }

    /// Set the default value for this feature
    pub fn with_default(mut self, default_value: f64) -> Self {
        self.default_value = default_value;
        self
    }

    /// Set whether higher values are worse (default: true)
    pub fn with_polarity(mut self, higher_is_worse: bool) -> Self {
        self.higher_is_worse = higher_is_worse;
        self
    }

    /// Check if a value is within the expected range
    pub fn is_valid_value(&self, value: f64) -> bool {
        if value.is_nan() || value.is_infinite() {
            return false;
        }

        if let Some(min) = self.min_value {
            if value < min {
                return false;
            }
        }

        if let Some(max) = self.max_value {
            if value > max {
                return false;
            }
        }

        true
    }

    /// Clamp a value to the valid range
    pub fn clamp_value(&self, value: f64) -> f64 {
        if value.is_nan() || value.is_infinite() {
            return self.default_value;
        }

        let mut clamped = value;

        if let Some(min) = self.min_value {
            if clamped < min {
                clamped = min;
            }
        }

        if let Some(max) = self.max_value {
            if clamped > max {
                clamped = max;
            }
        }

        clamped
    }
}

/// Container for an entity's computed feature vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    /// Unique identifier for the entity
    pub entity_id: EntityId,

    /// Raw feature values as computed by extractors
    pub features: HashMap<String, f64>,

    /// Normalized feature values (after scoring pipeline)
    pub normalized_features: HashMap<String, f64>,

    /// Additional metadata about the entity or extraction process
    pub metadata: HashMap<String, serde_json::Value>,

    /// Refactoring suggestions generated during analysis
    pub refactoring_suggestions: Vec<RefactoringSuggestion>,
}

impl FeatureVector {
    /// Create a new empty feature vector for an entity
    pub fn new(entity_id: impl Into<EntityId>) -> Self {
        Self {
            entity_id: entity_id.into(),
            features: HashMap::new(),
            normalized_features: HashMap::new(),
            metadata: HashMap::new(),
            refactoring_suggestions: Vec::new(),
        }
    }

    /// Add a feature value to the vector
    pub fn add_feature(&mut self, name: impl Into<String>, value: f64) -> &mut Self {
        self.features.insert(name.into(), value);
        self
    }

    /// Get a feature value by name
    pub fn get_feature(&self, name: &str) -> Option<f64> {
        self.features.get(name).copied()
    }

    /// Get a normalized feature value by name
    pub fn get_normalized_feature(&self, name: &str) -> Option<f64> {
        self.normalized_features.get(name).copied()
    }

    /// Add metadata for the entity
    pub fn add_metadata(&mut self, key: impl Into<String>, value: serde_json::Value) -> &mut Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Add a refactoring suggestion
    pub fn add_suggestion(&mut self, suggestion: RefactoringSuggestion) -> &mut Self {
        self.refactoring_suggestions.push(suggestion);
        self
    }

    /// Get the number of features in this vector
    pub fn feature_count(&self) -> usize {
        self.features.len()
    }

    /// Check if the vector contains a specific feature
    pub fn has_feature(&self, name: &str) -> bool {
        self.features.contains_key(name)
    }

    /// Get all feature names
    pub fn feature_names(&self) -> impl Iterator<Item = &String> {
        self.features.keys()
    }

    /// Compute the L2 norm of the feature vector
    pub fn l2_norm(&self) -> f64 {
        self.features.values().map(|v| v * v).sum::<f64>().sqrt()
    }

    /// Compute cosine similarity with another feature vector
    pub fn cosine_similarity(&self, other: &Self) -> f64 {
        let mut dot_product = 0.0;
        let mut norm_self_squared = 0.0;
        let mut norm_other_squared = 0.0;

        // Compute dot product and norms over shared features
        for (name, &value_a) in &self.features {
            norm_self_squared += value_a * value_a;

            if let Some(&value_b) = other.features.get(name) {
                dot_product += value_a * value_b;
            }
        }

        for &value_b in other.features.values() {
            norm_other_squared += value_b * value_b;
        }

        let denominator = (norm_self_squared * norm_other_squared).sqrt();
        if denominator == 0.0 {
            0.0
        } else {
            dot_product / denominator
        }
    }
}

/// Refactoring suggestion with priority and description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringSuggestion {
    /// Type of refactoring suggested
    pub refactoring_type: String,

    /// Human-readable description of the suggestion
    pub description: String,

    /// Priority level (0.0 = low, 1.0 = critical)
    pub priority: f64,

    /// Confidence in the suggestion (0.0 = uncertain, 1.0 = high confidence)
    pub confidence: f64,

    /// Location information (file path, line numbers, etc.)
    pub location: Option<serde_json::Value>,

    /// Additional context or reasoning
    pub context: Option<String>,
}

impl RefactoringSuggestion {
    /// Create a new refactoring suggestion
    pub fn new(
        refactoring_type: impl Into<String>,
        description: impl Into<String>,
        priority: f64,
        confidence: f64,
    ) -> Self {
        Self {
            refactoring_type: refactoring_type.into(),
            description: description.into(),
            priority: priority.clamp(0.0, 1.0),
            confidence: confidence.clamp(0.0, 1.0),
            location: None,
            context: None,
        }
    }

    /// Add location information to the suggestion
    pub fn with_location(mut self, location: serde_json::Value) -> Self {
        self.location = Some(location);
        self
    }

    /// Add context to the suggestion
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Check if this suggestion is high priority
    pub fn is_high_priority(&self) -> bool {
        self.priority >= 0.7
    }

    /// Check if this suggestion is high confidence
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }
}

/// Trait for extracting features from code entities.
///
/// This trait defines the interface for all feature extractors in the system.
/// Extractors are responsible for computing specific features from parsed code entities.
#[async_trait]
pub trait FeatureExtractor: Send + Sync {
    /// Get the name of this extractor
    fn name(&self) -> &str;

    /// Get the list of features this extractor provides
    fn features(&self) -> &[FeatureDefinition];

    /// Extract features from an entity
    async fn extract(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>>;

    /// Check if this extractor supports the given entity type
    fn supports_entity(&self, entity: &CodeEntity) -> bool {
        // Default: support all entities
        true
    }

    /// Get the definition of a specific feature
    fn get_feature_definition(&self, name: &str) -> Option<&FeatureDefinition> {
        self.features().iter().find(|f| f.name == name)
    }

    /// Validate that all feature values are within expected ranges
    fn validate_features(&self, features: &HashMap<String, f64>) -> Result<()> {
        for (name, &value) in features {
            if let Some(definition) = self.get_feature_definition(name) {
                if !definition.is_valid_value(value) {
                    return Err(ValknutError::validation(format!(
                        "Feature '{}' value {} is out of range",
                        name, value
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Simplified entity representation for feature extraction.
/// This will be expanded when we implement the full AST module.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeEntity {
    /// Unique identifier
    pub id: EntityId,

    /// Entity type (function, class, module, etc.)
    pub entity_type: String,

    /// Entity name
    pub name: String,

    /// Source file path
    pub file_path: String,

    /// Line number range
    pub line_range: Option<(usize, usize)>,

    /// Raw source code
    pub source_code: String,

    /// Additional properties
    pub properties: HashMap<String, serde_json::Value>,
}

impl CodeEntity {
    /// Create a new code entity
    pub fn new(
        id: impl Into<EntityId>,
        entity_type: impl Into<String>,
        name: impl Into<String>,
        file_path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            entity_type: entity_type.into(),
            name: name.into(),
            file_path: file_path.into(),
            line_range: None,
            source_code: String::new(),
            properties: HashMap::new(),
        }
    }

    /// Set the line range for this entity
    pub fn with_line_range(mut self, start: usize, end: usize) -> Self {
        self.line_range = Some((start, end));
        self
    }

    /// Set the source code for this entity
    pub fn with_source_code(mut self, source_code: impl Into<String>) -> Self {
        self.source_code = source_code.into();
        self
    }

    /// Add a property to this entity
    pub fn add_property(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.properties.insert(key.into(), value);
    }

    /// Get the number of lines in this entity
    pub fn line_count(&self) -> usize {
        if let Some((start, end)) = self.line_range {
            (end - start).max(1)
        } else {
            self.source_code.lines().count()
        }
    }
}

/// Context provided to feature extractors during extraction
#[derive(Debug)]
pub struct ExtractionContext {
    /// Global configuration
    pub config: Arc<crate::core::config::ValknutConfig>,

    /// Index of all entities for dependency analysis
    pub entity_index: HashMap<EntityId, CodeEntity>,

    /// Language-specific parser information
    pub language: String,

    /// Additional context data
    pub context_data: HashMap<String, serde_json::Value>,

    /// Optional pre-filter of candidate similarity peers per entity
    pub candidate_partitions: Option<Arc<HashMap<EntityId, Vec<EntityId>>>>,
}

impl ExtractionContext {
    /// Create a new extraction context
    pub fn new(
        config: Arc<crate::core::config::ValknutConfig>,
        language: impl Into<String>,
    ) -> Self {
        Self {
            config,
            entity_index: HashMap::new(),
            language: language.into(),
            context_data: HashMap::new(),
            candidate_partitions: None,
        }
    }

    /// Add an entity to the index
    pub fn add_entity(&mut self, entity: CodeEntity) {
        self.entity_index.insert(entity.id.clone(), entity);
    }

    /// Get an entity from the index
    pub fn get_entity(&self, id: &str) -> Option<&CodeEntity> {
        self.entity_index.get(id)
    }

    /// Add context data
    pub fn add_context_data(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.context_data.insert(key.into(), value);
    }

    /// Attach clique partitions for downstream similarity detectors.
    pub fn with_candidate_partitions(
        mut self,
        partitions: Arc<HashMap<EntityId, Vec<EntityId>>>,
    ) -> Self {
        self.candidate_partitions = Some(partitions);
        self
    }
}

/// Base feature extractor with common functionality
pub struct BaseFeatureExtractor {
    /// Name of this extractor
    name: String,

    /// Feature definitions provided by this extractor
    feature_definitions: Vec<FeatureDefinition>,
}

impl BaseFeatureExtractor {
    /// Create a new base feature extractor
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            feature_definitions: Vec::new(),
        }
    }

    /// Add a feature definition to this extractor
    pub fn add_feature(&mut self, definition: FeatureDefinition) {
        self.feature_definitions.push(definition);
    }

    /// Extract a feature value safely with error handling
    pub fn safe_extract<F>(&self, feature_name: &str, extraction_func: F) -> f64
    where
        F: FnOnce() -> Result<f64>,
    {
        match extraction_func() {
            Ok(value) => {
                // Validate and clamp the value
                if let Some(definition) = self.get_feature_definition(feature_name) {
                    definition.clamp_value(value)
                } else {
                    value
                }
            }
            Err(_) => {
                // Return default value on error
                self.get_feature_definition(feature_name)
                    .map(|def| def.default_value)
                    .unwrap_or(0.0)
            }
        }
    }
}

#[async_trait]
impl FeatureExtractor for BaseFeatureExtractor {
    fn name(&self) -> &str {
        &self.name
    }

    fn features(&self) -> &[FeatureDefinition] {
        &self.feature_definitions
    }

    async fn extract(
        &self,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        // Default implementation returns empty features
        Ok(HashMap::new())
    }
}

/// Registry for managing feature extractors
#[derive(Default)]
pub struct FeatureExtractorRegistry {
    /// Registered extractors
    extractors: HashMap<String, Arc<dyn FeatureExtractor>>,

    /// All available feature definitions
    feature_definitions: HashMap<String, FeatureDefinition>,
}

impl FeatureExtractorRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a feature extractor
    pub fn register(&mut self, extractor: Arc<dyn FeatureExtractor>) {
        let name = extractor.name().to_string();

        // Add feature definitions from this extractor
        for feature_def in extractor.features() {
            self.feature_definitions
                .insert(feature_def.name.clone(), feature_def.clone());
        }

        self.extractors.insert(name, extractor);
    }

    /// Get an extractor by name
    pub fn get_extractor(&self, name: &str) -> Option<Arc<dyn FeatureExtractor>> {
        self.extractors.get(name).cloned()
    }

    /// Get all registered extractors
    pub fn get_all_extractors(&self) -> impl Iterator<Item = &Arc<dyn FeatureExtractor>> {
        self.extractors.values()
    }

    /// Get extractors that support a specific entity type
    pub fn get_compatible_extractors(&self, entity: &CodeEntity) -> Vec<Arc<dyn FeatureExtractor>> {
        self.extractors
            .values()
            .filter(|extractor| extractor.supports_entity(entity))
            .cloned()
            .collect()
    }

    /// Get a feature definition by name
    pub fn get_feature_definition(&self, name: &str) -> Option<&FeatureDefinition> {
        self.feature_definitions.get(name)
    }

    /// Get all feature definitions
    pub fn get_all_feature_definitions(&self) -> impl Iterator<Item = &FeatureDefinition> {
        self.feature_definitions.values()
    }

    /// Extract features for an entity using all compatible extractors
    pub async fn extract_all_features(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
    ) -> Result<FeatureVector> {
        let mut feature_vector = FeatureVector::new(entity.id.clone());

        // Get compatible extractors
        let extractors = self.get_compatible_extractors(entity);

        // Extract features from each extractor
        for extractor in extractors {
            match extractor.extract(entity, context).await {
                Ok(features) => {
                    for (name, value) in features {
                        feature_vector.add_feature(name, value);
                    }
                }
                Err(e) => {
                    // Log error but continue with other extractors
                    tracing::warn!(
                        "Feature extraction failed for extractor '{}' on entity '{}': {}",
                        extractor.name(),
                        entity.id,
                        e
                    );
                }
            }
        }

        Ok(feature_vector)
    }
}
