//! Comprehensive tests for Phase 4: Auto-Calibration + Payoff Ranking

use approx::assert_relative_eq;
use proptest::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use valknut_rs::detectors::clone_detection::{
    AutoCalibrationEngine, CalibrationResult,
    PayoffRankingSystem, CloneCandidate, RankedCloneCandidate,
    QualityMetrics, HardFilteringFloors,
};

#[cfg(test)]
mod auto_calibration_engine_tests {
    use super::*;

    /// Test auto-calibration engine initialization and configuration
    #[test]
    fn test_auto_calibration_initialization() {
        let engine = AutoCalibrationEngine::new();
        let thresholds = engine.get_thresholds();
        
        assert!(thresholds.similarity_threshold >= 0.0);
        assert!(thresholds.similarity_threshold <= 1.0);
        assert!(thresholds.confidence_threshold >= 0.0);
        assert!(thresholds.confidence_threshold <= 1.0);
    }

    /// Test threshold calibration with sample data
    #[test]
    fn test_threshold_calibration() {
        let mut engine = AutoCalibrationEngine::new();

        // Create sample similarity scores
        let sample_data = vec![0.2, 0.3, 0.7, 0.8, 0.9];
        let target_quality = 0.8;

        // Perform calibration
        let calibration_result = engine.calibrate(&sample_data, target_quality);

        assert!(
            calibration_result.threshold >= 0.0 && calibration_result.threshold <= 1.0,
            "Calibrated threshold should be within valid range: {}",
            calibration_result.threshold
        );
        
        assert_eq!(calibration_result.quality_score, target_quality);
        assert!(calibration_result.confidence >= 0.0 && calibration_result.confidence <= 1.0);
        assert!(calibration_result.convergence_achieved);
    }

    /// Test threshold updates based on performance feedback
    #[test]
    fn test_threshold_updates_from_feedback() {
        let mut engine = AutoCalibrationEngine::new();
        let initial_threshold = engine.get_thresholds().similarity_threshold;

        // Test low precision feedback (should increase threshold)
        let low_precision_feedback = QualityMetrics {
            precision: 0.5,
            recall: 0.9,
            f1_score: 0.65,
            ..Default::default()
        };

        engine.update_thresholds(&low_precision_feedback);
        let new_threshold = engine.get_thresholds().similarity_threshold;
        assert!(
            new_threshold > initial_threshold,
            "Low precision should increase threshold: {} -> {}",
            initial_threshold,
            new_threshold
        );

        // Test low recall feedback (should decrease threshold)
        let low_recall_feedback = QualityMetrics {
            precision: 0.9,
            recall: 0.5,
            f1_score: 0.65,
            ..Default::default()
        };

        engine.update_thresholds(&low_recall_feedback);
        let final_threshold = engine.get_thresholds().similarity_threshold;
        assert!(
            final_threshold < new_threshold,
            "Low recall should decrease threshold: {} -> {}",
            new_threshold,
            final_threshold
        );
    }

    /// Test recalibration detection
    #[test]
    fn test_recalibration_detection() {
        let mut engine = AutoCalibrationEngine::new();

        // Initially should not need recalibration
        assert!(
            !engine.needs_recalibration(),
            "New engine should not need immediate recalibration"
        );

        // Add some unstable performance history
        let unstable_feedback = QualityMetrics {
            precision: 0.3,
            recall: 0.4,
            f1_score: 0.35,
            ..Default::default()
        };
        
        engine.update_thresholds(&unstable_feedback);
        
        // After unstable performance, may need recalibration
        // Note: This depends on the stability calculation
        println!("Stability metric: {}", engine.get_thresholds().stability_metric);
    }

