//! Directory analysis, graph partitioning, and reorganization logic

use dashmap::DashMap;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Directories to skip during directory analysis
const SKIP_DIRECTORIES: &[&str] = &[
    "node_modules", "target", ".git", "__pycache__", "dist", "build", ".next", "vendor", "venv",
];

/// Code file extensions recognized for analysis
const CODE_EXTENSIONS: &[&str] = &[
    "py", "js", "ts", "jsx", "tsx", "rs", "go", "java", "cpp", "c", "h", "hpp",
];

use crate::core::errors::{Result, ValknutError};
use crate::core::file_utils::FileReader;
use crate::lang::registry::adapter_for_file;
use tracing::warn;

use super::PrecomputedFileMetrics;

use super::config::{
    BranchReorgPack, DependencyEdge, DependencyGraph, DirectoryMetrics, DirectoryPartition,
    FileMove, FileNode, ImportStatement, ReorganizationEffort, ReorganizationGain, StructureConfig,
};

pub struct DirectoryAnalyzer {
    config: StructureConfig,
    metrics_cache: DashMap<PathBuf, DirectoryMetrics>,
}

impl DirectoryAnalyzer {
    pub fn new(config: StructureConfig) -> Self {
        Self {
            config,
            metrics_cache: DashMap::new(),
        }
    }

    /// Calculate directory metrics
    pub fn calculate_directory_metrics(&self, dir_path: &Path) -> Result<DirectoryMetrics> {
        // Check cache first
        if let Some(cached) = self.metrics_cache.get(dir_path) {
            return Ok(cached.clone());
        }

        let (files, subdirs, loc_distribution) = self.gather_directory_stats(dir_path)?;
        let total_loc = loc_distribution.iter().sum::<usize>();

        // Calculate dispersion metrics
        let gini = self.calculate_gini_coefficient(&loc_distribution);
        let entropy = self.calculate_entropy(&loc_distribution);

        // Calculate pressure metrics (clipped to [0,1])
        let file_pressure = (files as f64 / self.config.fsdir.max_files_per_dir as f64).min(1.0);
        let branch_pressure =
            (subdirs as f64 / self.config.fsdir.max_subdirs_per_dir as f64).min(1.0);
        let size_pressure = (total_loc as f64 / self.config.fsdir.max_dir_loc as f64).min(1.0);

        // Calculate distribution-based optimality scores
        // These score how close the directory is to "optimal" size based on a normal distribution
        let file_count_score = self.calculate_distribution_score(
            files,
            self.config.fsdir.optimal_files,
            self.config.fsdir.optimal_files_stddev,
        );
        let subdir_count_score = self.calculate_distribution_score(
            subdirs,
            self.config.fsdir.optimal_subdirs,
            self.config.fsdir.optimal_subdirs_stddev,
        );

        // Calculate dispersion combining gini and entropy
        let max_entropy = if files > 0 {
            (files as f64).log2()
        } else {
            1.0
        };
        let normalized_entropy = if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        };
        let dispersion = gini.max(1.0 - normalized_entropy);

        // Apply size normalization to prevent bias against larger codebases
        let size_normalization_factor = self.calculate_size_normalization_factor(files, total_loc);

        // Calculate overall imbalance score with normalization
        // Incorporate distribution scores: lower scores (further from optimal) increase imbalance
        // We use (1.0 - score) to convert optimality score to a "deviation" metric
        let file_deviation = 1.0 - file_count_score;
        let subdir_deviation = 1.0 - subdir_count_score;

        let raw_imbalance = 0.25 * file_pressure
            + 0.15 * branch_pressure
            + 0.20 * size_pressure
            + 0.10 * dispersion
            + 0.20 * file_deviation
            + 0.10 * subdir_deviation;

        let imbalance = raw_imbalance * size_normalization_factor;

        let metrics = DirectoryMetrics {
            files,
            subdirs,
            loc: total_loc,
            gini,
            entropy,
            file_pressure,
            branch_pressure,
            size_pressure,
            dispersion,
            file_count_score,
            subdir_count_score,
            imbalance,
        };

        // Cache the result
        self.metrics_cache
            .insert(dir_path.to_path_buf(), metrics.clone());

