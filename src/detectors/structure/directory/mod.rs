//! Directory analysis, graph partitioning, and reorganization logic

pub(crate) mod partitioning;
pub(crate) mod reorganization;
mod stats;

// Re-export stats functions for use by parent module
pub use stats::{
    calculate_distribution_score, calculate_entropy, calculate_gini_coefficient,
    calculate_size_normalization_factor,
};

use dashmap::DashMap;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::errors::Result;
use crate::core::file_utils::FileReader;
use crate::lang::registry::adapter_for_file;
use tracing::warn;

use super::PrecomputedFileMetrics;

use super::config::{
    is_code_extension, should_skip_directory, BranchReorgPack, DependencyEdge, DependencyGraph,
    DirectoryMetrics, DirectoryPartition, FileNode, ImportStatement, StructureConfig,
};

use partitioning::GraphPartitioner;
use reorganization::ReorganizationPlanner;

/// Analyzer for directory-level structure metrics and reorganization.
pub struct DirectoryAnalyzer {
    config: StructureConfig,
    metrics_cache: DashMap<PathBuf, DirectoryMetrics>,
}

/// Factory and analysis methods for [`DirectoryAnalyzer`].
impl DirectoryAnalyzer {
    /// Creates a new directory analyzer with the given configuration.
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
        let metrics = self.compute_metrics_from_distribution(files, subdirs, &loc_distribution);

        // Cache the result
        self.metrics_cache
            .insert(dir_path.to_path_buf(), metrics.clone());