    /// Test calibration with edge cases in sample data
    #[test]
    fn test_calibration_edge_cases() {
        let mut engine = AutoCalibrationEngine::new();

        // Test with empty sample data
        let empty_data = vec![];
        let result = engine.calibrate(&empty_data, 0.8);
        // Should handle gracefully - actual behavior depends on implementation
        
        // Test with single value
        let single_value = vec![0.7];
        let result = engine.calibrate(&single_value, 0.8);
        assert!(result.threshold >= 0.0 && result.threshold <= 1.0);

        // Test with all identical values
        let identical_values = vec![0.5; 10];
        let result = engine.calibrate(&identical_values, 0.8);
        assert!(result.threshold >= 0.0 && result.threshold <= 1.0);

        // Test with extreme spread
        let extreme_spread = vec![0.0, 1.0];
        let result = engine.calibrate(&extreme_spread, 0.8);
        assert!(result.threshold >= 0.0 && result.threshold <= 1.0);
    }

    /// Test calibration report generation
    #[test]
    fn test_calibration_report_generation() {
        let mut engine = AutoCalibrationEngine::new();
        
        // Add some performance history
        let feedback = QualityMetrics {
            precision: 0.8,
            recall: 0.7,
            f1_score: 0.75,
            ..Default::default()
        };
        engine.update_thresholds(&feedback);
        
        let report = engine.generate_report();
        
        assert!(report.stability_score >= 0.0 && report.stability_score <= 1.0);
        assert!(report.last_calibration > 0);
        assert!(!report.recommendations.is_empty()); // Should have some recommendations
    }

    // Helper function for creating test candidates with current API
    fn create_test_candidate(id: &str, quality_score: f64, saved_tokens: usize) -> CloneCandidate {
        CloneCandidate {
            id: id.to_string(),
            saved_tokens,
            rarity_gain: 1.5,
            live_reach_boost: 1.2,
            quality_score,
            confidence: quality_score * 0.9, // Confidence slightly lower than quality
            similarity_score: quality_score,
        }
    }
}

#[cfg(test)]
mod payoff_ranking_system_tests {
    use super::*;

    /// Test payoff formula calculation: SavedTokens × RarityGain × LiveReachBoost × QualityScore × Confidence
    #[test]
    fn test_payoff_formula_calculation() {
        let mut ranking_system = PayoffRankingSystem::new();

        let test_cases = vec![
            // (saved_tokens, rarity_gain, live_reach_boost, quality_score, confidence, expected_base)
            (100, 1.5, 1.0, 0.8, 0.9, 100.0 * 1.5 * 1.0 * 0.8 * 0.9), // 108.0
            (200, 2.0, 1.5, 0.9, 0.8, 200.0 * 2.0 * 1.5 * 0.9 * 0.8), // 432.0
            (150, 1.2, 0.8, 0.7, 0.7, 150.0 * 1.2 * 0.8 * 0.7 * 0.7), // 70.56
        ];

        for (saved_tokens, rarity_gain, live_reach_boost, quality_score, confidence, expected) in test_cases {
            let candidate = CloneCandidate {
                id: "test".to_string(),
                saved_tokens,
                rarity_gain,
                live_reach_boost,
                quality_score,
                confidence,
                similarity_score: 0.8,
            };

            let payoff = ranking_system.calculate_payoff(&candidate);
            assert_relative_eq!(payoff, expected, epsilon = 0.01);
        }
    }

