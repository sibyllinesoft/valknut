//! LSH performance metrics and statistics.

use tracing::info;

/// Performance metrics for LSH operations
#[derive(Debug, Default, Clone)]
pub struct LshPerformanceMetrics {
    /// Time spent generating MinHash signatures
    pub signature_generation_time: std::time::Duration,
    /// Time spent on similarity comparisons
    pub comparison_time: std::time::Duration,
    /// Time spent building LSH index
    pub index_build_time: std::time::Duration,
    /// Number of entities processed
    pub entities_processed: usize,
    /// Number of similarity comparisons performed
    pub comparisons_performed: usize,
    /// Number of cache hits
    pub cache_hits: usize,
    /// Number of cache misses
    pub cache_misses: usize,
}

impl LshPerformanceMetrics {
    /// Create new performance metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Log performance summary
    pub fn log_summary(&self) {
        info!("LSH Performance Summary:");
        info!(
            "  Signature generation: {:?}",
            self.signature_generation_time
        );
        info!("  Comparison time: {:?}", self.comparison_time);
        info!("  Index build time: {:?}", self.index_build_time);
        info!("  Entities processed: {}", self.entities_processed);
        info!("  Comparisons performed: {}", self.comparisons_performed);
        if self.cache_hits + self.cache_misses > 0 {
            let hit_rate = self.cache_hits as f64 / (self.cache_hits + self.cache_misses) as f64;
            info!("  Cache hit rate: {:.2}%", hit_rate * 100.0);
        }

        // Calculate average times
        if self.entities_processed > 0 {
            let avg_signature_time =
                self.signature_generation_time / self.entities_processed as u32;
            info!("  Average signature time: {:?}", avg_signature_time);
        }
        if self.comparisons_performed > 0 {
            let avg_comparison_time = self.comparison_time / self.comparisons_performed as u32;
            info!("  Average comparison time: {:?}", avg_comparison_time);
        }
    }

    /// Check if performance is within acceptable bounds
    pub fn validate_performance(&self) -> Result<(), String> {
        // Define performance thresholds
        const MAX_SIGNATURE_TIME_MS: u64 = 100; // 100ms per signature is too slow
        const MAX_COMPARISON_TIME_MS: u64 = 50; // 50ms per comparison is too slow

        if self.entities_processed > 0 {
            let avg_sig_time =
                self.signature_generation_time.as_millis() / self.entities_processed as u128;
            if avg_sig_time > MAX_SIGNATURE_TIME_MS as u128 {
                return Err(format!(
                    "Signature generation too slow: {}ms avg > {}ms threshold",
                    avg_sig_time, MAX_SIGNATURE_TIME_MS
                ));
            }
        }

        if self.comparisons_performed > 0 {
            let avg_comp_time =
                self.comparison_time.as_millis() / self.comparisons_performed as u128;
            if avg_comp_time > MAX_COMPARISON_TIME_MS as u128 {
                return Err(format!(
                    "Comparison too slow: {}ms avg > {}ms threshold",
                    avg_comp_time, MAX_COMPARISON_TIME_MS
                ));
            }
        }

        Ok(())
    }
}

/// Performance statistics for LSH similarity context
#[derive(Debug, Clone)]
pub struct LshContextStatistics {
    pub entities_count: usize,
    pub num_bands: usize,
    pub num_hashes: usize,
    pub theoretical_complexity: String,
}
