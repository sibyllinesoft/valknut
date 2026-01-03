//! Graph partitioning algorithms for directory reorganization.

use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::core::errors::Result;

use crate::detectors::structure::config::{
    DependencyGraph, DirectoryMetrics, DirectoryPartition, StructureConfig,
};

/// Graph partitioner for directory reorganization.
pub struct GraphPartitioner<'a> {
    config: &'a StructureConfig,
}

/// Partitioning and clustering methods for [`GraphPartitioner`].
impl<'a> GraphPartitioner<'a> {
    /// Creates a new graph partitioner with the given configuration.
    pub fn new(config: &'a StructureConfig) -> Self {
        Self { config }
    }

    /// Partition directory using graph algorithms
    pub fn partition_directory(
        &self,
        graph: &DependencyGraph,
        metrics: &DirectoryMetrics,
    ) -> Result<Vec<DirectoryPartition>> {
        if graph.node_count() == 0 {
            return Ok(Vec::new());
        }

        // Calculate optimal number of clusters
        let target_loc_per_subdir = self.config.fsdir.target_loc_per_subdir;
        let k = ((metrics.loc as f64 / target_loc_per_subdir as f64).round() as usize)
            .clamp(2, self.config.partitioning.max_clusters);

        let node_indices: Vec<_> = graph.node_indices().collect();

        // Use different algorithms based on graph size
        let communities = if node_indices.len() <= 8 {
            // Brute force optimal bipartition for small graphs
            self.brute_force_partition(&node_indices, graph, k)?
        } else {
            // Use label propagation followed by Kernighan-Lin refinement
            let initial_communities = self.label_propagation_partition(graph)?;
            self.refine_partition_with_kl(graph, initial_communities, k)?
        };

        // Convert communities to directory partitions
        self.communities_to_partitions(graph, communities, k)
    }

    /// Brute force optimal partitioning for small graphs
    fn brute_force_partition(
        &self,
        nodes: &[NodeIndex],
        graph: &DependencyGraph,
        k: usize,
    ) -> Result<Vec<Vec<NodeIndex>>> {
        if k == 2 && nodes.len() <= 8 {
            // Optimal bipartition using exhaustive search
            let best_partition = self.find_optimal_bipartition(nodes, graph)?;
            Ok(vec![best_partition.0, best_partition.1])
        } else {
            // TODO: replace this random fallback with multi-way partitioning (e.g. multi-level KL)
            // Fall back to simple random partitioning for larger k
            self.random_partition(nodes, k)
        }
    }

    /// Find optimal bipartition that minimizes cut and balances LOC
    fn find_optimal_bipartition(
        &self,
        nodes: &[NodeIndex],
        graph: &DependencyGraph,
    ) -> Result<(Vec<NodeIndex>, Vec<NodeIndex>)> {
        let n = nodes.len();
        let mut best_cut = usize::MAX;
        let mut best_balance = f64::MAX;
        let mut best_partition = (Vec::new(), Vec::new());

        // Try all possible bipartitions (2^n possibilities)
        for mask in 1..(1 << n) - 1 {
            let mut part1 = Vec::new();
            let mut part2 = Vec::new();
            let mut loc1 = 0;
            let mut loc2 = 0;

            for i in 0..n {
                if mask & (1 << i) != 0 {
                    part1.push(nodes[i]);
                    loc1 += graph.node_weight(nodes[i]).map(|n| n.loc).unwrap_or(0);
                } else {
                    part2.push(nodes[i]);
                    loc2 += graph.node_weight(nodes[i]).map(|n| n.loc).unwrap_or(0);
                }
            }

            // Calculate cut size and balance
            let cut_size = self.calculate_cut_size(graph, &part1, &part2);
            let total_loc = loc1 + loc2;
            let balance = if total_loc > 0 {
                (loc1 as f64 / total_loc as f64 - 0.5).abs()
            } else {
                0.0
            };

            // Check if within balance tolerance
            if balance <= self.config.partitioning.balance_tolerance {
                if cut_size < best_cut || (cut_size == best_cut && balance < best_balance) {
                    best_cut = cut_size;
                    best_balance = balance;
                    best_partition = (part1, part2);
                }
            }
        }

        if best_partition.0.is_empty() {
            // If no balanced partition found, use simple split
            let mid = n / 2;
            let part1 = nodes[..mid].to_vec();
            let part2 = nodes[mid..].to_vec();
            Ok((part1, part2))
        } else {
            Ok(best_partition)
        }
    }

    /// Calculate cut size between two partitions
    pub fn calculate_cut_size(
        &self,
        graph: &DependencyGraph,
        part1: &[NodeIndex],
        part2: &[NodeIndex],
    ) -> usize {
        let part2_set: HashSet<_> = part2.iter().copied().collect();

        let mut cut_size = 0;

        for &node in part1 {
            for edge in graph.edges(node) {
                if part2_set.contains(&edge.target()) {
                    cut_size += edge.weight().weight;
                }
            }
        }

        cut_size
    }

