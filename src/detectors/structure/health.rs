//! Entity health scoring using distributional models and logistic shaping.
//!
//! Provides health scores for code entities using:
//! - Lognormal distribution for entity sizes (AST nodes)
//! - Logistic shaping for flat→steep→saturating penalty curves

use serde::Serialize;

use super::config::{EntityHealthConfig, StructureConfig};

/// Health scorer for entity metrics
pub struct HealthScorer {
    config: EntityHealthConfig,
}

/// Health metrics for a single entity
#[derive(Debug, Clone, Serialize)]
pub struct EntityHealth {
    /// Raw metric value (AST nodes)
    pub raw_value: usize,
    /// Percentile from the configured distribution (0.0-1.0)
    pub percentile: f64,
    /// Health score after logistic shaping (0.0-1.0, higher is healthier)
    pub health: f64,
}

/// Factory and scoring methods for [`HealthScorer`].
impl HealthScorer {
    /// Creates a new health scorer from structure configuration.
    pub fn new(config: StructureConfig) -> Self {
        Self {
            config: config.entity_health,
        }
    }

    /// Creates a new health scorer from entity health configuration.
    pub fn from_entity_config(config: EntityHealthConfig) -> Self {
        Self { config }
    }

    /// Score function/method size health based on AST node count.
    pub fn score_function(&self, ast_nodes: usize) -> EntityHealth {
        self.score_with_params(ast_nodes, &self.config.function_size)
    }

    /// Score class/struct size health based on AST node count.
    pub fn score_class(&self, ast_nodes: usize) -> EntityHealth {
        self.score_with_params(ast_nodes, &self.config.class_size)
    }

    /// Score file size health based on AST node count.
    pub fn score_file(&self, ast_nodes: usize) -> EntityHealth {
        self.score_with_params(ast_nodes, &self.config.file_size)
    }

    /// Computes health score using the given entity size parameters.
    fn score_with_params(
        &self,
        ast_nodes: usize,
        params: &super::config::EntitySizeParams,
    ) -> EntityHealth {
        let percentile = lognormal_cdf(ast_nodes, params.optimal, params.percentile_95);
        let health = logistic_health(percentile, params.penalty_center, params.penalty_steepness);

        EntityHealth {
            raw_value: ast_nodes,
            percentile,
            health,
        }
    }
}

/// Compute the CDF of a lognormal distribution at the given value.
///
/// Parameters are specified as mode (optimal) and 95th percentile.
/// Returns the probability that a random variable is <= value.
pub fn lognormal_cdf(value: usize, optimal: usize, percentile_95: usize) -> f64 {
    if value == 0 {
        return 0.0;
    }
    if optimal == 0 || percentile_95 <= optimal {
        return if value >= optimal { 1.0 } else { 0.0 };
    }

    let value = value as f64;
    let optimal = optimal as f64;
    let p95 = percentile_95 as f64;

    // For lognormal: mode = exp(μ - σ²), so ln(mode) = μ - σ²
    // 95th percentile: Φ((ln(p95) - μ) / σ) = 0.95
    // So (ln(p95) - μ) / σ = 1.645 (z-score for 95th percentile)
    // Thus: ln(p95) = μ + 1.645σ
    //
    // From these: ln(p95) - ln(mode) = σ² + 1.645σ
    // Solve: σ² + 1.645σ - (ln(p95) - ln(mode)) = 0
    let log_ratio = p95.ln() - optimal.ln();
    let discriminant = 1.645_f64 * 1.645_f64 + 4.0 * log_ratio;

    if discriminant < 0.0 {
        return if value >= optimal { 1.0 } else { 0.0 };
    }

    let sigma = (-1.645 + discriminant.sqrt()) / 2.0;
    if sigma <= 0.0 {
        return if value >= optimal { 1.0 } else { 0.0 };
    }

    // μ = ln(mode) + σ²
    let mu = optimal.ln() + sigma * sigma;

    // CDF of lognormal: Φ((ln(x) - μ) / σ)
    let z = (value.ln() - mu) / sigma;
    standard_normal_cdf(z)
}

/// Computes the standard normal CDF using the error function approximation.
fn standard_normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / 2.0_f64.sqrt()))
}

/// Computes the error function using the Abramowitz and Stegun approximation.
fn erf(x: f64) -> f64 {
    // Constants for the approximation
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

    sign * y
}

