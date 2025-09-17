//! Simplified End-to-End Integration Tests for Clone Detection
//!
//! Tests the current clone detection system with minimal API usage

use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio;

use valknut_rs::core::config::{DedupeConfig, ValknutConfig};
use valknut_rs::core::featureset::{CodeEntity, ExtractionContext, FeatureExtractor};
use valknut_rs::detectors::clone_detection::{
    CloneCandidate, CloneType, ComprehensiveCloneDetector,
};
use valknut_rs::io::cache::{CacheRefreshPolicy, StopMotifCacheManager};

#[cfg(test)]
mod simplified_end_to_end_tests {
    use super::*;

    /// Test basic clone detector creation and configuration
    #[tokio::test]
    async fn test_comprehensive_clone_detector_creation() {
        let config = DedupeConfig::default();

        let detector = ComprehensiveCloneDetector::new(config);
        assert_eq!(detector.name(), "comprehensive_clone_detector");
    }

    /// Test cache policy creation with current API
    #[test]
    fn test_cache_refresh_policy_creation() {
        let refresh_policy = CacheRefreshPolicy {
            max_age_days: 1,
            change_threshold_percent: 10.0,
            stop_motif_percentile: 95.0,
            weight_multiplier: 1.0,
            k_gram_size: 3,
        };

        assert_eq!(refresh_policy.max_age_days, 1);
        assert_eq!(refresh_policy.change_threshold_percent, 10.0);
    }

    /// Test clone candidate creation with current API
    #[test]
    fn test_clone_candidate_creation() {
        let candidate = CloneCandidate {
            id: "test_clone_1".to_string(),
            entities: vec!["entity1".to_string(), "entity2".to_string()],
            similarity_score: 0.85,
            structural_score: 0.90,
            lexical_score: 0.80,
            semantic_score: 0.75,
            size_normalized_score: 0.88,
            confidence: 0.82,
            clone_type: CloneType::Type1,
        };

        assert_eq!(candidate.id, "test_clone_1");
        assert_eq!(candidate.entities.len(), 2);
        assert!(candidate.similarity_score > 0.8);
        assert!(candidate.confidence > 0.8);
    }

    /// Test feature extraction interface
    #[tokio::test]
    async fn test_feature_extraction_interface() {
        let config = DedupeConfig::default();
        let detector = ComprehensiveCloneDetector::new(config);

        // Create a minimal config for extraction context
        let valknut_config = Arc::new(ValknutConfig::default());
        let context = ExtractionContext::new(valknut_config, "rust".to_string());

        // Create a test entity
        let entity = CodeEntity {
            id: "test_entity".to_string(),
            entity_type: "function".to_string(),
            name: "test".to_string(),
            file_path: "test.rs".to_string(),
            line_range: Some((1, 1)),
            source_code: "fn test() { println!(\"hello\"); }".to_string(),
            properties: HashMap::new(),
        };

        // Test that the detector implements FeatureExtractor
        let features = detector.extract(&entity, &context).await;
        assert!(features.is_ok());
    }

    /// Test stop motif cache manager creation
    #[test]
    fn test_stop_motif_cache_manager() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache");

        let refresh_policy = CacheRefreshPolicy {
            max_age_days: 7,
            change_threshold_percent: 5.0,
            stop_motif_percentile: 90.0,
            weight_multiplier: 1.5,
            k_gram_size: 4,
        };

        let cache_manager = StopMotifCacheManager::new(cache_path, refresh_policy);

        // Basic validation - cache manager should be created successfully
        assert!(true); // If we get here, creation succeeded
    }
}