    /// Random partition as fallback
    fn random_partition(&self, nodes: &[NodeIndex], k: usize) -> Result<Vec<Vec<NodeIndex>>> {
        let mut communities = vec![Vec::new(); k];

        for (i, &node) in nodes.iter().enumerate() {
            communities[i % k].push(node);
        }

        Ok(communities)
    }

    /// Label propagation algorithm for community detection
    fn label_propagation_partition(&self, graph: &DependencyGraph) -> Result<Vec<Vec<NodeIndex>>> {
        let node_indices: Vec<_> = graph.node_indices().collect();
        let mut labels: HashMap<NodeIndex, usize> = HashMap::new();

        // Initialize each node with its own label
        for (i, &node) in node_indices.iter().enumerate() {
            labels.insert(node, i);
        }

        let max_iterations = 100;
        let mut changed = true;
        let mut iteration = 0;

        while changed && iteration < max_iterations {
            changed = false;

            // Randomize order to avoid bias
            let shuffled_nodes = node_indices.clone();
            // In a real implementation, would use proper randomization
            // shuffled_nodes.shuffle(&mut thread_rng());

            for &node in &shuffled_nodes {
                // Count labels of neighbors
                let mut neighbor_labels: HashMap<usize, f64> = HashMap::new();

                for edge in graph.edges(node) {
                    let neighbor = edge.target();
                    if let Some(&neighbor_label) = labels.get(&neighbor) {
                        *neighbor_labels.entry(neighbor_label).or_insert(0.0) +=
                            edge.weight().weight as f64;
                    }
                }

                // Find most frequent label
                if let Some((&new_label, _)) = neighbor_labels
                    .iter()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                {
                    if labels.get(&node) != Some(&new_label) {
                        labels.insert(node, new_label);
                        changed = true;
                    }
                }
            }

            iteration += 1;
        }

        // Group nodes by label
        let mut communities: HashMap<usize, Vec<NodeIndex>> = HashMap::new();
        for (&node, &label) in &labels {
            communities.entry(label).or_insert_with(Vec::new).push(node);
        }

        Ok(communities.into_values().collect())
    }

    /// Refine partition using Kernighan-Lin algorithm
    fn refine_partition_with_kl(
        &self,
        graph: &DependencyGraph,
        mut communities: Vec<Vec<NodeIndex>>,
        target_k: usize,
    ) -> Result<Vec<Vec<NodeIndex>>> {
        // Merge or split communities to reach target k
        while communities.len() > target_k {
            // Merge smallest communities
            communities.sort_by_key(|c| c.len());
            let smallest = communities.remove(0);
            let second_smallest = communities.remove(0);
            let mut merged = smallest;
            merged.extend(second_smallest);
            communities.push(merged);
        }

        while communities.len() < target_k {
            // Split largest community
            communities.sort_by_key(|c| c.len());
            let largest = match communities.pop() {
                Some(community) => community,
                None => break, // No more communities to split
            };
            if largest.len() >= self.config.partitioning.min_clusters {
                let mid = largest.len() / 2;
                let (first_half, second_half) = largest.split_at(mid);
                communities.push(first_half.to_vec());
                communities.push(second_half.to_vec());
            } else {
                communities.push(largest);
                break;
            }
        }

        // Apply Kernighan-Lin refinement
        self.kernighan_lin_refinement(graph, communities)
    }

    /// Kernighan-Lin refinement algorithm
    fn kernighan_lin_refinement(
        &self,
        graph: &DependencyGraph,
        mut communities: Vec<Vec<NodeIndex>>,
    ) -> Result<Vec<Vec<NodeIndex>>> {
        let max_iterations = 10;
        let mut improved = true;
        let mut iteration = 0;

        while improved && iteration < max_iterations {
            improved = self.try_improve_all_pairs(graph, &mut communities);
            iteration += 1;
        }

        Ok(communities)
    }

    /// Try to improve partition by swapping nodes between all community pairs.
    fn try_improve_all_pairs(
        &self,
        graph: &DependencyGraph,
        communities: &mut [Vec<NodeIndex>],
    ) -> bool {
        let mut improved = false;
        for i in 0..communities.len() {
            for j in i + 1..communities.len() {
                if self.try_swap_between_communities(graph, communities, i, j) {
                    improved = true;
                }
            }
        }
        improved
    }

    /// Try to find and apply a beneficial swap between two communities.
    fn try_swap_between_communities(
        &self,
        graph: &DependencyGraph,
        communities: &mut [Vec<NodeIndex>],
        i: usize,
        j: usize,
    ) -> bool {
        let Some((best_swap, cost_improvement)) =
            self.find_best_node_swap(graph, &communities[i], &communities[j])
        else {
            return false;
        };

        if cost_improvement <= 0.0 {
            return false;
        }

        let (from_comm, _to_comm, node) = best_swap;
        self.apply_node_swap(communities, i, j, from_comm, node);
        true
    }

