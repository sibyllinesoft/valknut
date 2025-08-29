"""
Feature normalization and scoring system.
"""

import logging
from typing import Dict, List, Optional, Tuple

import numpy as np
from scipy import stats

from valknut.core.config import WeightsConfig, NormalizationConfig
from valknut.core.featureset import FeatureVector
from valknut.core.bayesian_normalization import BayesianNormalizer

logger = logging.getLogger(__name__)


class FeatureNormalizer:
    """Normalizes features to [0, 1] range using various schemes."""
    
    def __init__(self, config: NormalizationConfig) -> None:
        self.config = config
        self._statistics: Dict[str, Dict[str, float]] = {}
        
        # Initialize Bayesian normalizer if using Bayesian schemes
        if config.scheme.endswith('_bayesian') or config.use_bayesian_fallbacks:
            self._bayesian_normalizer = BayesianNormalizer(scheme=config.scheme)
        else:
            self._bayesian_normalizer = None
    
    def fit(self, feature_vectors: List[FeatureVector]) -> None:
        """
        Fit normalizer to feature vectors.
        
        Args:
            feature_vectors: List of feature vectors to fit on
        """
        if not feature_vectors:
            logger.warning("No feature vectors provided for normalization fitting")
            return
        
        # If using Bayesian normalizer, delegate fitting to it
        if self._bayesian_normalizer is not None:
            self._bayesian_normalizer.fit(feature_vectors)
            
            # Optionally report confidence diagnostics
            if self.config.confidence_reporting:
                self._report_bayesian_diagnostics()
            return
        
        # Traditional normalization fitting
        # Collect all feature values
        feature_values: Dict[str, List[float]] = {}
        
        for vector in feature_vectors:
            for feature_name, value in vector.features.items():
                if feature_name not in feature_values:
                    feature_values[feature_name] = []
                feature_values[feature_name].append(value)
        
        # Calculate statistics for each feature
        for feature_name, values in feature_values.items():
            if not values:
                continue
            
            values_array = np.array(values)
            
            if self.config.scheme == "robust":
                # Robust statistics using median and IQR
                median = np.median(values_array)
                q75, q25 = np.percentile(values_array, [75, 25])
                iqr = q75 - q25
                
                self._statistics[feature_name] = {
                    "median": median,
                    "iqr": iqr,
                    "min": np.min(values_array),
                    "max": np.max(values_array),
                }
                
            elif self.config.scheme == "minmax":
                # Min-max normalization
                min_val = np.min(values_array)
                max_val = np.max(values_array)
                
                self._statistics[feature_name] = {
                    "min": min_val,
                    "max": max_val,
                    "range": max_val - min_val,
                }
                
            elif self.config.scheme == "zscore":
                # Z-score normalization
                mean = np.mean(values_array)
                std = np.std(values_array)
                
                self._statistics[feature_name] = {
                    "mean": mean,
                    "std": std,
                    "min": np.min(values_array),
                    "max": np.max(values_array),
                }
                
        # Debug logging to verify feature variance
        logger.info("Feature statistics after normalization fitting:")
        for feature_name, stats in self._statistics.items():
            if self.config.scheme == "robust":
                variance_metric = stats.get("iqr", 0)
                variance_label = "IQR"
            elif self.config.scheme == "zscore":
                variance_metric = stats.get("std", 0)
                variance_label = "std"
            else:  # minmax
                variance_metric = stats.get("range", 0)
                variance_label = "range"
            
            logger.info(f"  {feature_name}: min={stats.get('min', 'N/A'):.3f}, max={stats.get('max', 'N/A'):.3f}, {variance_label}={variance_metric:.3f}")
            
            if variance_metric <= 0:
                logger.warning(f"  ⚠️  {feature_name} has zero variance ({variance_label}={variance_metric}) - will normalize to 0.5!")
    
    def _report_bayesian_diagnostics(self) -> None:
        """Report Bayesian normalization diagnostics."""
        if self._bayesian_normalizer is None:
            return
        
        diagnostics = self._bayesian_normalizer.get_feature_diagnostics()
        logger.info("=== BAYESIAN NORMALIZATION SUMMARY ===")
        
        confidence_counts = {}
        informative_fallbacks = 0
        
        for feature_name, diag in diagnostics.items():
            confidence = diag["confidence"]
            confidence_counts[confidence] = confidence_counts.get(confidence, 0) + 1
            
            if diag["fallback_quality"] == "informative":
                informative_fallbacks += 1
            
            logger.info(
                f"  {feature_name}: "
                f"confidence={confidence}, "
                f"samples={diag['n_samples']}, "
                f"fallback={diag['fallback_quality']}"
            )
        
        logger.info(f"📊 Confidence distribution: {confidence_counts}")
        logger.info(f"✅ Informative fallbacks: {informative_fallbacks}/{len(diagnostics)} features")
        
        if informative_fallbacks > len(diagnostics) * 0.5:
            logger.info("🎯 Bayesian priors providing good variance estimates for most features")
        else:
            logger.warning("⚠️  Many features still have flat fallbacks - consider more diverse data")
    
    def normalize(self, feature_vector: FeatureVector) -> FeatureVector:
        """
        Normalize a feature vector.
        
        Args:
            feature_vector: Feature vector to normalize
            
        Returns:
            Feature vector with normalized features
        """
        # If using Bayesian normalizer, delegate to it
        if self._bayesian_normalizer is not None:
            return self._bayesian_normalizer.normalize(feature_vector)
        
        # Traditional normalization
        normalized = FeatureVector(entity_id=feature_vector.entity_id)
        normalized.features = feature_vector.features.copy()
        normalized.metadata = feature_vector.metadata.copy()
        
        for feature_name, value in feature_vector.features.items():
            if feature_name not in self._statistics:
                # No statistics available, keep original value
                normalized.normalized_features[feature_name] = value
                continue
            
            stats_dict = self._statistics[feature_name]
            
            try:
                if self.config.scheme == "robust":
                    normalized_value = self._normalize_robust(value, stats_dict)
                elif self.config.scheme == "minmax":
                    normalized_value = self._normalize_minmax(value, stats_dict)
                elif self.config.scheme == "zscore":
                    normalized_value = self._normalize_zscore(value, stats_dict)
                else:
                    normalized_value = value
                
                # Clip to bounds
                min_bound, max_bound = self.config.clip_bounds
                normalized_value = np.clip(normalized_value, min_bound, max_bound)
                
                normalized.normalized_features[feature_name] = float(normalized_value)
                
            except Exception as e:
                logger.warning(f"Failed to normalize {feature_name}: {e}")
                normalized.normalized_features[feature_name] = value
        
        return normalized
    
    def _normalize_robust(self, value: float, stats: Dict[str, float]) -> float:
        """Normalize using robust statistics (median, IQR)."""
        median = stats["median"]
        iqr = stats["iqr"]
        
        if iqr <= 0:
            return 0.5  # Middle value if no variation
        
        # Robust z-score: (value - median) / (1.5 * IQR)
        z_score = (value - median) / (1.5 * iqr)
        
        # Clamp to [-3, 3] and map to [0, 1]
        z_score = np.clip(z_score, -3, 3)
        normalized = (z_score + 3) / 6
        
        return normalized
    
    def _normalize_minmax(self, value: float, stats: Dict[str, float]) -> float:
        """Normalize using min-max scaling."""
        min_val = stats["min"]
        range_val = stats.get("range", stats["max"] - min_val)
        
        if range_val <= 0:
            return 0.0
        
        return (value - min_val) / range_val
    
    def _normalize_zscore(self, value: float, stats: Dict[str, float]) -> float:
        """Normalize using z-score."""
        mean = stats["mean"]
        std = stats["std"]
        
        if std <= 0:
            return 0.5  # Middle value if no variation
        
        # Calculate z-score
        z_score = (value - mean) / std
        
        # Clamp to [-3, 3] and map to [0, 1]
        z_score = np.clip(z_score, -3, 3)
        normalized = (z_score + 3) / 6
        
        return normalized