        Ok(metrics)
    }

    /// Compute metrics from gathered statistics
    fn compute_metrics_from_distribution(
        &self,
        files: usize,
        subdirs: usize,
        loc_distribution: &[usize],
    ) -> DirectoryMetrics {
        let total_loc = loc_distribution.iter().sum::<usize>();

        // Calculate dispersion metrics
        let gini = calculate_gini_coefficient(loc_distribution);
        let entropy = calculate_entropy(loc_distribution);

        // Calculate pressure metrics (clipped to [0,1])
        let file_pressure = (files as f64 / self.config.fsdir.max_files_per_dir as f64).min(1.0);
        let branch_pressure =
            (subdirs as f64 / self.config.fsdir.max_subdirs_per_dir as f64).min(1.0);
        let size_pressure = (total_loc as f64 / self.config.fsdir.max_dir_loc as f64).min(1.0);

        // Calculate distribution-based optimality scores
        let file_count_score = calculate_distribution_score(
            files,
            self.config.fsdir.optimal_files,
            self.config.fsdir.optimal_files_stddev,
        );
        let subdir_count_score = calculate_distribution_score(
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
        let size_normalization_factor = calculate_size_normalization_factor(files, total_loc);

        // Calculate overall imbalance score with normalization
        let file_deviation = 1.0 - file_count_score;
        let subdir_deviation = 1.0 - subdir_count_score;

        let raw_imbalance = 0.25 * file_pressure
            + 0.15 * branch_pressure
            + 0.20 * size_pressure
            + 0.10 * dispersion
            + 0.20 * file_deviation
            + 0.10 * subdir_deviation;

        let imbalance = raw_imbalance * size_normalization_factor;

        DirectoryMetrics {
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
        }
    }

    /// Gather basic directory statistics
    fn gather_directory_stats(&self, dir_path: &Path) -> Result<(usize, usize, Vec<usize>)> {
        let mut files = 0;
        let mut subdirs = 0;
        let mut loc_distribution = Vec::new();

        for entry in std::fs::read_dir(dir_path)? {
            let path = entry?.path();

            if path.is_dir() {
                subdirs += 1;
                continue;
            }

            let is_code = path
                .extension()
                .and_then(|e| e.to_str())
                .map_or(false, |ext| self.is_code_file(ext));

            if is_code {
                files += 1;
                loc_distribution.push(self.count_lines_of_code(&path)?);
            }
        }

        Ok((files, subdirs, loc_distribution))
    }

    /// Check if file extension indicates a code file
    fn is_code_file(&self, extension: &str) -> bool {
        is_code_extension(extension)
    }

    /// Count lines of code in a file (excludes empty lines and // comments only)
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
            let path = entry?.path();

            if path.is_dir() {
                subdirs += 1;
                continue;
            }

            let is_code = path
                .extension()
                .and_then(|e| e.to_str())
                .map_or(false, |ext| self.is_code_file(ext));

            if is_code {
                files += 1;
                // Use pre-computed LOC if available, otherwise fall back to reading
                let loc = metrics_map
                    .get(&path)
                    .map(|m| m.loc)
                    .map_or_else(|| self.count_lines_of_code(&path), Ok)?;
                loc_distribution.push(loc);
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
        let metrics = self.compute_metrics_from_distribution(files, subdirs, &loc_distribution);

        // Cache the result
        self.metrics_cache
            .insert(dir_path.to_path_buf(), metrics.clone());

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
        if !self.should_consider_for_reorg(&metrics) {
            return Ok(None);
        }

        // Build dependency graph and partition
        let dependency_graph = self.build_dependency_graph(dir_path)?;
        let partitioner = GraphPartitioner::new(&self.config);
        let partitions = partitioner.partition_directory(&dependency_graph, &metrics)?;

        if partitions.is_empty() {
            return Ok(None);
        }

        // Calculate expected gains using reorganization planner
        let planner = ReorganizationPlanner::new(&self.config);
        let gain = planner.calculate_reorganization_gain(
            &metrics,
            &partitions,
            dir_path,
            |p| self.build_dependency_graph(p),
        )?;

        if gain.imbalance_delta < self.config.fsdir.min_branch_recommendation_gain {
            return Ok(None);
        }

        // Calculate effort estimation and file moves
        let effort = planner.calculate_reorganization_effort(&partitions)?;
        let file_moves = planner.generate_file_moves(&partitions, dir_path)?;

        let pack = BranchReorgPack {
            kind: "branch_reorg".to_string(),
            dir: dir_path.to_path_buf(),
            current: metrics,
            proposal: partitions,
            file_moves,
            gain,
            effort,
            rules: planner.generate_reorganization_rules(),
        };

        Ok(Some(pack))
    }

    /// Check if directory should be considered for reorganization
    fn should_consider_for_reorg(&self, metrics: &DirectoryMetrics) -> bool {
        // Check if directory meets threshold for consideration
        if metrics.imbalance < 0.6 {
            return false;
        }

        // Additional conditions
        let meets_conditions = metrics.files > self.config.fsdir.max_files_per_dir
            || metrics.loc > self.config.fsdir.max_dir_loc
            || metrics.dispersion >= 0.5;

        if !meets_conditions {
            return false;
        }

        // Skip small directories
        if metrics.files <= 5 && metrics.loc <= 600 {
            return false;
        }

        true
    }

    /// Analyze directory for reorganization potential
    pub fn analyze_directory_for_reorg(&self, dir_path: &Path) -> Result<Option<BranchReorgPack>> {
        let metrics = self.calculate_directory_metrics(dir_path)?;

        // Check if directory meets threshold for consideration
        if !self.should_consider_for_reorg(&metrics) {
            return Ok(None);
        }

        // Build dependency graph and partition
        let dependency_graph = self.build_dependency_graph(dir_path)?;
        let partitioner = GraphPartitioner::new(&self.config);
        let partitions = partitioner.partition_directory(&dependency_graph, &metrics)?;

        if partitions.is_empty() {
            return Ok(None);
        }

        // Calculate expected gains using reorganization planner
        let planner = ReorganizationPlanner::new(&self.config);
        let gain = planner.calculate_reorganization_gain(
            &metrics,
            &partitions,
            dir_path,
            |p| self.build_dependency_graph(p),
        )?;

        if gain.imbalance_delta < self.config.fsdir.min_branch_recommendation_gain {
            return Ok(None);
        }

        // Calculate effort estimation and file moves
        let effort = planner.calculate_reorganization_effort(&partitions)?;
        let file_moves = planner.generate_file_moves(&partitions, dir_path)?;

        let pack = BranchReorgPack {
            kind: "branch_reorg".to_string(),
            dir: dir_path.to_path_buf(),
            current: metrics,
            proposal: partitions,
            file_moves,
            gain,
            effort,
            rules: planner.generate_reorganization_rules(),
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
            if let Some((file_path, node)) = self.try_create_file_node(&entry)? {
                let node_idx = graph.add_node(node);
                path_to_node.insert(file_path, node_idx);
            }
        }

        // Second pass: analyze imports and create edges
        for (file_path, &source_node) in &path_to_node {
            self.add_import_edges(file_path, source_node, dir_path, &path_to_node, &mut graph);
        }

        Ok(graph)
    }

    /// Try to create a FileNode from a directory entry if it's a code file.
    fn try_create_file_node(
        &self,
        entry: &std::fs::DirEntry,
    ) -> Result<Option<(PathBuf, FileNode)>> {
        let file_path = entry.path();
        if !file_path.is_file() {
            return Ok(None);
        }

        let ext = match file_path.extension().and_then(|e| e.to_str()) {
            Some(ext) if self.is_code_file(ext) => ext,
            _ => return Ok(None),
        };
        let _ = ext; // Used in the match guard above

        let loc = self.count_lines_of_code(&file_path)?;
        let metadata = std::fs::metadata(&file_path)?;

        let node = FileNode {
            path: file_path.clone(),
            loc,
            size_bytes: metadata.len() as usize,
        };

        Ok(Some((file_path, node)))
    }

    /// Add dependency edges for imports from a source file.
    fn add_import_edges(
        &self,
        file_path: &Path,
        source_node: NodeIndex,
        dir_path: &Path,
        path_to_node: &HashMap<PathBuf, NodeIndex>,
        graph: &mut DependencyGraph,
    ) {
        let imports = match self.extract_imports(file_path) {
            Ok(imports) => imports,
            Err(_) => return,
        };

        for import in imports {
            let target_path = match self.resolve_import_to_local_file(&import, dir_path) {
                Some(path) => path,
                None => continue,
            };

            let target_node = match path_to_node.get(&target_path) {
                Some(&node) => node,
                None => continue,
            };

            let edge = DependencyEdge {
                weight: 1,
                relationship_type: import.import_type,
            };
            graph.add_edge(source_node, target_node, edge);
        }
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
        should_skip_directory(path)
    }
}


#[cfg(test)]
mod tests;
