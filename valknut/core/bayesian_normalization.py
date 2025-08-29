"""
Enhanced normalization system with Bayesian priors for zero-variance fallbacks.

This module provides intelligent fallback strategies when features have zero variance,
using domain knowledge and Bayesian reasoning to generate informative normalized values.
"""

import logging
import numpy as np
from typing import Dict, List, Tuple, Optional, Any
from dataclasses import dataclass
from enum import Enum

from valknut.core.featureset import FeatureVector

logger = logging.getLogger(__name__)


class VarianceConfidence(Enum):
    """Confidence levels for variance estimation."""
    HIGH = "high"        # >50 samples with good variance
    MEDIUM = "medium"    # 10-50 samples with some variance  
    LOW = "low"          # 5-10 samples with minimal variance
    VERY_LOW = "very_low"  # 2-5 samples
    INSUFFICIENT = "insufficient"  # <2 samples or zero variance


@dataclass
class FeaturePrior:
    """Bayesian prior knowledge for a feature."""
    name: str
    
    # Prior distribution parameters (Beta distribution)
    alpha: float = 1.0  # Success count + 1
    beta: float = 1.0   # Failure count + 1
    
    # Expected range based on domain knowledge
    expected_min: float = 0.0
    expected_max: float = 1.0
    expected_mean: float = 0.5
    
    # Variance confidence parameters
    min_samples_for_confidence: int = 10
    variance_threshold: float = 0.01
    
    # Feature-specific metadata
    feature_type: str = "generic"  # complexity, centrality, cycles, etc.
    higher_is_worse: bool = True
    typical_distribution: str = "right_skewed"  # right_skewed, normal, bimodal


