//! Cohesion graph operations for entity analysis and community detection.

use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::HashSet;

use crate::core::errors::Result;
use crate::detectors::structure::config::{CohesionEdge, CohesionGraph, EntityNode, StructureConfig};

/// Build entity cohesion graph from entities
pub fn build_cohesion_graph(entities: Vec<EntityNode>) -> CohesionGraph {
    let mut graph = Graph::new_undirected();

    if entities.len() < 2 {
        // Need at least 2 entities for cohesion analysis
        for entity in entities {
            graph.add_node(entity);
        }
        return graph;
    }

    // Add entity nodes to graph
    let mut entity_nodes = Vec::new();
    for entity in entities {
        let node_idx = graph.add_node(entity);
        entity_nodes.push(node_idx);
    }

    // Calculate cohesion between all pairs of entities
    for i in 0..entity_nodes.len() {
        for j in i + 1..entity_nodes.len() {
            let entity_a = &graph[entity_nodes[i]];
            let entity_b = &graph[entity_nodes[j]];

            let jaccard_similarity = calculate_jaccard_similarity(&entity_a.symbols, &entity_b.symbols);

            // Only add edges for significant cohesion
            if jaccard_similarity > 0.1 {
                let shared_symbols = entity_a.symbols.intersection(&entity_b.symbols).count();
                let edge = CohesionEdge {
                    similarity: jaccard_similarity,
                    shared_symbols,
                };
                graph.add_edge(entity_nodes[i], entity_nodes[j], edge);
            }
        }
    }

    graph
}

/// Calculate Jaccard similarity between two symbol sets
pub fn calculate_jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }

    let intersection = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// Community finder for cohesion graphs
pub struct CommunityFinder<'a> {
    config: &'a StructureConfig,
}

/// Community detection methods for [`CommunityFinder`].
impl<'a> CommunityFinder<'a> {
    /// Creates a new community finder with the given configuration.
    pub fn new(config: &'a StructureConfig) -> Self {
        Self { config }
    }

    /// Find cohesion communities in entity graph
    pub fn find_communities(&self, graph: &CohesionGraph) -> Result<Vec<Vec<NodeIndex>>> {
        let node_indices: Vec<_> = graph.node_indices().collect();

        if node_indices.len() < 2 {
            return Ok(vec![node_indices]);
        }

        let edges = self.collect_sorted_edges(graph);
        let (mut communities, mut assigned_nodes) = self.build_communities_from_edges(&edges);

        // Add remaining nodes as singleton communities
        for node in node_indices {
            if !assigned_nodes.contains(&node) {
                communities.push(vec![node]);
            }
        }

        // Filter and limit communities
        communities.retain(|comm| comm.len() >= self.config.fsfile.min_entities_per_split);
        communities.truncate(3);

        Ok(communities)
    }

    /// Collect edges sorted by cohesion strength (descending)
    fn collect_sorted_edges(&self, graph: &CohesionGraph) -> Vec<(NodeIndex, NodeIndex, f64)> {
        let mut edges: Vec<_> = graph
            .edge_indices()
            .filter_map(|edge_idx| {
                let (source, target) = graph.edge_endpoints(edge_idx)?;
                let weight = graph.edge_weight(edge_idx)?;
                Some((source, target, weight.similarity))
            })
            .collect();

        edges.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        edges
    }

    /// Build communities greedily from sorted edges
    fn build_communities_from_edges(
        &self,
        edges: &[(NodeIndex, NodeIndex, f64)],
    ) -> (Vec<Vec<NodeIndex>>, HashSet<NodeIndex>) {
        let mut communities: Vec<Vec<NodeIndex>> = Vec::new();
        let mut assigned_nodes = HashSet::new();

        for &(source, target, similarity) in edges {
            if similarity < 0.2 {
                break;
            }

            let source_comm = self.find_node_community(&communities, source);
            let target_comm = self.find_node_community(&communities, target);

            self.assign_nodes_to_community(
                source,
                target,
                source_comm,
                target_comm,
                &mut communities,
                &mut assigned_nodes,
            );
        }

        (communities, assigned_nodes)
    }

    /// Find which community a node belongs to
    fn find_node_community(&self, communities: &[Vec<NodeIndex>], node: NodeIndex) -> Option<usize> {
        communities.iter().position(|comm| comm.contains(&node))
    }

    /// Assign nodes to communities based on their current membership
    fn assign_nodes_to_community(
        &self,
        source: NodeIndex,
        target: NodeIndex,
        source_comm: Option<usize>,
        target_comm: Option<usize>,
        communities: &mut Vec<Vec<NodeIndex>>,
        assigned_nodes: &mut HashSet<NodeIndex>,
    ) {
        match (source_comm, target_comm) {
            (Some(idx), None) => self.add_to_community(target, idx, communities, assigned_nodes),
            (None, Some(idx)) => self.add_to_community(source, idx, communities, assigned_nodes),
            (None, None) => self.create_new_community(source, target, communities, assigned_nodes),
            (Some(_), Some(_)) => {} // Both already assigned
        }
    }

    /// Add a node to an existing community
    fn add_to_community(
        &self,
        node: NodeIndex,
        comm_idx: usize,
        communities: &mut [Vec<NodeIndex>],
        assigned_nodes: &mut HashSet<NodeIndex>,
    ) {
        if !assigned_nodes.contains(&node) {
            communities[comm_idx].push(node);
            assigned_nodes.insert(node);
        }
    }

    /// Create a new community with two nodes
    fn create_new_community(
        &self,
        source: NodeIndex,
        target: NodeIndex,
        communities: &mut Vec<Vec<NodeIndex>>,
        assigned_nodes: &mut HashSet<NodeIndex>,
    ) {
        let mut new_community = Vec::new();
        if !assigned_nodes.contains(&source) {
            new_community.push(source);
            assigned_nodes.insert(source);
        }
        if !assigned_nodes.contains(&target) {
            new_community.push(target);
            assigned_nodes.insert(target);
        }
        if !new_community.is_empty() {
            communities.push(new_community);
        }
    }
}

/// Estimate clone factor from cohesion graph
pub fn estimate_clone_factor(graph: &CohesionGraph) -> f64 {
    let node_count = graph.node_count();
    if node_count < 2 {
        return 0.0;
    }

    let mut heavy_edges = 0usize;
    for edge_idx in graph.edge_indices() {
        if let Some(edge) = graph.edge_weight(edge_idx) {
            if edge.similarity >= 0.75 && edge.shared_symbols >= 3 {
                heavy_edges += 1;
            }
        }
    }

    if heavy_edges == 0 {
        return 0.0;
    }

    let max_edges = (node_count.saturating_sub(1) * node_count) / 2;
    if max_edges == 0 {
        return 0.0;
    }

    (heavy_edges as f64 / max_edges as f64).min(1.0)
}