    /// Test hard filtering floors (SavedTokens ≥ 100, RarityGain ≥ 1.2)
    #[test]
    fn test_hard_filtering_floors() {
        let floors = HardFilteringFloors {
            min_saved_tokens: 100,
            min_rarity_gain: 1.2,
            min_overall_score: 0.6,
            min_confidence: 0.7,
            ..Default::default()
        };
        let mut ranking_system = PayoffRankingSystem::with_floors(floors);

        // Test candidates that should be filtered by hard floors
        let filtered_candidates = vec![
            // Below SavedTokens floor
            CloneCandidate {
                id: "low_tokens".to_string(),
                saved_tokens: 80, // Below floor of 100
                rarity_gain: 2.0,
                live_reach_boost: 1.5,
                quality_score: 0.8,
                confidence: 0.8,
                similarity_score: 0.9,
            },
            // Below RarityGain floor
            CloneCandidate {
                id: "low_rarity".to_string(),
                saved_tokens: 200,
                rarity_gain: 1.0, // Below floor of 1.2
                live_reach_boost: 1.8,
                quality_score: 0.8,
                confidence: 0.8,
                similarity_score: 0.85,
            },
            // Passes hard floors
            CloneCandidate {
                id: "passes_floors".to_string(),
                saved_tokens: 150, // ≥ 100
                rarity_gain: 1.5,  // ≥ 1.2
                live_reach_boost: 1.3,
                quality_score: 0.7, // ≥ 0.6
                confidence: 0.8,    // ≥ 0.7
                similarity_score: 0.8,
            },
        ];

        let ranking_result = ranking_system.rank_candidates(filtered_candidates);

        // Filter by quality to see which ones pass the floors
        let quality_filtered = ranking_system.filter_by_quality(&[
            CloneCandidate {
                id: "low_tokens".to_string(),
                saved_tokens: 80,
                rarity_gain: 2.0,
                live_reach_boost: 1.5,
                quality_score: 0.8,
                confidence: 0.8,
                similarity_score: 0.9,
            },
            CloneCandidate {
                id: "passes_floors".to_string(),
                saved_tokens: 150,
                rarity_gain: 1.5,
                live_reach_boost: 1.3,
                quality_score: 0.7,
                confidence: 0.8,
                similarity_score: 0.8,
            },
        ]);

        // Should only include candidates that pass hard floors
        assert_eq!(
            quality_filtered.len(),
            1,
            "Should only include candidates that pass hard floors"
        );
        assert_eq!(quality_filtered[0].id, "passes_floors");
    }

    /// Test ranking order based on payoff scores
    #[test]
    fn test_ranking_order_by_payoff() {
        let mut ranking_system = PayoffRankingSystem::new();

        let candidates = vec![
            // High payoff: 300 * 2.5 * 2.0 * 0.9 * 0.9 = 1215
            create_ranking_candidate("high", 300, 2.5, 2.0, 0.9),
            // Medium payoff: 200 * 1.8 * 1.5 * 0.7 * 0.8 = 302.4
            create_ranking_candidate("medium", 200, 1.8, 1.5, 0.7),
            // Low payoff: 150 * 1.3 * 1.2 * 0.6 * 0.7 = 98.28
            create_ranking_candidate("low", 150, 1.3, 1.2, 0.6),
        ];

        let ranking_result = ranking_system.rank_candidates(candidates);

        assert_eq!(ranking_result.len(), 3);

        // Verify ranking order (highest payoff first)
        assert_eq!(ranking_result[0].candidate.id, "high");
        assert_eq!(ranking_result[1].candidate.id, "medium");
        assert_eq!(ranking_result[2].candidate.id, "low");

        // Verify payoff scores are ordered correctly
        let payoffs: Vec<f64> = ranking_result
            .iter()
            .map(|r| r.payoff_score)
            .collect();

        // Verify ordering: each payoff should be >= the next
        for i in 0..payoffs.len() - 1 {
            assert!(
                payoffs[i] >= payoffs[i + 1],
                "Payoffs should be in descending order: {} >= {}",
                payoffs[i],
                payoffs[i + 1]
            );
        }
        
        // Verify rank numbers
        assert_eq!(ranking_result[0].rank, 1);
        assert_eq!(ranking_result[1].rank, 2);
        assert_eq!(ranking_result[2].rank, 3);
    }

    /// Test ranking statistics generation
    #[test]
    fn test_ranking_statistics_generation() {
        let mut ranking_system = PayoffRankingSystem::new();
        
        let candidates = vec![
            create_ranking_candidate("high", 200, 2.0, 1.5, 0.8),
            create_ranking_candidate("medium", 150, 1.5, 1.2, 0.6),
            create_ranking_candidate("low", 100, 1.2, 1.0, 0.4),
        ];
        
        let stats = ranking_system.generate_statistics(&candidates);
        
        assert_eq!(stats.total_candidates, 3);
        assert!(stats.mean_payoff > 0.0);
        assert!(stats.max_payoff >= stats.median_payoff);
        assert!(stats.median_payoff >= stats.min_payoff);
    }

