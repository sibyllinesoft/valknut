"""
Complexity feature extractors - cyclomatic, cognitive, nesting, parameters.
"""

import re
from typing import Dict, List, Set

from valknut.core.featureset import BaseFeatureExtractor
from valknut.lang.common_ast import Entity, EntityKind, ParseIndex


class ComplexityExtractor(BaseFeatureExtractor):
    """Extractor for complexity-related features."""
    
    @property
    def name(self) -> str:
        return "complexity"
    
    def _initialize_features(self) -> None:
        """Initialize complexity features."""
        self._add_feature(
            "cyclomatic",
            "McCabe cyclomatic complexity",
            min_value=1.0,
            max_value=100.0,
            default_value=1.0,
        )
        self._add_feature(
            "cognitive", 
            "Cognitive complexity (nesting-weighted)",
            min_value=0.0,
            max_value=200.0,
            default_value=0.0,
        )
        self._add_feature(
            "max_nesting",
            "Maximum nesting depth",
            min_value=0.0,
            max_value=20.0,
            default_value=0.0,
        )
        self._add_feature(
            "param_count",
            "Number of parameters",
            min_value=0.0,
            max_value=20.0,
            default_value=0.0,
        )
        self._add_feature(
            "branch_fanout",
            "Average branches per decision point",
            min_value=0.0,
            max_value=10.0,
            default_value=0.0,
        )
    
    def supports_entity(self, entity: Entity) -> bool:
        """Support functions, methods, and files."""
        return entity.kind in {
            EntityKind.FUNCTION,
            EntityKind.METHOD,
            EntityKind.FILE,
            EntityKind.CLASS,
        }
    
    def extract(self, entity: Entity, index: ParseIndex) -> Dict[str, float]:
        """Extract complexity features."""
        features = {}
        
        # Get source code
        if entity.raw_text is None:
            # For file-level entities, aggregate from children
            if entity.kind == EntityKind.FILE:
                return self._extract_file_level(entity, index)
            else:
                return {f.name: f.default_value for f in self.features}
        
        source = entity.raw_text
        
        features["cyclomatic"] = self._safe_extract(
            entity, index, "cyclomatic",
            lambda: self._calculate_cyclomatic(source, entity.language)
        )
        
        features["cognitive"] = self._safe_extract(
            entity, index, "cognitive", 
            lambda: self._calculate_cognitive(source, entity.language)
        )
        
        features["max_nesting"] = self._safe_extract(
            entity, index, "max_nesting",
            lambda: self._calculate_max_nesting(source, entity.language)
        )
        
        features["param_count"] = self._safe_extract(
            entity, index, "param_count",
            lambda: float(len(entity.parameters))
        )
        
        features["branch_fanout"] = self._safe_extract(
            entity, index, "branch_fanout",
            lambda: self._calculate_branch_fanout(source, entity.language)
        )
        
        return features
    
    def _extract_file_level(self, entity: Entity, index: ParseIndex) -> Dict[str, float]:
        """Extract file-level complexity by aggregating from children."""
        children = index.get_children(entity.id)
        
        if not children:
            return {f.name: f.default_value for f in self.features}
        
        # Aggregate complexity from all functions/methods in file
        total_cyclomatic = 0.0
        total_cognitive = 0.0
        max_nesting = 0.0
        max_params = 0.0
        total_fanout = 0.0
        count = 0
        
        for child in children:
            if child.kind in {EntityKind.FUNCTION, EntityKind.METHOD}:
                child_features = self.extract(child, index)
                total_cyclomatic += child_features.get("cyclomatic", 1.0)
                total_cognitive += child_features.get("cognitive", 0.0)
                max_nesting = max(max_nesting, child_features.get("max_nesting", 0.0))
                max_params = max(max_params, child_features.get("param_count", 0.0))
                total_fanout += child_features.get("branch_fanout", 0.0)
                count += 1
        
        return {
            "cyclomatic": total_cyclomatic,
            "cognitive": total_cognitive,
            "max_nesting": max_nesting,
            "param_count": max_params,
            "branch_fanout": total_fanout / max(count, 1),
        }
    
    def _calculate_cyclomatic(self, source: str, language: str) -> float:
        """Calculate McCabe cyclomatic complexity."""
        # Decision points that add to cyclomatic complexity
        decision_patterns = self._get_decision_patterns(language)
        
        complexity = 1  # Base complexity
        
        for pattern in decision_patterns:
            matches = re.findall(pattern, source, re.MULTILINE | re.IGNORECASE)
            complexity += len(matches)
        
        return float(complexity)
    
    def _calculate_cognitive(self, source: str, language: str) -> float:
        """Calculate cognitive complexity with nesting weights."""
        lines = source.split('\n')
        complexity = 0.0
        nesting_level = 0
        
        # Patterns that increase cognitive complexity
        cognitive_patterns = self._get_cognitive_patterns(language)
        nesting_patterns = self._get_nesting_patterns(language)
        
        for line in lines:
            stripped = line.strip()
            
            # Check for nesting increase
            for pattern in nesting_patterns:
                if re.search(pattern, stripped):
                    nesting_level += 1
                    break
            
            # Check for cognitive complexity increases
            for pattern, weight in cognitive_patterns.items():
                if re.search(pattern, stripped):
                    complexity += weight * (1 + nesting_level)
        
        return complexity
    
    def _calculate_max_nesting(self, source: str, language: str) -> float:
        """Calculate maximum nesting depth."""
        lines = source.split('\n')
        current_nesting = 0
        max_nesting = 0
        
        # Patterns for nesting
        open_patterns = self._get_nesting_open_patterns(language)
        close_patterns = self._get_nesting_close_patterns(language)
        
        for line in lines:
            stripped = line.strip()
            
            # Count opening braces/blocks
            for pattern in open_patterns:
                current_nesting += len(re.findall(pattern, stripped))
            
            max_nesting = max(max_nesting, current_nesting)
            
            # Count closing braces/blocks
            for pattern in close_patterns:
                current_nesting -= len(re.findall(pattern, stripped))
            
            current_nesting = max(0, current_nesting)  # Don't go negative
        
        return float(max_nesting)
    
    def _calculate_branch_fanout(self, source: str, language: str) -> float:
        """Calculate average branches per decision point."""
        decision_patterns = self._get_decision_patterns(language)
        branch_patterns = self._get_branch_patterns(language)
        
        total_decisions = 0
        total_branches = 0
        
        for pattern in decision_patterns:
            decisions = re.findall(pattern, source, re.MULTILINE | re.IGNORECASE)
            total_decisions += len(decisions)
        
        for pattern in branch_patterns:
            branches = re.findall(pattern, source, re.MULTILINE | re.IGNORECASE)
            total_branches += len(branches)
        
        if total_decisions == 0:
            return 0.0
        
        return total_branches / total_decisions
    
    def _get_decision_patterns(self, language: str) -> List[str]:
        """Get regex patterns for decision points."""
        common_patterns = [
            r'\bif\b',
            r'\belse\s+if\b',
            r'\belif\b',
            r'\bwhile\b',
            r'\bfor\b',
            r'\btry\b',
            r'\bcatch\b',
            r'\bexcept\b',
            r'\b\?\s*.*?\s*:',  # Ternary operator
        ]
        
        language_specific = {
            'python': [r'\bfor\b.*\bin\b', r'\bwith\b'],
            'javascript': [r'\bswitch\b', r'\bcase\b'],
            'typescript': [r'\bswitch\b', r'\bcase\b'],
            'rust': [r'\bmatch\b', r'\bif\s+let\b', r'\bwhile\s+let\b'],
            'java': [r'\bswitch\b', r'\bcase\b'],
            'go': [r'\bswitch\b', r'\bcase\b', r'\bselect\b'],
        }
        
        patterns = common_patterns[:]
        if language in language_specific:
            patterns.extend(language_specific[language])
        
        return patterns
    
    def _get_cognitive_patterns(self, language: str) -> Dict[str, float]:
        """Get patterns with cognitive complexity weights."""
        return {
            r'\bif\b': 1.0,
            r'\belse\b': 1.0,
            r'\bwhile\b': 1.0,
            r'\bfor\b': 1.0,
            r'\btry\b': 1.0,
            r'\bcatch\b': 1.0,
            r'\bexcept\b': 1.0,
            r'\band\b': 0.5,
            r'\bor\b': 0.5,
            r'&&': 0.5,
            r'\|\|': 0.5,
        }
    
    def _get_nesting_patterns(self, language: str) -> List[str]:
        """Get patterns that increase nesting."""
        return [
            r'\bif\b',
            r'\bwhile\b',
            r'\bfor\b',
            r'\btry\b',
            r'\bwith\b',
            r'\bclass\b.*:',
            r'\bdef\b.*:',
            r'\bfunction\b.*{',
            r'^[^#]*{[^}]*$',  # Opening brace
        ]
    
    def _get_nesting_open_patterns(self, language: str) -> List[str]:
        """Get patterns that open a nesting block."""
        if language in {'python'}:
            return [r':$']  # Python uses colons
        else:
            return [r'\{']  # Most languages use braces
    
    def _get_nesting_close_patterns(self, language: str) -> List[str]:
        """Get patterns that close a nesting block."""
        if language in {'python'}:
            # For Python, we'd need to track indentation properly
            # This is simplified
            return []
        else:
            return [r'\}']  # Most languages use braces
    
    def _get_branch_patterns(self, language: str) -> List[str]:
        """Get patterns for counting branches."""
        return [
            r'\belse\b',
            r'\belif\b',
            r'\belse\s+if\b',
            r'\bcase\b',
            r'\bcatch\b',
            r'\bexcept\b.*:',
            r'\bfinally\b',
        ]