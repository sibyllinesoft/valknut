"""
Impact Packs: Clone Consolidation and Cycle-Cut recommendations.

This module implements two types of strategic refactoring recommendations:
1. ClonePacks - Template extraction from near-duplicate code
2. CyclePacks/ChokepointPacks - Dependency cycle breaking recommendations
"""

import logging
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple, Union
from uuid import uuid4

import networkx as nx

from valknut.lang.common_ast import Entity, EntityKind, ParseIndex
from valknut.detectors.coverage import CoverageReportParser
from valknut.detectors.structure import FilesystemStructureAnalyzer, StructureConfig

logger = logging.getLogger(__name__)


@dataclass
class CloneMember:
    """A member of a clone group."""
    entity_id: str
    path: str
    lines: str  # e.g., "120-176"
    similarity: float


@dataclass  
class TemplateParameter:
    """A parameter extracted from clone analysis."""
    name: str
    type_hint: str
    
    
@dataclass
class OptionalBlock:
    """An optional code block that appears in some but not all clones."""
    name: str
    appears_in: List[str]  # paths where this block appears
    lines: str


@dataclass
class CloneTemplate:
    """Template extracted from a clone group."""
    representative: Dict[str, str]  # path and lines of the representative
    parameters: List[TemplateParameter]
    optional_blocks: List[OptionalBlock]


@dataclass
class SuggestedTarget:
    """Suggested location for extracted code."""
    language: str
    path: str
    export: str


@dataclass
class PackValue:
    """Value metrics for a pack."""
    dup_loc_removed: Optional[int] = None
    score_drop_estimate: Optional[float] = None
    cycles_removed: Optional[int] = None
    scc_count_delta: Optional[int] = None
    avg_path_len_delta: Optional[float] = None
    cross_community_edges_reduced: Optional[int] = None


@dataclass
class PackEffort:
    """Effort estimation for a pack."""
    loc_touched: Optional[int] = None
    callsites: Optional[int] = None
    modules_touched: Optional[int] = None
    imports_to_rehome_est: Optional[int] = None


@dataclass
class ClonePack:
    """A clone consolidation recommendation."""
    pack_id: str
    kind: str = "clone_consolidation"
    members: List[CloneMember] = field(default_factory=list)
    template: Optional[CloneTemplate] = None
    suggested_target: Optional[SuggestedTarget] = None
    value: PackValue = field(default_factory=PackValue)
    effort: PackEffort = field(default_factory=PackEffort)
    steps: List[str] = field(default_factory=list)
    explanations: List[str] = field(default_factory=list)


@dataclass
class CyclePack:
    """A cycle-cutting recommendation."""
    pack_id: str
    kind: str = "cycle_cut"
    scc_members: List[str] = field(default_factory=list)
    cut_nodes: List[str] = field(default_factory=list)
    value: PackValue = field(default_factory=PackValue)
    effort: PackEffort = field(default_factory=PackEffort)
    steps: List[str] = field(default_factory=list)
    explanations: List[str] = field(default_factory=list)


@dataclass
class ChokepointPack:
    """A chokepoint elimination recommendation."""
    pack_id: str
    chokepoint_node: str
    kind: str = "chokepoint_elimination"
    affected_communities: List[str] = field(default_factory=list)
    value: PackValue = field(default_factory=PackValue)
    effort: PackEffort = field(default_factory=PackEffort)
    steps: List[str] = field(default_factory=list)
    explanations: List[str] = field(default_factory=list)


@dataclass
class UncoveredSegment:
    """A segment of uncovered code for coverage improvement."""
    file_path: str
    start_line: int
    end_line: int
    context_lines: List[str] = field(default_factory=list)  # Key lines with context, not full block
    entity_name: Optional[str] = None
    entity_id: Optional[str] = None  # Entity identifier for tracking
    entity_type: Optional[str] = None  # function, class, etc
    complexity_hints: List[str] = field(default_factory=list)  # conditional, loop, etc
    
    def get_summary_line(self) -> str:
        """Get a one-line summary of this segment."""
        context = " | ".join(self.context_lines[:2]) if self.context_lines else ""
        return f"{self.file_path}:{self.start_line}-{self.end_line} {context}"


@dataclass
class CoveragePack:
    """A coverage improvement recommendation."""
    pack_id: str
    kind: str = "coverage_improvement"
    uncovered_segments: List[UncoveredSegment] = field(default_factory=list)
    current_coverage_pct: float = 0.0
    target_coverage_pct: float = 0.0
    estimated_lines_to_cover: int = 0
    value: PackValue = field(default_factory=PackValue)
    effort: PackEffort = field(default_factory=PackEffort)
    steps: List[str] = field(default_factory=list)
    explanations: List[str] = field(default_factory=list)


ImpactPack = Union[ClonePack, CyclePack, ChokepointPack, CoveragePack]


