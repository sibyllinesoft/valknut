"""
Feature extraction framework and registry.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Protocol

from valknut.lang.common_ast import Entity, ParseIndex


@dataclass
class FeatureDefinition:
    """Definition of a feature."""
    
    name: str
    description: str
    data_type: type
    min_value: Optional[float] = None
    max_value: Optional[float] = None
    default_value: float = 0.0
    higher_is_worse: bool = True  # True if higher values indicate more refactoring need


class FeatureExtractor(Protocol):
    """Protocol for feature extractors."""
    
    @property
    def name(self) -> str:
        """Feature extractor name."""
        ...
    
    @property
    def features(self) -> List[FeatureDefinition]:
        """List of features this extractor provides."""
        ...
    
    def extract(self, entity: Entity, index: ParseIndex) -> Dict[str, float]:
        """
        Extract features for an entity.
        
        Args:
            entity: Entity to analyze
            index: Parse index
            
        Returns:
            Dictionary of feature name -> value
        """
        ...
    
    def supports_entity(self, entity: Entity) -> bool:
        """Check if this extractor supports the given entity type."""
        ...


class BaseFeatureExtractor(ABC):
    """Base class for feature extractors."""
    
    def __init__(self) -> None:
        self._feature_defs: Dict[str, FeatureDefinition] = {}
        self._initialize_features()
    
    @abstractmethod
    def _initialize_features(self) -> None:
        """Initialize feature definitions (implemented by subclasses)."""
        ...
    
    @property
    @abstractmethod
    def name(self) -> str:
        """Feature extractor name."""
        ...
    
    @property
    def features(self) -> List[FeatureDefinition]:
        """List of features this extractor provides."""
        return list(self._feature_defs.values())
    
    def get_feature_definition(self, feature_name: str) -> Optional[FeatureDefinition]:
        """Get definition for a specific feature."""
        return self._feature_defs.get(feature_name)
    
    def _add_feature(
        self,
        name: str,
        description: str,
        data_type: type = float,
        min_value: Optional[float] = None,
        max_value: Optional[float] = None,
        default_value: float = 0.0,
        higher_is_worse: bool = True,
    ) -> None:
        """Add a feature definition."""
        self._feature_defs[name] = FeatureDefinition(
            name=name,
            description=description,
            data_type=data_type,
            min_value=min_value,
            max_value=max_value,
            default_value=default_value,
            higher_is_worse=higher_is_worse,
        )
    
    @abstractmethod
    def extract(self, entity: Entity, index: ParseIndex) -> Dict[str, float]:
        """Extract features for an entity."""
        ...
    
    def supports_entity(self, entity: Entity) -> bool:
        """Check if this extractor supports the given entity type."""
        return True  # By default, support all entities
    
    def _safe_extract(
        self, 
        entity: Entity, 
        index: ParseIndex,
        feature_name: str,
        extraction_func,
    ) -> float:
        """
        Safely extract a feature with error handling.
        
        Args:
            entity: Entity to analyze
            index: Parse index
            feature_name: Name of feature being extracted
            extraction_func: Function to extract the feature
            
        Returns:
            Feature value or default value if extraction fails
        """
        try:
            return extraction_func()
        except Exception:
            # Log warning and return default
            feature_def = self._feature_defs.get(feature_name)
            return feature_def.default_value if feature_def else 0.0


@dataclass
class FeatureVector:
    """Container for an entity's feature vector."""
    
    entity_id: str
    features: Dict[str, float] = field(default_factory=dict)
    normalized_features: Dict[str, float] = field(default_factory=dict)
    metadata: Dict[str, Any] = field(default_factory=dict)
    refactoring_suggestions: List[Any] = field(default_factory=list)
    
    def get_feature(self, name: str, normalized: bool = False) -> Optional[float]:
        """Get feature value."""
        if normalized:
            return self.normalized_features.get(name)
        return self.features.get(name)
    
    def set_feature(self, name: str, value: float, normalized: bool = False) -> None:
        """Set feature value."""
        if normalized:
            self.normalized_features[name] = value
        else:
            self.features[name] = value
    
    def get_all_features(self, normalized: bool = False) -> Dict[str, float]:
        """Get all features."""
        return self.normalized_features if normalized else self.features
    
    def add_refactoring_suggestion(self, suggestion: Any) -> None:
        """Add a refactoring suggestion."""
        self.refactoring_suggestions.append(suggestion)
    
    def get_refactoring_suggestions(self, severity: str = None) -> List[Any]:
        """Get refactoring suggestions, optionally filtered by severity."""
        if severity is None:
            return self.refactoring_suggestions
        return [s for s in self.refactoring_suggestions if s.severity == severity]
    
    def has_high_priority_refactoring(self) -> bool:
        """Check if entity has high-priority refactoring suggestions."""
        return any(s.severity == "high" for s in self.refactoring_suggestions)


class FeatureExtractorRegistry:
    """Registry for feature extractors."""
    
    def __init__(self) -> None:
        self._extractors: Dict[str, FeatureExtractor] = {}
        self._feature_to_extractor: Dict[str, str] = {}
    
    def register(self, extractor: FeatureExtractor) -> None:
        """Register a feature extractor."""
        self._extractors[extractor.name] = extractor
        
        # Map features to extractor
        for feature_def in extractor.features:
            self._feature_to_extractor[feature_def.name] = extractor.name
    
    def get_extractor(self, name: str) -> Optional[FeatureExtractor]:
        """Get extractor by name."""
        return self._extractors.get(name)
    
    def get_extractors(self) -> List[FeatureExtractor]:
        """Get all extractors."""
        return list(self._extractors.values())
    
    def get_extractor_for_feature(self, feature_name: str) -> Optional[FeatureExtractor]:
        """Get the extractor that provides a specific feature."""
        extractor_name = self._feature_to_extractor.get(feature_name)
        if extractor_name:
            return self._extractors.get(extractor_name)
        return None
    
    def get_all_features(self) -> List[FeatureDefinition]:
        """Get all feature definitions from all extractors."""
        features = []
        for extractor in self._extractors.values():
            features.extend(extractor.features)
        return features
    
    def extract_all_features(
        self, 
        entity: Entity, 
        index: ParseIndex,
        extractor_names: Optional[List[str]] = None,
    ) -> FeatureVector:
        """
        Extract features using all or specified extractors.
        
        Args:
            entity: Entity to analyze
            index: Parse index
            extractor_names: Optional list of specific extractors to use
            
        Returns:
            Feature vector
        """
        vector = FeatureVector(entity_id=entity.id)
        
        extractors_to_use = self._extractors.values()
        if extractor_names:
            extractors_to_use = [
                self._extractors[name] 
                for name in extractor_names 
                if name in self._extractors
            ]
        
        for extractor in extractors_to_use:
            if extractor.supports_entity(entity):
                try:
                    features = extractor.extract(entity, index)
                    vector.features.update(features)
                    
                    # Check if entity has refactoring suggestions (from RefactoringAnalyzer)
                    if hasattr(entity, 'refactoring_suggestions'):
                        vector.refactoring_suggestions.extend(entity.refactoring_suggestions)
                        
                except Exception as e:
                    # Log error and continue with other extractors
                    vector.metadata[f"{extractor.name}_error"] = str(e)
        
        return vector


# Global feature extractor registry
feature_registry = FeatureExtractorRegistry()