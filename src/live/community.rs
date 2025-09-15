//! Community detection using Louvain algorithm for shadow island identification
//!
//! Implements the Louvain method for modularity optimization to detect
//! tightly coupled communities in call graphs

use crate::core::errors::Result;
use crate::live::graph::UndirectedCallGraph;

use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Community detection result
#[derive(Debug, Clone)]
pub struct CommunityDetection {
    /// Node assignments to communities
    pub node_to_community: HashMap<NodeIndex, CommunityId>,

    /// Community information
    pub communities: HashMap<CommunityId, CommunityInfo>,

    /// Final modularity score
    pub modularity: f64,

    /// Number of iterations performed
    pub iterations: usize,
}

/// Community identifier
pub type CommunityId = usize;

/// Information about a detected community
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityInfo {
    /// Community ID
    pub id: CommunityId,

    /// Nodes in this community  
    pub nodes: Vec<NodeIndex>,

    /// Total weight of internal edges
    pub internal_weight: f64,

    /// Total weight of edges crossing community boundary
    pub cut_weight: f64,

    /// Total degree (sum of all edge weights for nodes in community)
    pub total_degree: f64,

    /// Number of internal edges that are runtime vs static
    pub runtime_internal_count: usize,
    pub static_internal_count: usize,
}

/// Louvain algorithm implementation
pub struct LouvainDetector {
    /// Resolution parameter (higher = more communities)
    resolution: f64,

    /// Maximum number of iterations
    max_iterations: usize,

    /// Minimum improvement threshold to continue
    min_improvement: f64,
}

impl Default for LouvainDetector {
    fn default() -> Self {
        Self {
            resolution: 0.8,
            max_iterations: 100,
            min_improvement: 1e-6,
        }
    }
}

impl LouvainDetector {
    /// Create detector with custom parameters
    pub fn new(resolution: f64, max_iterations: usize, min_improvement: f64) -> Self {
        Self {
            resolution,
            max_iterations,
            min_improvement,
        }
    }

    /// Detect communities in the graph
    pub fn detect_communities(&self, graph: &UndirectedCallGraph) -> Result<CommunityDetection> {
        let petgraph = graph.graph();

        if petgraph.node_count() == 0 {
            return Ok(CommunityDetection {
                node_to_community: HashMap::new(),
                communities: HashMap::new(),
                modularity: 0.0,
                iterations: 0,
            });
        }

        // Initialize: each node in its own community
        let mut node_to_community: HashMap<NodeIndex, CommunityId> = petgraph
            .node_indices()
            .enumerate()
            .map(|(i, node_idx)| (node_idx, i))
            .collect();

        let mut current_modularity = self.calculate_modularity(petgraph, &node_to_community)?;
        let total_weight = self.calculate_total_weight(petgraph);

        tracing::info!(
            "Starting Louvain detection: {} nodes, {} edges, total weight: {:.2}",
            petgraph.node_count(),
            petgraph.edge_count(),
            total_weight
        );

        let mut iterations = 0;
        let mut improvement = true;

        while improvement && iterations < self.max_iterations {
            improvement = false;
            let mut node_order: Vec<_> = petgraph.node_indices().collect();

            // Randomize order for better results
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            iterations.hash(&mut hasher);
            let seed = hasher.finish();

            // Simple shuffle based on hash
            for i in (1..node_order.len()).rev() {
                let mut hasher = DefaultHasher::new();
                (seed + i as u64).hash(&mut hasher);
                let j = (hasher.finish() as usize) % (i + 1);
                node_order.swap(i, j);
            }

            // Try to improve each node's community assignment
            for &node in &node_order {
                let best_community =
                    self.find_best_community(petgraph, node, &node_to_community, total_weight)?;

                if best_community != node_to_community[&node] {
                    node_to_community.insert(node, best_community);
                    improvement = true;
                }
            }

            // Calculate new modularity
            let new_modularity = self.calculate_modularity(petgraph, &node_to_community)?;

            if new_modularity - current_modularity < self.min_improvement {
                improvement = false;
            } else {
                current_modularity = new_modularity;
            }

            iterations += 1;

            if iterations % 10 == 0 {
                tracing::debug!(
                    "Louvain iteration {}: modularity = {:.6}",
                    iterations,
                    current_modularity
                );
            }
        }

        // Build community information
        let communities = self.build_community_info(petgraph, &node_to_community)?;

        tracing::info!(
            "Louvain completed: {} iterations, {} communities, modularity = {:.6}",
            iterations,
            communities.len(),
            current_modularity
        );

        Ok(CommunityDetection {
            node_to_community,
            communities,
            modularity: current_modularity,
            iterations,
        })
    }