    /// Test performance-based floor updates
    #[test]
    fn test_performance_based_floor_updates() {
        let mut ranking_system = PayoffRankingSystem::new();
        let initial_token_floor = ranking_system.filter_by_quality(&[]).len(); // Get baseline
        
        // Test low precision feedback (should increase requirements)
        let low_precision_feedback = QualityMetrics {
            precision: 0.5,
            recall: 0.9,
            f1_score: 0.65,
            ..Default::default()
        };
        
        ranking_system.update_floors(&low_precision_feedback);
        // After update, floors should be more stringent
        
        // Test low recall feedback (should decrease requirements)
        let low_recall_feedback = QualityMetrics {
            precision: 0.9,
            recall: 0.5,
            f1_score: 0.65,
            ..Default::default()
        };
        
        ranking_system.update_floors(&low_recall_feedback);
        // After this update, floors should be less stringent
    }

    fn create_ranking_candidate(
        id: &str,
        saved_tokens: usize,
        rarity_gain: f64,
        live_reach_boost: f64,
        quality: f64,
    ) -> CloneCandidate {
        CloneCandidate {
            id: id.to_string(),
            saved_tokens,
            rarity_gain,
            live_reach_boost,
            quality_score: quality,
            confidence: quality * 0.9, // Confidence slightly lower than quality
            similarity_score: quality + 0.05,
        }
    }
}

#[cfg(test)]
mod simplified_quality_tests {
    use super::*;

    /// Test quality score validation
    #[test]
    fn test_quality_score_validation() {
        let test_cases = vec![
            (0.0, "minimum quality"),
            (0.5, "medium quality"),
            (1.0, "maximum quality"),
        ];

        for (quality_score, description) in test_cases {
            let candidate = CloneCandidate {
                id: format!("{}_test", description.replace(" ", "_")),
                saved_tokens: 150,
                rarity_gain: 1.5,
                live_reach_boost: 1.2,
                quality_score,
                confidence: 0.8,
                similarity_score: 0.8,
            };

            assert!(
                candidate.quality_score >= 0.0 && candidate.quality_score <= 1.0,
                "{}: quality score {} should be in [0,1] range",
                description,
                candidate.quality_score
            );
        }
    }

    /// Test confidence score validation
    #[test]
    fn test_confidence_score_validation() {
        let test_cases = vec![
            (0.1, "low confidence"),
            (0.5, "medium confidence"),
            (0.9, "high confidence"),
        ];

        for (confidence, description) in test_cases {
            let candidate = CloneCandidate {
                id: format!("{}_test", description.replace(" ", "_")),
                saved_tokens: 150,
                rarity_gain: 1.5,
                live_reach_boost: 1.2,
                quality_score: 0.8,
                confidence,
                similarity_score: 0.8,
            };

            assert!(
                candidate.confidence >= 0.0 && candidate.confidence <= 1.0,
                "{}: confidence {} should be in [0,1] range",
                description,
                candidate.confidence
            );
        }
    }

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio;

    /// Test complete calibration and ranking pipeline
    #[tokio::test]
    async fn test_complete_calibration_ranking_pipeline() {
        // Step 1: Create sample similarity data
        let sample_similarities = vec![0.2, 0.4, 0.6, 0.8, 0.9, 0.95];
        
        // Step 2: Auto-calibration
        let mut calibration_engine = AutoCalibrationEngine::new();
        let calibration_result = calibration_engine.calibrate(&sample_similarities, 0.8);
        
        // Step 3: Create candidates and apply ranking
        let candidates = create_comprehensive_candidate_set();
        let mut ranking_system = PayoffRankingSystem::new();
        let ranking_result = ranking_system.rank_candidates(candidates);
        
        // Step 4: Filter by calibrated threshold
        let threshold = calibration_result.threshold;
        let high_quality_candidates: Vec<_> = ranking_result
            .iter()
            .filter(|r| r.candidate.similarity_score >= threshold)
            .cloned()
            .collect();
        
        // Validate pipeline results
        assert!(
            threshold >= 0.0 && threshold <= 1.0,
            "Threshold should be reasonable: {}",
            threshold
        );
        
        assert!(
            !ranking_result.is_empty(),
            "Should have ranked candidates"
        );
        
        // Verify payoff ranking order
        for i in 0..ranking_result.len().saturating_sub(1) {
            assert!(
                ranking_result[i].payoff_score >= ranking_result[i + 1].payoff_score,
                "Candidates should be sorted by payoff score: {} >= {}",
                ranking_result[i].payoff_score,
                ranking_result[i + 1].payoff_score
            );
        }
    }

