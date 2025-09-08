"""
Refactor brief generation system for LLM consumption.
"""

import logging
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Set

from valknut.core.config import BriefsConfig
from valknut.core.featureset import FeatureVector
from valknut.core.scoring import WeightedScorer
from valknut.lang.common_ast import Entity, EntityKind, ParseIndex

logger = logging.getLogger(__name__)


@dataclass
class DependencySlice:
    """Dependency information for an entity."""
    
    imports: List[str] = field(default_factory=list)
    callees_depth_limited: List[str] = field(default_factory=list)
    dependencies: List[str] = field(default_factory=list)


@dataclass
class FindingInfo:
    """Information about code smells and issues found."""
    
    duplicates: List[Dict[str, Any]] = field(default_factory=list)
    cycles: List[Dict[str, Any]] = field(default_factory=list)
    type_friction: List[str] = field(default_factory=list)
    cohesion: List[str] = field(default_factory=list)
    complexity: List[str] = field(default_factory=list)


@dataclass 
class RefactorBrief:
    """LLM-ready refactor brief for an entity."""
    
    entity_id: str
    language: str
    path: str
    kind: str
    score: float
    summary: str
    
    # Code information
    signatures: List[str] = field(default_factory=list)
    loc: int = 0
    
    # Analysis results
    dependency_slice: Optional[DependencySlice] = None
    invariants: List[str] = field(default_factory=list)
    findings: Optional[FindingInfo] = None
    candidate_refactors: List[str] = field(default_factory=list)
    safety_checklist: List[str] = field(default_factory=list)
    
    # Feature details
    top_features: List[Dict[str, Any]] = field(default_factory=list)
    explanations: List[str] = field(default_factory=list)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        result = {
            "entity_id": self.entity_id,
            "language": self.language,
            "path": self.path,
            "kind": self.kind,
            "score": self.score,
            "summary": self.summary,
            "signatures": self.signatures,
            "loc": self.loc,
        }
        
        if self.dependency_slice:
            result["dependency_slice"] = {
                "imports": self.dependency_slice.imports,
                "callees_depth<=2": self.dependency_slice.callees_depth_limited,
                "dependencies": self.dependency_slice.dependencies,
            }
        
        result["invariants"] = self.invariants
        
        if self.findings:
            findings_dict = {}
            if self.findings.duplicates:
                findings_dict["duplicates"] = self.findings.duplicates
            if self.findings.cycles:
                findings_dict["cycles"] = self.findings.cycles
            if self.findings.type_friction:
                findings_dict["type_friction"] = self.findings.type_friction
            if self.findings.cohesion:
                findings_dict["cohesion"] = self.findings.cohesion
            if self.findings.complexity:
                findings_dict["complexity"] = self.findings.complexity
            
            if findings_dict:
                result["findings"] = findings_dict
        
        result["candidate_refactors"] = self.candidate_refactors
        result["safety_checklist"] = self.safety_checklist
        
        # Add feature information
        if self.top_features:
            result["top_features"] = self.top_features
        
        if self.explanations:
            result["explanations"] = self.explanations
        
        return result