class WeightedScorer:
    """Calculates weighted scores from normalized features."""
    
    def __init__(self, config: WeightsConfig) -> None:
        self.config = config
        self._normalized_weights = self._normalize_weights()
    
    def _normalize_weights(self) -> Dict[str, float]:
        """Normalize weights to sum to 1.0."""
        weight_dict = {
            "complexity": self.config.complexity,
            "clone_mass": self.config.clone_mass, 
            "centrality": self.config.centrality,
            "cycles": self.config.cycles,
            "type_friction": self.config.type_friction,
            "smell_prior": self.config.smell_prior,
        }
        
        total_weight = sum(weight_dict.values())
        
        if total_weight <= 0:
            logger.warning("Total weight is zero, using equal weights")
            return {k: 1.0 / len(weight_dict) for k in weight_dict}
        
        return {k: v / total_weight for k, v in weight_dict.items()}
    
    def score(self, feature_vector: FeatureVector) -> float:
        """
        Calculate weighted score for a feature vector.
        
        Args:
            feature_vector: Feature vector with normalized features
            
        Returns:
            Weighted score between 0 and 1
        """
        if not feature_vector.normalized_features:
            return 0.0
        
        total_score = 0.0
        used_weight = 0.0
        
        # Map feature categories to actual features
        feature_mapping = self._get_feature_mapping()
        
        for category, weight in self._normalized_weights.items():
            if weight <= 0:
                continue
            
            category_features = feature_mapping.get(category, [])
            category_score = 0.0
            feature_count = 0
            
            for feature_name in category_features:
                if feature_name in feature_vector.normalized_features:
                    category_score += feature_vector.normalized_features[feature_name]
                    feature_count += 1
            
            if feature_count > 0:
                # Average the features in this category
                category_score /= feature_count
                total_score += weight * category_score
                used_weight += weight
        
        # Normalize by actual used weight
        if used_weight > 0:
            total_score /= used_weight
        
        return np.clip(total_score, 0.0, 1.0)
    
    def _get_feature_mapping(self) -> Dict[str, List[str]]:
        """Map weight categories to actual feature names."""
        return {
            "complexity": [
                "cyclomatic", "cognitive", "max_nesting", "param_count", "branch_fanout"
            ],
            "clone_mass": [
                "clone_mass", "clone_groups_count", "max_clone_similarity"
            ],
            "centrality": [
                "betweenness_approx", "fan_in", "fan_out", "closeness", "eigenvector"
            ],
            "cycles": [
                "in_cycle", "cycle_size"
            ],
            "type_friction": [
                "typed_coverage_ratio", "any_ratio", "casts_per_kloc", 
                "non_null_bang_ratio", "unsafe_blocks_per_kloc"
            ],
            "smell_prior": [
                "smell_score", "god_class_score", "long_method_score", "feature_envy_score"
            ],
        }
    
    def explain_score(self, feature_vector: FeatureVector) -> List[str]:
        """
        Generate explanations for why an entity got its score.
        
        Args:
            feature_vector: Feature vector to explain
            
        Returns:
            List of explanation strings
        """
        explanations = []
        feature_mapping = self._get_feature_mapping()
        
        # Get top contributing features
        contributions = []
        
        for category, weight in self._normalized_weights.items():
            if weight <= 0:
                continue
            
            category_features = feature_mapping.get(category, [])
            category_values = []
            
            for feature_name in category_features:
                if feature_name in feature_vector.normalized_features:
                    value = feature_vector.normalized_features[feature_name]
                    category_values.append((feature_name, value))
            
            if category_values:
                # Use highest feature value in category
                max_feature, max_value = max(category_values, key=lambda x: x[1])
                contribution = weight * max_value
                contributions.append((category, max_feature, max_value, contribution))
        
        # Sort by contribution (highest first)
        contributions.sort(key=lambda x: x[3], reverse=True)
        
        # Generate explanations for top contributors
        for category, feature, value, contribution in contributions[:3]:
            if value > 0.7:  # High values
                explanations.append(
                    self._get_feature_explanation(category, feature, value)
                )
            elif contribution > 0.1:  # Significant contributors
                explanations.append(
                    f"{category.replace('_', ' ').title()} score: {value:.2f}"
                )
        
        # Add special cases
        if "clone_mass" in feature_vector.normalized_features:
            clone_mass = feature_vector.normalized_features["clone_mass"]
            if clone_mass > 0.5:
                explanations.append(
                    f"High duplication (clone_mass {clone_mass:.2f})"
                )
        
        if "in_cycle" in feature_vector.normalized_features:
            if feature_vector.normalized_features["in_cycle"] > 0.5:
                cycle_size = feature_vector.normalized_features.get("cycle_size", 0)
                explanations.append(
                    f"Participates in dependency cycle (size {cycle_size:.2f})"
                )
        
        if "fan_in" in feature_vector.normalized_features:
            fan_in = feature_vector.normalized_features["fan_in"]
            if fan_in > 0.7:
                explanations.append(
                    "High inbound centrality; risky change surface"
                )
        
        return explanations[:5]  # Limit to top 5
    
    def _get_feature_explanation(self, category: str, feature: str, value: float) -> str:
        """Get human-readable explanation for a feature."""
        explanations_map = {
            "complexity": {
                "cyclomatic": f"High cyclomatic complexity ({value:.2f})",
                "cognitive": f"High cognitive complexity ({value:.2f})",
                "max_nesting": f"Deep nesting levels ({value:.2f})",
                "param_count": f"Many parameters ({value:.2f})",
            },
            "clone_mass": {
                "clone_mass": f"High duplication ratio ({value:.2f})",
                "clone_groups_count": f"Multiple clone instances ({value:.2f})",
            },
            "centrality": {
                "betweenness_approx": f"High betweenness centrality ({value:.2f})",
                "fan_in": f"Many incoming dependencies ({value:.2f})",
                "fan_out": f"Many outgoing dependencies ({value:.2f})",
            }
        }
        
        if category in explanations_map and feature in explanations_map[category]:
            return explanations_map[category][feature]
        
        return f"High {feature.replace('_', ' ')} ({value:.2f})"