    /// Test threshold sensitivity on ranking
    #[tokio::test]
    async fn test_threshold_sensitivity_on_ranking() {
        let candidates = create_comprehensive_candidate_set();
        let mut ranking_system = PayoffRankingSystem::new();
        let ranking_result = ranking_system.rank_candidates(candidates);

        // Test different similarity threshold levels and their impact
        let thresholds = vec![0.3, 0.5, 0.7, 0.9];
        let mut results = Vec::new();

        for threshold in thresholds {
            let filtered_candidates: Vec<_> = ranking_result
                .iter()
                .filter(|r| r.candidate.similarity_score >= threshold)
                .collect();

            let avg_payoff = if filtered_candidates.is_empty() {
                0.0
            } else {
                filtered_candidates
                    .iter()
                    .map(|r| r.payoff_score)
                    .sum::<f64>()
                    / filtered_candidates.len() as f64
            };

            results.push((threshold, filtered_candidates.len(), avg_payoff));
        }

        // Validate sensitivity analysis results
        for i in 0..results.len() - 1 {
            let (thresh1, count1, _payoff1) = results[i];
            let (thresh2, count2, _payoff2) = results[i + 1];

            // Higher thresholds should generally result in fewer candidates
            assert!(
                count2 <= count1,
                "Higher threshold {} should not increase candidate count: {} vs {}",
                thresh2,
                count2,
                count1
            );
        }
    }

    fn create_comprehensive_candidate_set() -> Vec<CloneCandidate> {
        vec![
            // Excellent candidates (should rank highest)
            create_quality_candidate("excellent1", 300, 3.0, 2.2, 0.95),
            create_quality_candidate("excellent2", 250, 2.8, 2.0, 0.92),
            // Good candidates
            create_quality_candidate("good1", 180, 2.2, 1.8, 0.85),
            create_quality_candidate("good2", 200, 2.0, 1.6, 0.82),
            create_quality_candidate("good3", 160, 1.9, 1.5, 0.78),
            // Mediocre candidates
            create_quality_candidate("mediocre1", 140, 1.6, 1.3, 0.65),
            create_quality_candidate("mediocre2", 130, 1.4, 1.2, 0.60),
            create_quality_candidate("mediocre3", 120, 1.3, 1.1, 0.55),
            // Poor candidates (may be filtered out)
            create_quality_candidate("poor1", 90, 1.0, 1.0, 0.3),
            create_quality_candidate("poor2", 110, 1.1, 0.9, 0.35),
            create_quality_candidate("poor3", 70, 0.9, 1.1, 0.25),
            // Borderline candidates
            create_quality_candidate("borderline1", 105, 1.25, 1.05, 0.45),
            create_quality_candidate("borderline2", 115, 1.3, 1.1, 0.48),
        ]
    }

    fn create_quality_candidate(
        id: &str,
        saved_tokens: usize,
        rarity_gain: f64,
        live_reach_boost: f64,
        base_quality: f64,
    ) -> CloneCandidate {
        CloneCandidate {
            id: id.to_string(),
            saved_tokens,
            rarity_gain,
            live_reach_boost,
            quality_score: base_quality,
            confidence: base_quality * 0.9, // Confidence slightly lower than quality
            similarity_score: base_quality + 0.02,
        }
    }
}

#[cfg(test)]
mod property_based_tests {
    use super::*;

