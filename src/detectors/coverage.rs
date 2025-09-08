//! Coverage analysis detector - placeholder implementation.

use std::collections::HashMap;
use async_trait::async_trait;
use crate::core::featureset::{FeatureExtractor, FeatureDefinition, CodeEntity, ExtractionContext};
use crate::core::errors::Result;

#[derive(Debug, Default)]
pub struct CoverageExtractor;

#[async_trait]
impl FeatureExtractor for CoverageExtractor {
    fn name(&self) -> &str { "coverage" }
    fn features(&self) -> &[FeatureDefinition] { &[] }
    async fn extract(&self, _entity: &CodeEntity, _context: &ExtractionContext) -> Result<HashMap<String, f64>> {
        Ok(HashMap::new())
    }
}