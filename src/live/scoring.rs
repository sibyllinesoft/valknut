//! Scoring systems for live reachability analysis
//! 
//! Implements LiveReach scoring and ShadowIslandScore calculation
//! for identifying problematic code communities

use crate::core::errors::{Result, ValknutError};
use crate::live::types::{NodeStats, LiveReachScore, LiveReachComponents};
use crate::live::graph::CallGraph;
use crate::live::community::{CommunityDetection, CommunityInfo, CommunityId};

use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

/// Configuration for scoring algorithms
#[derive(Debug, Clone)]
pub struct ScoringConfig {
    /// LiveReach component weights (must sum to 1.0)
    pub live_reach_weights: LiveReachWeights,
    
    /// ShadowIsland scoring parameters
    pub shadow_island_params: ShadowIslandParams,
    
    /// Recency time window for scoring
    pub recency_window_days: u32,
}

/// Weights for LiveReach score components
#[derive(Debug, Clone)]
pub struct LiveReachWeights {
    /// Weight for caller count component
    pub callers: f64,
    
    /// Weight for call count component  
    pub calls: f64,
    
    /// Weight for seed reachability component
    pub seed_reachable: f64,
    
    /// Weight for recency component
    pub recency: f64,
}

/// Parameters for ShadowIslandScore calculation
#[derive(Debug, Clone)]
pub struct ShadowIslandParams {
    /// Exponent for runtime internal fraction (δ parameter)
    pub runtime_penalty_exponent: f64,
    
    /// Minimum community size to consider
    pub min_community_size: usize,
    
    /// Weight for size component in score
    pub size_weight: f64,
}

/// Live reachability scorer
pub struct LiveReachScorer {
    config: ScoringConfig,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            live_reach_weights: LiveReachWeights {
                callers: 0.5,
                calls: 0.2,
                seed_reachable: 0.2,
                recency: 0.1,
            },
            shadow_island_params: ShadowIslandParams {
                runtime_penalty_exponent: 0.5,
                min_community_size: 5,
                size_weight: 1.0,
            },
            recency_window_days: 30,
        }
    }
}

impl ScoringConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        let weights = &self.live_reach_weights;
        let sum = weights.callers + weights.calls + weights.seed_reachable + weights.recency;
        
        if (sum - 1.0).abs() > 1e-6 {
            return Err(ValknutError::validation(
                format!("LiveReach weights must sum to 1.0, got {:.6}", sum)
            ));
        }
        
        let params = &self.shadow_island_params;
        if params.runtime_penalty_exponent < 0.0 {
            return Err(ValknutError::validation(
                "Runtime penalty exponent must be non-negative"
            ));
        }
        
        if params.min_community_size == 0 {
            return Err(ValknutError::validation(
                "Minimum community size must be greater than 0"
            ));
        }
        
        if params.size_weight < 0.0 {
            return Err(ValknutError::validation(
                "Size weight must be non-negative"
            ));
        }
        
        if self.recency_window_days == 0 {
            return Err(ValknutError::validation(
                "Recency window must be greater than 0"
            ));
        }
        
        Ok(())
    }
}

impl LiveReachScorer {
    /// Create a new scorer with configuration
    pub fn new(config: ScoringConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }
    
    /// Calculate LiveReach scores for all nodes in the graph
    pub fn calculate_live_reach_scores(
        &self,
        graph: &CallGraph,
        analysis_time: DateTime<Utc>,
    ) -> Result<HashMap<String, LiveReachScore>> {
        let mut scores = HashMap::new();
        
        // Collect all node statistics
        let node_stats: Vec<_> = graph.iter_nodes().collect();
        
        if node_stats.is_empty() {
            return Ok(scores);
        }
        
        // Calculate rank-normalized values for comparative components
        let (callers_ranks, calls_ranks) = self.calculate_rank_normalizations(&node_stats)?;
        
        // Calculate scores for each node
        for (symbol, stats) in node_stats {
            let components = self.calculate_live_reach_components(
                symbol,
                stats,
                &callers_ranks,
                &calls_ranks,
                analysis_time,
            )?;
            
            let score = self.combine_components(&components);
            
            scores.insert(symbol.to_string(), LiveReachScore {
                score,
                components,
            });
        }
        
        Ok(scores)
    }
    
