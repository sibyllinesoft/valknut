//! Payoff ranking system for clone candidates

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{HardFilteringFloors, QualityMetrics};

/// Payoff ranking system for prioritizing clone candidates
#[derive(Debug)]
pub struct PayoffRankingSystem {
    /// Hard filtering floors for quality assurance
    filtering_floors: HardFilteringFloors,

    /// Cached payoff calculations
    payoff_cache: HashMap<String, f64>,
}

impl PayoffRankingSystem {
    /// Create a new payoff ranking system
    pub fn new() -> Self {
        Self {
            filtering_floors: HardFilteringFloors::default(),
            payoff_cache: HashMap::new(),
        }
    }

    /// Create with custom filtering floors
    pub fn with_floors(filtering_floors: HardFilteringFloors) -> Self {
        Self {
            filtering_floors,
            payoff_cache: HashMap::new(),
        }
    }

    /// Calculate payoff score for a clone candidate
    pub fn calculate_payoff(&mut self, candidate: &CloneCandidate) -> f64 {
        // Check cache first
        let cache_key = format!("{}_{}", candidate.saved_tokens, candidate.similarity_score);
        if let Some(&cached_payoff) = self.payoff_cache.get(&cache_key) {
            return cached_payoff;
        }

        // Apply hard filtering floors first
        if !self.meets_minimum_requirements(candidate) {
            return 0.0;
        }

        // Calculate base payoff using the formula:
        // Payoff = SavedTokens × RarityGain × LiveReachBoost
        let base_payoff =
            (candidate.saved_tokens as f64) * candidate.rarity_gain * candidate.live_reach_boost;

        // Apply quality adjustment
        let quality_adjusted = base_payoff * candidate.quality_score;

        // Apply confidence penalty
        let confidence_adjusted = quality_adjusted * candidate.confidence;

        // Cache the result
        self.payoff_cache.insert(cache_key, confidence_adjusted);

        confidence_adjusted
    }

    /// Check if candidate meets minimum requirements
    fn meets_minimum_requirements(&self, candidate: &CloneCandidate) -> bool {
        candidate.saved_tokens >= self.filtering_floors.min_saved_tokens
            && candidate.rarity_gain >= self.filtering_floors.min_rarity_gain
            && candidate.live_reach_boost >= self.filtering_floors.min_live_reach_boost
            && candidate.quality_score >= self.filtering_floors.min_overall_score
            && candidate.confidence >= self.filtering_floors.min_confidence
    }

    /// Rank a list of candidates by payoff score
    pub fn rank_candidates(
        &mut self,
        candidates: Vec<CloneCandidate>,
    ) -> Vec<RankedCloneCandidate> {
        let mut ranked: Vec<RankedCloneCandidate> = candidates
            .into_iter()
            .map(|candidate| {
                let payoff = self.calculate_payoff(&candidate);
                RankedCloneCandidate {
                    candidate,
                    payoff_score: payoff,
                    rank: 0, // Will be set after sorting
                }
            })
            .collect();

        // Sort by payoff score (highest first)
        ranked.sort_by(|a, b| {
            b.payoff_score
                .partial_cmp(&a.payoff_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Set rank numbers
        for (index, ranked_candidate) in ranked.iter_mut().enumerate() {
            ranked_candidate.rank = index + 1;
        }

        ranked
    }

    /// Filter candidates that don't meet quality thresholds
    pub fn filter_by_quality(&self, candidates: &[CloneCandidate]) -> Vec<CloneCandidate> {
        candidates
            .iter()
            .filter(|candidate| self.meets_minimum_requirements(candidate))
            .cloned()
            .collect()
    }

    /// Generate ranking statistics
    pub fn generate_statistics(&mut self, candidates: &[CloneCandidate]) -> RankingStatistics {
        let ranked = self.rank_candidates(candidates.to_vec());

        let total_candidates = candidates.len();
        let filtered_candidates = self.filter_by_quality(candidates).len();

        let payoff_scores: Vec<f64> = ranked.iter().map(|r| r.payoff_score).collect();
        let mean_payoff = if payoff_scores.is_empty() {
            0.0
        } else {
            payoff_scores.iter().sum::<f64>() / payoff_scores.len() as f64
        };

        let median_payoff = if payoff_scores.is_empty() {
            0.0
        } else {
            let mut sorted_payoffs = payoff_scores.clone();
            sorted_payoffs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mid = sorted_payoffs.len() / 2;
            if sorted_payoffs.len() % 2 == 0 {
                (sorted_payoffs[mid - 1] + sorted_payoffs[mid]) / 2.0
            } else {
                sorted_payoffs[mid]
            }
        };

        let max_payoff = payoff_scores.iter().cloned().fold(0.0, f64::max);
        let min_payoff = payoff_scores.iter().cloned().fold(f64::INFINITY, f64::min);

        RankingStatistics {
            total_candidates,
            filtered_candidates,
            mean_payoff,
            median_payoff,
            max_payoff,
            min_payoff: if min_payoff == f64::INFINITY {
                0.0
            } else {
                min_payoff
            },
            payoff_distribution: self.calculate_payoff_distribution(&payoff_scores),
        }
    }

    /// Calculate payoff distribution buckets
    fn calculate_payoff_distribution(&self, payoffs: &[f64]) -> Vec<(f64, usize)> {
        if payoffs.is_empty() {
            return Vec::new();
        }

        let max_payoff = payoffs.iter().cloned().fold(0.0, f64::max);
        let bucket_size = max_payoff / 10.0; // 10 buckets
        let mut buckets = vec![0; 10];

        for &payoff in payoffs {
            let bucket_index = ((payoff / bucket_size).floor() as usize).min(9);
            buckets[bucket_index] += 1;
        }

        buckets
            .into_iter()
            .enumerate()
            .map(|(i, count)| (i as f64 * bucket_size, count))
            .collect()
    }

    /// Update filtering floors based on performance data
    pub fn update_floors(&mut self, performance_data: &QualityMetrics) {
        // Adjust floors based on precision/recall trade-off
        if performance_data.precision < 0.8 {
            // Too many false positives, increase requirements
            self.filtering_floors.min_saved_tokens =
                (self.filtering_floors.min_saved_tokens as f64 * 1.1) as usize;
            self.filtering_floors.min_confidence *= 1.05;
        } else if performance_data.recall < 0.8 {
            // Too many false negatives, decrease requirements
            self.filtering_floors.min_saved_tokens =
                (self.filtering_floors.min_saved_tokens as f64 * 0.9) as usize;
            self.filtering_floors.min_confidence *= 0.95;
        }

        // Ensure floors stay within reasonable bounds
        self.filtering_floors.min_saved_tokens =
            self.filtering_floors.min_saved_tokens.max(10).min(1000);
        self.filtering_floors.min_confidence =
            self.filtering_floors.min_confidence.max(0.1).min(0.95);
    }
}

/// Clone candidate for ranking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneCandidate {
    pub id: String,
    pub saved_tokens: usize,
    pub rarity_gain: f64,
    pub live_reach_boost: f64,
    pub quality_score: f64,
    pub confidence: f64,
    pub similarity_score: f64,
}

/// Ranked clone candidate with payoff score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedCloneCandidate {
    pub candidate: CloneCandidate,
    pub payoff_score: f64,
    pub rank: usize,
}

/// Statistics about the ranking process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingStatistics {
    pub total_candidates: usize,
    pub filtered_candidates: usize,
    pub mean_payoff: f64,
    pub median_payoff: f64,
    pub max_payoff: f64,
    pub min_payoff: f64,
    pub payoff_distribution: Vec<(f64, usize)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candidate(
        saved_tokens: usize,
        rarity_gain: f64,
        quality_score: f64,
    ) -> CloneCandidate {
        CloneCandidate {
            id: format!("test_{}", saved_tokens),
            saved_tokens,
            rarity_gain,
            live_reach_boost: 1.0,
            quality_score,
            confidence: 0.8,
            similarity_score: 0.9,
        }
    }

