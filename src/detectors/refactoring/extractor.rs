//! Feature extractor implementation for refactoring analysis.
//!
//! This module provides the `RefactoringExtractor` which implements
//! the `FeatureExtractor` trait for refactoring-based feature extraction.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tracing::warn;

use super::{RefactoringAnalysisResult, RefactoringAnalyzer, RefactoringConfig, RefactoringType};
use crate::core::ast_service::AstService;
use crate::core::errors::Result;
use crate::core::featureset::{CodeEntity, ExtractionContext, FeatureDefinition, FeatureExtractor};
use crate::core::file_utils::ranges_overlap;

/// Feature extractor for refactoring analysis with file-level caching.
pub struct RefactoringExtractor {
    analyzer: Arc<RefactoringAnalyzer>,
    feature_definitions: Vec<FeatureDefinition>,
    file_cache: DashMap<String, Arc<RefactoringAnalysisResult>>,
}

/// Factory, caching, and configuration methods for [`RefactoringExtractor`].
impl RefactoringExtractor {
    /// Create a refactoring extractor backed by the provided analyzer
    pub fn new(analyzer: RefactoringAnalyzer) -> Self {
        let feature_definitions = vec![
            FeatureDefinition::new(
                "refactoring_recommendation_count",
                "Number of refactoring opportunities detected for this entity",
            )
            .with_range(0.0, 50.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_total_impact",
                "Sum of estimated impact values for matching refactoring recommendations",
            )
            .with_range(0.0, 200.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_avg_impact",
                "Average estimated impact for matching refactoring recommendations",
            )
            .with_range(0.0, 10.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_avg_priority",
                "Average priority score for matching refactoring recommendations",
            )
            .with_range(0.0, 10.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_max_priority",
                "Highest priority score among matching refactoring recommendations",
            )
            .with_range(0.0, 10.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_file_score",
                "Overall refactoring score for the containing file",
            )
            .with_range(0.0, 100.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_extract_method_count",
                "Occurrences of extract-method opportunities",
            )
            .with_range(0.0, 50.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_extract_class_count",
                "Occurrences of extract-class opportunities",
            )
            .with_range(0.0, 50.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_duplicate_code_count",
                "Occurrences of duplicate-code elimination opportunities",
            )
            .with_range(0.0, 50.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_simplify_conditionals_count",
                "Occurrences of complex conditional simplification opportunities",
            )
            .with_range(0.0, 50.0)
            .with_default(0.0),
        ];

        Self {
            analyzer: Arc::new(analyzer),
            feature_definitions,
            file_cache: DashMap::new(),
        }
    }

    /// Construct an extractor with explicit configuration and AST service
    pub fn with_config(config: RefactoringConfig, ast_service: Arc<AstService>) -> Self {
        Self::new(RefactoringAnalyzer::new(config, ast_service))
    }

    /// Fetch (and cache) the refactoring analysis for a file
    async fn file_analysis(&self, file_path: &str) -> Result<Arc<RefactoringAnalysisResult>> {
        let key = file_path.to_owned();

        if let Some(entry) = self.file_cache.get(&key) {
            return Ok(entry.clone());
        }

        let path = PathBuf::from(file_path);
        match self.analyzer.analyze_file(&path).await {
            Ok(result) => {
                let arc = Arc::new(result);
                self.file_cache.insert(key, arc.clone());
                Ok(arc)
            }
            Err(error) => {
                warn!(
                    "Refactoring extractor failed to analyze {}: {}",
                    file_path, error
                );
                let placeholder = Arc::new(RefactoringAnalysisResult {
                    file_path: file_path.to_string(),
                    recommendations: Vec::new(),
                    refactoring_score: 0.0,
                });
                self.file_cache.insert(key, placeholder.clone());
                Ok(placeholder)
            }
        }
    }

    /// Initialise the feature vector with configured defaults
    fn initialise_feature_map(&self) -> HashMap<String, f64> {
        let mut map = HashMap::with_capacity(self.feature_definitions.len());
        for definition in &self.feature_definitions {
            map.insert(definition.name.clone(), definition.default_value);
        }
        map
    }
}

/// Default implementation for [`RefactoringExtractor`].
impl Default for RefactoringExtractor {
    /// Returns an extractor with default analyzer configuration.
    fn default() -> Self {
        Self::new(RefactoringAnalyzer::default())
    }
}

/// [`FeatureExtractor`] implementation for refactoring analysis.
#[async_trait]
impl FeatureExtractor for RefactoringExtractor {
    /// Returns the extractor name ("refactoring").
    fn name(&self) -> &str {
        "refactoring"
    }
    /// Returns the refactoring feature definitions.
    fn features(&self) -> &[FeatureDefinition] {
        &self.feature_definitions
    }
    /// Extracts refactoring features for an entity.
    async fn extract(
        &self,
        entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = self.initialise_feature_map();

        // Attempt to load analysis for the containing file
        let analysis = self.file_analysis(&entity.file_path).await?;

        let entity_range = entity.line_range.unwrap_or_else(|| {
            let lines = entity.line_count().max(1);
            (1, lines)
        });

        let mut total_impact = 0.0_f64;
        let mut total_priority = 0.0_f64;
        let mut recommendations_considered = 0.0_f64;
        let mut max_priority = 0.0_f64;
        let mut extract_method = 0.0_f64;
        let mut extract_class = 0.0_f64;
        let mut eliminate_duplication = 0.0_f64;
        let mut simplify_conditionals = 0.0_f64;

        for recommendation in &analysis.recommendations {
            let location = recommendation.location;
            if !ranges_overlap(entity_range, location) {
                continue;
            }

            recommendations_considered += 1.0;
            total_impact += recommendation.estimated_impact;
            total_priority += recommendation.priority_score;
            max_priority = max_priority.max(recommendation.priority_score);

            match recommendation.refactoring_type {
                RefactoringType::ExtractMethod => extract_method += 1.0,
                RefactoringType::ExtractClass => extract_class += 1.0,
                RefactoringType::EliminateDuplication => {
                    eliminate_duplication += 1.0;
                }
                RefactoringType::SimplifyConditionals => simplify_conditionals += 1.0,
                RefactoringType::ReduceComplexity
                | RefactoringType::ImproveNaming
                | RefactoringType::RemoveDeadCode => {
                    // Keep hook for future detailed features
                }
            }
        }

        if recommendations_considered > 0.0 {
            let avg_impact = total_impact / recommendations_considered;
            let avg_priority = total_priority / recommendations_considered;

            features.insert(
                "refactoring_recommendation_count".to_string(),
                recommendations_considered,
            );
            features.insert("refactoring_total_impact".to_string(), total_impact);
            features.insert("refactoring_avg_impact".to_string(), avg_impact);
            features.insert("refactoring_avg_priority".to_string(), avg_priority);
            features.insert("refactoring_max_priority".to_string(), max_priority);
            features.insert(
                "refactoring_extract_method_count".to_string(),
                extract_method,
            );
            features.insert("refactoring_extract_class_count".to_string(), extract_class);
            features.insert(
                "refactoring_duplicate_code_count".to_string(),
                eliminate_duplication,
            );
            features.insert(
                "refactoring_simplify_conditionals_count".to_string(),
                simplify_conditionals,
            );
        }

        // Propagate the file-level refactoring score regardless of overlap results
        features.insert(
            "refactoring_file_score".to_string(),
            analysis.refactoring_score,
        );

        Ok(features)
    }
}