    /// Calculate ShadowIslandScore for communities
    pub fn calculate_shadow_island_scores(
        &self,
        detection: &CommunityDetection,
        live_reach_scores: &HashMap<String, LiveReachScore>,
        graph: &CallGraph,
    ) -> Result<HashMap<CommunityId, f64>> {
        let mut scores = HashMap::new();
        
        for (community_id, info) in &detection.communities {
            if info.size() < self.config.shadow_island_params.min_community_size {
                continue; // Skip small communities
            }
            
            let score = self.calculate_community_shadow_score(
                info,
                live_reach_scores,
                graph,
            )?;
            
            scores.insert(*community_id, score);
        }
        
        Ok(scores)
    }
    
    /// Calculate rank normalizations for callers and calls
    fn calculate_rank_normalizations(
        &self,
        node_stats: &[(&str, &NodeStats)],
    ) -> Result<(HashMap<String, f64>, HashMap<String, f64>)> {
        let mut callers_values: Vec<_> = node_stats.iter()
            .map(|(symbol, stats)| (symbol.to_string(), stats.live_callers as f64))
            .collect();
        
        let mut calls_values: Vec<_> = node_stats.iter()
            .map(|(symbol, stats)| (symbol.to_string(), stats.live_calls as f64))
            .collect();
        
        // Sort by values for ranking
        callers_values.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        calls_values.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        
        let n = node_stats.len() as f64;
        
        // Assign rank-normalized scores (0.0 to 1.0)
        let callers_ranks: HashMap<String, f64> = callers_values.into_iter()
            .enumerate()
            .map(|(rank, (symbol, _))| {
                let normalized_rank = if n > 1.0 { rank as f64 / (n - 1.0) } else { 0.5 };
                (symbol, normalized_rank)
            })
            .collect();
        
        let calls_ranks: HashMap<String, f64> = calls_values.into_iter()
            .enumerate()
            .map(|(rank, (symbol, _))| {
                let normalized_rank = if n > 1.0 { rank as f64 / (n - 1.0) } else { 0.5 };
                (symbol, normalized_rank)
            })
            .collect();
        
        Ok((callers_ranks, calls_ranks))
    }
    
    /// Calculate individual components of LiveReach score
    fn calculate_live_reach_components(
        &self,
        symbol: &str,
        stats: &NodeStats,
        callers_ranks: &HashMap<String, f64>,
        calls_ranks: &HashMap<String, f64>,
        analysis_time: DateTime<Utc>,
    ) -> Result<LiveReachComponents> {
        let callers_component = callers_ranks.get(symbol).copied().unwrap_or(0.0);
        let calls_component = calls_ranks.get(symbol).copied().unwrap_or(0.0);
        
        let seed_component = if stats.seed_reachable { 1.0 } else { 0.0 };
        
        let recency_component = self.calculate_recency_component(stats, analysis_time);
        
        Ok(LiveReachComponents {
            callers_component,
            calls_component,
            seed_component,
            recency_component,
        })
    }
    
    /// Calculate recency component based on last_seen timestamp
    fn calculate_recency_component(&self, stats: &NodeStats, analysis_time: DateTime<Utc>) -> f64 {
        if let Some(last_seen) = stats.last_seen {
            let window_duration = Duration::days(self.config.recency_window_days as i64);
            let staleness = analysis_time - last_seen;
            
            // Clamp staleness to window, then invert (1.0 = recent, 0.0 = stale)
            let staleness_ratio = (staleness.num_seconds() as f64 / window_duration.num_seconds() as f64)
                .min(1.0)
                .max(0.0);
            
            1.0 - staleness_ratio
        } else {
            0.0 // Never seen = stale
        }
    }
    
    /// Combine components into final LiveReach score using sigmoid
    fn combine_components(&self, components: &LiveReachComponents) -> f64 {
        let weights = &self.config.live_reach_weights;
        
        let weighted_sum = 
            weights.callers * components.callers_component +
            weights.calls * components.calls_component +
            weights.seed_reachable * components.seed_component +
            weights.recency * components.recency_component;
        
        // Apply sigmoid transformation: σ(x) = 1 / (1 + e^(-x))
        // Scale input to reasonable range for sigmoid
        let scaled_input = (weighted_sum - 0.5) * 6.0; // Map [0,1] to roughly [-3,3]
        
        1.0 / (1.0 + (-scaled_input).exp())
    }
    
