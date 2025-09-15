//! Graph construction and analysis for live reachability
//!
//! Builds multigraphs from aggregated call edge data, computes node statistics,
//! and creates weighted projections for community detection

use crate::core::errors::{Result, ValknutError};
use crate::live::types::{AggregatedEdge, EdgeKind, NodeStats};

use chrono::{DateTime, Utc};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::{Directed, Graph, Undirected};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Multigraph with both runtime and static edges
pub struct CallGraph {
    /// Directed graph with symbol IDs as node weights
    graph: Graph<String, MultiEdge, Directed>,

    /// Map from symbol ID to node index
    symbol_to_node: HashMap<String, NodeIndex>,

    /// Node statistics for live reach calculation
    node_stats: HashMap<NodeIndex, NodeStats>,

    /// Set of entrypoint nodes (from static analysis)
    entrypoints: HashSet<NodeIndex>,

    /// Analysis window
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
}

/// Edge data supporting both runtime and static calls
#[derive(Debug, Clone, Default)]
pub struct MultiEdge {
    /// Runtime call statistics
    pub runtime_calls: u64,
    pub runtime_callers: u32,
    pub runtime_first_seen: Option<DateTime<Utc>>,
    pub runtime_last_seen: Option<DateTime<Utc>>,

    /// Static call statistics  
    pub static_calls: u64,
    pub static_first_seen: Option<DateTime<Utc>>,
    pub static_last_seen: Option<DateTime<Utc>>,
}

/// Weighted undirected graph for community detection
pub struct UndirectedCallGraph {
    /// Undirected graph with combined weights
    graph: Graph<String, f64, Undirected>,

    /// Map from symbol ID to node index
    symbol_to_node: HashMap<String, NodeIndex>,

    /// Map from undirected node index to directed node index
    undirected_to_directed: HashMap<NodeIndex, NodeIndex>,
}

/// Graph statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub runtime_edges: usize,
    pub static_edges: usize,
    pub mixed_edges: usize, // Edges with both runtime and static
    pub entrypoint_nodes: usize,
    pub isolated_nodes: usize,
}

impl CallGraph {
    /// Create a new empty call graph
    pub fn new(window_start: DateTime<Utc>, window_end: DateTime<Utc>) -> Self {
        Self {
            graph: Graph::new(),
            symbol_to_node: HashMap::new(),
            node_stats: HashMap::new(),
            entrypoints: HashSet::new(),
            window_start,
            window_end,
        }
    }

    /// Build graph from aggregated edges
    pub fn from_aggregated_edges(
        edges: &[AggregatedEdge],
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
        static_weight: f64,
    ) -> Result<Self> {
        let mut graph = Self::new(window_start, window_end);

        // Add all edges to the graph
        for edge in edges {
            graph.add_aggregated_edge(edge)?;
        }

        // Compute node statistics
        graph.compute_node_stats(static_weight)?;

        tracing::info!(
            "Built call graph with {} nodes and {} edges",
            graph.graph.node_count(),
            graph.graph.edge_count()
        );

        Ok(graph)
    }

    /// Add an aggregated edge to the graph
    fn add_aggregated_edge(&mut self, edge: &AggregatedEdge) -> Result<()> {
        // Get or create nodes
        let caller_node = self.get_or_create_node(&edge.caller);
        let callee_node = self.get_or_create_node(&edge.callee);

        // Find existing edge or create new one
        let edge_index = if let Some(edge_idx) = self.graph.find_edge(caller_node, callee_node) {
            edge_idx
        } else {
            self.graph
                .add_edge(caller_node, callee_node, MultiEdge::default())
        };

        // Update edge data
        let edge_weight = self
            .graph
            .edge_weight_mut(edge_index)
            .ok_or_else(|| ValknutError::graph("Failed to get edge weight"))?;

        match edge.kind {
            EdgeKind::Runtime => {
                edge_weight.runtime_calls += edge.calls;
                edge_weight.runtime_callers += edge.callers;
                edge_weight.runtime_first_seen = Some(
                    edge_weight
                        .runtime_first_seen
                        .unwrap_or(edge.first_timestamp())
                        .min(edge.first_timestamp()),
                );
                edge_weight.runtime_last_seen = Some(
                    edge_weight
                        .runtime_last_seen
                        .unwrap_or(edge.last_timestamp())
                        .max(edge.last_timestamp()),
                );
            }
            EdgeKind::Static => {
                edge_weight.static_calls += edge.calls;
                edge_weight.static_first_seen = Some(
                    edge_weight
                        .static_first_seen
                        .unwrap_or(edge.first_timestamp())
                        .min(edge.first_timestamp()),
                );
                edge_weight.static_last_seen = Some(
                    edge_weight
                        .static_last_seen
                        .unwrap_or(edge.last_timestamp())
                        .max(edge.last_timestamp()),
                );
            }
        }

        Ok(())
    }