class BayesianNormalizer:
    """Enhanced normalizer with Bayesian priors for intelligent fallbacks."""
    
    def __init__(self, scheme: str = "robust_bayesian"):
        self.scheme = scheme
        self._statistics = {}
        self._priors = self._initialize_feature_priors()
        self._variance_confidence = {}
        
    def _initialize_feature_priors(self) -> Dict[str, FeaturePrior]:
        """Initialize domain-specific priors for common features."""
        priors = {}
        
        # Complexity features - typically right-skewed, most functions are simple
        complexity_features = [
            ("cyclomatic", 1.0, 20.0, 3.0, "right_skewed"),
            ("cognitive", 0.0, 50.0, 5.0, "right_skewed"), 
            ("max_nesting", 0.0, 10.0, 2.0, "right_skewed"),
            ("param_count", 0.0, 15.0, 3.0, "right_skewed"),
            ("branch_fanout", 0.0, 10.0, 2.0, "right_skewed"),
        ]
        
        for name, min_val, max_val, mean_val, dist in complexity_features:
            # Use domain knowledge to set Beta parameters
            # More weight on lower values (right-skewed)
            priors[name] = FeaturePrior(
                name=name,
                alpha=2.0,  # Slight preference for lower complexity
                beta=5.0,   # Strong preference against high complexity
                expected_min=min_val,
                expected_max=max_val,
                expected_mean=mean_val,
                feature_type="complexity",
                typical_distribution=dist,
                variance_threshold=0.1
            )
        
        # Graph centrality features - often zero with occasional spikes
        centrality_features = [
            ("betweenness_approx", 0.0, 1.0, 0.1, "highly_skewed"),
            ("fan_in", 0.0, 50.0, 2.0, "right_skewed"),
            ("fan_out", 0.0, 20.0, 3.0, "right_skewed"),
            ("closeness", 0.0, 1.0, 0.3, "bimodal"),
            ("eigenvector", 0.0, 1.0, 0.2, "highly_skewed"),
        ]
        
        for name, min_val, max_val, mean_val, dist in centrality_features:
            priors[name] = FeaturePrior(
                name=name,
                alpha=1.0,  # Many nodes have zero centrality
                beta=10.0,  # Strong preference for low centrality
                expected_min=min_val,
                expected_max=max_val,
                expected_mean=mean_val,
                feature_type="centrality",
                typical_distribution=dist,
                variance_threshold=0.05
            )
        
        # Cycle features - binary or small integers
        cycle_features = [
            ("in_cycle", 0.0, 1.0, 0.2, "bernoulli"),
            ("cycle_size", 0.0, 20.0, 0.5, "right_skewed"),
        ]
        
        for name, min_val, max_val, mean_val, dist in cycle_features:
            priors[name] = FeaturePrior(
                name=name,
                alpha=1.0,
                beta=4.0,  # Most code is not in cycles
                expected_min=min_val,
                expected_max=max_val,
                expected_mean=mean_val,
                feature_type="cycles",
                typical_distribution=dist,
                variance_threshold=0.02
            )
        
        # Clone/duplication features
        clone_features = [
            ("clone_mass", 0.0, 1.0, 0.1, "right_skewed"),
            ("similarity", 0.0, 1.0, 0.3, "bimodal"),
        ]
        
        for name, min_val, max_val, mean_val, dist in clone_features:
            priors[name] = FeaturePrior(
                name=name,
                alpha=1.0,
                beta=8.0,  # Most code has low duplication
                expected_min=min_val,
                expected_max=max_val,
                expected_mean=mean_val,
                feature_type="clones",
                typical_distribution=dist,
                variance_threshold=0.1
            )
        
        return priors
    
    def fit(self, feature_vectors: List[FeatureVector]) -> None:
        """Fit normalizer with Bayesian variance estimation."""
        if not feature_vectors:
            logger.warning("No feature vectors provided for Bayesian fitting")
            return
        
        # Collect feature values
        feature_values: Dict[str, List[float]] = {}
        for vector in feature_vectors:
            for feature_name, value in vector.features.items():
                if feature_name not in feature_values:
                    feature_values[feature_name] = []
                feature_values[feature_name].append(value)
        
        # Calculate statistics with Bayesian enhancement
        for feature_name, values in feature_values.items():
            if not values:
                continue
                
            values_array = np.array(values)
            n_samples = len(values)
            
            # Calculate basic statistics
            empirical_stats = self._calculate_empirical_stats(values_array)
            
            # Get or create prior for this feature
            prior = self._priors.get(feature_name, self._create_generic_prior(feature_name))
            
            # Calculate variance confidence
            confidence = self._assess_variance_confidence(values_array, prior)
            self._variance_confidence[feature_name] = confidence
            
            # Combine empirical data with prior knowledge
            posterior_stats = self._calculate_posterior_stats(
                empirical_stats, prior, n_samples, confidence
            )
            
            self._statistics[feature_name] = {
                **posterior_stats,
                "n_samples": n_samples,
                "confidence": confidence.value,
                "prior_weight": self._calculate_prior_weight(n_samples, confidence),
                "empirical_variance": empirical_stats.get("variance", 0.0),
                "posterior_variance": posterior_stats.get("variance", 0.0),
            }
        
        # Log Bayesian diagnostics
        self._log_bayesian_diagnostics()
    
    def _calculate_empirical_stats(self, values: np.ndarray) -> Dict[str, float]:
        """Calculate empirical statistics from observed data."""
        return {
            "mean": np.mean(values),
            "std": np.std(values),
            "variance": np.var(values),
            "min": np.min(values),
            "max": np.max(values),
            "median": np.median(values),
            "iqr": np.percentile(values, 75) - np.percentile(values, 25),
            "range": np.max(values) - np.min(values),
        }
    
    def _assess_variance_confidence(self, values: np.ndarray, prior: FeaturePrior) -> VarianceConfidence:
        """Assess confidence in variance estimation."""
        n_samples = len(values)
        empirical_variance = np.var(values)
        
        # Insufficient samples
        if n_samples < 2:
            return VarianceConfidence.INSUFFICIENT
        elif n_samples < 5:
            return VarianceConfidence.VERY_LOW
        elif n_samples < 10:
            return VarianceConfidence.LOW
        
        # Check if variance meets threshold
        if empirical_variance < prior.variance_threshold:
            if n_samples >= prior.min_samples_for_confidence:
                return VarianceConfidence.LOW  # High confidence that variance is truly low
            else:
                return VarianceConfidence.VERY_LOW  # Might increase with more samples
        
        # Good variance with sufficient samples
        if n_samples >= prior.min_samples_for_confidence:
            return VarianceConfidence.HIGH
        else:
            return VarianceConfidence.MEDIUM
    
    def _calculate_posterior_stats(
        self, 
        empirical: Dict[str, float], 
        prior: FeaturePrior, 
        n_samples: int,
        confidence: VarianceConfidence
    ) -> Dict[str, float]:
        """Calculate posterior statistics combining empirical data with priors."""
        
        # Calculate prior weight based on confidence and sample size
        prior_weight = self._calculate_prior_weight(n_samples, confidence)
        empirical_weight = 1.0 - prior_weight
        
        # Posterior mean (weighted combination)
        posterior_mean = (
            prior_weight * prior.expected_mean + 
            empirical_weight * empirical["mean"]
        )
        
        # Posterior variance using Bayesian updating
        if confidence == VarianceConfidence.INSUFFICIENT:
            # Use pure prior when no empirical variance available
            posterior_variance = self._prior_variance_estimate(prior)
            posterior_std = np.sqrt(posterior_variance)
            posterior_range = prior.expected_max - prior.expected_min
            posterior_iqr = posterior_range * 0.5  # Rough estimate
            
        else:
            # Combine empirical and prior variance
            prior_variance = self._prior_variance_estimate(prior)
            posterior_variance = (
                prior_weight * prior_variance +
                empirical_weight * empirical["variance"]
            )
            posterior_std = np.sqrt(posterior_variance)
            
            # Adjust range and IQR based on posterior
            posterior_range = max(
                empirical["range"],
                (prior.expected_max - prior.expected_min) * prior_weight
            )
            
            posterior_iqr = max(
                empirical["iqr"],
                posterior_std * 1.35  # Approximate IQR for normal distribution
            )
        
        return {
            "mean": posterior_mean,
            "std": posterior_std,
            "variance": posterior_variance,
            "min": min(empirical["min"], prior.expected_min),
            "max": max(empirical["max"], prior.expected_max),
            "range": posterior_range,
            "iqr": posterior_iqr,
            "median": posterior_mean,  # Approximate
        }
    
    def _calculate_prior_weight(self, n_samples: int, confidence: VarianceConfidence) -> float:
        """Calculate how much weight to give to prior vs empirical data."""
        if confidence == VarianceConfidence.INSUFFICIENT:
            return 1.0  # Pure prior
        elif confidence == VarianceConfidence.VERY_LOW:
            return 0.8  # Mostly prior
        elif confidence == VarianceConfidence.LOW:
            return 0.6  # More prior than empirical
        elif confidence == VarianceConfidence.MEDIUM:
            return 0.3  # More empirical than prior
        else:  # HIGH
            return 0.1  # Mostly empirical
    
    def _prior_variance_estimate(self, prior: FeaturePrior) -> float:
        """Estimate variance from prior knowledge using Beta distribution."""
        # For Beta(α, β), variance = αβ/[(α+β)²(α+β+1)]
        alpha, beta = prior.alpha, prior.beta
        beta_variance = (alpha * beta) / ((alpha + beta)**2 * (alpha + beta + 1))
        
        # Scale to feature range
        feature_range = prior.expected_max - prior.expected_min
        return beta_variance * (feature_range ** 2)
    
    def _create_generic_prior(self, feature_name: str) -> FeaturePrior:
        """Create a generic prior for unknown features."""
        return FeaturePrior(
            name=feature_name,
            alpha=1.0,
            beta=1.0,
            expected_min=0.0,
            expected_max=1.0,
            expected_mean=0.3,  # Slight bias toward lower values
            feature_type="unknown"
        )
    
    def _log_bayesian_diagnostics(self) -> None:
        """Log Bayesian diagnostic information."""
        logger.info("=== BAYESIAN NORMALIZATION DIAGNOSTICS ===")
        
        confidence_counts = {}
        for feature_name, stats in self._statistics.items():
            confidence = stats["confidence"]
            confidence_counts[confidence] = confidence_counts.get(confidence, 0) + 1
            
            logger.info(
                f"  {feature_name}: "
                f"samples={stats['n_samples']}, "
                f"confidence={confidence}, "
                f"prior_weight={stats['prior_weight']:.2f}, "
                f"emp_var={stats['empirical_variance']:.4f}, "
                f"post_var={stats['posterior_variance']:.4f}"
            )
        
        logger.info(f"Confidence distribution: {confidence_counts}")
        
        # Warn about low confidence features
        low_confidence = [
            name for name, stats in self._statistics.items() 
            if stats["confidence"] in ["insufficient", "very_low"]
        ]
        
        if low_confidence:
            logger.warning(f"⚠️  Low confidence features: {low_confidence}")
            logger.warning("Consider increasing sample diversity or using domain priors")
    
    def normalize(self, feature_vector: FeatureVector) -> FeatureVector:
        """Normalize using Bayesian posterior statistics."""
        normalized = FeatureVector(entity_id=feature_vector.entity_id)
        normalized.features = feature_vector.features.copy()
        normalized.metadata = feature_vector.metadata.copy()
        
        for feature_name, value in feature_vector.features.items():
            if feature_name not in self._statistics:
                # Use prior-only normalization for unknown features
                prior = self._priors.get(feature_name, self._create_generic_prior(feature_name))
                normalized_value = self._normalize_with_prior_only(value, prior)
            else:
                stats = self._statistics[feature_name]
                normalized_value = self._normalize_with_posterior(value, stats)
            
            # Clip to bounds
            normalized_value = np.clip(normalized_value, 0.0, 1.0)
            normalized.normalized_features[feature_name] = float(normalized_value)
        
        return normalized
    
    def _normalize_with_posterior(self, value: float, stats: Dict[str, float]) -> float:
        """Normalize using posterior statistics."""
        if self.scheme == "robust_bayesian":
            return self._normalize_robust_bayesian(value, stats)
        elif self.scheme == "minmax_bayesian":
            return self._normalize_minmax_bayesian(value, stats)
        else:  # zscore_bayesian
            return self._normalize_zscore_bayesian(value, stats)
    
    def _normalize_robust_bayesian(self, value: float, stats: Dict[str, float]) -> float:
        """Robust normalization with Bayesian fallback."""
        median = stats["median"]
        iqr = stats["iqr"]
        
        if iqr <= 0:
            # Use Bayesian confidence weighting instead of flat 0.5
            confidence = stats["confidence"]
            if confidence == "insufficient":
                # Use prior expectation
                return self._value_to_unit_interval(value, stats["min"], stats["max"])
            else:
                # Use median as fallback but with confidence-based spread
                min_val, max_val = stats["min"], stats["max"]
                if max_val > min_val:
                    return (value - min_val) / (max_val - min_val)
                else:
                    return 0.5  # True flat case
        
        # Normal robust normalization
        z_score = (value - median) / (1.5 * iqr)
        z_score = np.clip(z_score, -3, 3)
        return (z_score + 3) / 6
    
    def _normalize_minmax_bayesian(self, value: float, stats: Dict[str, float]) -> float:
        """Min-max normalization with Bayesian range estimation."""
        min_val = stats["min"]
        range_val = stats["range"]
        
        if range_val <= 0:
            # Use posterior mean as informed fallback
            return self._confidence_weighted_fallback(stats)
        
        return (value - min_val) / range_val
    
    def _normalize_zscore_bayesian(self, value: float, stats: Dict[str, float]) -> float:
        """Z-score normalization with Bayesian variance estimation."""
        mean = stats["mean"]
        std = stats["std"]
        
        if std <= 0:
            # Use Bayesian confidence-weighted fallback
            return self._confidence_weighted_fallback(stats)
        
        z_score = (value - mean) / std
        z_score = np.clip(z_score, -3, 3)
        return (z_score + 3) / 6
    
    def _confidence_weighted_fallback(self, stats: Dict[str, float]) -> float:
        """Generate confidence-weighted fallback value."""
        confidence = stats["confidence"]
        mean = stats["mean"]
        min_val, max_val = stats["min"], stats["max"]
        
        # Convert mean to [0,1] interval
        if max_val > min_val:
            base_value = (mean - min_val) / (max_val - min_val)
        else:
            base_value = 0.5
        
        # Add confidence-based noise to break ties intelligently
        if confidence == "high":
            noise_factor = 0.02  # Very small noise
        elif confidence == "medium":
            noise_factor = 0.05
        elif confidence == "low":
            noise_factor = 0.1
        else:  # very_low or insufficient
            noise_factor = 0.15
        
        # Add structured noise based on posterior variance
        posterior_var = stats.get("posterior_variance", 0.01)
        noise = np.random.normal(0, noise_factor * np.sqrt(posterior_var))
        
        return np.clip(base_value + noise, 0.0, 1.0)
    
    def _normalize_with_prior_only(self, value: float, prior: FeaturePrior) -> float:
        """Normalize using only prior knowledge (for unknown features)."""
        # Simple min-max with prior range
        range_val = prior.expected_max - prior.expected_min
        if range_val <= 0:
            return 0.5
        
        normalized = (value - prior.expected_min) / range_val
        return np.clip(normalized, 0.0, 1.0)
    
    def _value_to_unit_interval(self, value: float, min_val: float, max_val: float) -> float:
        """Convert value to [0,1] interval."""
        if max_val <= min_val:
            return 0.5
        return np.clip((value - min_val) / (max_val - min_val), 0.0, 1.0)
    
    def get_feature_diagnostics(self) -> Dict[str, Dict[str, Any]]:
        """Get comprehensive diagnostics for all features."""
        diagnostics = {}
        
        for feature_name, stats in self._statistics.items():
            prior = self._priors.get(feature_name)
            diagnostics[feature_name] = {
                "confidence": stats["confidence"],
                "n_samples": stats["n_samples"], 
                "empirical_variance": stats["empirical_variance"],
                "posterior_variance": stats["posterior_variance"],
                "prior_weight": stats["prior_weight"],
                "prior_type": prior.feature_type if prior else "unknown",
                "fallback_quality": "informative" if stats["posterior_variance"] > 0.001 else "flat"
            }
        
        return diagnostics