class CloneConsolidator:
    """Consolidates clone groups into actionable refactoring plans."""
    
    def __init__(self, min_similarity: float = 0.85, min_total_loc: int = 60, max_parameters: int = 6):
        self.min_similarity = min_similarity
        self.min_total_loc = min_total_loc
        self.max_parameters = max_parameters
    
    def build_clonepacks(
        self, 
        index: ParseIndex, 
        clone_groups: List[Dict], 
        entities: Dict[str, Entity]
    ) -> List[ClonePack]:
        """Build ClonePacks from echo clone groups."""
        if not clone_groups:
            return []
        
        # Map clone spans to entities
        clone_sets = self._group_clones_by_entity(clone_groups, entities)
        
        # Filter by criteria
        clone_sets = self._filter_clone_sets(clone_sets)
        
        # Build packs
        packs = []
        for i, clone_set in enumerate(clone_sets):
            pack = self._build_clone_pack(clone_set, entities, i)
            if pack:
                packs.append(pack)
        
        return packs
    
    def _group_clones_by_entity(
        self, 
        clone_groups: List[Dict], 
        entities: Dict[str, Entity]
    ) -> List[List[Dict]]:
        """Group clones by the entities they belong to."""
        # For now, simple grouping - in practice would use interval trees
        clone_sets = []
        
        for group in clone_groups:
            if len(group.get("members", [])) < 2:
                continue
                
            members = group["members"]
            if all(
                member.get("similarity", 0) >= self.min_similarity 
                for member in members
            ):
                clone_sets.append(members)
        
        return clone_sets
    
    def _filter_clone_sets(self, clone_sets: List[List[Dict]]) -> List[List[Dict]]:
        """Filter clone sets by minimum total LOC and other criteria."""
        filtered = []
        
        for clone_set in clone_sets:
            total_loc = 0
            for member in clone_set:
                # Parse lines range to get LOC
                lines_str = member.get("lines", "1-1")
                if "-" in lines_str:
                    start, end = map(int, lines_str.split("-"))
                    total_loc += end - start + 1
            
            if total_loc >= self.min_total_loc:
                filtered.append(clone_set)
        
        return filtered
    
    def _build_clone_pack(
        self, 
        clone_set: List[Dict], 
        entities: Dict[str, Entity], 
        pack_index: int
    ) -> Optional[ClonePack]:
        """Build a ClonePack from a clone set."""
        if not clone_set:
            return None
        
        # Create members
        members = []
        for member in clone_set:
            clone_member = CloneMember(
                entity_id=member.get("entity_id", ""),
                path=member.get("path", ""),
                lines=member.get("lines", ""),
                similarity=member.get("similarity", 0.0)
            )
            members.append(clone_member)
        
        # Find medoid (most similar to all others)
        medoid_idx = self._find_medoid(clone_set)
        representative = clone_set[medoid_idx]
        
        # Extract template parameters (simplified)
        parameters = self._extract_parameters(clone_set)
        optional_blocks = self._extract_optional_blocks(clone_set)
        
        # Create template
        template = CloneTemplate(
            representative={
                "path": representative.get("path", ""),
                "lines": representative.get("lines", "")
            },
            parameters=parameters,
            optional_blocks=optional_blocks
        )
        
        # Suggest target location
        suggested_target = self._suggest_target_location(clone_set)
        
        # Calculate value and effort
        value, effort = self._calculate_clone_value_effort(clone_set, len(parameters))
        
        # Generate steps and explanations
        steps = self._generate_clone_steps(template, suggested_target, len(members))
        explanations = self._generate_clone_explanations(clone_set, len(parameters))
        
        pack_id = f"clonepack:SET{pack_index}"
        
        return ClonePack(
            pack_id=pack_id,
            members=members,
            template=template,
            suggested_target=suggested_target,
            value=value,
            effort=effort,
            steps=steps,
            explanations=explanations
        )
    
    def _find_medoid(self, clone_set: List[Dict]) -> int:
        """Find the medoid (most central) member of a clone set."""
        if len(clone_set) == 1:
            return 0
        
        best_idx = 0
        best_score = float('-inf')
        
        for i, member in enumerate(clone_set):
            # Sum similarity to all other members
            total_similarity = sum(
                other.get("similarity", 0.0) 
                for j, other in enumerate(clone_set) 
                if i != j
            )
            
            if total_similarity > best_score:
                best_score = total_similarity
                best_idx = i
        
        return best_idx
    
    def _extract_parameters(self, clone_set: List[Dict]) -> List[TemplateParameter]:
        """Extract parameters from clone variations."""
        # Simplified parameter extraction
        # In practice, would use tree-sitter token diff
        
        params = []
        
        # Look for common patterns that suggest parameterization
        if len(clone_set) >= 2:
            # Mock parameter extraction - real implementation would analyze AST diffs
            params.extend([
                TemplateParameter("format", "str"),
                TemplateParameter("limit", "int")
            ])
        
        # Apply max parameters constraint
        if len(params) > self.max_parameters:
            # Suggest parameter object instead
            return [TemplateParameter("config", "ConfigObject")]
        
        return params
    
    def _extract_optional_blocks(self, clone_set: List[Dict]) -> List[OptionalBlock]:
        """Extract optional code blocks that appear in some clones."""
        # Simplified - real implementation would analyze structural diffs
        return []
    
    def _suggest_target_location(self, clone_set: List[Dict]) -> SuggestedTarget:
        """Suggest where to place the extracted function."""
        # Find common ancestor directory
        paths = [member.get("path", "") for member in clone_set]
        
        # Determine language from first path
        language = "python"  # Default
        if paths and paths[0]:
            path = Path(paths[0])
            if path.suffix in {".ts", ".tsx"}:
                language = "typescript"
            elif path.suffix in {".js", ".jsx"}:
                language = "javascript"  
            elif path.suffix == ".rs":
                language = "rust"
        
        # Language-specific target suggestions
        target_map = {
            "python": {"path": "pkg/util/refactor_shared.py", "export": "shared_transform"},
            "typescript": {"path": "src/lib/shared.ts", "export": "sharedTransform"},
            "javascript": {"path": "src/utils/shared.js", "export": "sharedTransform"},
            "rust": {"path": "src/util/shared.rs", "export": "shared_transform"}
        }
        
        target = target_map.get(language, target_map["python"])
        
        return SuggestedTarget(
            language=language,
            path=target["path"],
            export=target["export"]
        )
    
    def _calculate_clone_value_effort(
        self, 
        clone_set: List[Dict], 
        param_count: int
    ) -> Tuple[PackValue, PackEffort]:
        """Calculate value and effort metrics for clone pack."""
        # Calculate duplicated LOC removed
        total_dup_loc = 0
        for member in clone_set:
            lines_str = member.get("lines", "1-1")
            if "-" in lines_str:
                start, end = map(int, lines_str.split("-"))
                total_dup_loc += end - start + 1
        
        # Estimate score drop (how much complexity reduction)
        score_drop = min(0.2, total_dup_loc / 1000.0)
        
        value = PackValue(
            dup_loc_removed=total_dup_loc,
            score_drop_estimate=score_drop
        )
        
        # Calculate effort
        callsites = len(clone_set)  # Each clone becomes a callsite
        loc_touched = total_dup_loc + (callsites * 2)  # Original code + replacement calls
        
        effort = PackEffort(
            loc_touched=loc_touched,
            callsites=callsites
        )
        
        return value, effort
    
    def _generate_clone_steps(
        self, 
        template: CloneTemplate, 
        target: SuggestedTarget, 
        member_count: int
    ) -> List[str]:
        """Generate step-by-step instructions for clone consolidation."""
        steps = [
            f"Extract common body to {target.path} as `{target.export}`."
        ]
        
        if template.parameters:
            param_names = ", ".join(f"{p.name}:{p.type_hint}" for p in template.parameters)
            steps.append(f"Add parameters: {param_names}.")
        
        if template.optional_blocks:
            steps.append("Handle optional blocks with conditional parameters or hooks.")
        
        steps.append(f"Replace {member_count} clone instances with calls; preserve exceptions & return contracts.")
        
        return steps
    
    def _generate_clone_explanations(
        self, 
        clone_set: List[Dict], 
        param_count: int
    ) -> List[str]:
        """Generate explanations for why this clone pack is valuable."""
        explanations = []
        
        # Count unique modules/files
        unique_paths = set(member.get("path", "") for member in clone_set)
        module_count = len(unique_paths)
        
        if module_count > 1:
            explanations.append(
                f"High clone mass across {module_count} modules; "
                f"parameters differ by {param_count} identifiers/literals."
            )
        else:
            explanations.append(
                f"Local code duplication with {param_count} varying parameters - "
                "good candidate for extraction."
            )
        
        return explanations


