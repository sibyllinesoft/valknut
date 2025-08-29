"""
Graph analysis features - centrality, cycles, fan-in/fan-out.
"""

from typing import Dict, List, Optional, Set

import networkx as nx

from valknut.core.featureset import BaseFeatureExtractor
from valknut.lang.common_ast import Entity, EntityKind, ParseIndex


class GraphExtractor(BaseFeatureExtractor):
    """Extractor for graph-based features."""
    
    @property
    def name(self) -> str:
        return "graph"
    
    def _initialize_features(self) -> None:
        """Initialize graph features."""
        self._add_feature(
            "betweenness_approx",
            "Approximate betweenness centrality",
            min_value=0.0,
            max_value=1.0,
            default_value=0.0,
        )
        self._add_feature(
            "fan_in",
            "Number of incoming dependencies",
            min_value=0.0,
            max_value=100.0,
            default_value=0.0,
        )
        self._add_feature(
            "fan_out", 
            "Number of outgoing dependencies",
            min_value=0.0,
            max_value=100.0,
            default_value=0.0,
        )
        self._add_feature(
            "in_cycle",
            "Whether entity is part of a dependency cycle",
            min_value=0.0,
            max_value=1.0,
            default_value=0.0,
        )
        self._add_feature(
            "cycle_size",
            "Size of largest cycle entity participates in",
            min_value=0.0,
            max_value=1.0,  # Normalized by total nodes
            default_value=0.0,
        )
        self._add_feature(
            "closeness",
            "Closeness centrality",
            min_value=0.0,
            max_value=1.0,
            default_value=0.0,
        )
        self._add_feature(
            "eigenvector",
            "Eigenvector centrality",
            min_value=0.0,
            max_value=1.0,
            default_value=0.0,
        )
    
    def supports_entity(self, entity: Entity) -> bool:
        """Support all entity types."""
        return True
    
    def extract(self, entity: Entity, index: ParseIndex) -> Dict[str, float]:
        """Extract graph-based features."""
        features = {}
        
        # Use import graph as primary, call graph as secondary
        graph = index.import_graph
        if graph is None:
            graph = index.call_graph
        
        if graph is None or entity.id not in graph:
            return {f.name: f.default_value for f in self.features}
        
        features["betweenness_approx"] = self._safe_extract(
            entity, index, "betweenness_approx",
            lambda: self._calculate_betweenness_approx(entity.id, graph)
        )
        
        features["fan_in"] = self._safe_extract(
            entity, index, "fan_in",
            lambda: float(graph.in_degree(entity.id))
        )
        
        features["fan_out"] = self._safe_extract(
            entity, index, "fan_out",
            lambda: float(graph.out_degree(entity.id))
        )
        
        cycle_info = self._safe_extract(
            entity, index, "cycle_info",
            lambda: self._calculate_cycle_info(entity.id, graph)
        )
        
        if isinstance(cycle_info, dict):
            features["in_cycle"] = cycle_info.get("in_cycle", 0.0)
            features["cycle_size"] = cycle_info.get("cycle_size", 0.0)
        else:
            features["in_cycle"] = 0.0
            features["cycle_size"] = 0.0
        
        features["closeness"] = self._safe_extract(
            entity, index, "closeness",
            lambda: self._calculate_closeness(entity.id, graph)
        )
        
        features["eigenvector"] = self._safe_extract(
            entity, index, "eigenvector", 
            lambda: self._calculate_eigenvector(entity.id, graph)
        )
        
        return features
    
    def _calculate_betweenness_approx(self, entity_id: str, graph: nx.DiGraph) -> float:
        """Calculate approximate betweenness centrality with sampling."""
        if graph.number_of_nodes() < 10:
            # For small graphs, calculate exact betweenness
            centrality = nx.betweenness_centrality(graph)
            return centrality.get(entity_id, 0.0)
        
        # Use sampling for larger graphs
        k = min(64, graph.number_of_nodes() // 4)
        try:
            centrality = nx.betweenness_centrality(graph, k=k)
            return centrality.get(entity_id, 0.0)
        except Exception:
            return 0.0
    
    def _calculate_cycle_info(self, entity_id: str, graph: nx.DiGraph) -> Dict[str, float]:
        """Calculate cycle-related information."""
        try:
            # Find strongly connected components
            sccs = list(nx.strongly_connected_components(graph))
            
            # Find which SCC contains this entity
            entity_scc = None
            entity_scc_size = 0
            
            for scc in sccs:
                if entity_id in scc:
                    entity_scc = scc
                    entity_scc_size = len(scc)
                    break
            
            in_cycle = 1.0 if entity_scc_size > 1 else 0.0
            cycle_size = entity_scc_size / graph.number_of_nodes() if entity_scc_size > 1 else 0.0
            
            return {
                "in_cycle": in_cycle,
                "cycle_size": cycle_size,
            }
        except Exception:
            return {"in_cycle": 0.0, "cycle_size": 0.0}
    
    def _calculate_closeness(self, entity_id: str, graph: nx.DiGraph) -> float:
        """Calculate closeness centrality."""
        try:
            centrality = nx.closeness_centrality(graph)
            return centrality.get(entity_id, 0.0)
        except Exception:
            return 0.0
    
    def _calculate_eigenvector(self, entity_id: str, graph: nx.DiGraph) -> float:
        """Calculate eigenvector centrality."""
        try:
            centrality = nx.eigenvector_centrality(graph, max_iter=1000)
            return centrality.get(entity_id, 0.0)
        except Exception:
            # Eigenvector centrality can fail on some graphs
            return 0.0


class CallGraphBuilder:
    """Helper class for building call graphs from parsed code."""
    
    def __init__(self) -> None:
        self.graph = nx.DiGraph()
    
    def add_call(self, caller: str, callee: str) -> None:
        """Add a call relationship."""
        self.graph.add_edge(caller, callee)
    
    def add_entity(self, entity_id: str, entity_type: str = "unknown") -> None:
        """Add an entity to the graph."""
        self.graph.add_node(entity_id, entity_type=entity_type)
    
    def get_graph(self) -> nx.DiGraph:
        """Get the constructed graph."""
        return self.graph


class ImportGraphBuilder:
    """Helper class for building import graphs from parsed code."""
    
    def __init__(self) -> None:
        self.graph = nx.DiGraph()
    
    def add_import(self, importer: str, imported: str) -> None:
        """Add an import relationship."""
        self.graph.add_edge(importer, imported)
    
    def add_module(self, module_id: str, module_path: str) -> None:
        """Add a module to the graph."""
        self.graph.add_node(module_id, path=module_path)
    
    def get_graph(self) -> nx.DiGraph:
        """Get the constructed graph."""
        return self.graph
    
    def analyze_circular_imports(self) -> List[List[str]]:
        """Find circular import chains."""
        try:
            sccs = nx.strongly_connected_components(self.graph)
            cycles = [list(scc) for scc in sccs if len(scc) > 1]
            return cycles
        except Exception:
            return []


def build_entity_graph(entities: List[Entity], relationships: List[tuple[str, str]]) -> nx.DiGraph:
    """
    Build a graph from entities and relationships.
    
    Args:
        entities: List of entities
        relationships: List of (source, target) relationships
        
    Returns:
        Directed graph
    """
    graph = nx.DiGraph()
    
    # Add entities as nodes
    for entity in entities:
        graph.add_node(
            entity.id,
            name=entity.name,
            kind=entity.kind.value,
            loc=entity.loc,
        )
    
    # Add relationships as edges
    for source, target in relationships:
        if source in graph and target in graph:
            graph.add_edge(source, target)
    
    return graph


def calculate_graph_metrics(graph: nx.DiGraph) -> Dict[str, Dict[str, float]]:
    """
    Calculate various graph metrics for all nodes.
    
    Args:
        graph: Input graph
        
    Returns:
        Dictionary mapping node_id to metrics dict
    """
    metrics = {}
    
    if not graph.nodes():
        return metrics
    
    try:
        # Centrality metrics
        betweenness = nx.betweenness_centrality(graph, k=min(64, len(graph.nodes) // 4))
        closeness = nx.closeness_centrality(graph)
        
        try:
            eigenvector = nx.eigenvector_centrality(graph, max_iter=1000)
        except:
            eigenvector = {node: 0.0 for node in graph.nodes()}
        
        # Cycle analysis
        sccs = list(nx.strongly_connected_components(graph))
        node_to_scc = {}
        for i, scc in enumerate(sccs):
            for node in scc:
                node_to_scc[node] = i
        
        # Combine metrics
        for node in graph.nodes():
            scc_index = node_to_scc.get(node, -1)
            scc_size = len(sccs[scc_index]) if scc_index >= 0 else 1
            
            metrics[node] = {
                "betweenness_approx": betweenness.get(node, 0.0),
                "closeness": closeness.get(node, 0.0),
                "eigenvector": eigenvector.get(node, 0.0),
                "fan_in": float(graph.in_degree(node)),
                "fan_out": float(graph.out_degree(node)),
                "in_cycle": 1.0 if scc_size > 1 else 0.0,
                "cycle_size": scc_size / len(graph.nodes()) if scc_size > 1 else 0.0,
            }
    
    except Exception:
        # Fallback to basic metrics only
        for node in graph.nodes():
            metrics[node] = {
                "betweenness_approx": 0.0,
                "closeness": 0.0,
                "eigenvector": 0.0,
                "fan_in": float(graph.in_degree(node)),
                "fan_out": float(graph.out_degree(node)),
                "in_cycle": 0.0,
                "cycle_size": 0.0,
            }
    
    return metrics