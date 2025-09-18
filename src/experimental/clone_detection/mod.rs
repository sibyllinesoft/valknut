//! Experimental clone detection placeholder implementation.
//!
//! The production clone detector previously shipped in this module never reached feature
//! completeness. The simplified version below keeps the public API compiling while clearly
//! signalling that the behaviour is not ready for general use.

use crate::core::config::DedupeConfig;
use crate::core::errors::Result;
use crate::core::featureset::{CodeEntity, ExtractionContext, FeatureExtractor};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Placeholder result returned by the experimental detector.
#[derive(Debug, Default, Clone)]
pub struct CloneAnalysisResult {
    pub max_similarity: f64,
}

/// Stubbed experimental clone detector.
#[derive(Debug, Default)]
pub struct ComprehensiveCloneDetector {
    _config: DedupeConfig,
}

impl ComprehensiveCloneDetector {
    pub fn new(config: DedupeConfig) -> Self {
        Self { _config: config }
    }

    pub fn with_cache(self, _cache: crate::io::cache::StopMotifCache) -> Self {
        self
    }

    pub async fn analyze_entity_for_clones(
        &self,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<CloneAnalysisResult> {
        Ok(CloneAnalysisResult::default())
    }
}

#[async_trait]
impl FeatureExtractor for ComprehensiveCloneDetector {
    fn name(&self) -> &'static str {
        "experimental_clone_detector"
    }

    fn features(&self) -> &[crate::core::featureset::FeatureDefinition] {
        &[]
    }

    async fn extract(
        &self,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        Ok(HashMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn stub_detector_runs() {
        let detector = ComprehensiveCloneDetector::new(DedupeConfig::default());
        assert_eq!(detector.name(), "experimental_clone_detector");
        assert!(detector.features().is_empty());

        let entity = CodeEntity::new("entity", "function", "example", "file.rs");
        let context = ExtractionContext::new(
            Arc::new(crate::core::config::ValknutConfig::default()),
            "rust",
        );
        let result = detector
            .extract(&entity, &context)
            .await
            .expect("stub should succeed");
        assert!(result.is_empty());
    }
}