/// Apply logistic shaping to convert percentile to health score.
///
/// Returns a value in [0, 1] where 1 is healthiest.
/// The curve is flat for low percentiles, steep near `center`, and saturates for high percentiles.
///
/// - `u`: percentile from the distribution (0.0-1.0)
/// - `center`: percentile where the penalty ramp centers (e.g., 0.90)
/// - `steepness`: controls how sharp the transition is (smaller = sharper)
pub fn logistic_health(u: f64, center: f64, steepness: f64) -> f64 {
    if steepness <= 0.0 {
        // Degenerate case: step function at center
        return if u < center { 1.0 } else { 0.0 };
    }

    // penalty = sigmoid((u - center) / steepness)
    // health = 1 - penalty
    let t = (u - center) / steepness;
    let penalty = 1.0 / (1.0 + (-t).exp());
    1.0 - penalty
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lognormal_cdf_at_mode() {
        // At the mode, CDF should be less than 0.5 (mode < median for lognormal)
        let cdf = lognormal_cdf(200, 200, 800);
        assert!(cdf > 0.0 && cdf < 0.5, "CDF at mode should be < 0.5, got {}", cdf);
    }

    #[test]
    fn test_lognormal_cdf_at_95th() {
        // At the 95th percentile, CDF should be close to 0.95
        let cdf = lognormal_cdf(800, 200, 800);
        assert!(
            (cdf - 0.95).abs() < 0.02,
            "CDF at P95 should be ~0.95, got {}",
            cdf
        );
    }

    #[test]
    fn test_lognormal_cdf_monotonic() {
        let cdf_100 = lognormal_cdf(100, 200, 800);
        let cdf_200 = lognormal_cdf(200, 200, 800);
        let cdf_400 = lognormal_cdf(400, 200, 800);
        let cdf_800 = lognormal_cdf(800, 200, 800);

        assert!(cdf_100 < cdf_200, "CDF should be monotonic");
        assert!(cdf_200 < cdf_400, "CDF should be monotonic");
        assert!(cdf_400 < cdf_800, "CDF should be monotonic");
    }

    #[test]
    fn test_logistic_health_shape() {
        // Below center: health should be high (close to 1)
        let health_low = logistic_health(0.5, 0.90, 0.05);
        assert!(health_low > 0.95, "Health well below center should be ~1, got {}", health_low);

        // At center: health should be 0.5
        let health_center = logistic_health(0.90, 0.90, 0.05);
        assert!(
            (health_center - 0.5).abs() < 0.01,
            "Health at center should be 0.5, got {}",
            health_center
        );

        // Above center: health should decrease
        let health_high = logistic_health(0.99, 0.90, 0.05);
        assert!(health_high < 0.2, "Health above center should be low, got {}", health_high);
        assert!(health_high > 0.0, "Health should still be positive in the tail");

        // Verify monotonic decrease
        let health_95 = logistic_health(0.95, 0.90, 0.05);
        assert!(
            health_center > health_95 && health_95 > health_high,
            "Health should decrease monotonically"
        );
    }

    #[test]
    fn test_health_scorer_function() {
        let config = StructureConfig::default();
        let scorer = HealthScorer::new(config);

        // Small function should be healthy
        let health_small = scorer.score_function(100);
        assert!(health_small.health > 0.9, "Small function should be healthy, got {}", health_small.health);

        // Large function should be unhealthy
        let health_large = scorer.score_function(2000);
        assert!(health_large.health < 0.3, "Large function should be unhealthy, got {}", health_large.health);

        // Health should be between 0 and 1
        assert!(health_small.health >= 0.0 && health_small.health <= 1.0);
        assert!(health_large.health >= 0.0 && health_large.health <= 1.0);
    }

    #[test]
    fn test_health_scorer_class() {
        let config = StructureConfig::default();
        let scorer = HealthScorer::new(config);

        // Small class should be healthy
        let health_small = scorer.score_class(300);
        assert!(health_small.health > 0.9, "Small class should be healthy, got {}", health_small.health);

        // Large class should be unhealthy
        let health_large = scorer.score_class(5000);
        assert!(health_large.health < 0.3, "Large class should be unhealthy, got {}", health_large.health);
    }

    #[test]
    fn test_health_scorer_file() {
        let config = StructureConfig::default();
        let scorer = HealthScorer::new(config);

        // Small file should be healthy
        let health_small = scorer.score_file(1000);
        assert!(health_small.health > 0.9, "Small file should be healthy, got {}", health_small.health);

        // Large file should be unhealthy
        let health_large = scorer.score_file(10000);
        assert!(health_large.health < 0.3, "Large file should be unhealthy, got {}", health_large.health);
    }

    #[test]
    fn test_standard_normal_cdf() {
        // Known values
        assert!((standard_normal_cdf(0.0) - 0.5).abs() < 0.001);
        assert!((standard_normal_cdf(1.645) - 0.95).abs() < 0.01);
        assert!((standard_normal_cdf(-1.645) - 0.05).abs() < 0.01);
        assert!((standard_normal_cdf(1.96) - 0.975).abs() < 0.01);
    }
}