class CycleCutter:
    """Cuts dependency cycles using greedy feedback vertex set approximation."""
    
    def __init__(self, centrality_samples: int = 64):
        self.centrality_samples = centrality_samples
    
    def build_cycle_packs(
        self, 
        import_graph: nx.DiGraph, 
        entities: Dict[str, Entity]
    ) -> List[CyclePack]:
        """Build CyclePacks from strongly connected components."""
        # Find SCCs with more than one node
        sccs = [scc for scc in nx.strongly_connected_components(import_graph) if len(scc) > 1]
        
        if not sccs:
            return []
        
        packs = []
        for i, scc in enumerate(sccs):
            pack = self._build_cycle_pack(import_graph, scc, entities, i)
            if pack:
                packs.append(pack)
        
        return packs
    
    def _build_cycle_pack(
        self, 
        graph: nx.DiGraph, 
        scc: Set[str], 
        entities: Dict[str, Entity], 
        pack_index: int
    ) -> Optional[CyclePack]:
        """Build a single cycle pack from an SCC."""
        if len(scc) <= 1:
            return None
        
        # Extract subgraph for this SCC
        subgraph = graph.subgraph(scc).copy()
        
        # Find minimal cut using greedy approximation
        cut_nodes = self._find_minimal_cut(subgraph, graph)
        
        if not cut_nodes:
            return None
        
        # Calculate value metrics
        value = self._calculate_cycle_value(subgraph, cut_nodes, graph)
        
        # Calculate effort
        effort = self._calculate_cycle_effort(cut_nodes, graph)
        
        # Generate steps and explanations  
        steps = self._generate_cycle_steps(cut_nodes)
        explanations = self._generate_cycle_explanations(scc, cut_nodes)
        
        pack_id = f"cyclepack:SCC{pack_index}"
        
        return CyclePack(
            pack_id=pack_id,
            scc_members=list(scc),
            cut_nodes=cut_nodes,
            value=value,
            effort=effort,
            steps=steps,
            explanations=explanations
        )
    
    def _find_minimal_cut(self, subgraph: nx.DiGraph, full_graph: nx.DiGraph) -> List[str]:
        """Find minimal set of nodes to remove to break cycles."""
        cut_nodes = []
        remaining = subgraph.copy()
        
        max_iterations = 100  # Prevent infinite loops
        iteration = 0
        
        while self._has_cycle(remaining) and iteration < max_iterations:
            scores = {}
            
            # Safety check: ensure we have nodes to work with
            if not remaining.nodes():
                break
            
            for node in remaining.nodes():
                # Score based on betweenness, degree, and boundary edges
                betweenness = self._approximate_betweenness(remaining, node)
                degree = remaining.in_degree(node) + remaining.out_degree(node)
                boundary_edges = self._count_boundary_edges(node, subgraph.nodes(), full_graph)
                
                scores[node] = 0.5 * betweenness + 0.3 * degree + 0.2 * boundary_edges
            
            # Safety check: ensure we have scored nodes
            if not scores:
                break
                
            # Pick highest scoring node
            best_node = max(scores.keys(), key=lambda n: scores[n])
            cut_nodes.append(best_node)
            remaining.remove_node(best_node)
            
            iteration += 1
        
        return cut_nodes
    
    def _has_cycle(self, graph: nx.DiGraph) -> bool:
        """Check if graph has cycles."""
        try:
            list(nx.simple_cycles(graph, length_bound=100))  # Quick check
            return True
        except nx.NetworkXError:
            return False
        except StopIteration:
            return False
        # If we get here, there are cycles
        return True
    
    def _approximate_betweenness(self, graph: nx.DiGraph, node: str) -> float:
        """Approximate betweenness centrality with sampling."""
        if len(graph.nodes()) <= 10:
            # Small graph - compute exactly
            centrality = nx.betweenness_centrality(graph)
            return centrality.get(node, 0.0)
        
        # Use sampling for larger graphs
        try:
            centrality = nx.betweenness_centrality(graph, k=min(self.centrality_samples, len(graph.nodes())))
            return centrality.get(node, 0.0)
        except:
            return 0.0
    
    def _count_boundary_edges(self, node: str, scc_nodes: Set[str], full_graph: nx.DiGraph) -> int:
        """Count edges from this node to outside the SCC."""
        count = 0
        
        # Outgoing edges to outside SCC
        for successor in full_graph.successors(node):
            if successor not in scc_nodes:
                count += 1
        
        # Incoming edges from outside SCC  
        for predecessor in full_graph.predecessors(node):
            if predecessor not in scc_nodes:
                count += 1
        
        return count
    
    def _calculate_cycle_value(
        self, 
        subgraph: nx.DiGraph, 
        cut_nodes: List[str], 
        full_graph: nx.DiGraph
    ) -> PackValue:
        """Calculate value metrics for cycle cutting."""
        # Count cycles removed (approximate)
        cycles_removed = len(cut_nodes) * 2  # Rough estimate
        
        # SCC count delta (breaking one SCC into smaller pieces)
        scc_count_delta = len(cut_nodes) - 1  # Optimistic estimate
        
        # Path length improvement (simplified)
        avg_path_len_delta = min(0.5, len(cut_nodes) * 0.1)
        
        return PackValue(
            cycles_removed=cycles_removed,
            scc_count_delta=scc_count_delta,
            avg_path_len_delta=avg_path_len_delta
        )
    
    def _calculate_cycle_effort(self, cut_nodes: List[str], graph: nx.DiGraph) -> PackEffort:
        """Calculate effort required for cycle cutting."""
        modules_touched = len(cut_nodes)
        
        # Estimate imports that need rehoming
        imports_to_rehome = 0
        for node in cut_nodes:
            imports_to_rehome += graph.in_degree(node) + graph.out_degree(node)
        
        imports_to_rehome = min(imports_to_rehome, 20)  # Cap estimate
        
        return PackEffort(
            modules_touched=modules_touched,
            imports_to_rehome_est=imports_to_rehome
        )
    
    def _generate_cycle_steps(self, cut_nodes: List[str]) -> List[str]:
        """Generate steps for cycle cutting."""
        if not cut_nodes:
            return []
        
        primary_node = cut_nodes[0]
        steps = [
            f"Extract interface or facade for functionality in {primary_node}.",
            f"Invert dependencies to use the interface instead of direct imports.",
        ]
        
        if len(cut_nodes) > 1:
            steps.append("Move shared utilities to common module if needed.")
        
        return steps
    
    def _generate_cycle_explanations(self, scc: Set[str], cut_nodes: List[str]) -> List[str]:
        """Generate explanations for cycle cutting."""
        explanations = []
        
        primary_node = cut_nodes[0] if cut_nodes else "target module"
        
        explanations.append(
            f"Cutting {primary_node} breaks circular dependency in "
            f"{len(scc)}-node SCC and improves modularity."
        )
        
        return explanations