class RankingSystem:
    """Handles entity ranking with tie-breaking."""
    
    def __init__(self, weights_config: WeightsConfig) -> None:
        self.scorer = WeightedScorer(weights_config)
    
    def rank_entities(
        self, 
        feature_vectors: List[FeatureVector],
        top_k: Optional[int] = None,
    ) -> List[Tuple[FeatureVector, float]]:
        """
        Rank entities by their refactor scores.
        
        Args:
            feature_vectors: List of feature vectors
            top_k: Optional limit on number of results
            
        Returns:
            List of (feature_vector, score) tuples sorted by score descending
        """
        if not feature_vectors:
            return []
        
        scored_entities = []
        
        for vector in feature_vectors:
            score = self.scorer.score(vector)
            scored_entities.append((vector, score))
        
        # Sort by score descending, then by tie-breakers
        scored_entities.sort(key=self._sort_key, reverse=True)
        
        if top_k:
            scored_entities = scored_entities[:top_k]
        
        return scored_entities
    
    def _sort_key(self, item: Tuple[FeatureVector, float]) -> Tuple[float, float, float]:
        """Generate sort key with tie-breakers."""
        vector, score = item
        
        # Primary: score
        # Tie-breaker 1: in_cycle (prefer entities in cycles)
        in_cycle = vector.normalized_features.get("in_cycle", 0.0)
        
        # Tie-breaker 2: fan_in (prefer high fan-in)
        fan_in = vector.normalized_features.get("fan_in", 0.0)
        
        return (score, in_cycle, fan_in)