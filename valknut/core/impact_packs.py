"""
Impact Packs: Clone Consolidation and Cycle-Cut recommendations.

This module implements two types of strategic refactoring recommendations:
1. ClonePacks - Template extraction from near-duplicate code
2. CyclePacks/ChokepointPacks - Dependency cycle breaking recommendations
"""

import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple, Union
from uuid import uuid4

import networkx as nx

from valknut.lang.common_ast import Entity, EntityKind, ParseIndex


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


ImpactPack = Union[ClonePack, CyclePack, ChokepointPack]


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


class ImpactPackBuilder:
    """Main builder for all impact packs."""
    
    def __init__(
        self, 
        enable_clone_packs: bool = True,
        enable_cycle_packs: bool = True, 
        enable_chokepoint_packs: bool = True,
        max_packs: int = 20,
        non_overlap: bool = True,
        centrality_samples: int = 64,
        min_similarity: float = 0.85,
        min_total_loc: int = 60,
        max_parameters: int = 6,
        top_n: int = 3
    ):
        self.enable_clone_packs = enable_clone_packs
        self.enable_cycle_packs = enable_cycle_packs
        self.enable_chokepoint_packs = enable_chokepoint_packs
        self.max_packs = max_packs
        self.non_overlap = non_overlap
        
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
    
    def build_all_packs(
        self, 
        index: ParseIndex, 
        clone_groups: List[Dict],
        entities: Dict[str, Entity]
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
        
        # Rank and select
        ranked_packs = self._rank_packs(all_packs)
        
        # Apply non-overlap constraint
        if self.non_overlap:
            ranked_packs = self._apply_non_overlap(ranked_packs)
        
        # Limit to max_packs
        return ranked_packs[:self.max_packs]
    
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
        
        return entities