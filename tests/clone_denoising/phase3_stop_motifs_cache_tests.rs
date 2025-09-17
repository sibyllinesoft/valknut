//! Simplified Phase 3 Tests: Stop-Motifs Cache System
//!
//! Tests basic cache functionality with current API

use tempfile::TempDir;

use valknut_rs::io::cache::{CacheRefreshPolicy, StopMotifCacheManager};

#[cfg(test)]
mod simplified_cache_tests {
    use super::*;

    /// Test basic cache policy creation
    #[test]
    fn test_cache_refresh_policy_creation() {
        let policy = CacheRefreshPolicy {
            max_age_days: 7,
            change_threshold_percent: 5.0,
            stop_motif_percentile: 95.0,
            weight_multiplier: 1.5,
            k_gram_size: 4,
        };

        assert_eq!(policy.max_age_days, 7);
        assert_eq!(policy.change_threshold_percent, 5.0);
        assert_eq!(policy.stop_motif_percentile, 95.0);
        assert_eq!(policy.weight_multiplier, 1.5);
        assert_eq!(policy.k_gram_size, 4);
    }

    /// Test cache manager creation with temporary directory
    #[test]
    fn test_stop_motif_cache_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache");

        let refresh_policy = CacheRefreshPolicy {
            max_age_days: 1,
            change_threshold_percent: 10.0,
            stop_motif_percentile: 90.0,
            weight_multiplier: 1.0,
            k_gram_size: 3,
        };

        let _cache_manager = StopMotifCacheManager::new(cache_path, refresh_policy);

        // Basic validation - if we get here, creation succeeded
    }

    /// Test different cache refresh policies
    #[test]
    fn test_different_cache_policies() {
        let conservative_policy = CacheRefreshPolicy {
            max_age_days: 30,               // Long cache duration
            change_threshold_percent: 20.0, // High threshold
            stop_motif_percentile: 99.0,    // Very selective
            weight_multiplier: 0.5,         // Lower weight
            k_gram_size: 5,                 // Larger k-grams
        };

        let aggressive_policy = CacheRefreshPolicy {
            max_age_days: 1,               // Short cache duration
            change_threshold_percent: 1.0, // Low threshold
            stop_motif_percentile: 80.0,   // Less selective
            weight_multiplier: 2.0,        // Higher weight
            k_gram_size: 2,                // Smaller k-grams
        };

        // Validate conservative policy
        assert!(conservative_policy.max_age_days > aggressive_policy.max_age_days);
        assert!(
            conservative_policy.change_threshold_percent
                > aggressive_policy.change_threshold_percent
        );
        assert!(
            conservative_policy.stop_motif_percentile > aggressive_policy.stop_motif_percentile
        );

        // Validate aggressive policy
        assert!(aggressive_policy.weight_multiplier > conservative_policy.weight_multiplier);
        assert!(aggressive_policy.k_gram_size < conservative_policy.k_gram_size);
    }
}