    /// Get or create a node for a symbol
    fn get_or_create_node(&mut self, symbol: &str) -> NodeIndex {
        if let Some(&node_idx) = self.symbol_to_node.get(symbol) {
            node_idx
        } else {
            let node_idx = self.graph.add_node(symbol.to_string());
            self.symbol_to_node.insert(symbol.to_string(), node_idx);
            node_idx
        }
    }

    /// Compute node statistics for live reach scoring
    fn compute_node_stats(&mut self, static_weight: f64) -> Result<()> {
        // Initialize stats for all nodes
        for node_idx in self.graph.node_indices() {
            self.node_stats.insert(node_idx, NodeStats::default());
        }

        // Compute incoming edge statistics
        for edge_idx in self.graph.edge_indices() {
            if let Some((_source, target)) = self.graph.edge_endpoints(edge_idx) {
                if let Some(edge_weight) = self.graph.edge_weight(edge_idx) {
                    let target_stats = self.node_stats.get_mut(&target).unwrap();

                    // Count runtime callers and calls
                    if edge_weight.runtime_calls > 0 {
                        target_stats.live_callers += edge_weight.runtime_callers;
                        target_stats.live_calls += edge_weight.runtime_calls;

                        // Update first/last seen for target
                        if let Some(first_seen) = edge_weight.runtime_first_seen {
                            target_stats.first_seen = Some(
                                target_stats
                                    .first_seen
                                    .unwrap_or(first_seen)
                                    .min(first_seen),
                            );
                        }
                        if let Some(last_seen) = edge_weight.runtime_last_seen {
                            target_stats.last_seen =
                                Some(target_stats.last_seen.unwrap_or(last_seen).max(last_seen));
                        }
                    }
                }
            }
        }

        // Compute seed reachability (breadth-first search from entrypoints)
        self.compute_seed_reachability(static_weight)?;

        Ok(())
    }

    /// Compute which nodes are reachable from entrypoints
    fn compute_seed_reachability(&mut self, static_weight: f64) -> Result<()> {
        use std::collections::VecDeque;

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Start from all entrypoints
        for &entrypoint in &self.entrypoints {
            if !visited.contains(&entrypoint) {
                queue.push_back(entrypoint);
                visited.insert(entrypoint);
            }
        }

        // If no entrypoints defined, use nodes with high incoming runtime calls
        if queue.is_empty() {
            let mut node_scores: Vec<_> = self
                .node_stats
                .iter()
                .map(|(node_idx, stats)| (*node_idx, stats.live_calls as f64))
                .collect();

            node_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // Use top 10% of nodes by live calls as pseudo-entrypoints
            let num_entrypoints = (node_scores.len() / 10).max(1).min(50);
            for (node_idx, _) in node_scores.into_iter().take(num_entrypoints) {
                if !visited.contains(&node_idx) {
                    queue.push_back(node_idx);
                    visited.insert(node_idx);
                }
            }
        }

        // BFS traversal using both static and runtime edges
        while let Some(node_idx) = queue.pop_front() {
            // Mark as seed reachable
            if let Some(stats) = self.node_stats.get_mut(&node_idx) {
                stats.seed_reachable = true;
            }

            // Explore outgoing edges
            let mut edges = self.graph.edges(node_idx);
            while let Some(edge) = edges.next() {
                let edge_weight = edge.weight();
                let target = edge.target();

                // Include edge if it has sufficient weight (runtime + static)
                let total_weight = edge_weight.runtime_calls as f64
                    + edge_weight.static_calls as f64 * static_weight;

                if total_weight > 0.0 && !visited.contains(&target) {
                    visited.insert(target);
                    queue.push_back(target);
                }
            }
        }

        Ok(())
    }

