//! Experimental boilerplate learning placeholder implementation.
//!
//! The original self-learning system shipped with the previous prototype relied on
//! incomplete heuristics and a large number of unimplemented language hooks. To keep the
//! experimental API compile-ready while signalling unfinished work, this stub exposes the
//! public surface without providing behaviour.

use crate::core::errors::Result;
use std::path::Path;

/// Minimal configuration placeholder for the experimental boilerplate module.
#[derive(Debug, Clone, Default)]
pub struct BoilerplateLearningConfig {}

/// Placeholder learning report returned by the experimental system.
#[derive(Debug, Clone, Default)]
pub struct LearningReport {}

/// Placeholder motif type used by experimental APIs.
#[derive(Debug, Clone, Default)]
pub struct ExperimentalMotif;

/// Experimental boilerplate learning system stub.
#[derive(Debug, Default)]
pub struct BoilerplateLearningSystem;

impl BoilerplateLearningSystem {
    pub fn new(_config: BoilerplateLearningConfig) -> Self {
        Self
    }

    pub async fn learn_from_codebase(&mut self, _codebase_path: &Path) -> Result<LearningReport> {
        Ok(LearningReport::default())
    }

    pub fn needs_refresh(&self) -> bool {
        false
    }

    pub fn get_shingle_weight(&self, _shingle: &str) -> f64 {
        1.0
    }

    pub fn get_motif_weight(&self, _motif: &ExperimentalMotif) -> f64 {
        1.0
    }

    pub fn is_hub_pattern(&self, _pattern: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_learning_completes() {
        let mut system = BoilerplateLearningSystem::new(BoilerplateLearningConfig::default());
        let report = system
            .learn_from_codebase(Path::new("."))
            .await
            .expect("stub should succeed");
        let _ = report;
    }
}