class BriefGenerator:
    """Generates refactor briefs for entities."""
    
    def __init__(self, config: BriefsConfig, scorer: WeightedScorer) -> None:
        self.config = config
        self.scorer = scorer
    
    def generate_brief(
        self,
        entity: Entity,
        feature_vector: FeatureVector,
        score: float,
        index: ParseIndex,
    ) -> RefactorBrief:
        """
        Generate a refactor brief for an entity.
        
        Args:
            entity: Entity to generate brief for
            feature_vector: Feature vector with scores
            score: Overall refactor score
            index: Parse index for context
            
        Returns:
            Refactor brief
        """
        brief = RefactorBrief(
            entity_id=entity.id,
            language=entity.language,
            path=str(entity.location.file_path),
            kind=entity.kind.value,
            score=score,
            summary="",  # Will be generated
            loc=entity.loc,
        )
        
        # Generate signature information
        if self.config.include_signatures:
            brief.signatures = self._extract_signatures(entity)
        
        # Build dependency slice
        brief.dependency_slice = self._build_dependency_slice(entity, index)
        
        # Extract invariants
        brief.invariants = self._extract_invariants(entity)
        
        # Analyze findings
        brief.findings = self._analyze_findings(entity, feature_vector, index)
        
        # Suggest refactorings
        if self.config.include_detected_refactors:
            brief.candidate_refactors = self._suggest_refactors(entity, feature_vector)
        
        # Generate safety checklist
        brief.safety_checklist = self._generate_safety_checklist(entity, index)
        
        # Get explanations from scorer
        brief.explanations = self.scorer.explain_score(feature_vector)
        
        # Extract top features
        brief.top_features = self._get_top_features(feature_vector)
        
        # Generate summary
        brief.summary = self._generate_summary(entity, feature_vector, brief)
        
        return brief
    
    def _extract_signatures(self, entity: Entity) -> List[str]:
        """Extract function/method signatures."""
        signatures = []
        
        if entity.signature:
            signatures.append(entity.signature)
        elif entity.kind in {EntityKind.FUNCTION, EntityKind.METHOD}:
            # Generate basic signature
            params = ", ".join(entity.parameters) if entity.parameters else ""
            return_type = f" -> {entity.return_type}" if entity.return_type else ""
            
            if entity.kind == EntityKind.METHOD:
                signature = f"def {entity.name}({params}){return_type}: ..."
            else:
                signature = f"function {entity.name}({params}){return_type} {{ ... }}"
            
            signatures.append(signature)
        
        return signatures
    
    def _build_dependency_slice(self, entity: Entity, index: ParseIndex) -> DependencySlice:
        """Build dependency slice information."""
        slice_info = DependencySlice()
        
        # Get imports for the entity's file
        file_entities = index.get_by_file(entity.location.file_path)
        for file_entity in file_entities:
            if file_entity.kind == EntityKind.FILE:
                slice_info.imports.extend(file_entity.imports)
                break
        
        # Get callees with depth limit
        if index.call_graph:
            callees = self._get_callees_with_depth(
                entity.id, 
                index.call_graph,
                max_depth=self.config.callee_depth
            )
            slice_info.callees_depth_limited = callees
        
        # Get general dependencies
        if index.import_graph:
            try:
                if entity.id in index.import_graph:
                    dependencies = list(index.import_graph.successors(entity.id))
                    slice_info.dependencies = dependencies[:10]  # Limit to avoid bloat
            except Exception:
                pass
        
        return slice_info
    
    def _get_callees_with_depth(self, entity_id: str, call_graph, max_depth: int) -> List[str]:
        """Get callees up to a certain depth."""
        import networkx as nx
        
        callees = []
        
        try:
            if entity_id in call_graph:
                # Use BFS to get callees within depth
                visited = {entity_id}
                queue = [(entity_id, 0)]
                
                while queue:
                    current, depth = queue.pop(0)
                    
                    if depth >= max_depth:
                        continue
                    
                    for callee in call_graph.successors(current):
                        if callee not in visited:
                            visited.add(callee)
                            callees.append(callee)
                            queue.append((callee, depth + 1))
            
        except Exception as e:
            logger.warning(f"Failed to get callees for {entity_id}: {e}")
        
        return callees[:20]  # Limit results
    
    def _extract_invariants(self, entity: Entity) -> List[str]:
        """Extract code invariants and contracts."""
        invariants = []
        
        # Extract from docstring if available
        if entity.docstring:
            docstring = entity.docstring.lower()
            
            # Look for common invariant patterns
            if "returns" in docstring and ("non-null" in docstring or "not none" in docstring):
                invariants.append("returns non-null value on success")
            
            if "raises" in docstring or "throws" in docstring:
                invariants.append("raises exception on invalid input")
            
            if "side effect" in docstring:
                invariants.append("has documented side effects")
        
        # Language-specific invariants
        if entity.language == "python":
            if entity.return_type:
                invariants.append(f"returns {entity.return_type}")
                
        elif entity.language in ["typescript", "javascript"]:
            if "Promise" in str(entity.return_type):
                invariants.append("returns Promise (async operation)")
        
        # Parameter invariants
        if len(entity.parameters) > 5:
            invariants.append("takes many parameters - consider parameter object")
        
        return invariants
    
    def _analyze_findings(self, entity: Entity, feature_vector: FeatureVector, index: ParseIndex) -> FindingInfo:
        """Analyze findings from feature extraction."""
        findings = FindingInfo()
        
        # Clone findings
        if "clone_mass" in feature_vector.normalized_features:
            clone_mass = feature_vector.normalized_features["clone_mass"]
            if clone_mass > 0.3:
                # Try to get clone details from echo extractor
                findings.duplicates = self._get_clone_details(entity, feature_vector)
        
        # Cycle findings
        if "in_cycle" in feature_vector.normalized_features:
            if feature_vector.normalized_features["in_cycle"] > 0.5:
                cycle_size = feature_vector.normalized_features.get("cycle_size", 0)
                findings.cycles = [{
                    "members": ["..."],  # Would need graph analysis to get actual members
                    "size": int(cycle_size * 100)  # Rough estimate
                }]
        
        # Type friction findings
        type_issues = []
        if "any_ratio" in feature_vector.normalized_features:
            any_ratio = feature_vector.normalized_features["any_ratio"]
            if any_ratio > 0.3:
                type_issues.append(f"High 'any' type usage ({any_ratio:.1%})")
        
        if "casts_per_kloc" in feature_vector.normalized_features:
            casts = feature_vector.normalized_features["casts_per_kloc"]
            if casts > 0.1:
                type_issues.append(f"Frequent type casts ({casts:.1f} per 1k LOC)")
        
        findings.type_friction = type_issues
        
        # Cohesion findings
        cohesion_issues = []
        if "lcom_like" in feature_vector.normalized_features:
            lcom = feature_vector.normalized_features["lcom_like"]
            if lcom > 0.7:
                cohesion_issues.append("Low cohesion - methods don't share data")
        
        if "param_count" in feature_vector.normalized_features:
            param_count = feature_vector.normalized_features["param_count"]
            if param_count > 0.8:
                cohesion_issues.append("Too many parameters - consider parameter object")
        
        findings.cohesion = cohesion_issues
        
        # Complexity findings
        complexity_issues = []
        if "cyclomatic" in feature_vector.normalized_features:
            complexity = feature_vector.normalized_features["cyclomatic"]
            if complexity > 0.8:
                complexity_issues.append(f"High cyclomatic complexity")
        
        if "max_nesting" in feature_vector.normalized_features:
            nesting = feature_vector.normalized_features["max_nesting"]
            if nesting > 0.7:
                complexity_issues.append("Deep nesting levels")
        
        findings.complexity = complexity_issues
        
        return findings
    
    def _get_clone_details(self, entity: Entity, feature_vector: FeatureVector) -> List[Dict[str, Any]]:
        """Get clone details from echo extractor if available."""
        # This would ideally get details from the echo extractor
        # For now, return a placeholder
        clone_mass = feature_vector.normalized_features.get("clone_mass", 0)
        if clone_mass > 0.3:
            return [{
                "other_path": "similar_file.py",  # Placeholder
                "similarity": min(0.99, clone_mass + 0.2),
                "lines": "120-180",  # Placeholder
            }]
        return []
    
    def _suggest_refactors(self, entity: Entity, feature_vector: FeatureVector) -> List[str]:
        """Suggest appropriate refactorings based on features."""
        suggestions = []
        
        # High complexity -> Extract Method
        if feature_vector.normalized_features.get("cyclomatic", 0) > 0.7:
            suggestions.append("Extract Method")
        
        # High parameter count -> Introduce Parameter Object
        if feature_vector.normalized_features.get("param_count", 0) > 0.8:
            suggestions.append("Introduce Parameter Object")
        
        # High clone mass -> Extract Common Code
        if feature_vector.normalized_features.get("clone_mass", 0) > 0.5:
            suggestions.append("Extract Common Code")
        
        # In cycle -> Break Dependency Cycle
        if feature_vector.normalized_features.get("in_cycle", 0) > 0.5:
            suggestions.append("Break Dependency Cycle (interface/inversion)")
        
        # Low cohesion -> Split Class/Method
        if feature_vector.normalized_features.get("lcom_like", 0) > 0.7:
            suggestions.append("Split Class" if entity.kind == EntityKind.CLASS else "Split Method")
        
        # High fan-out -> Reduce Dependencies
        if feature_vector.normalized_features.get("fan_out", 0) > 0.7:
            suggestions.append("Reduce Dependencies")
        
        # Type friction -> Improve Type Safety
        if feature_vector.normalized_features.get("any_ratio", 0) > 0.5:
            suggestions.append("Improve Type Safety")
        
        return suggestions[:5]  # Limit to top 5
    
    def _generate_safety_checklist(self, entity: Entity, index: ParseIndex) -> List[str]:
        """Generate safety checklist for refactoring."""
        checklist = []
        
        # High fan-in entities need more care
        fan_in = entity.metrics.get("fan_in", 0)
        if fan_in > 5:
            checklist.append(f"Update {int(fan_in)} dependent callsites")
        
        # Entities with many parameters need interface updates
        if len(entity.parameters) > 3:
            checklist.append("Update all call sites with parameter changes")
        
        # Complex entities need comprehensive testing
        cyclomatic = entity.metrics.get("cyclomatic", 1)
        if cyclomatic > 10:
            checklist.append("Add comprehensive test coverage for all branches")
        
        # Entities in cycles need careful ordering
        in_cycle = entity.metrics.get("in_cycle", False)
        if in_cycle:
            checklist.append("Plan refactoring order to avoid breaking cycles")
        
        # Generic safety items
        checklist.extend([
            "Run full test suite after changes",
            "Update documentation and comments",
            "Consider backward compatibility",
        ])
        
        return checklist
    
    def _get_top_features(self, feature_vector: FeatureVector) -> List[Dict[str, Any]]:
        """Get top contributing features."""
        features = []
        
        for name, value in feature_vector.normalized_features.items():
            if value > 0.5:  # Only include significant features
                features.append({
                    "name": name,
                    "value": value,
                    "normalized": True,
                })
        
        # Sort by value descending
        features.sort(key=lambda x: x["value"], reverse=True)
        
        return features[:10]  # Top 10 features
    
    def _generate_summary(self, entity: Entity, feature_vector: FeatureVector, brief: RefactorBrief) -> str:
        """Generate a concise summary of the refactoring need."""
        issues = []
        
        # Primary issues
        if feature_vector.normalized_features.get("clone_mass", 0) > 0.5:
            issues.append("high duplication")
        
        if feature_vector.normalized_features.get("cyclomatic", 0) > 0.7:
            issues.append("complex logic")
        
        if feature_vector.normalized_features.get("in_cycle", 0) > 0.5:
            issues.append("in cycle")
        
        if feature_vector.normalized_features.get("param_count", 0) > 0.8:
            issues.append("many parameters")
        
        # Entity type context
        entity_type = entity.kind.value
        
        # Main suggestions
        main_refactors = brief.candidate_refactors[:2]
        
        if issues and main_refactors:
            issue_str = " and ".join(issues)
            refactor_str = " and ".join(main_refactors).lower()
            return f"{entity_type.title()} with {issue_str}; suggest {refactor_str}"
        elif issues:
            issue_str = " and ".join(issues)
            return f"{entity_type.title()} with {issue_str}"
        elif main_refactors:
            refactor_str = " and ".join(main_refactors).lower()
            return f"{entity_type.title()} candidate for {refactor_str}"
        else:
            return f"Refactoring candidate {entity_type}"