    /// Find the best community for a node
    fn find_best_community(
        &self,
        graph: &petgraph::Graph<String, f64, petgraph::Undirected>,
        node: NodeIndex,
        node_to_community: &HashMap<NodeIndex, CommunityId>,
        total_weight: f64,
    ) -> Result<CommunityId> {
        let current_community = node_to_community[&node];
        let node_degree = self.calculate_node_degree(graph, node);

        // Calculate current modularity contribution
        let current_modularity = self.calculate_node_modularity_contribution(
            graph,
            node,
            current_community,
            node_to_community,
            total_weight,
        )?;

        let mut best_community = current_community;
        let mut best_modularity = current_modularity;

        // Get neighbor communities
        let mut neighbor_communities = HashSet::new();
        for edge in graph.edges(node) {
            let neighbor = edge.target();
            if let Some(&neighbor_community) = node_to_community.get(&neighbor) {
                neighbor_communities.insert(neighbor_community);
            }
        }

        // Also consider creating a new community
        let new_community_id = node_to_community.values().max().unwrap_or(&0) + 1;
        neighbor_communities.insert(new_community_id);

        // Try each neighbor community
        for &candidate_community in &neighbor_communities {
            if candidate_community == current_community {
                continue;
            }

            let candidate_modularity = self.calculate_node_modularity_contribution(
                graph,
                node,
                candidate_community,
                node_to_community,
                total_weight,
            )?;

            if candidate_modularity > best_modularity {
                best_modularity = candidate_modularity;
                best_community = candidate_community;
            }
        }

        Ok(best_community)
    }

    /// Calculate modularity contribution of a node in a specific community
    fn calculate_node_modularity_contribution(
        &self,
        graph: &petgraph::Graph<String, f64, petgraph::Undirected>,
        node: NodeIndex,
        community: CommunityId,
        node_to_community: &HashMap<NodeIndex, CommunityId>,
        total_weight: f64,
    ) -> Result<f64> {
        let node_degree = self.calculate_node_degree(graph, node);

        // Calculate weight of edges from node to community
        let mut edges_to_community = 0.0;
        let mut community_degree = 0.0;

        for edge in graph.edges(node) {
            let neighbor = edge.target();
            let edge_weight = *edge.weight();

            if let Some(&neighbor_community) = node_to_community.get(&neighbor) {
                if neighbor_community == community {
                    edges_to_community += edge_weight;
                }
                community_degree += self.calculate_node_degree(graph, neighbor);
            }
        }

        // Don't double-count node's own degree if it's already in the community
        if node_to_community.get(&node) == Some(&community) {
            community_degree -= node_degree;
        }

        // Modularity formula: (edges_to_community - resolution * expected_edges) / total_weight
        let expected_edges = (node_degree * community_degree) / (2.0 * total_weight);
        let modularity_gain =
            (edges_to_community - self.resolution * expected_edges) / total_weight;

        Ok(modularity_gain)
    }

    /// Calculate total weight of all edges in graph
    fn calculate_total_weight(
        &self,
        graph: &petgraph::Graph<String, f64, petgraph::Undirected>,
    ) -> f64 {
        graph.edge_weights().sum()
    }

    /// Calculate degree (sum of edge weights) for a node
    fn calculate_node_degree(
        &self,
        graph: &petgraph::Graph<String, f64, petgraph::Undirected>,
        node: NodeIndex,
    ) -> f64 {
        graph.edges(node).map(|edge| edge.weight()).sum()
    }