    #[test]
    fn test_payoff_calculation() {
        let mut ranking_system = PayoffRankingSystem::new();
        let candidate = create_test_candidate(200, 2.0, 0.8);

        let payoff = ranking_system.calculate_payoff(&candidate);

        // Expected: 200 * 2.0 * 1.0 * 0.8 * 0.8 = 256.0
        assert!((payoff - 256.0).abs() < 1e-6);
    }

    #[test]
    fn test_hard_filtering_floors() {
        let floors = HardFilteringFloors {
            min_saved_tokens: 150,
            min_confidence: 0.9,
            ..Default::default()
        };
        let mut ranking_system = PayoffRankingSystem::with_floors(floors);

        let good_candidate = CloneCandidate {
            confidence: 0.95, // Above minimum required
            ..create_test_candidate(200, 2.0, 0.8)
        };
        let bad_candidate = CloneCandidate {
            confidence: 0.5,                        // Below minimum
            ..create_test_candidate(100, 1.0, 0.8)  // Also below token minimum
        };

        assert!(ranking_system.calculate_payoff(&good_candidate) > 0.0);
        assert_eq!(ranking_system.calculate_payoff(&bad_candidate), 0.0);
    }

    #[test]
    fn test_ranking() {
        let mut ranking_system = PayoffRankingSystem::new();
        let candidates = vec![
            create_test_candidate(100, 1.0, 0.8), // Low payoff
            create_test_candidate(300, 3.0, 0.9), // High payoff
            create_test_candidate(200, 2.0, 0.7), // Medium payoff
        ];

        let ranked = ranking_system.rank_candidates(candidates);

        // Should be sorted by payoff (highest first)
        assert!(ranked[0].payoff_score >= ranked[1].payoff_score);
        assert!(ranked[1].payoff_score >= ranked[2].payoff_score);

        // Check rank numbers
        assert_eq!(ranked[0].rank, 1);
        assert_eq!(ranked[1].rank, 2);
        assert_eq!(ranked[2].rank, 3);
    }

    #[test]
    fn test_statistics_generation() {
        let mut ranking_system = PayoffRankingSystem::new();
        let candidates = vec![
            create_test_candidate(100, 1.0, 0.8),
            create_test_candidate(200, 2.0, 0.9),
            create_test_candidate(300, 1.5, 0.7),
        ];

        let stats = ranking_system.generate_statistics(&candidates);

        assert_eq!(stats.total_candidates, 3);
        assert!(stats.mean_payoff > 0.0);
        assert!(stats.max_payoff >= stats.median_payoff);
        assert!(stats.median_payoff >= stats.min_payoff);
    }

    #[test]
    fn test_payoff_caching() {
        let mut ranking_system = PayoffRankingSystem::new();
        let candidate = create_test_candidate(200, 2.0, 0.8);

        let payoff1 = ranking_system.calculate_payoff(&candidate);
        let payoff2 = ranking_system.calculate_payoff(&candidate);

        // Should return same result from cache
        assert_eq!(payoff1, payoff2);
    }
}