    /// Move a node from one community to another.
    fn apply_node_swap(
        &self,
        communities: &mut [Vec<NodeIndex>],
        i: usize,
        j: usize,
        from_comm: usize,
        node: NodeIndex,
    ) {
        if from_comm == i {
            communities[i].retain(|&n| n != node);
            communities[j].push(node);
        } else {
            communities[j].retain(|&n| n != node);
            communities[i].push(node);
        }
    }

    /// Calculate overall cost/cut of partition
    fn calculate_partition_cost(
        &self,
        graph: &DependencyGraph,
        communities: &[Vec<NodeIndex>],
    ) -> f64 {
        let mut total_cut = 0.0;

        for i in 0..communities.len() {
            for j in i + 1..communities.len() {
                total_cut +=
                    self.calculate_cut_size(graph, &communities[i], &communities[j]) as f64;
            }
        }

        total_cut
    }

    /// Find best node swap between two communities
    fn find_best_node_swap(
        &self,
        graph: &DependencyGraph,
        comm1: &[NodeIndex],
        comm2: &[NodeIndex],
    ) -> Option<((usize, usize, NodeIndex), f64)> {
        let mut best_swap = None;
        let mut best_improvement = 0.0;

        // Try moving each node from comm1 to comm2
        for &node in comm1 {
            let improvement = self.calculate_swap_improvement(graph, node, comm1, comm2);
            if improvement > best_improvement {
                best_improvement = improvement;
                best_swap = Some((0, 1, node));
            }
        }

        // Try moving each node from comm2 to comm1
        for &node in comm2 {
            let improvement = self.calculate_swap_improvement(graph, node, comm2, comm1);
            if improvement > best_improvement {
                best_improvement = improvement;
                best_swap = Some((1, 0, node));
            }
        }

        best_swap.map(|swap| (swap, best_improvement))
    }

    /// Calculate improvement from swapping a node between communities
    fn calculate_swap_improvement(
        &self,
        graph: &DependencyGraph,
        node: NodeIndex,
        from_comm: &[NodeIndex],
        to_comm: &[NodeIndex],
    ) -> f64 {
        let from_set: HashSet<_> = from_comm.iter().copied().collect();
        let to_set: HashSet<_> = to_comm.iter().copied().collect();

        let mut internal_edges_lost = 0;
        let mut external_edges_gained = 0;

        for edge in graph.edges(node) {
            let neighbor = edge.target();
            let weight = edge.weight().weight;

            if from_set.contains(&neighbor) {
                // Losing internal edge in from_comm
                internal_edges_lost += weight;
            } else if to_set.contains(&neighbor) {
                // Gaining internal edge in to_comm
                external_edges_gained += weight;
            }
        }

        // Improvement = edges gained internally - edges lost internally
        (external_edges_gained as f64) - (internal_edges_lost as f64)
    }

    /// Convert graph communities to directory partitions
    fn communities_to_partitions(
        &self,
        graph: &DependencyGraph,
        communities: Vec<Vec<NodeIndex>>,
        k: usize,
    ) -> Result<Vec<DirectoryPartition>> {
        let mut partitions = Vec::new();

        for (i, community) in communities.into_iter().take(k).enumerate() {
            let mut files = Vec::new();
            let mut total_loc = 0;

            for node_idx in community {
                if let Some(file_node) = graph.node_weight(node_idx) {
                    // Ensure we store the complete absolute path
                    let complete_path = if file_node.path.is_absolute() {
                        file_node.path.clone()
                    } else {
                        std::env::current_dir()
                            .unwrap_or_default()
                            .join(&file_node.path)
                    };
                    files.push(complete_path);
                    total_loc += file_node.loc;
                }
            }

            // Generate deterministic name for partition
            let name = generate_partition_name(&files, i, &self.config.partitioning.naming_fallbacks);

            partitions.push(DirectoryPartition {
                name,
                files,
                loc: total_loc,
            });
        }

        Ok(partitions)
    }
}

/// Generate deterministic partition name based on file paths
pub fn generate_partition_name(files: &[PathBuf], index: usize, naming_fallbacks: &[String]) -> String {
    // Extract common tokens from file paths
    let mut token_counts: HashMap<String, usize> = HashMap::new();

    for file_path in files {
        if let Some(stem) = file_path.file_stem().and_then(|s| s.to_str()) {
            // Split on common separators and count tokens
            for token in stem.split(['_', '-', '.']) {
                let token = token.to_lowercase();
                if token.len() > 2 && !token.chars().all(|c| c.is_ascii_digit()) {
                    *token_counts.entry(token).or_insert(0) += 1;
                }
            }
        }
    }

    // Find most common meaningful token
    if let Some((best_token, _)) = token_counts
        .iter()
        .filter(|(token, &count)| {
            count > 1 && !["file", "test", "spec"].contains(&token.as_str())
        })
        .max_by_key(|(_, &count)| count)
    {
        return best_token.clone();
    }

    // Fall back to predefined names
    naming_fallbacks
        .get(index)
        .cloned()
        .unwrap_or_else(|| format!("partition_{}", index))
}