class ChokepointDetector:
    """Detects and creates packs for architectural chokepoints."""
    
    def __init__(self, centrality_samples: int = 64, top_n: int = 3):
        self.centrality_samples = centrality_samples
        self.top_n = top_n
    
    def build_chokepoint_packs(
        self, 
        import_graph: nx.DiGraph, 
        entities: Dict[str, Entity]
    ) -> List[ChokepointPack]:
        """Build chokepoint elimination packs."""
        # Compute betweenness centrality
        if len(import_graph.nodes()) < 10:
            centrality = nx.betweenness_centrality(import_graph)
        else:
            centrality = nx.betweenness_centrality(
                import_graph, 
                k=min(self.centrality_samples, len(import_graph.nodes()))
            )
        
        # Find high-centrality nodes
        sorted_nodes = sorted(centrality.items(), key=lambda x: x[1], reverse=True)
        
        # Take top percentile
        percentile_95 = int(len(sorted_nodes) * 0.05)  # Top 5%
        high_centrality_nodes = sorted_nodes[:max(percentile_95, self.top_n)]
        
        packs = []
        for i, (node, centrality_score) in enumerate(high_centrality_nodes[:self.top_n]):
            if centrality_score > 0.05:  # Meaningful centrality threshold
                pack = self._build_chokepoint_pack(node, centrality_score, import_graph, i)
                if pack:
                    packs.append(pack)
        
        return packs
    
    def _build_chokepoint_pack(
        self, 
        node: str, 
        centrality_score: float, 
        graph: nx.DiGraph, 
        pack_index: int
    ) -> Optional[ChokepointPack]:
        """Build a chokepoint pack for a high-centrality node."""
        # Identify affected communities (simplified - just count neighbors)
        neighbors = set(graph.predecessors(node)) | set(graph.successors(node))
        affected_communities = [f"community_{i}" for i in range(min(len(neighbors) // 3, 5))]
        
        # Calculate value
        value = PackValue(
            cross_community_edges_reduced=len(neighbors) // 2
        )
        
        # Calculate effort
        effort = PackEffort(
            modules_touched=1,
            imports_to_rehome_est=len(neighbors)
        )
        
        # Generate steps and explanations
        steps = self._generate_chokepoint_steps(node)
        explanations = self._generate_chokepoint_explanations(node, centrality_score, len(neighbors))
        
        pack_id = f"chokepointpack:HUB{pack_index}"
        
        return ChokepointPack(
            pack_id=pack_id,
            chokepoint_node=node,
            affected_communities=affected_communities,
            value=value,
            effort=effort,
            steps=steps,
            explanations=explanations
        )
    
    def _generate_chokepoint_steps(self, node: str) -> List[str]:
        """Generate steps for chokepoint elimination."""
        return [
            f"Split {node} into focused modules by responsibility.",
            "Extract interfaces for cross-cutting concerns.",
            "Move shared utilities to dedicated common layer."
        ]
    
    def _generate_chokepoint_explanations(
        self, 
        node: str, 
        centrality: float, 
        neighbor_count: int
    ) -> List[str]:
        """Generate explanations for chokepoint elimination."""
        return [
            f"High-centrality node ({centrality:.3f}) with {neighbor_count} dependencies - "
            "splitting reduces coupling and improves testability."
        ]


class CoveragePackBuilder:
    """Builds coverage improvement packs from coverage reports."""
    
    def __init__(self, top_n: int = 10, min_segment_size: int = 3, max_context_lines: int = 5):
        self.top_n = top_n
        self.min_segment_size = min_segment_size
        self.max_context_lines = max_context_lines
    
    def build_coverage_packs(
        self, 
        coverage_report: Optional[dict],
        parse_index: ParseIndex,
        entities: Dict[str, Entity]
    ) -> List[CoveragePack]:
        """Build coverage improvement packs from coverage report."""
        if not coverage_report:
            return []
        
        try:
            from valknut.detectors.coverage import CoverageReportParser, CoverageContextExtractor
        except ImportError:
            logger.warning("Coverage analysis not available - install coverage dependencies")
            return []
        
        # Find significant uncovered segments
        uncovered_segments = self._find_uncovered_segments(coverage_report, parse_index, entities)
        
        if not uncovered_segments:
            return []
        
        # Build packs by grouping segments
        packs = self._build_packs_from_segments(uncovered_segments)
        
        return packs[:self.top_n]
    
    def _find_uncovered_segments(
        self, 
        coverage_data: dict,
        parse_index: ParseIndex, 
        entities: Dict[str, Entity]
    ) -> List[UncoveredSegment]:
        """Find significant uncovered code segments."""
        segments = []
        
        # Process each file in coverage data
        for file_path, file_coverage in coverage_data.get('files', {}).items():
            if not file_coverage.get('uncovered_lines'):
                continue
            
            # Group consecutive uncovered lines into segments
            file_segments = self._group_uncovered_lines(file_path, file_coverage, entities)
            segments.extend(file_segments)
        
        # Sort by priority (most impactful first)
        segments.sort(key=lambda s: self._calculate_segment_priority(s), reverse=True)
        
        return segments
    
    def _group_uncovered_lines(
        self, 
        file_path: str, 
        file_coverage: dict, 
        entities: Dict[str, Entity]
    ) -> List[UncoveredSegment]:
        """Group consecutive uncovered lines into segments."""
        uncovered_lines = sorted(file_coverage.get('uncovered_lines', []))
        if not uncovered_lines:
            return []
        
        segments = []
        current_segment_start = uncovered_lines[0]
        current_segment_end = uncovered_lines[0]
        
        for line_num in uncovered_lines[1:]:
            if line_num == current_segment_end + 1:
                # Extend current segment
                current_segment_end = line_num
            else:
                # Finish current segment and start new one
                if current_segment_end - current_segment_start + 1 >= self.min_segment_size:
                    segment = self._create_segment(
                        file_path, current_segment_start, current_segment_end, entities
                    )
                    if segment:
                        segments.append(segment)
                
                current_segment_start = line_num
                current_segment_end = line_num
        
        # Handle final segment
        if current_segment_end - current_segment_start + 1 >= self.min_segment_size:
            segment = self._create_segment(
                file_path, current_segment_start, current_segment_end, entities
            )
            if segment:
                segments.append(segment)
        
        return segments
    
    def _create_segment(
        self, 
        file_path: str, 
        start_line: int, 
        end_line: int, 
        entities: Dict[str, Entity]
    ) -> Optional[UncoveredSegment]:
        """Create an uncovered segment with context."""
        # Find containing entity
        containing_entity = None
        for entity in entities.values():
            entity_file_path = getattr(entity, 'file_path', None) or entity.location.file_path
            entity_start_line = getattr(entity, 'start_line', None) or entity.location.start_line
            entity_end_line = getattr(entity, 'end_line', None) or entity.location.end_line
            
            if (entity_file_path and Path(entity_file_path).name == Path(file_path).name and
                entity_start_line and entity_end_line and
                entity_start_line <= start_line <= entity_end_line):
                containing_entity = entity
                break
        
        # Read file content to get context lines
        context_lines = []
        complexity_hints = []
        
        try:
            # Try to find source file
            source_file = None
            for entity in entities.values():
                entity_file_path = getattr(entity, 'file_path', None) or entity.location.file_path
                if entity_file_path and Path(entity_file_path).name == Path(file_path).name:
                    source_file = entity_file_path
                    break
            
            if source_file and Path(source_file).exists():
                with open(source_file, 'r', encoding='utf-8', errors='ignore') as f:
                    file_lines = f.readlines()
                
                # Extract key context lines (not the full span)
                context_lines, complexity_hints = self._extract_context_summary(
                    file_lines, start_line, end_line
                )
        
        except Exception as e:
            logger.debug(f"Could not read context for {file_path}: {e}")
        
        return UncoveredSegment(
            file_path=file_path,
            start_line=start_line,
            end_line=end_line,
            context_lines=context_lines,
            entity_name=containing_entity.name if containing_entity else None,
            entity_id=getattr(containing_entity, 'id', None) or (containing_entity.location.file_path if containing_entity else None),
            entity_type=containing_entity.kind.name.lower() if containing_entity else None,
            complexity_hints=complexity_hints
        )
    
    def _extract_context_summary(
        self, 
        file_lines: List[str], 
        start_line: int, 
        end_line: int
    ) -> Tuple[List[str], List[str]]:
        """Extract a summary of key context lines, not the full block."""
        context_lines = []
        complexity_hints = []
        
        # Get a few key lines to give context
        lines_to_check = []
        
        # Always include the first line of the block
        if 1 <= start_line <= len(file_lines):
            lines_to_check.append(start_line)
        
        # Include middle line if block is large
        if end_line - start_line >= 5:
            mid_line = (start_line + end_line) // 2
            lines_to_check.append(mid_line)
        
        # Always include last line if different from first
        if end_line != start_line and 1 <= end_line <= len(file_lines):
            lines_to_check.append(end_line)
        
        # Extract context from these key lines
        for line_num in lines_to_check[:self.max_context_lines]:
            if 1 <= line_num <= len(file_lines):
                line_content = file_lines[line_num - 1].strip()
                
                # Skip empty lines
                if not line_content:
                    continue
                
                # Add line with line number for reference
                context_lines.append(f"L{line_num}: {line_content}")
                
                # Analyze for complexity
                complexity = self._analyze_line_complexity(line_content)
                if complexity:
                    complexity_hints.append(complexity)
        
        # If we have a large block, add a summary comment
        block_size = end_line - start_line + 1
        if block_size > len(context_lines) + 2:
            context_lines.append(f"# (... {block_size - len(context_lines)} more lines omitted for brevity)")
        
        return context_lines, list(set(complexity_hints))  # Remove duplicates
    
    def _analyze_line_complexity(self, line_content: str) -> Optional[str]:
        """Analyze line for complexity hints."""
        line = line_content.strip().lower()
        
        if any(keyword in line for keyword in ['if ', 'elif ', 'else:', 'while ', 'for ']):
            return "conditional"
        elif any(keyword in line for keyword in ['try:', 'except:', 'except ', 'finally:', 'raise']):
            return "exception_handler"
        elif any(keyword in line for keyword in ['def ', 'class ', 'async def']):
            return "definition"
        elif any(keyword in line for keyword in ['return', 'yield', 'break', 'continue']):
            return "control_flow"
        elif any(op in line for op in ['and ', 'or ', 'not ']):
            return "logical_operation"
        
        return None
    
    def _calculate_segment_priority(self, segment: UncoveredSegment) -> float:
        """Calculate priority score for a segment."""
        priority = 0.0
        
        # Size factor
        segment_size = segment.end_line - segment.start_line + 1
        priority += min(segment_size / 20.0, 1.0) * 0.4
        
        # Entity type factor
        if segment.entity_type == "function":
            priority += 0.3
        elif segment.entity_type == "method":
            priority += 0.3
        elif segment.entity_type == "class":
            priority += 0.2
        
        # Complexity factor
        complexity_bonus = len(segment.complexity_hints) * 0.1
        priority += min(complexity_bonus, 0.3)
        
        # Public entity bonus (simple heuristic)
        if segment.entity_name and not segment.entity_name.startswith('_'):
            priority += 0.1
        
        return priority
    
    def _build_packs_from_segments(
        self, 
        segments: List[UncoveredSegment]
    ) -> List[CoveragePack]:
        """Build coverage packs by grouping related segments."""
        if not segments:
            return []
        
        # For now, create one pack per file to keep it simple
        packs_by_file = {}
        
        for segment in segments:
            file_path = segment.file_path
            
            if file_path not in packs_by_file:
                packs_by_file[file_path] = {
                    'segments': [],
                    'total_lines': 0,
                    'current_coverage': 0.0
                }
            
            packs_by_file[file_path]['segments'].append(segment)
            packs_by_file[file_path]['total_lines'] += (segment.end_line - segment.start_line + 1)
        
        # Build packs
        packs = []
        for i, (file_path, pack_data) in enumerate(packs_by_file.items()):
            if not pack_data['segments']:
                continue
            
            pack = self._create_coverage_pack(file_path, pack_data, i)
            packs.append(pack)
        
        return packs
    
    def _create_coverage_pack(
        self, 
        file_path: str, 
        pack_data: dict, 
        pack_index: int
    ) -> CoveragePack:
        """Create a coverage pack for a file."""
        segments = pack_data['segments']
        total_uncovered_lines = pack_data['total_lines']
        
        # Calculate current and target coverage
        current_coverage = pack_data.get('current_coverage', 50.0)  # Default estimate
        target_coverage = min(current_coverage + (total_uncovered_lines / 10.0), 95.0)
        
        # Calculate value and effort
        value = PackValue(
            dup_loc_removed=None,  # Not applicable
            score_drop_estimate=None,  # Not applicable
            cycles_removed=None,  # Not applicable
            scc_count_delta=None,  # Not applicable
            avg_path_len_delta=None,  # Not applicable
            cross_community_edges_reduced=None  # Not applicable
        )
        
        # Effort estimation
        effort = PackEffort(
            loc_touched=total_uncovered_lines * 2,  # Uncovered lines + test lines
            callsites=None,  # Not applicable
            modules_touched=1,  # Just this file
            imports_to_rehome_est=None  # Not applicable
        )
        
        # Generate steps
        steps = self._generate_coverage_steps(segments)
        
        # Generate explanations
        explanations = self._generate_coverage_explanations(file_path, segments, current_coverage)
        
        pack_id = f"coveragepack:FILE{pack_index}"
        
        return CoveragePack(
            pack_id=pack_id,
            uncovered_segments=segments,
            current_coverage_pct=current_coverage,
            target_coverage_pct=target_coverage,
            estimated_lines_to_cover=total_uncovered_lines,
            value=value,
            effort=effort,
            steps=steps,
            explanations=explanations
        )
    
    def _generate_coverage_steps(self, segments: List[UncoveredSegment]) -> List[str]:
        """Generate actionable steps for improving coverage."""
        steps = []
        
        # Group by entity type
        function_segments = [s for s in segments if s.entity_type in ['function', 'method']]
        conditional_segments = [s for s in segments if 'conditional' in s.complexity_hints]
        exception_segments = [s for s in segments if 'exception_handler' in s.complexity_hints]
        
        if function_segments:
            steps.append(f"Add unit tests for {len(function_segments)} uncovered function(s)")
        
        if conditional_segments:
            steps.append(f"Add test cases for {len(conditional_segments)} conditional branch(es)")
        
        if exception_segments:
            steps.append(f"Add error handling tests for {len(exception_segments)} exception path(s)")
        
        # Generic step
        if not steps:
            steps.append(f"Add tests to cover {len(segments)} uncovered code segment(s)")
        
        steps.append("Run coverage analysis to verify improvement")
        
        return steps
    
    def _generate_coverage_explanations(
        self, 
        file_path: str, 
        segments: List[UncoveredSegment], 
        current_coverage: float
    ) -> List[str]:
        """Generate explanations for coverage improvement value."""
        explanations = []
        
        total_lines = sum(s.end_line - s.start_line + 1 for s in segments)
        filename = Path(file_path).name
        
        explanations.append(
            f"File {filename} has {len(segments)} significant uncovered segments "
            f"({total_lines} lines total) at {current_coverage:.1f}% coverage"
        )
        
        # Highlight high-value segments
        high_priority = [s for s in segments if self._calculate_segment_priority(s) > 0.7]
        if high_priority:
            explanations.append(
                f"{len(high_priority)} high-priority segments include complex logic or public APIs"
            )
        
        return explanations


class ImpactPackBuilder:
    """Main builder for all impact packs."""
    
    def __init__(
        self, 
        enable_clone_packs: bool = True,
        enable_cycle_packs: bool = True, 
        enable_chokepoint_packs: bool = True,
        enable_coverage_packs: bool = True,
        enable_structure_packs: bool = True,
        max_packs: int = 20,
        non_overlap: bool = True,
        centrality_samples: int = 64,
        min_similarity: float = 0.85,
        min_total_loc: int = 60,
        max_parameters: int = 6,
        top_n: int = 3,
        coverage_report_path: Optional[str] = None,
        coverage_format_hint: Optional[str] = None,
        structure_config: Optional[StructureConfig] = None
    ):
        self.enable_clone_packs = enable_clone_packs
        self.enable_cycle_packs = enable_cycle_packs
        self.enable_chokepoint_packs = enable_chokepoint_packs
        self.enable_coverage_packs = enable_coverage_packs
        self.enable_structure_packs = enable_structure_packs
        self.max_packs = max_packs
        self.non_overlap = non_overlap
        self.coverage_report_path = coverage_report_path
        self.coverage_format_hint = coverage_format_hint
        
        # Structure analyzer
        self.structure_config = structure_config or StructureConfig()
        if self.enable_structure_packs:
            self.structure_analyzer = FilesystemStructureAnalyzer(self.structure_config)
        else:
            self.structure_analyzer = None
        
        # Initialize components with specific parameters
        self.clone_consolidator = CloneConsolidator(
            min_similarity=min_similarity,
            min_total_loc=min_total_loc,
            max_parameters=max_parameters
        )
        self.cycle_cutter = CycleCutter()
        self.chokepoint_detector = ChokepointDetector(
            centrality_samples=centrality_samples,
            top_n=top_n
        )
        self.coverage_pack_builder = CoveragePackBuilder(
            top_n=top_n,
            min_segment_size=3,
            max_context_lines=5
        )
    
    def build_all_packs(
        self, 
        index: ParseIndex, 
        clone_groups: List[Dict],
        entities: Dict[str, Entity],
        files: Optional[List[Path]] = None,
        parse_indices: Optional[Dict[str, ParseIndex]] = None
    ) -> List[ImpactPack]:
        """Build all types of impact packs."""
        all_packs = []
        
        # Clone consolidation packs
        if self.enable_clone_packs and clone_groups:
            clone_packs = self.clone_consolidator.build_clonepacks(index, clone_groups, entities)
            all_packs.extend(clone_packs)
        
        # Cycle-cutting packs
        if self.enable_cycle_packs:
            cycle_packs = self.cycle_cutter.build_cycle_packs(index.import_graph, entities)
            all_packs.extend(cycle_packs)
        
        # Chokepoint packs
        if self.enable_chokepoint_packs:
            chokepoint_packs = self.chokepoint_detector.build_chokepoint_packs(index.import_graph, entities)
            all_packs.extend(chokepoint_packs)
        
        # Coverage improvement packs
        if self.enable_coverage_packs:
            coverage_report = self._load_coverage_report()
            if coverage_report:
                coverage_packs = self.coverage_pack_builder.build_coverage_packs(coverage_report, index, entities)
                all_packs.extend(coverage_packs)
        
        # Structure reorganization packs
        if self.enable_structure_packs and self.structure_analyzer and files and parse_indices:
            try:
                file_split_packs, branch_packs = self.structure_analyzer.analyze_structure(
                    files, parse_indices, index.import_graph
                )
                
                # Convert to ImpactPack format
                for pack in file_split_packs:
                    impact_pack = ImpactPack(
                        kind=pack.kind,
                        affected_entities=[],  # Structure packs affect files, not entities
                        value=pack.value.get("total_value", 0.0),
                        effort=pack.effort.get("total_effort", 1.0),
                        description=f"Split large file: {Path(pack.file).name}",
                        pack_data=pack.__dict__
                    )
                    all_packs.append(impact_pack)
                    
                for pack in branch_packs:
                    impact_pack = ImpactPack(
                        kind=pack.kind,
                        affected_entities=[],  # Structure packs affect directories, not entities  
                        value=pack.value.get("imbalance_gain", 0.0),
                        effort=pack.effort.get("files_moved", 1.0),
                        description=f"Reorganize directory: {Path(pack.dir).name}",
                        pack_data=pack.__dict__
                    )
                    all_packs.append(impact_pack)
                    
                logger.debug(f"Generated {len(file_split_packs)} file-split and {len(branch_packs)} branch-reorg structure packs")
                
            except Exception as e:
                logger.warning(f"Structure analysis failed: {e}")
        
        # Rank and select
        ranked_packs = self._rank_packs(all_packs)
        
        # Apply non-overlap constraint
        if self.non_overlap:
            ranked_packs = self._apply_non_overlap(ranked_packs)
        
        # Limit to max_packs
        return ranked_packs[:self.max_packs]
    
    def _load_coverage_report(self) -> Optional[dict]:
        """Load coverage report from configured path."""
        if not self.coverage_report_path:
            # Try to find common coverage report paths
            common_paths = [
                "coverage.json",
                ".coverage", 
                "coverage/coverage.json",
                "coverage/lcov.info",
                "coverage/cobertura.xml",
                "nyc_output/coverage-final.json",
                "htmlcov/coverage.json",
                "build/reports/jacoco/test/jacocoTestReport.xml"
            ]
            
            for path_str in common_paths:
                path = Path(path_str)
                if path.exists():
                    logger.info(f"Auto-detected coverage report: {path}")
                    self.coverage_report_path = str(path)
                    break
        
        if not self.coverage_report_path:
            logger.debug("No coverage report found")
            return None
        
        try:
            from valknut.detectors.coverage import CoverageReportParser
            
            parser = CoverageReportParser()
            report = parser.parse(Path(self.coverage_report_path), self.coverage_format_hint)
            
            if report:
                # Convert to dict format expected by coverage pack builder
                coverage_dict = {
                    'files': {}
                }
                
                for file_path, file_coverage in report.files.items():
                    coverage_dict['files'][file_path] = {
                        'uncovered_lines': file_coverage.uncovered_lines,
                        'total_lines': file_coverage.total_lines,
                        'covered_lines': file_coverage.covered_lines,
                        'coverage_percentage': file_coverage.coverage_percentage
                    }
                
                coverage_dict['total_coverage_percentage'] = report.total_coverage_percentage
                logger.info(f"Loaded coverage report with {len(report.files)} files")
                return coverage_dict
            
        except Exception as e:
            logger.warning(f"Failed to load coverage report {self.coverage_report_path}: {e}")
        
        return None
    
    def _rank_packs(self, packs: List[ImpactPack]) -> List[ImpactPack]:
        """Rank packs by value/effort ratio."""
        scored_packs = []
        
        for pack in packs:
            value_score = self._calculate_value_score(pack)
            effort_score = self._calculate_effort_score(pack)
            
            # Avoid division by zero
            ratio = value_score / max(effort_score, 1.0)
            scored_packs.append((ratio, pack))
        
        # Sort by ratio descending
        scored_packs.sort(key=lambda x: x[0], reverse=True)
        
        return [pack for _, pack in scored_packs]
    
    def _calculate_value_score(self, pack: ImpactPack) -> float:
        """Calculate value score for ranking."""
        value = pack.value
        
        if isinstance(pack, ClonePack):
            return (value.dup_loc_removed or 0) / 100.0 + (value.score_drop_estimate or 0) * 10
        elif isinstance(pack, CyclePack):
            return (value.cycles_removed or 0) + 0.5 * (value.scc_count_delta or 0) + 10 * (value.avg_path_len_delta or 0)
        elif isinstance(pack, ChokepointPack):
            return (value.cross_community_edges_reduced or 0) * 2.0
        elif isinstance(pack, CoveragePack):
            # Value based on coverage improvement potential
            coverage_gain = pack.target_coverage_pct - pack.current_coverage_pct
            lines_factor = min(pack.estimated_lines_to_cover / 50.0, 2.0)  # Cap at 2x for 50+ lines
            return coverage_gain * 0.1 + lines_factor * 0.5  # Weight both coverage gain and impact size
        
        return 1.0
    
    def _calculate_effort_score(self, pack: ImpactPack) -> float:
        """Calculate effort score for ranking."""
        effort = pack.effort
        
        if isinstance(pack, ClonePack):
            return (effort.loc_touched or 0) / 10.0 + (effort.callsites or 0)
        elif isinstance(pack, CyclePack):
            return (effort.modules_touched or 0) + (effort.imports_to_rehome_est or 0) / 3.0
        elif isinstance(pack, ChokepointPack):
            return (effort.modules_touched or 0) + (effort.imports_to_rehome_est or 0) / 5.0
        elif isinstance(pack, CoveragePack):
            # Effort based on lines to test (test lines usually ~2x code lines)
            return (effort.loc_touched or 0) / 20.0  # Normalize test effort
        
        return 1.0
    
    def _apply_non_overlap(self, packs: List[ImpactPack]) -> List[ImpactPack]:
        """Apply non-overlap constraint."""
        if not self.non_overlap:
            return packs
        
        selected = []
        used_entities = set()
        
        for pack in packs:
            pack_entities = self._get_pack_entities(pack)
            
            # Check for overlap
            if not pack_entities.intersection(used_entities):
                selected.append(pack)
                used_entities.update(pack_entities)
        
        return selected
    
    def _get_pack_entities(self, pack: ImpactPack) -> Set[str]:
        """Get set of entities involved in a pack."""
        entities = set()
        
        if isinstance(pack, ClonePack):
            entities.update(member.entity_id for member in pack.members)
        elif isinstance(pack, CyclePack):
            entities.update(pack.scc_members)
            entities.update(pack.cut_nodes)
        elif isinstance(pack, ChokepointPack):
            entities.add(pack.chokepoint_node)
        elif isinstance(pack, CoveragePack):
            # Use file paths and entity IDs from uncovered segments
            for segment in pack.uncovered_segments:
                entities.add(segment.file_path)
                if segment.entity_id:
                    entities.add(segment.entity_id)
        
        return entities