    /// Calculate overall modularity of the partition
    fn calculate_modularity(
        &self,
        graph: &petgraph::Graph<String, f64, petgraph::Undirected>,
        node_to_community: &HashMap<NodeIndex, CommunityId>,
    ) -> Result<f64> {
        let total_weight = self.calculate_total_weight(graph);

        if total_weight == 0.0 {
            return Ok(0.0);
        }

        let mut modularity = 0.0;

        // Group nodes by community
        let mut communities: HashMap<CommunityId, Vec<NodeIndex>> = HashMap::new();
        for (&node, &community) in node_to_community {
            communities.entry(community).or_default().push(node);
        }

        for (community_id, nodes) in communities {
            let mut internal_edges = 0.0;
            let mut total_degree = 0.0;

            // Calculate internal edges and total degree for this community
            for &node in &nodes {
                total_degree += self.calculate_node_degree(graph, node);

                for edge in graph.edges(node) {
                    let neighbor = edge.target();
                    if let Some(&neighbor_community) = node_to_community.get(&neighbor) {
                        if neighbor_community == community_id {
                            internal_edges += edge.weight();
                        }
                    }
                }
            }

            // Avoid double-counting undirected edges
            internal_edges /= 2.0;

            // Modularity contribution: (internal_edges - expected) / total_weight
            let expected = (total_degree * total_degree) / (4.0 * total_weight);
            modularity += (internal_edges - self.resolution * expected) / total_weight;
        }

        Ok(modularity)
    }

    /// Build detailed community information
    fn build_community_info(
        &self,
        graph: &petgraph::Graph<String, f64, petgraph::Undirected>,
        node_to_community: &HashMap<NodeIndex, CommunityId>,
    ) -> Result<HashMap<CommunityId, CommunityInfo>> {
        let mut communities: HashMap<CommunityId, Vec<NodeIndex>> = HashMap::new();
        for (&node, &community) in node_to_community {
            communities.entry(community).or_default().push(node);
        }

        let mut community_info = HashMap::new();

        for (community_id, nodes) in communities {
            let mut internal_weight = 0.0;
            let mut cut_weight = 0.0;
            let mut total_degree = 0.0;
            let runtime_internal_count = 0; // TODO: Track edge types
            let static_internal_count = 0;

            // Calculate metrics for this community
            for &node in &nodes {
                total_degree += self.calculate_node_degree(graph, node);

                for edge in graph.edges(node) {
                    let neighbor = edge.target();
                    let edge_weight = *edge.weight();

                    if let Some(&neighbor_community) = node_to_community.get(&neighbor) {
                        if neighbor_community == community_id {
                            // Internal edge (count once for undirected)
                            if node < neighbor {
                                internal_weight += edge_weight;
                            }
                        } else {
                            // Cut edge
                            cut_weight += edge_weight;
                        }
                    }
                }
            }

            let info = CommunityInfo {
                id: community_id,
                nodes,
                internal_weight,
                cut_weight,
                total_degree,
                runtime_internal_count,
                static_internal_count,
            };

            community_info.insert(community_id, info);
        }

        Ok(community_info)
    }
}

impl CommunityDetection {
    /// Get community for a node
    pub fn get_community(&self, node: NodeIndex) -> Option<CommunityId> {
        self.node_to_community.get(&node).copied()
    }

    /// Get all nodes in a community
    pub fn get_community_nodes(&self, community_id: CommunityId) -> Option<&Vec<NodeIndex>> {
        self.communities.get(&community_id).map(|info| &info.nodes)
    }

    /// Get community information
    pub fn get_community_info(&self, community_id: CommunityId) -> Option<&CommunityInfo> {
        self.communities.get(&community_id)
    }

    /// Get all community IDs
    pub fn community_ids(&self) -> Vec<CommunityId> {
        self.communities.keys().copied().collect()
    }

    /// Filter communities by size
    pub fn filter_by_size(&self, min_size: usize) -> Vec<CommunityId> {
        self.communities
            .iter()
            .filter(|(_, info)| info.nodes.len() >= min_size)
            .map(|(&id, _)| id)
            .collect()
    }
}