    /// Add entrypoint nodes (from static analysis)
    pub fn add_entrypoint(&mut self, symbol: &str) {
        let node_idx = self.get_or_create_node(symbol);
        self.entrypoints.insert(node_idx);
    }

    /// Create undirected weighted projection for community detection
    pub fn create_undirected_projection(&self, static_weight: f64) -> UndirectedCallGraph {
        let mut undirected_graph = Graph::new_undirected();
        let mut symbol_to_node = HashMap::new();
        let mut undirected_to_directed = HashMap::new();

        // Add all nodes
        for (symbol, &directed_node) in &self.symbol_to_node {
            let undirected_node = undirected_graph.add_node(symbol.clone());
            symbol_to_node.insert(symbol.clone(), undirected_node);
            undirected_to_directed.insert(undirected_node, directed_node);
        }

        // Add edges with combined weights
        let mut edge_weights: HashMap<(NodeIndex, NodeIndex), f64> = HashMap::new();

        for edge_idx in self.graph.edge_indices() {
            if let Some((source, target)) = self.graph.edge_endpoints(edge_idx) {
                if let Some(edge_weight) = self.graph.edge_weight(edge_idx) {
                    let source_symbol = &self.graph[source];
                    let target_symbol = &self.graph[target];

                    let undirected_source = symbol_to_node[source_symbol];
                    let undirected_target = symbol_to_node[target_symbol];

                    // Create undirected edge key (smaller index first)
                    let edge_key = if undirected_source < undirected_target {
                        (undirected_source, undirected_target)
                    } else {
                        (undirected_target, undirected_source)
                    };

                    // Combine runtime and static weights
                    let weight = edge_weight.runtime_calls as f64
                        + edge_weight.static_calls as f64 * static_weight;

                    *edge_weights.entry(edge_key).or_insert(0.0) += weight;
                }
            }
        }

        // Add weighted edges to undirected graph
        for ((source, target), weight) in edge_weights {
            if weight > 0.0 {
                undirected_graph.add_edge(source, target, weight);
            }
        }

        UndirectedCallGraph {
            graph: undirected_graph,
            symbol_to_node,
            undirected_to_directed,
        }
    }

    /// Get graph statistics
    pub fn get_stats(&self) -> GraphStats {
        let mut runtime_edges = 0;
        let mut static_edges = 0;
        let mut mixed_edges = 0;

        for edge_idx in self.graph.edge_indices() {
            if let Some(edge_weight) = self.graph.edge_weight(edge_idx) {
                let has_runtime = edge_weight.runtime_calls > 0;
                let has_static = edge_weight.static_calls > 0;

                match (has_runtime, has_static) {
                    (true, true) => mixed_edges += 1,
                    (true, false) => runtime_edges += 1,
                    (false, true) => static_edges += 1,
                    (false, false) => {} // Shouldn't happen
                }
            }
        }

        let isolated_nodes = self
            .graph
            .node_indices()
            .filter(|&node_idx| {
                self.graph.edges(node_idx).count() == 0
                    && self
                        .graph
                        .edges_directed(node_idx, petgraph::Direction::Incoming)
                        .count()
                        == 0
            })
            .count();

        GraphStats {
            total_nodes: self.graph.node_count(),
            total_edges: self.graph.edge_count(),
            runtime_edges,
            static_edges,
            mixed_edges,
            entrypoint_nodes: self.entrypoints.len(),
            isolated_nodes,
        }
    }

    /// Get node statistics
    pub fn get_node_stats(&self, symbol: &str) -> Option<&NodeStats> {
        self.symbol_to_node
            .get(symbol)
            .and_then(|&node_idx| self.node_stats.get(&node_idx))
    }

    /// Get all nodes and their statistics
    pub fn iter_nodes(&self) -> impl Iterator<Item = (&str, &NodeStats)> {
        self.graph
            .node_weights()
            .zip(self.graph.node_indices())
            .filter_map(|(symbol, node_idx)| {
                self.node_stats
                    .get(&node_idx)
                    .map(|stats| (symbol.as_str(), stats))
            })
    }

    /// Get symbol for node index
    pub fn get_symbol(&self, node_idx: NodeIndex) -> Option<&str> {
        self.graph.node_weight(node_idx).map(|s| s.as_str())
    }

    /// Get node index for symbol
    pub fn get_node_index(&self, symbol: &str) -> Option<NodeIndex> {
        self.symbol_to_node.get(symbol).copied()
    }
}

