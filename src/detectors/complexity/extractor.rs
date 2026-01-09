//! Feature extractor implementation for AST-based complexity analysis.
//!
//! This module provides the `AstComplexityExtractor` which implements
//! the `FeatureExtractor` trait for complexity-based feature extraction.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tracing::warn;

use super::{AstComplexityAnalyzer, ComplexityAnalysisResult, ComplexityConfig};
use crate::core::ast_service::AstService;
use crate::core::errors::Result;
use crate::core::featureset::{CodeEntity, ExtractionContext, FeatureDefinition, FeatureExtractor};
use crate::core::file_utils::ranges_overlap;

/// Feature extractor implementation for AST-based complexity
pub struct AstComplexityExtractor {
    analyzer: AstComplexityAnalyzer,
    feature_definitions: Vec<FeatureDefinition>,
    analysis_cache: DashMap<String, Arc<Vec<ComplexityAnalysisResult>>>,
}

/// Factory, caching, and analysis methods for [`AstComplexityExtractor`].
impl AstComplexityExtractor {
    /// Creates a new AST complexity extractor with the given configuration.
    pub fn new(config: ComplexityConfig, ast_service: Arc<AstService>) -> Self {
        let feature_definitions = vec![
            FeatureDefinition::new("cyclomatic_complexity", "McCabe cyclomatic complexity")
                .with_range(1.0, 50.0)
                .with_default(1.0)
                .with_polarity(true),
            FeatureDefinition::new("cognitive_complexity", "Cognitive complexity with nesting")
                .with_range(0.0, 100.0)
                .with_default(0.0)
                .with_polarity(true),
            FeatureDefinition::new("nesting_depth", "Maximum nesting depth")
                .with_range(0.0, 10.0)
                .with_default(0.0)
                .with_polarity(true),
            FeatureDefinition::new("parameter_count", "Number of function parameters")
                .with_range(0.0, 20.0)
                .with_default(0.0)
                .with_polarity(true),
            FeatureDefinition::new("lines_of_code", "Lines of code")
                .with_range(1.0, 1000.0)
                .with_default(1.0)
                .with_polarity(true),
        ];

        Self {
            analyzer: AstComplexityAnalyzer::new(config, ast_service),
            feature_definitions,
            analysis_cache: DashMap::new(),
        }
    }

    /// Get or compute complexity results for a file (cached).
    async fn file_results(&self, file_path: &str) -> Result<Arc<Vec<ComplexityAnalysisResult>>> {
        let key = file_path.to_owned();

        if let Some(entry) = self.analysis_cache.get(&key) {
            return Ok(entry.clone());
        }

        let source = match tokio::fs::read_to_string(file_path).await {
            Ok(contents) => contents,
            Err(error) => {
                warn!(
                    "Complexity extractor failed to read {}: {}",
                    file_path, error
                );
                let empty = Arc::new(Vec::new());
                self.analysis_cache.insert(key, empty.clone());
                return Ok(empty);
            }
        };

        match self
            .analyzer
            .analyze_file_with_results(file_path, &source)
            .await
        {
            Ok(results) => {
                let arc = Arc::new(results);
                self.analysis_cache.insert(key, arc.clone());
                Ok(arc)
            }
            Err(error) => {
                warn!(
                    "Complexity extractor failed to analyze {}: {}",
                    file_path, error
                );
                let empty = Arc::new(Vec::new());
                self.analysis_cache.insert(key, empty.clone());
                Ok(empty)
            }
        }
    }

    /// Initialize a feature map with default values.
    fn initialise_feature_map(&self) -> HashMap<String, f64> {
        let mut map = HashMap::with_capacity(self.feature_definitions.len());
        for definition in &self.feature_definitions {
            map.insert(definition.name.clone(), definition.default_value);
        }
        map
    }
}

/// [`FeatureExtractor`] implementation for AST-based complexity analysis.
#[async_trait]
impl FeatureExtractor for AstComplexityExtractor {
    /// Returns the extractor name ("ast_complexity").
    fn name(&self) -> &str {
        "ast_complexity"
    }

    /// Returns the complexity feature definitions.
    fn features(&self) -> &[FeatureDefinition] {
        &self.feature_definitions
    }

    /// Extracts complexity features for an entity from AST analysis.
    async fn extract(
        &self,
        entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = self.initialise_feature_map();
        let results = self.file_results(&entity.file_path).await?;
        let relevant = find_relevant_results(entity, &results);

        if !relevant.is_empty() {
            aggregate_metrics_into_features(&relevant, &mut features);
        }

        ensure_loc_value(&mut features, entity);
        Ok(features)
    }
}

/// Find complexity results relevant to the given entity.
fn find_relevant_results<'a>(
    entity: &CodeEntity,
    results: &'a [ComplexityAnalysisResult],
) -> Vec<&'a ComplexityAnalysisResult> {
    let entity_range = entity.line_range.unwrap_or_else(|| {
        let lines = entity.line_count().max(1);
        (1, lines)
    });

    let mut relevant: Vec<&ComplexityAnalysisResult> = results
        .iter()
        .filter(|result| {
            result.entity_id == entity.id
                || (result.entity_name == entity.name && result.file_path == entity.file_path)
                || ranges_overlap(entity_range, result_line_range(result))
        })
        .collect();

    // Fallback: use highest complexity result if no match found
    if relevant.is_empty() && !results.is_empty() {
        if let Some(worst) = results.iter().max_by(|a, b| {
            a.metrics
                .cyclomatic_complexity
                .partial_cmp(&b.metrics.cyclomatic_complexity)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            relevant.push(worst);
        }
    }

    relevant
}

/// Aggregate maximum metrics from relevant results into the feature map.
fn aggregate_metrics_into_features(
    relevant: &[&ComplexityAnalysisResult],
    features: &mut HashMap<String, f64>,
) {
    let (mut cyclomatic, mut cognitive, mut nesting, mut parameters, mut loc) =
        (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);

    for result in relevant {
        let m = &result.metrics;
        cyclomatic = cyclomatic.max(m.cyclomatic_complexity);
        cognitive = cognitive.max(m.cognitive_complexity);
        nesting = nesting.max(m.max_nesting_depth);
        parameters = parameters.max(m.parameter_count);
        loc = loc.max(m.lines_of_code);
    }

    features.insert("cyclomatic_complexity".to_string(), cyclomatic);
    features.insert("cognitive_complexity".to_string(), cognitive);
    features.insert("nesting_depth".to_string(), nesting);
    features.insert("parameter_count".to_string(), parameters);
    if loc > 0.0 {
        features.insert("lines_of_code".to_string(), loc);
    }
}

/// Ensure a lines_of_code value exists, computing from entity if needed.
fn ensure_loc_value(features: &mut HashMap<String, f64>, entity: &CodeEntity) {
    features.entry("lines_of_code".to_string()).or_insert_with(|| {
        entity
            .line_range
            .map(|(start, end)| {
                if end >= start {
                    (end - start + 1) as f64
                } else {
                    entity.line_count() as f64
                }
            })
            .unwrap_or_else(|| entity.line_count() as f64)
    });
}

/// Compute line range from a complexity analysis result.
fn result_line_range(result: &ComplexityAnalysisResult) -> (usize, usize) {
    let start = result.start_line.max(1);
    let span = result.metrics.lines_of_code.max(1.0) as usize;
    let end = start + span.saturating_sub(1);
    (start, end)
}