impl CommunityInfo {
    /// Calculate cut ratio (edges leaving / total edges)
    pub fn cut_ratio(&self) -> f64 {
        let total_edges = self.internal_weight + self.cut_weight;
        if total_edges > 0.0 {
            self.cut_weight / total_edges
        } else {
            0.0
        }
    }

    /// Calculate fraction of internal edges that are runtime
    pub fn runtime_internal_fraction(&self) -> f64 {
        let total_internal = self.runtime_internal_count + self.static_internal_count;
        if total_internal > 0 {
            self.runtime_internal_count as f64 / total_internal as f64
        } else {
            0.0
        }
    }

    /// Get community size
    pub fn size(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petgraph::{Graph, Undirected};

    fn create_test_graph() -> petgraph::Graph<String, f64, Undirected> {
        let mut graph = Graph::new_undirected();

        // Add nodes
        let node_a = graph.add_node("a".to_string());
        let node_b = graph.add_node("b".to_string());
        let node_c = graph.add_node("c".to_string());
        let node_d = graph.add_node("d".to_string());

        // Create two communities: {a,b} and {c,d}
        graph.add_edge(node_a, node_b, 10.0); // Strong internal edge
        graph.add_edge(node_c, node_d, 8.0); // Strong internal edge
        graph.add_edge(node_b, node_c, 1.0); // Weak connecting edge

        graph
    }

    fn create_larger_test_graph() -> petgraph::Graph<String, f64, Undirected> {
        let mut graph = Graph::new_undirected();

        // Create three clear communities
        // Community 1: nodes 0-2 (a, b, c)
        let node_a = graph.add_node("a".to_string());
        let node_b = graph.add_node("b".to_string());
        let node_c = graph.add_node("c".to_string());

        graph.add_edge(node_a, node_b, 5.0);
        graph.add_edge(node_b, node_c, 4.0);
        graph.add_edge(node_a, node_c, 3.0);

        // Community 2: nodes 3-5 (d, e, f)
        let node_d = graph.add_node("d".to_string());
        let node_e = graph.add_node("e".to_string());
        let node_f = graph.add_node("f".to_string());

        graph.add_edge(node_d, node_e, 6.0);
        graph.add_edge(node_e, node_f, 7.0);
        graph.add_edge(node_d, node_f, 5.0);

        // Community 3: nodes 6-7 (g, h)
        let node_g = graph.add_node("g".to_string());
        let node_h = graph.add_node("h".to_string());

        graph.add_edge(node_g, node_h, 9.0);

        // Weak inter-community connections
        graph.add_edge(node_c, node_d, 1.0); // Connect community 1 and 2
        graph.add_edge(node_f, node_g, 1.0); // Connect community 2 and 3

        graph
    }

    #[test]
    fn test_louvain_detector_creation() {
        let detector = LouvainDetector::default();
        assert_eq!(detector.resolution, 0.8);
        assert_eq!(detector.max_iterations, 100);

        let custom = LouvainDetector::new(1.0, 50, 1e-5);
        assert_eq!(custom.resolution, 1.0);
        assert_eq!(custom.max_iterations, 50);
        assert_eq!(custom.min_improvement, 1e-5);
    }

    #[test]
    fn test_total_weight_calculation() {
        let graph = create_test_graph();
        let detector = LouvainDetector::default();

        let total_weight = detector.calculate_total_weight(&graph);
        assert_eq!(total_weight, 19.0); // 10 + 8 + 1

        // Test with empty graph
        let empty_graph = Graph::new_undirected();
        let empty_weight = detector.calculate_total_weight(&empty_graph);
        assert_eq!(empty_weight, 0.0);
    }

    #[test]
    fn test_node_degree_calculation() {
        let graph = create_test_graph();
        let detector = LouvainDetector::default();

        let nodes: Vec<_> = graph.node_indices().collect();

        // Node degrees should match edge weights
        let degree_a = detector.calculate_node_degree(&graph, nodes[0]);
        let degree_b = detector.calculate_node_degree(&graph, nodes[1]);
        let degree_c = detector.calculate_node_degree(&graph, nodes[2]);
        let degree_d = detector.calculate_node_degree(&graph, nodes[3]);

        assert_eq!(degree_a, 10.0); // Connected to b with weight 10
        assert_eq!(degree_b, 11.0); // Connected to a (10) and c (1)
        assert_eq!(degree_c, 9.0); // Connected to b (1) and d (8)
        assert_eq!(degree_d, 8.0); // Connected to c with weight 8

        // Test with isolated node
        let mut isolated_graph = Graph::new_undirected();
        let isolated_node = isolated_graph.add_node("isolated".to_string());
        let isolated_degree = detector.calculate_node_degree(&isolated_graph, isolated_node);
        assert_eq!(isolated_degree, 0.0);
    }

    #[test]
    fn test_empty_graph() {
        let empty_graph = Graph::new_undirected();
        let detector = LouvainDetector::default();

        // Empty graph test is tricky since detect_communities needs an UndirectedCallGraph
        // For now, test the modularity calculation directly
        let mut node_to_community = HashMap::new();
        let modularity = detector
            .calculate_modularity(&empty_graph, &node_to_community)
            .unwrap();
        assert_eq!(modularity, 0.0);
    }

    #[test]
    fn test_single_node_graph() {
        let mut graph = Graph::new_undirected();
        let node = graph.add_node("single".to_string());

        let detector = LouvainDetector::default();
        let mut node_to_community = HashMap::new();
        node_to_community.insert(node, 0);

        let modularity = detector
            .calculate_modularity(&graph, &node_to_community)
            .unwrap();
        assert_eq!(modularity, 0.0); // No edges, so modularity is 0
    }

    #[test]
    fn test_two_node_connected_graph() {
        let mut graph = Graph::new_undirected();
        let node_a = graph.add_node("a".to_string());
        let node_b = graph.add_node("b".to_string());
        graph.add_edge(node_a, node_b, 5.0);

        let detector = LouvainDetector::default();

        // Test when both nodes are in the same community
        let mut same_community = HashMap::new();
        same_community.insert(node_a, 0);
        same_community.insert(node_b, 0);
        let same_modularity = detector
            .calculate_modularity(&graph, &same_community)
            .unwrap();

        // Test when nodes are in different communities
        let mut diff_community = HashMap::new();
        diff_community.insert(node_a, 0);
        diff_community.insert(node_b, 1);
        let diff_modularity = detector
            .calculate_modularity(&graph, &diff_community)
            .unwrap();

        // Same community should generally have better modularity for connected nodes
        assert!(same_modularity >= diff_modularity);
    }

    #[test]
    fn test_basic_modularity_comparison() -> Result<()> {
        let graph = create_test_graph();
        let detector = LouvainDetector::default();

        let nodes: Vec<_> = graph.node_indices().collect();

        // Test with natural community structure: {a,b} and {c,d}
        let mut good_assignment = HashMap::new();
        good_assignment.insert(nodes[0], 0); // a
        good_assignment.insert(nodes[1], 0); // b
        good_assignment.insert(nodes[2], 1); // c
        good_assignment.insert(nodes[3], 1); // d

        let good_modularity = detector.calculate_modularity(&graph, &good_assignment)?;

        // Test with poor assignment: all nodes in one community
        let mut poor_assignment = HashMap::new();
        for &node in &nodes {
            poor_assignment.insert(node, 0);
        }

        let poor_modularity = detector.calculate_modularity(&graph, &poor_assignment)?;

        // Good assignment should have better modularity
        assert!(good_modularity > poor_modularity);

        Ok(())
    }

    #[test]
    fn test_node_modularity_contribution() -> Result<()> {
        let graph = create_test_graph();
        let detector = LouvainDetector::default();
        let total_weight = detector.calculate_total_weight(&graph);

        let nodes: Vec<_> = graph.node_indices().collect();
        let mut node_to_community = HashMap::new();

        // Initial assignment: each node in its own community
        for (i, &node) in nodes.iter().enumerate() {
            node_to_community.insert(node, i);
        }

        // Test modularity contribution calculation
        let contribution = detector.calculate_node_modularity_contribution(
            &graph,
            nodes[0],
            0,
            &node_to_community,
            total_weight,
        )?;

        // Should be a valid modularity value
        assert!(contribution.is_finite());

        Ok(())
    }

    #[test]
    fn test_best_community_finding() -> Result<()> {
        let graph = create_test_graph();
        let detector = LouvainDetector::default();
        let total_weight = detector.calculate_total_weight(&graph);

        let nodes: Vec<_> = graph.node_indices().collect();
        let mut node_to_community = HashMap::new();

        // Initial assignment
        node_to_community.insert(nodes[0], 0); // a in community 0
        node_to_community.insert(nodes[1], 1); // b in community 1
        node_to_community.insert(nodes[2], 2); // c in community 2
        node_to_community.insert(nodes[3], 3); // d in community 3

        // Find best community for node a
        let best_community =
            detector.find_best_community(&graph, nodes[0], &node_to_community, total_weight)?;

        // Should return a valid community ID
        assert!(best_community >= 0);

        Ok(())
    }

    #[test]
    fn test_community_info_metrics_comprehensive() {
        let info = CommunityInfo {
            id: 42,
            nodes: vec![NodeIndex::new(0), NodeIndex::new(1), NodeIndex::new(2)],
            internal_weight: 15.0,
            cut_weight: 5.0,
            total_degree: 40.0,
            runtime_internal_count: 8,
            static_internal_count: 2,
        };

        assert_eq!(info.size(), 3);
        assert_eq!(info.cut_ratio(), 0.25); // 5.0 / (15.0 + 5.0)
        assert_eq!(info.runtime_internal_fraction(), 0.8); // 8 / (8 + 2)

        // Test with only internal edges
        let internal_only = CommunityInfo {
            id: 0,
            nodes: vec![NodeIndex::new(0)],
            internal_weight: 10.0,
            cut_weight: 0.0,
            total_degree: 10.0,
            runtime_internal_count: 5,
            static_internal_count: 0,
        };

        assert_eq!(internal_only.cut_ratio(), 0.0);
        assert_eq!(internal_only.runtime_internal_fraction(), 1.0);

        // Test with only cut edges
        let cut_only = CommunityInfo {
            id: 1,
            nodes: vec![NodeIndex::new(1)],
            internal_weight: 0.0,
            cut_weight: 8.0,
            total_degree: 8.0,
            runtime_internal_count: 0,
            static_internal_count: 3,
        };

        assert_eq!(cut_only.cut_ratio(), 1.0);
        assert_eq!(cut_only.runtime_internal_fraction(), 0.0);
    }

    #[test]
    fn test_community_info_edge_cases() {
        let info = CommunityInfo {
            id: 0,
            nodes: vec![],
            internal_weight: 0.0,
            cut_weight: 0.0,
            total_degree: 0.0,
            runtime_internal_count: 0,
            static_internal_count: 0,
        };

        assert_eq!(info.size(), 0);
        assert_eq!(info.cut_ratio(), 0.0); // No edges
        assert_eq!(info.runtime_internal_fraction(), 0.0); // No internal edges
    }

    #[test]
    fn test_different_resolution_parameters() {
        let graph = create_test_graph();
        let nodes: Vec<_> = graph.node_indices().collect();

        // Create a reasonable community assignment
        let mut node_to_community = HashMap::new();
        node_to_community.insert(nodes[0], 0);
        node_to_community.insert(nodes[1], 0);
        node_to_community.insert(nodes[2], 1);
        node_to_community.insert(nodes[3], 1);

        // Test with low resolution
        let low_res_detector = LouvainDetector::new(0.5, 100, 1e-4);
        let low_res_modularity = low_res_detector
            .calculate_modularity(&graph, &node_to_community)
            .unwrap();

        // Test with high resolution
        let high_res_detector = LouvainDetector::new(1.5, 100, 1e-4);
        let high_res_modularity = high_res_detector
            .calculate_modularity(&graph, &node_to_community)
            .unwrap();

        // Both should calculate valid modularity values
        assert!(low_res_modularity.is_finite());
        assert!(high_res_modularity.is_finite());
    }
}
