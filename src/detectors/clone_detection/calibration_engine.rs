//! Auto-calibration engine for adaptive thresholds

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{AdaptiveThresholds, CachedCalibration, NoiseMetrics, QualityMetrics};

/// Auto-calibration engine for adaptive threshold tuning
#[derive(Debug)]
pub struct AutoCalibrationEngine {
    /// Current adaptive thresholds
    thresholds: AdaptiveThresholds,

    /// Cached calibration results
    calibration_cache: HashMap<String, CachedCalibration>,

    /// Performance history for trend analysis
    performance_history: Vec<QualityMetrics>,
}

impl AutoCalibrationEngine {
    /// Create a new auto-calibration engine
    pub fn new() -> Self {
        Self {
            thresholds: AdaptiveThresholds::default(),
            calibration_cache: HashMap::new(),
            performance_history: Vec::new(),
        }
    }

    /// Calibrate thresholds based on sample data
    pub fn calibrate(&mut self, sample_data: &[f64], target_quality: f64) -> CalibrationResult {
        // Simplified calibration algorithm
        let mean = sample_data.iter().sum::<f64>() / sample_data.len() as f64;
        let variance =
            sample_data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / sample_data.len() as f64;
        let std_dev = variance.sqrt();

        // Set threshold based on statistical analysis
        let optimal_threshold = mean - std_dev;

        // Update adaptive thresholds
        self.thresholds.similarity_threshold = optimal_threshold.max(0.0).min(1.0);
        self.thresholds.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        CalibrationResult {
            threshold: optimal_threshold,
            quality_score: target_quality,
            confidence: self.calculate_confidence(&sample_data, optimal_threshold),
            iterations: 1,
            convergence_achieved: true,
            performance_metrics: QualityMetrics::default(),
        }
    }

    /// Calculate confidence in the calibration
    fn calculate_confidence(&self, data: &[f64], threshold: f64) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        // Simple confidence based on data spread
        let above_threshold = data.iter().filter(|&&x| x >= threshold).count();
        let total = data.len();

        if total == 0 {
            0.0
        } else {
            1.0 - (above_threshold as f64 / total as f64 - 0.5).abs() * 2.0
        }
    }

    /// Get current adaptive thresholds
    pub fn get_thresholds(&self) -> &AdaptiveThresholds {
        &self.thresholds
    }

    /// Update thresholds based on performance feedback
    pub fn update_thresholds(&mut self, feedback: &QualityMetrics) {
        self.performance_history.push(feedback.clone());

        // Simple adaptation based on precision and recall
        if feedback.precision < 0.8 {
            // Too many false positives, increase threshold
            self.thresholds.similarity_threshold *= 1.1;
        } else if feedback.recall < 0.8 {
            // Too many false negatives, decrease threshold
            self.thresholds.similarity_threshold *= 0.9;
        }

        // Clamp to valid range
        self.thresholds.similarity_threshold =
            self.thresholds.similarity_threshold.max(0.1).min(0.95);

        // Update stability metric
        self.update_stability_metric();
    }

    /// Update stability metric based on recent performance
    fn update_stability_metric(&mut self) {
        if self.performance_history.len() < 2 {
            return;
        }

        // Calculate variance in recent F1 scores
        let recent_f1: Vec<f64> = self
            .performance_history
            .iter()
            .rev()
            .take(10)
            .map(|m| m.f1_score)
            .collect();

        if recent_f1.len() > 1 {
            let mean_f1 = recent_f1.iter().sum::<f64>() / recent_f1.len() as f64;
            let variance = recent_f1
                .iter()
                .map(|f1| (f1 - mean_f1).powi(2))
                .sum::<f64>()
                / (recent_f1.len() - 1) as f64;

            // Stability is inverse of variance (higher stability = lower variance)
            self.thresholds.stability_metric = 1.0 / (1.0 + variance);
        }
    }

    /// Check if recalibration is needed
    pub fn needs_recalibration(&self) -> bool {
        // Recalibrate if stability is low or it's been a long time
        let time_since_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - self.thresholds.last_updated;

        self.thresholds.stability_metric < 0.7 || time_since_update > 3600 // 1 hour
    }

    /// Generate calibration report
    pub fn generate_report(&self) -> CalibrationReport {
        CalibrationReport {
            current_thresholds: self.thresholds.clone(),
            performance_trend: self.calculate_performance_trend(),
            recommendations: self.generate_recommendations(),
            last_calibration: self.thresholds.last_updated,
            stability_score: self.thresholds.stability_metric,
        }
    }

    /// Calculate performance trend
    fn calculate_performance_trend(&self) -> f64 {
        if self.performance_history.len() < 2 {
            return 0.0;
        }

        let recent = &self.performance_history[self.performance_history.len() - 1];
        let earlier = &self.performance_history[0];

        recent.f1_score - earlier.f1_score
    }

    /// Generate recommendations based on current state
    fn generate_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        if self.thresholds.stability_metric < 0.5 {
            recommendations
                .push("Consider increasing sample size for more stable calibration".to_string());
        }

        if self.thresholds.similarity_threshold > 0.9 {
            recommendations
                .push("Very high similarity threshold may miss valid clones".to_string());
        }

        if self.thresholds.similarity_threshold < 0.3 {
            recommendations
                .push("Very low similarity threshold may produce false positives".to_string());
        }

        if self.performance_history.is_empty() {
            recommendations.push("No performance data available for optimization".to_string());
        }

        recommendations
    }
}

/// Result of calibration process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationResult {
    pub threshold: f64,
    pub quality_score: f64,
    pub confidence: f64,
    pub iterations: usize,
    pub convergence_achieved: bool,
    pub performance_metrics: QualityMetrics,
}

/// Calibration report for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationReport {
    pub current_thresholds: AdaptiveThresholds,
    pub performance_trend: f64,
    pub recommendations: Vec<String>,
    pub last_calibration: u64,
    pub stability_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calibration_engine_creation() {
        let engine = AutoCalibrationEngine::new();
        let thresholds = engine.get_thresholds();
        assert!(thresholds.similarity_threshold > 0.0);
        assert!(thresholds.similarity_threshold <= 1.0);
    }

    #[test]
    fn test_calibration() {
        let mut engine = AutoCalibrationEngine::new();
        let sample_data = vec![0.1, 0.2, 0.3, 0.8, 0.9, 1.0];
        let result = engine.calibrate(&sample_data, 0.8);

        assert!(result.threshold >= 0.0);
        assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
    }

    #[test]
    fn test_threshold_adaptation() {
        let mut engine = AutoCalibrationEngine::new();
        let initial_threshold = engine.get_thresholds().similarity_threshold;

        // Low precision should increase threshold
        let low_precision = QualityMetrics {
            precision: 0.5,
            recall: 0.9,
            f1_score: 0.65,
            ..Default::default()
        };

        engine.update_thresholds(&low_precision);
        let new_threshold = engine.get_thresholds().similarity_threshold;

        assert!(new_threshold > initial_threshold);
    }

    #[test]
    fn test_recalibration_need() {
        let mut engine = AutoCalibrationEngine::new();

        // Set the engine to a stable initial state
        engine.thresholds.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        engine.thresholds.stability_metric = 0.8; // High stability

        // Initially should not need recalibration
        assert!(!engine.needs_recalibration());

        // After adding unstable performance, should need recalibration
        engine.thresholds.stability_metric = 0.5; // Low stability
        assert!(engine.needs_recalibration());
    }
}