    /// Calculate ShadowIslandScore for a community
    fn calculate_community_shadow_score(
        &self,
        info: &CommunityInfo,
        live_reach_scores: &HashMap<String, LiveReachScore>,
        graph: &CallGraph,
    ) -> Result<f64> {
        // Calculate median LiveReach for nodes in community
        let mut community_live_reach_scores = Vec::new();
        
        for &node_idx in &info.nodes {
            if let Some(symbol) = graph.get_symbol(node_idx) {
                if let Some(score) = live_reach_scores.get(symbol) {
                    community_live_reach_scores.push(score.score);
                }
            }
        }
        
        if community_live_reach_scores.is_empty() {
            return Ok(0.0);
        }
        
        community_live_reach_scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_live_reach = if community_live_reach_scores.len() % 2 == 0 {
            let mid = community_live_reach_scores.len() / 2;
            (community_live_reach_scores[mid - 1] + community_live_reach_scores[mid]) / 2.0
        } else {
            community_live_reach_scores[community_live_reach_scores.len() / 2]
        };
        
        // Calculate cut ratio
        let cut_ratio = info.cut_ratio();
        
        // Calculate size factor: log1p(|C|)
        let size_factor = (info.size() as f64).ln_1p() * self.config.shadow_island_params.size_weight;
        
        // Calculate runtime internal fraction penalty
        let runtime_internal = info.runtime_internal_fraction();
        let runtime_penalty = (1.0 - runtime_internal).powf(self.config.shadow_island_params.runtime_penalty_exponent);
        
        // ShadowIslandScore formula:
        // (1 - median_live_reach) * (1 - cut_ratio) * log1p(|C|) * (1 - runtime_internal)^δ
        let score = (1.0 - median_live_reach) * (1.0 - cut_ratio) * size_factor * runtime_penalty;
        
        Ok(score.max(0.0).min(1.0)) // Clamp to [0, 1]
    }
    
    /// Generate analysis notes for a community
    pub fn generate_community_notes(
        &self,
        info: &CommunityInfo,
        shadow_score: f64,
        live_reach_scores: &HashMap<String, LiveReachScore>,
        graph: &CallGraph,
    ) -> Vec<String> {
        let mut notes = Vec::new();
        
        // High shadow island score
        if shadow_score >= 0.8 {
            notes.push("High shadow island score - consider refactoring".to_string());
        } else if shadow_score >= 0.6 {
            notes.push("Moderate shadow island score - monitor for growth".to_string());
        }
        
        // Low cut ratio (tight coupling)
        if info.cut_ratio() < 0.1 {
            notes.push("Tightly coupled - few external dependencies".to_string());
        }
        
        // High static-only edges
        if info.runtime_internal_fraction() < 0.2 {
            notes.push(">80% static-only edges - potentially unused code".to_string());
        }
        
        // Check staleness (nodes not seen recently)
        let stale_nodes = info.nodes.iter()
            .filter_map(|&node_idx| graph.get_symbol(node_idx))
            .filter_map(|symbol| live_reach_scores.get(symbol))
            .filter(|score| score.components.recency_component < 0.1)
            .count();
        
        if stale_nodes > info.size() / 2 {
            notes.push(format!("Stale code - {} nodes not seen recently", stale_nodes));
        }
        
        // Large community size
        if info.size() >= 20 {
            notes.push("Large community - consider breaking apart".to_string());
        }
        
        // Low overall live reach
        let avg_live_reach: f64 = info.nodes.iter()
            .filter_map(|&node_idx| graph.get_symbol(node_idx))
            .filter_map(|symbol| live_reach_scores.get(symbol))
            .map(|score| score.score)
            .sum::<f64>() / info.size() as f64;
        
        if avg_live_reach < 0.3 {
            notes.push("Low average live reach - rarely called in production".to_string());
        }
        
        notes
    }
}

/// Utility functions for scoring statistics
pub mod stats {
    
    /// Calculate percentile for a value in a sorted vector
    pub fn percentile(sorted_values: &[f64], value: f64) -> f64 {
        if sorted_values.is_empty() {
            return 0.0;
        }
        
        let count_below = sorted_values.iter()
            .take_while(|&&v| v < value)
            .count();
        
        count_below as f64 / sorted_values.len() as f64
    }
    