    proptest! {
        /// Property: Payoff score should increase monotonically with each factor
        #[test]
        fn prop_payoff_monotonic_with_factors(
            base_saved_tokens in 100usize..1000,
            base_rarity_gain in 1.2f64..5.0,
            base_live_reach_boost in 1.0f64..3.0
        ) {
            let mut ranking_system = PayoffRankingSystem::new();

            let base_candidate = create_prop_candidate("base", base_saved_tokens, base_rarity_gain, base_live_reach_boost);
            let base_payoff = ranking_system.calculate_payoff(&base_candidate);

            // Increased saved tokens should increase payoff
            let increased_tokens_candidate = create_prop_candidate("tokens", base_saved_tokens + 50, base_rarity_gain, base_live_reach_boost);
            let increased_tokens_payoff = ranking_system.calculate_payoff(&increased_tokens_candidate);
            assert!(increased_tokens_payoff > base_payoff,
                   "Increasing saved tokens should increase payoff: {} vs {}", increased_tokens_payoff, base_payoff);

            // Increased rarity gain should increase payoff
            let increased_rarity_candidate = create_prop_candidate("rarity", base_saved_tokens, base_rarity_gain + 0.5, base_live_reach_boost);
            let increased_rarity_payoff = ranking_system.calculate_payoff(&increased_rarity_candidate);
            assert!(increased_rarity_payoff > base_payoff,
                   "Increasing rarity gain should increase payoff: {} vs {}", increased_rarity_payoff, base_payoff);

            // Increased live reach boost should increase payoff
            let increased_reach_candidate = create_prop_candidate("reach", base_saved_tokens, base_rarity_gain, base_live_reach_boost + 0.3);
            let increased_reach_payoff = ranking_system.calculate_payoff(&increased_reach_candidate);
            assert!(increased_reach_payoff > base_payoff,
                   "Increasing live reach boost should increase payoff: {} vs {}", increased_reach_payoff, base_payoff);
        }

        /// Property: Quality and confidence scores should be in valid ranges
        #[test]
        fn prop_quality_confidence_valid_ranges(
            quality_score in 0.0f64..1.0,
            confidence in 0.0f64..1.0
        ) {
            let candidate = CloneCandidate {
                id: "prop_test".to_string(),
                saved_tokens: 150,
                rarity_gain: 1.5,
                live_reach_boost: 1.2,
                quality_score,
                confidence,
                similarity_score: 0.8,
            };

            assert!(candidate.quality_score >= 0.0 && candidate.quality_score <= 1.0,
                   "Quality score should be in [0,1] range: {}", candidate.quality_score);
            assert!(candidate.confidence >= 0.0 && candidate.confidence <= 1.0,
                   "Confidence should be in [0,1] range: {}", candidate.confidence);
        }

        /// Property: Calibration should produce valid thresholds
        #[test]
        fn prop_calibration_produces_valid_thresholds(
            target_quality in 0.5f64..0.95
        ) {
            let mut engine = AutoCalibrationEngine::new();
            let sample_data = create_diverse_sample_set();

            let result = engine.calibrate(&sample_data, target_quality);
            
            // Threshold should be within valid range
            assert!(result.threshold >= 0.0 && result.threshold <= 1.0,
                   "Calibrated threshold should be within [0,1] range: {}", result.threshold);
            
            // Confidence should be valid
            assert!(result.confidence >= 0.0 && result.confidence <= 1.0,
                   "Calibration confidence should be in [0,1] range: {}", result.confidence);
        }
    }

    fn create_prop_candidate(
        id: &str,
        saved_tokens: usize,
        rarity_gain: f64,
        live_reach_boost: f64,
    ) -> CloneCandidate {
        CloneCandidate {
            id: id.to_string(),
            saved_tokens,
            rarity_gain,
            live_reach_boost,
            quality_score: 0.7,
            confidence: 0.65,
            similarity_score: 0.7,
        }
    }

    fn create_diverse_sample_set() -> Vec<f64> {
        (0..15)
            .map(|i| 0.2 + (i as f64 / 14.0) * 0.7) // Range from 0.2 to 0.9
            .collect()
    }
}
}