        Ok(metrics)
    }

    /// Gather basic directory statistics
    fn gather_directory_stats(&self, dir_path: &Path) -> Result<(usize, usize, Vec<usize>)> {
        let mut files = 0;
        let mut subdirs = 0;
        let mut loc_distribution = Vec::new();

        for entry in std::fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                subdirs += 1;
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if self.is_code_file(ext) {
                        files += 1;
                        let loc = self.count_lines_of_code(&path)?;
                        loc_distribution.push(loc);
                    }
                }
            }
        }

        Ok((files, subdirs, loc_distribution))
    }

    /// Check if file extension indicates a code file
    fn is_code_file(&self, extension: &str) -> bool {
        CODE_EXTENSIONS.contains(&extension)
    }

    /// Count lines of code in a file
    fn count_lines_of_code(&self, file_path: &Path) -> Result<usize> {
        let content = FileReader::read_to_string(file_path)?;
        Ok(content
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("//"))
            .count())
    }

    /// Gather directory stats using pre-computed metrics (avoids file I/O)
    fn gather_directory_stats_with_metrics(
        &self,
        dir_path: &Path,
        metrics_map: &HashMap<PathBuf, &PrecomputedFileMetrics>,
    ) -> Result<(usize, usize, Vec<usize>)> {
        let mut files = 0;
        let mut subdirs = 0;
        let mut loc_distribution = Vec::new();

        for entry in std::fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                subdirs += 1;
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if self.is_code_file(ext) {
                        files += 1;
                        // Use pre-computed LOC if available, otherwise fall back to reading
                        let loc = if let Some(metrics) = metrics_map.get(&path) {
                            metrics.loc
                        } else {
                            self.count_lines_of_code(&path)?
                        };
                        loc_distribution.push(loc);
                    }
                }
            }
        }

        Ok((files, subdirs, loc_distribution))
    }

    /// Calculate directory metrics using pre-computed file data
    pub fn calculate_directory_metrics_with_metrics(
        &self,
        dir_path: &Path,
        metrics_map: &HashMap<PathBuf, &PrecomputedFileMetrics>,
    ) -> Result<DirectoryMetrics> {
        // Check cache first
        if let Some(cached) = self.metrics_cache.get(dir_path) {
            return Ok(cached.clone());
        }

        let (files, subdirs, loc_distribution) =
            self.gather_directory_stats_with_metrics(dir_path, metrics_map)?;
        let total_loc = loc_distribution.iter().sum::<usize>();

        // Calculate dispersion metrics
        let gini = self.calculate_gini_coefficient(&loc_distribution);
        let entropy = self.calculate_entropy(&loc_distribution);

        // Calculate pressure metrics (clipped to [0,1])
        let file_pressure = (files as f64 / self.config.fsdir.max_files_per_dir as f64).min(1.0);
        let branch_pressure =
            (subdirs as f64 / self.config.fsdir.max_subdirs_per_dir as f64).min(1.0);
        let size_pressure = (total_loc as f64 / self.config.fsdir.max_dir_loc as f64).min(1.0);

        // Calculate distribution-based optimality scores
        let file_count_score = self.calculate_distribution_score(
            files,
            self.config.fsdir.optimal_files,
            self.config.fsdir.optimal_files_stddev,
        );
        let subdir_count_score = self.calculate_distribution_score(
            subdirs,
            self.config.fsdir.optimal_subdirs,
            self.config.fsdir.optimal_subdirs_stddev,
        );

        // Calculate dispersion combining gini and entropy
        let max_entropy = if files > 0 {
            (files as f64).log2()
        } else {
            1.0
        };
        let normalized_entropy = if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        };
        let dispersion = gini.max(1.0 - normalized_entropy);

        // Apply size normalization
        let size_normalization_factor = self.calculate_size_normalization_factor(files, total_loc);

        // Calculate overall imbalance score
        let file_deviation = 1.0 - file_count_score;
        let subdir_deviation = 1.0 - subdir_count_score;

        let raw_imbalance = 0.25 * file_pressure
            + 0.15 * branch_pressure
            + 0.20 * size_pressure
            + 0.10 * dispersion
            + 0.20 * file_deviation
            + 0.10 * subdir_deviation;

        let imbalance = (raw_imbalance * size_normalization_factor).min(1.0);

        let metrics = DirectoryMetrics {
            files,
            subdirs,
            loc: total_loc,
            gini,
            entropy,
            file_pressure,
            branch_pressure,
            size_pressure,
            dispersion,
            file_count_score,
            subdir_count_score,
            imbalance,
        };

        // Cache the result
        self.metrics_cache.insert(dir_path.to_path_buf(), metrics.clone());

        Ok(metrics)
    }

    /// Analyze directory for reorganization using pre-computed metrics
    pub fn analyze_directory_for_reorg_with_metrics(
        &self,
        dir_path: &Path,
        metrics_map: &HashMap<PathBuf, &PrecomputedFileMetrics>,
    ) -> Result<Option<BranchReorgPack>> {
        let metrics = self.calculate_directory_metrics_with_metrics(dir_path, metrics_map)?;

        // Check if directory meets threshold for consideration
        if metrics.imbalance < 0.6 {
            return Ok(None);
        }

        // Additional conditions
        let meets_conditions = metrics.files > self.config.fsdir.max_files_per_dir
            || metrics.loc > self.config.fsdir.max_dir_loc
            || metrics.dispersion >= 0.5;

        if !meets_conditions {
            return Ok(None);
        }

        // Skip small directories
        if metrics.files <= 5 && metrics.loc <= 600 {
            return Ok(None);
        }

        // Build dependency graph and partition
        let dependency_graph = self.build_dependency_graph(dir_path)?;
        let partitions = self.partition_directory(&dependency_graph, &metrics)?;

        if partitions.is_empty() {
            return Ok(None);
        }

        // Calculate expected gains
        let gain = self.calculate_reorganization_gain(&metrics, &partitions, dir_path)?;

        if gain.imbalance_delta < self.config.fsdir.min_branch_recommendation_gain {
            return Ok(None);
        }

        // Calculate effort estimation and file moves
        let effort = self.calculate_reorganization_effort(&partitions, dir_path)?;
        let file_moves = self.generate_file_moves(&partitions, dir_path)?;

        let pack = BranchReorgPack {
            kind: "branch_reorg".to_string(),
            dir: dir_path.to_path_buf(),
            current: metrics,
            proposal: partitions,
            file_moves,
            gain,
            effort,
            rules: self.generate_reorganization_rules(dir_path),
        };

        Ok(Some(pack))
    }

    /// Calculate a distribution-based optimality score.
    ///
    /// Returns a score in [0, 1] where 1.0 means the value equals the optimal (mean),
    /// and the score decreases as the value deviates from optimal. The score is
    /// computed as the ratio of the normal distribution density at the given value
    /// to the density at the mean (which is the maximum density).
    ///
    /// This simplifies to: `score = exp(-0.5 * ((value - optimal) / stddev)²)`
    pub fn calculate_distribution_score(&self, value: usize, optimal: usize, stddev: f64) -> f64 {
        if stddev <= 0.0 {
            // If stddev is zero or negative, return 1.0 only if value equals optimal
            return if value == optimal { 1.0 } else { 0.0 };
        }

        let z = (value as f64 - optimal as f64) / stddev;
        (-0.5 * z * z).exp()
    }

    /// Calculate Gini coefficient for LOC distribution with O(n log n) optimization
    pub fn calculate_gini_coefficient(&self, values: &[usize]) -> f64 {
        if values.len() <= 1 {
            return 0.0;
        }

        let n = values.len() as f64;
        let sum: f64 = values.iter().map(|&v| v as f64).sum();

        if sum == 0.0 {
            return 0.0;
        }

        // O(n log n) algorithm using the standard Gini formula
        // Sort the values first (O(n log n))
        let mut sorted_values = values.to_vec();
        sorted_values.sort_unstable();

        // Calculate using the efficient formula: Gini = (2 * sum(i * y_i) / (n * sum(y_i))) - (n + 1) / n
        // where i is the rank (1-indexed) and y_i is the sorted value
        let mut weighted_sum = 0.0;
        for (i, &val) in sorted_values.iter().enumerate() {
            weighted_sum += (i as f64 + 1.0) * val as f64;
        }

        let gini = (2.0 * weighted_sum) / (n * sum) - (n + 1.0) / n;
        gini.max(0.0) // Ensure non-negative result
    }

    /// Calculate entropy for LOC distribution with parallel optimization
    pub fn calculate_entropy(&self, values: &[usize]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        let total: usize = values.iter().sum();
        if total == 0 {
            return 0.0;
        }

        // For small arrays, use sequential computation
        if values.len() < 100 {
            return values
                .iter()
                .filter(|&&x| x > 0)
                .map(|&x| {
                    let p = x as f64 / total as f64;
                    -p * p.log2()
                })
                .sum();
        }

        // For larger arrays, use parallel computation
        let total_f64 = total as f64;
        values
            .par_iter()
            .filter(|&&x| x > 0)
            .map(|&x| {
                let p = x as f64 / total_f64;
                -p * p.log2()
            })
            .sum()
    }

    /// Analyze directory for reorganization potential
    pub fn analyze_directory_for_reorg(&self, dir_path: &Path) -> Result<Option<BranchReorgPack>> {
        let metrics = self.calculate_directory_metrics(dir_path)?;

        // Check if directory meets threshold for consideration
        if metrics.imbalance < 0.6 {
            return Ok(None);
        }

        // Additional conditions
        let meets_conditions = metrics.files > self.config.fsdir.max_files_per_dir
            || metrics.loc > self.config.fsdir.max_dir_loc
            || metrics.dispersion >= 0.5;

        if !meets_conditions {
            return Ok(None);
        }

        // Skip small directories
        if metrics.files <= 5 && metrics.loc <= 600 {
            return Ok(None);
        }

        // Build dependency graph and partition
        let dependency_graph = self.build_dependency_graph(dir_path)?;
        let partitions = self.partition_directory(&dependency_graph, &metrics)?;

        if partitions.is_empty() {
            return Ok(None);
        }

        // Calculate expected gains
        let gain = self.calculate_reorganization_gain(&metrics, &partitions, dir_path)?;

        if gain.imbalance_delta < self.config.fsdir.min_branch_recommendation_gain {
            return Ok(None);
        }

        // Calculate effort estimation and file moves
        let effort = self.calculate_reorganization_effort(&partitions, dir_path)?;
        let file_moves = self.generate_file_moves(&partitions, dir_path)?;

        let pack = BranchReorgPack {
            kind: "branch_reorg".to_string(),
            dir: dir_path.to_path_buf(),
            current: metrics,
            proposal: partitions,
            file_moves,
            gain,
            effort,
            rules: self.generate_reorganization_rules(dir_path),
        };

        Ok(Some(pack))
    }

    /// Build internal dependency graph for directory
    pub fn build_dependency_graph(&self, dir_path: &Path) -> Result<DependencyGraph> {
        let mut graph = petgraph::Graph::new();
        let mut path_to_node: HashMap<PathBuf, NodeIndex> = HashMap::new();

        // First pass: create nodes for all code files in directory
        for entry in std::fs::read_dir(dir_path)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.is_file() {
                if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                    if self.is_code_file(ext) {
                        let loc = self.count_lines_of_code(&file_path)?;
                        let metadata = std::fs::metadata(&file_path)?;

                        let file_node = FileNode {
                            path: file_path.clone(),
                            loc,
                            size_bytes: metadata.len() as usize,
                        };

                        let node_idx = graph.add_node(file_node);
                        path_to_node.insert(file_path, node_idx);
                    }
                }
            }
        }

        // Second pass: analyze imports and create edges
        for (file_path, &source_node) in &path_to_node {
            if let Ok(imports) = self.extract_imports(file_path) {
                for import in imports {
                    // Resolve import to file path within the same directory
                    if let Some(target_path) = self.resolve_import_to_local_file(&import, dir_path)
                    {
                        if let Some(&target_node) = path_to_node.get(&target_path) {
                            // Add edge from source to target with weight based on import frequency
                            let edge = DependencyEdge {
                                weight: 1, // Could be enhanced to count import usage frequency
                                relationship_type: import.import_type,
                            };

                            graph.add_edge(source_node, target_node, edge);
                        }
                    }
                }
            }
        }

        Ok(graph)
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
    fn calculate_cut_size(
        &self,
        graph: &DependencyGraph,
        part1: &[NodeIndex],
        part2: &[NodeIndex],
    ) -> usize {
        let part1_set: HashSet<_> = part1.iter().copied().collect();
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
            improved = false;

            // Try to improve each pair of communities
            for i in 0..communities.len() {
                for j in i + 1..communities.len() {
                    let _initial_cost = self.calculate_partition_cost(graph, &communities);

                    // Try swapping nodes between communities i and j
                    if let Some((best_swap, cost_improvement)) =
                        self.find_best_node_swap(graph, &communities[i], &communities[j])
                    {
                        if cost_improvement > 0.0 {
                            // Apply the swap
                            let (from_comm, _to_comm, node) = best_swap;
                            if from_comm == i {
                                communities[i].retain(|&n| n != node);
                                communities[j].push(node);
                            } else {
                                communities[j].retain(|&n| n != node);
                                communities[i].push(node);
                            }
                            improved = true;
                        }
                    }
                }
            }

            iteration += 1;
        }

        Ok(communities)
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
            let name = self.generate_partition_name(&files, i);

            partitions.push(DirectoryPartition {
                name,
                files,
                loc: total_loc,
            });
        }

        Ok(partitions)
    }

    /// Generate deterministic partition name based on file paths
    fn generate_partition_name(&self, files: &[PathBuf], index: usize) -> String {
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
        self.config
            .partitioning
            .naming_fallbacks
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("partition_{}", index))
    }

    /// Calculate expected gains from reorganization
    pub fn calculate_reorganization_gain(
        &self,
        current_metrics: &DirectoryMetrics,
        partitions: &[DirectoryPartition],
        dir_path: &Path,
    ) -> Result<ReorganizationGain> {
        // Calculate imbalance for each proposed partition
        let mut partition_imbalances = Vec::new();

        for partition in partitions {
            // Create a temporary directory metrics for this partition
            let partition_files = partition.files.len();
            let _partition_subdirs = 0; // New partitions start with 0 subdirs
            let partition_loc = partition.loc;

            // Simulate LOC distribution within partition (simplified)
            let avg_loc_per_file = if partition_files > 0 {
                partition_loc / partition_files
            } else {
                0
            };
            let loc_distribution: Vec<usize> =
                (0..partition_files).map(|_| avg_loc_per_file).collect();

            // Calculate metrics for this partition
            let gini = self.calculate_gini_coefficient(&loc_distribution);
            let entropy = self.calculate_entropy(&loc_distribution);

            // Calculate pressure metrics
            let file_pressure =
                (partition_files as f64 / self.config.fsdir.max_files_per_dir as f64).min(1.0);
            let branch_pressure = 0.0; // No subdirs in new partition
            let size_pressure =
                (partition_loc as f64 / self.config.fsdir.max_dir_loc as f64).min(1.0);

            // Calculate dispersion
            let max_entropy = if partition_files > 0 {
                (partition_files as f64).log2()
            } else {
                1.0
            };
            let normalized_entropy = if max_entropy > 0.0 {
                entropy / max_entropy
            } else {
                0.0
            };
            let dispersion = gini.max(1.0 - normalized_entropy);

            // Apply size normalization
            let size_normalization_factor =
                self.calculate_size_normalization_factor(partition_files, partition_loc);

            // Calculate imbalance for this partition
            let raw_imbalance = 0.35 * file_pressure
                + 0.25 * branch_pressure
                + 0.25 * size_pressure
                + 0.15 * dispersion;

            let partition_imbalance = raw_imbalance * size_normalization_factor;
            partition_imbalances.push(partition_imbalance);
        }

        // Calculate average imbalance of new partitions
        let avg_new_imbalance = if !partition_imbalances.is_empty() {
            partition_imbalances.iter().sum::<f64>() / partition_imbalances.len() as f64
        } else {
            current_metrics.imbalance
        };

        // Imbalance improvement (positive means improvement)
        let imbalance_delta = (current_metrics.imbalance - avg_new_imbalance).max(0.0);

        // Calculate cross-edges reduced by analyzing dependency graph
        let cross_edges_reduced = self.estimate_cross_edges_reduced(partitions, dir_path)?;

        Ok(ReorganizationGain {
            imbalance_delta,
            cross_edges_reduced,
        })
    }

    /// Estimate how many cross-partition edges would be reduced
    fn estimate_cross_edges_reduced(
        &self,
        partitions: &[DirectoryPartition],
        dir_path: &Path,
    ) -> Result<usize> {
        // Build dependency graph to analyze edge cuts
        let dependency_graph = self.build_dependency_graph(dir_path)?;

        // Create partition mapping
        let mut file_to_partition: HashMap<PathBuf, usize> = HashMap::new();
        for (partition_idx, partition) in partitions.iter().enumerate() {
            for file_path in &partition.files {
                file_to_partition.insert(file_path.clone(), partition_idx);
            }
        }

        // Count edges that would cross partition boundaries
        let mut cross_edges = 0;
        let mut _total_internal_edges = 0;

        for edge_idx in dependency_graph.edge_indices() {
            if let Some((source, target)) = dependency_graph.edge_endpoints(edge_idx) {
                if let (Some(source_node), Some(target_node)) = (
                    dependency_graph.node_weight(source),
                    dependency_graph.node_weight(target),
                ) {
                    _total_internal_edges += 1;

                    // Check if this edge would cross partition boundaries
                    if let (Some(&source_partition), Some(&target_partition)) = (
                        file_to_partition.get(&source_node.path),
                        file_to_partition.get(&target_node.path),
                    ) {
                        if source_partition != target_partition {
                            cross_edges += 1;
                        }
                    }
                }
            }
        }

        // Return estimated edges that would be internal after reorganization
        Ok(cross_edges)
    }

    /// Calculate effort estimation for reorganization
    pub fn calculate_reorganization_effort(
        &self,
        partitions: &[DirectoryPartition],
        _dir_path: &Path,
    ) -> Result<ReorganizationEffort> {
        let files_moved = partitions.iter().map(|p| p.files.len()).sum();

        // Rough estimation: 2 import updates per moved file on average
        let import_updates_est = files_moved * 2;

        Ok(ReorganizationEffort {
            files_moved,
            import_updates_est,
        })
    }

    /// Generate reorganization rules
    fn generate_reorganization_rules(&self, _dir_path: &Path) -> Vec<String> {
        vec![
            "Create subdirectories for each partition".to_string(),
            "Update relative import statements".to_string(),
            "Preserve file names and structure within partitions".to_string(),
            "Test imports after reorganization".to_string(),
        ]
    }

    /// Generate file moves for reorganization
    pub fn generate_file_moves(
        &self,
        partitions: &[DirectoryPartition],
        dir_path: &Path,
    ) -> Result<Vec<FileMove>> {
        let mut file_moves = Vec::new();

        for partition in partitions {
            for file_path in &partition.files {
                // Create destination path in new subdirectory
                let file_name = file_path
                    .file_name()
                    .ok_or_else(|| ValknutError::internal("Invalid file path"))?;

                let destination = dir_path.join(&partition.name).join(file_name);

                file_moves.push(FileMove {
                    from: file_path.clone(),
                    to: destination,
                });
            }
        }

        Ok(file_moves)
    }

    /// Calculate size normalization factor for directory metrics
    pub fn calculate_size_normalization_factor(&self, files: usize, total_loc: usize) -> f64 {
        // Prevent small codebases from being over-penalized
        // and large ones from being under-penalized
        let base_files = 10.0;
        let base_loc = 1000.0;

        let file_factor = (files as f64 / base_files).ln_1p() / base_files.ln();
        let loc_factor = (total_loc as f64 / base_loc).ln_1p() / base_loc.ln();

        // Combine factors and normalize to [0.5, 1.5] range
        let combined = (file_factor + loc_factor) * 0.5;
        1.0 + combined.tanh() * 0.5
    }

    /// Extract imports from source file
    fn extract_imports(&self, file_path: &Path) -> Result<Vec<ImportStatement>> {
        let content = FileReader::read_to_string(file_path)?;
        match adapter_for_file(file_path) {
            Ok(mut adapter) => adapter.extract_imports(&content),
            Err(err) => {
                warn!(
                    "Directory analyzer could not create adapter for {}: {}",
                    file_path.display(),
                    err
                );
                Ok(Vec::new())
            }
        }
    }

    /// Resolve import statement to local file path
    fn resolve_import_to_local_file(
        &self,
        import: &ImportStatement,
        dir_path: &Path,
    ) -> Option<PathBuf> {
        // This is a simplified resolution - in practice would be more sophisticated
        let module_name = &import.module;

        // Check if it's a relative import within the same directory
        if module_name.starts_with('.') {
            return None; // Skip relative imports for now
        }

        // Try common file extensions
        let extensions = ["py", "js", "ts", "jsx", "tsx", "rs"];

        for ext in &extensions {
            let potential_path = dir_path.join(format!("{}.{}", module_name, ext));
            if potential_path.exists() {
                return Some(potential_path);
            }
        }

        None
    }

    /// Discover directories recursively for analysis
    pub async fn discover_directories(&self, root_path: &Path) -> Result<Vec<PathBuf>> {
        let mut directories = Vec::new();
        self.collect_directories_recursive(root_path, &mut directories)?;
        Ok(directories)
    }

    /// Collect directories recursively
    fn collect_directories_recursive(
        &self,
        path: &Path,
        directories: &mut Vec<PathBuf>,
    ) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            if entry_path.is_dir() {
                if !self.should_skip_directory(&entry_path) {
                    directories.push(entry_path.clone());
                    self.collect_directories_recursive(&entry_path, directories)?;
                }
            }
        }
        Ok(())
    }

    /// Check if directory should be skipped from analysis
    fn should_skip_directory(&self, path: &Path) -> bool {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        // Skip common ignore patterns
        SKIP_DIRECTORIES.contains(&filename)
    }
}


#[cfg(test)]
mod tests;