    /// Calculate median of a vector
    pub fn median(values: &mut [f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        if values.len() % 2 == 0 {
            let mid = values.len() / 2;
            (values[mid - 1] + values[mid]) / 2.0
        } else {
            values[values.len() / 2]
        }
    }
    
    /// Calculate standard statistics for a dataset
    pub fn basic_stats(values: &[f64]) -> (f64, f64, f64, f64, f64) {
        if values.is_empty() {
            return (0.0, 0.0, 0.0, 0.0, 0.0);
        }
        
        let sum: f64 = values.iter().sum();
        let mean = sum / values.len() as f64;
        
        let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        let variance = values.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();
        
        (mean, std_dev, min, max, sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    
    #[test]
    fn test_scoring_config_validation() {
        let mut config = ScoringConfig::default();
        assert!(config.validate().is_ok());
        
        // Invalid weight sum
        config.live_reach_weights.callers = 0.8;
        assert!(config.validate().is_err());
        
        // Fix weights
        config.live_reach_weights = LiveReachWeights {
            callers: 0.4,
            calls: 0.3,
            seed_reachable: 0.2,
            recency: 0.1,
        };
        assert!(config.validate().is_ok());
        
        // Invalid parameters
        config.shadow_island_params.min_community_size = 0;
        assert!(config.validate().is_err());
        
        config.shadow_island_params.min_community_size = 5;
        config.recency_window_days = 0;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_live_reach_weights_sum() {
        let weights = LiveReachWeights {
            callers: 0.5,
            calls: 0.2, 
            seed_reachable: 0.2,
            recency: 0.1,
        };
        
        let sum = weights.callers + weights.calls + weights.seed_reachable + weights.recency;
        assert!((sum - 1.0).abs() < 1e-6);
    }
    
    #[test]
    fn test_recency_component_calculation() {
        let config = ScoringConfig::default();
        let scorer = LiveReachScorer::new(config).unwrap();
        
        let analysis_time = Utc::now();
        
        // Recent node (1 day ago)
        let recent_stats = NodeStats {
            live_callers: 10,
            live_calls: 100,
            last_seen: Some(analysis_time - Duration::days(1)),
            first_seen: Some(analysis_time - Duration::days(30)),
            seed_reachable: true,
        };
        
        let recent_score = scorer.calculate_recency_component(&recent_stats, analysis_time);
        assert!(recent_score > 0.9); // Should be high
        
        // Stale node (25 days ago)
        let stale_stats = NodeStats {
            live_callers: 5,
            live_calls: 50,
            last_seen: Some(analysis_time - Duration::days(25)),
            first_seen: Some(analysis_time - Duration::days(30)),
            seed_reachable: false,
        };
        
        let stale_score = scorer.calculate_recency_component(&stale_stats, analysis_time);
        assert!(stale_score < 0.3); // Should be low
        
        // Never seen
        let never_seen_stats = NodeStats {
            live_callers: 0,
            live_calls: 0,
            last_seen: None,
            first_seen: None,
            seed_reachable: false,
        };
        
        let never_score = scorer.calculate_recency_component(&never_seen_stats, analysis_time);
        assert_eq!(never_score, 0.0);
    }
    
    #[test]
    fn test_component_combination_sigmoid() {
        let config = ScoringConfig::default();
        let scorer = LiveReachScorer::new(config).unwrap();
        
        // High activity components
        let high_components = LiveReachComponents {
            callers_component: 1.0,
            calls_component: 1.0,
            seed_component: 1.0,
            recency_component: 1.0,
        };
        
        let high_score = scorer.combine_components(&high_components);
        assert!(high_score > 0.8);
        
        // Low activity components
        let low_components = LiveReachComponents {
            callers_component: 0.0,
            calls_component: 0.0,
            seed_component: 0.0,
            recency_component: 0.0,
        };
        
        let low_score = scorer.combine_components(&low_components);
        assert!(low_score < 0.2);
        
        // Mixed components
        let mixed_components = LiveReachComponents {
            callers_component: 0.5,
            calls_component: 0.3,
            seed_component: 1.0,
            recency_component: 0.2,
        };
        
        let mixed_score = scorer.combine_components(&mixed_components);
        assert!(mixed_score > 0.3 && mixed_score < 0.8);
    }
    
    #[test]
    fn test_stats_utilities() {
        use stats::*;
        
        let mut values = vec![1.0, 3.0, 2.0, 5.0, 4.0];
        
        let med = median(&mut values);
        assert_eq!(med, 3.0);
        
        let perc = percentile(&values, 3.0);
        assert_eq!(perc, 0.4); // 2 values below 3.0 out of 5
        
        let (mean, std_dev, min, max, sum) = basic_stats(&values);
        assert_eq!(mean, 3.0);
        assert_eq!(min, 1.0);
        assert_eq!(max, 5.0);
        assert_eq!(sum, 15.0);
        assert!(std_dev > 0.0);
    }
    
    #[test]
    fn test_empty_stats() {
        use stats::*;
        
        let empty: Vec<f64> = vec![];
        let mut empty_mut = vec![];
        
        assert_eq!(median(&mut empty_mut), 0.0);
        assert_eq!(percentile(&empty, 1.0), 0.0);
        
        let (mean, std_dev, min, max, sum) = basic_stats(&empty);
        assert_eq!(mean, 0.0);
        assert_eq!(std_dev, 0.0);
        assert_eq!(sum, 0.0);
    }
}