impl UndirectedCallGraph {
    /// Get the underlying petgraph
    pub fn graph(&self) -> &Graph<String, f64, Undirected> {
        &self.graph
    }

    /// Get symbol for node index
    pub fn get_symbol(&self, node_idx: NodeIndex) -> Option<&str> {
        self.graph.node_weight(node_idx).map(|s| s.as_str())
    }

    /// Get node index for symbol
    pub fn get_node_index(&self, symbol: &str) -> Option<NodeIndex> {
        self.symbol_to_node.get(symbol).copied()
    }

    /// Map undirected node index to directed node index
    pub fn to_directed_index(&self, undirected_idx: NodeIndex) -> Option<NodeIndex> {
        self.undirected_to_directed.get(&undirected_idx).copied()
    }

    /// Get edge weight between two symbols
    pub fn get_edge_weight(&self, from: &str, to: &str) -> Option<f64> {
        let from_idx = self.get_node_index(from)?;
        let to_idx = self.get_node_index(to)?;

        self.graph
            .find_edge(from_idx, to_idx)
            .and_then(|edge_idx| self.graph.edge_weight(edge_idx).copied())
    }

    /// Get all neighbors of a node with their edge weights
    pub fn get_neighbors(&self, symbol: &str) -> Vec<(String, f64)> {
        if let Some(node_idx) = self.get_node_index(symbol) {
            self.graph
                .edges(node_idx)
                .map(|edge| {
                    let neighbor_symbol = self.graph[edge.target()].clone();
                    let weight = *edge.weight();
                    (neighbor_symbol, weight)
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl MultiEdge {
    /// Get total weight (runtime + static with scaling factor)
    pub fn total_weight(&self, static_weight: f64) -> f64 {
        self.runtime_calls as f64 + self.static_calls as f64 * static_weight
    }

    /// Check if edge has runtime activity
    pub fn has_runtime(&self) -> bool {
        self.runtime_calls > 0
    }

    /// Check if edge has static analysis data
    pub fn has_static(&self) -> bool {
        self.static_calls > 0
    }

    /// Get the most recent timestamp
    pub fn last_seen(&self) -> Option<DateTime<Utc>> {
        match (self.runtime_last_seen, self.static_last_seen) {
            (Some(r), Some(s)) => Some(r.max(s)),
            (Some(r), None) => Some(r),
            (None, Some(s)) => Some(s),
            (None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_edge(caller: &str, callee: &str, kind: EdgeKind, calls: u64) -> AggregatedEdge {
        AggregatedEdge {
            caller: caller.to_string(),
            callee: callee.to_string(),
            kind,
            calls,
            callers: 1,
            first_ts: 1699999000,
            last_ts: 1699999999,
        }
    }

    fn create_test_edge_with_timestamps(
        caller: &str,
        callee: &str,
        kind: EdgeKind,
        calls: u64,
        first_ts: u64,
        last_ts: u64,
    ) -> AggregatedEdge {
        AggregatedEdge {
            caller: caller.to_string(),
            callee: callee.to_string(),
            kind,
            calls,
            callers: 1,
            first_ts: first_ts as i64,
            last_ts: last_ts as i64,
        }
    }

    #[test]
    fn test_empty_graph() {
        use chrono::Duration;
        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let graph = CallGraph::new(start, end);

        let stats = graph.get_stats();
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.total_edges, 0);
        assert_eq!(stats.runtime_edges, 0);
        assert_eq!(stats.static_edges, 0);
        assert_eq!(stats.mixed_edges, 0);
        assert_eq!(stats.entrypoint_nodes, 0);
        assert_eq!(stats.isolated_nodes, 0);

        // Test empty graph methods
        assert!(graph.get_node_stats("nonexistent").is_none());
        assert!(graph.get_node_index("nonexistent").is_none());
        assert!(graph.get_symbol(NodeIndex::new(0)).is_none());

        let node_iter: Vec<_> = graph.iter_nodes().collect();
        assert!(node_iter.is_empty());
    }

    #[test]
    fn test_simple_graph() -> Result<()> {
        use chrono::Duration;
        let edges = vec![
            create_test_edge("a", "b", EdgeKind::Runtime, 10),
            create_test_edge("b", "c", EdgeKind::Static, 5),
        ];

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let graph = CallGraph::from_aggregated_edges(&edges, start, end, 0.1)?;

        let stats = graph.get_stats();
        assert_eq!(stats.total_nodes, 3);
        assert_eq!(stats.total_edges, 2);
        assert_eq!(stats.runtime_edges, 1);
        assert_eq!(stats.static_edges, 1);
        assert_eq!(stats.mixed_edges, 0);

        // Test node access methods
        assert!(graph.get_node_index("a").is_some());
        assert!(graph.get_node_index("b").is_some());
        assert!(graph.get_node_index("c").is_some());
        assert!(graph.get_node_index("nonexistent").is_none());

        let node_a_idx = graph.get_node_index("a").unwrap();
        assert_eq!(graph.get_symbol(node_a_idx), Some("a"));

        // Test iteration
        let node_iter: Vec<_> = graph.iter_nodes().collect();
        assert_eq!(node_iter.len(), 3);

        Ok(())
    }

    #[test]
    fn test_node_stats_comprehensive() -> Result<()> {
        use chrono::Duration;
        let edges = vec![
            create_test_edge("a", "b", EdgeKind::Runtime, 10),
            create_test_edge("c", "b", EdgeKind::Runtime, 5),
            create_test_edge("b", "d", EdgeKind::Static, 3),
        ];

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let graph = CallGraph::from_aggregated_edges(&edges, start, end, 0.1)?;

        // Node "b" should have live calls from both "a" and "c"
        let b_stats = graph.get_node_stats("b").unwrap();
        assert_eq!(b_stats.live_calls, 15);
        assert_eq!(b_stats.live_callers, 2);
        assert!(b_stats.last_seen.is_some());

        // Node "a" should be a caller only
        let a_stats = graph.get_node_stats("a").unwrap();
        assert_eq!(a_stats.live_calls, 0);
        assert_eq!(a_stats.live_callers, 0);

        // Node "d" should be called only
        let d_stats = graph.get_node_stats("d").unwrap();
        assert_eq!(d_stats.live_calls, 0);
        assert_eq!(d_stats.live_callers, 0); // Only static calls, not runtime

        Ok(())
    }

    #[test]
    fn test_undirected_projection_comprehensive() -> Result<()> {
        use chrono::Duration;
        let edges = vec![
            create_test_edge("a", "b", EdgeKind::Runtime, 10),
            create_test_edge("b", "a", EdgeKind::Static, 5),
            create_test_edge("b", "c", EdgeKind::Runtime, 3),
            create_test_edge("c", "d", EdgeKind::Static, 2),
        ];

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let graph = CallGraph::from_aggregated_edges(&edges, start, end, 0.2)?;

        let undirected = graph.create_undirected_projection(0.2);

        // Should have 4 nodes
        assert_eq!(undirected.graph.node_count(), 4);

        // Edge a-b should have combined weight: 10 (runtime) + 5*0.2 (static) = 11.0
        let weight_ab = undirected.get_edge_weight("a", "b");
        assert_eq!(weight_ab, Some(11.0));

        // Edge b-c should have weight: 3 (runtime only)
        let weight_bc = undirected.get_edge_weight("b", "c");
        assert_eq!(weight_bc, Some(3.0));

        // Edge c-d should have weight: 2*0.2 (static only) = 0.4
        let weight_cd = undirected.get_edge_weight("c", "d");
        assert_eq!(weight_cd, Some(0.4));

        // Non-existent edge should return None
        let weight_ad = undirected.get_edge_weight("a", "d");
        assert_eq!(weight_ad, None);

        // Test node access
        assert!(undirected.get_node_index("a").is_some());
        assert_eq!(
            undirected.get_symbol(undirected.get_node_index("a").unwrap()),
            Some("a")
        );

        // Test neighbors
        let neighbors_b = undirected.get_neighbors("b");
        assert_eq!(neighbors_b.len(), 2); // Connected to a and c

        let neighbors_nonexistent = undirected.get_neighbors("nonexistent");
        assert!(neighbors_nonexistent.is_empty());

        // Test directed index mapping
        let undirected_a = undirected.get_node_index("a").unwrap();
        let directed_a = undirected.to_directed_index(undirected_a);
        assert!(directed_a.is_some());

        Ok(())
    }

    #[test]
    fn test_entrypoints_comprehensive() -> Result<()> {
        use chrono::Duration;
        let edges = vec![
            create_test_edge("main", "func1", EdgeKind::Static, 1),
            create_test_edge("func1", "func2", EdgeKind::Runtime, 5),
            create_test_edge("func2", "func3", EdgeKind::Runtime, 3),
            create_test_edge("isolated", "isolated2", EdgeKind::Static, 1),
        ];

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let mut graph = CallGraph::from_aggregated_edges(&edges, start, end, 0.1)?;

        // Add multiple entrypoints
        graph.add_entrypoint("main");
        graph.add_entrypoint("isolated");

        let stats = graph.get_stats();
        assert_eq!(stats.entrypoint_nodes, 2);

        // Before recomputing, seed_reachable should be false
        assert!(!graph.get_node_stats("main").unwrap().seed_reachable);

        // After recomputing seed reachability
        graph.compute_seed_reachability(0.1)?;

        // All nodes reachable from entrypoints should be marked
        assert!(graph.get_node_stats("main").unwrap().seed_reachable);
        assert!(graph.get_node_stats("func1").unwrap().seed_reachable);
        assert!(graph.get_node_stats("func2").unwrap().seed_reachable);
        assert!(graph.get_node_stats("func3").unwrap().seed_reachable);
        assert!(graph.get_node_stats("isolated").unwrap().seed_reachable);
        assert!(graph.get_node_stats("isolated2").unwrap().seed_reachable);

        Ok(())
    }

    #[test]
    fn test_entrypoints_auto_detection() -> Result<()> {
        use chrono::Duration;
        // Create graph with no explicit entrypoints - should use high-call nodes
        let edges = vec![
            create_test_edge("a", "popular", EdgeKind::Runtime, 100), // High call count
            create_test_edge("b", "popular", EdgeKind::Runtime, 50),
            create_test_edge("c", "rare", EdgeKind::Runtime, 1),
            create_test_edge("popular", "target", EdgeKind::Runtime, 10),
        ];

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let mut graph = CallGraph::from_aggregated_edges(&edges, start, end, 0.1)?;

        // No explicit entrypoints
        assert_eq!(graph.entrypoints.len(), 0);

        // Compute seed reachability - should auto-detect popular nodes
        graph.compute_seed_reachability(0.1)?;

        // "popular" should be seed reachable (high live calls)
        let popular_stats = graph.get_node_stats("popular").unwrap();
        assert!(popular_stats.seed_reachable);
        assert_eq!(popular_stats.live_calls, 150); // 100 + 50

        Ok(())
    }

    #[test]
    fn test_multi_edge_comprehensive() {
        let mut edge = MultiEdge::default();

        // Test default values
        assert_eq!(edge.total_weight(0.1), 0.0);
        assert!(!edge.has_runtime());
        assert!(!edge.has_static());
        assert!(edge.last_seen().is_none());

        // Add runtime data
        edge.runtime_calls = 10;
        edge.runtime_last_seen = Some(Utc::now());

        assert_eq!(edge.total_weight(0.1), 10.0);
        assert!(edge.has_runtime());
        assert!(!edge.has_static());
        assert!(edge.last_seen().is_some());

        // Add static data
        edge.static_calls = 5;
        edge.static_last_seen = Some(Utc::now() - chrono::Duration::hours(1));

        assert_eq!(edge.total_weight(0.2), 11.0); // 10 + 5*0.2
        assert!(edge.has_runtime());
        assert!(edge.has_static());

        // last_seen should be the more recent timestamp
        let last_seen = edge.last_seen().unwrap();
        assert!(last_seen >= edge.static_last_seen.unwrap());

        // Test with different static weights
        assert_eq!(edge.total_weight(0.0), 10.0); // No static weight
        assert_eq!(edge.total_weight(1.0), 15.0); // Full static weight
    }

    #[test]
    fn test_mixed_edges_comprehensive() -> Result<()> {
        use chrono::Duration;
        // Create multiple edges with same caller/callee but different kinds
        let edges = vec![
            create_test_edge("a", "b", EdgeKind::Runtime, 10),
            create_test_edge("a", "b", EdgeKind::Static, 5), // Same edge, different kind
            create_test_edge("c", "d", EdgeKind::Runtime, 20), // Pure runtime
            create_test_edge("e", "f", EdgeKind::Static, 15), // Pure static
        ];

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let graph = CallGraph::from_aggregated_edges(&edges, start, end, 0.1)?;

        let stats = graph.get_stats();
        assert_eq!(stats.total_nodes, 6);
        assert_eq!(stats.total_edges, 3); // a->b merged, c->d, e->f separate
        assert_eq!(stats.mixed_edges, 1); // a->b
        assert_eq!(stats.runtime_edges, 1); // c->d
        assert_eq!(stats.static_edges, 1); // e->f

        Ok(())
    }

    #[test]
    fn test_isolated_nodes_detection() -> Result<()> {
        use chrono::Duration;
        let edges = vec![
            create_test_edge("a", "b", EdgeKind::Runtime, 10),
            // c and d will be isolated - no edges
        ];

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let mut graph = CallGraph::from_aggregated_edges(&edges, start, end, 0.1)?;

        // Manually add isolated nodes (in practice, these might come from static analysis)
        graph.get_or_create_node("isolated1");
        graph.get_or_create_node("isolated2");

        let stats = graph.get_stats();
        assert_eq!(stats.total_nodes, 4); // a, b, isolated1, isolated2
        assert_eq!(stats.total_edges, 1);
        assert_eq!(stats.isolated_nodes, 2); // isolated1, isolated2

        Ok(())
    }

    #[test]
    fn test_timestamp_handling() -> Result<()> {
        use chrono::Duration;
        let now_ts = 1700000000u64;
        let edges = vec![
            create_test_edge_with_timestamps(
                "a",
                "b",
                EdgeKind::Runtime,
                10,
                now_ts - 3600,
                now_ts,
            ),
            create_test_edge_with_timestamps(
                "a",
                "c",
                EdgeKind::Runtime,
                5,
                now_ts - 7200,
                now_ts - 1800,
            ), // Changed to Runtime
        ];

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let graph = CallGraph::from_aggregated_edges(&edges, start, end, 0.1)?;

        // Check that timestamps are preserved (only runtime edges set last_seen on nodes)
        let b_stats = graph.get_node_stats("b").unwrap();
        assert!(b_stats.last_seen.is_some());

        let c_stats = graph.get_node_stats("c").unwrap();
        assert!(c_stats.last_seen.is_some());

        Ok(())
    }

    #[test]
    fn test_large_graph_performance() -> Result<()> {
        use chrono::Duration;

        // Create a larger graph to test performance characteristics
        let mut edges = Vec::new();
        for i in 0..100 {
            for j in 0..10 {
                if i != j {
                    edges.push(create_test_edge(
                        &format!("node_{}", i),
                        &format!("node_{}", j),
                        EdgeKind::Runtime,
                        (i + j) as u64,
                    ));
                }
            }
        }

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let graph = CallGraph::from_aggregated_edges(&edges, start, end, 0.1)?;

        let stats = graph.get_stats();
        assert_eq!(stats.total_nodes, 100);
        assert!(stats.total_edges > 0);

        // Test undirected projection creation performance
        let undirected = graph.create_undirected_projection(0.1);
        assert_eq!(undirected.graph.node_count(), 100);

        Ok(())
    }

    #[test]
    fn test_edge_weight_calculations() -> Result<()> {
        use chrono::Duration;
        let edges = vec![
            // Test various combinations of runtime/static calls
            create_test_edge("a", "b", EdgeKind::Runtime, 100),
            create_test_edge("a", "b", EdgeKind::Static, 50), // Should merge with above
            create_test_edge("c", "d", EdgeKind::Static, 25),
        ];

        let start = Utc::now() - Duration::days(1);
        let end = Utc::now();
        let graph = CallGraph::from_aggregated_edges(&edges, start, end, 0.1)?;

        let undirected = graph.create_undirected_projection(0.2);

        // Edge a-b: 100 runtime + 50*0.2 static = 110.0
        assert_eq!(undirected.get_edge_weight("a", "b"), Some(110.0));

        // Edge c-d: 25*0.2 static only = 5.0
        assert_eq!(undirected.get_edge_weight("c", "d"), Some(5.0));

        // Test with different static weights
        let undirected_no_static = graph.create_undirected_projection(0.0);
        assert_eq!(undirected_no_static.get_edge_weight("a", "b"), Some(100.0)); // Runtime only
        assert_eq!(undirected_no_static.get_edge_weight("c", "d"), None); // Static ignored, so no edge

        Ok(())
    }
}
