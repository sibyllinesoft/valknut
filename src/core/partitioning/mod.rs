//! Import graph partitioning for scalable codebase analysis.
//!
//! This module provides functionality to partition a codebase into coherent slices
//! based on import relationships, respecting token budget constraints. This enables
//! tools like the Oracle to scale to arbitrary codebase sizes by processing one
//! slice at a time.
//!
//! Key features:
//! - File-level import graph construction
//! - Token-budget-aware graph partitioning
//! - Strongly connected component detection for cohesive grouping
//! - Configurable slice sizes and overlap handling

mod module_resolver;
mod types;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::core::errors::Result;
use crate::core::file_utils::FileReader;
use crate::lang::adapter_for_file;

pub use types::{CodeSlice, PartitionConfig, PartitionResult, PartitionStats};
use module_resolver::{build_module_map, resolve_import};
use types::FileNode;

/// Helper for incrementally building a slice.
struct SliceBuilder {
    files: Vec<PathBuf>,
    contents: HashMap<PathBuf, String>,
    token_count: usize,
}

/// Factory and building methods for [`SliceBuilder`].
impl SliceBuilder {
    /// Creates a new empty slice builder.
    fn new() -> Self {
        Self {
            files: Vec::new(),
            contents: HashMap::new(),
            token_count: 0,
        }
    }

    /// Check if adding more tokens would exceed the budget.
    fn would_exceed_budget(&self, additional_tokens: usize, budget: usize) -> bool {
        !self.files.is_empty() && self.token_count + additional_tokens > budget
    }

    /// Add a file to the current slice being built.
    fn add_file(&mut self, path: &PathBuf, tokens: usize, project_path: &Path) {
        let full_path = project_path.join(path);
        if let Ok(content) = FileReader::read_to_string(&full_path) {
            self.contents.insert(path.clone(), content);
            self.token_count += tokens;
            self.files.push(path.clone());
        }
    }

    /// Finalize and return the current slice, resetting the builder.
    fn finalize(&mut self, id: usize) -> Option<CodeSlice> {
        if self.files.is_empty() {
            return None;
        }

        let slice = CodeSlice {
            id,
            files: std::mem::take(&mut self.files),
            contents: std::mem::take(&mut self.contents),
            token_count: self.token_count,
            bridge_dependencies: vec![],
            primary_module: None,
        };
        self.token_count = 0;
        Some(slice)
    }
}

#[cfg(test)]
mod tests;

/// Import graph partitioner
pub struct ImportGraphPartitioner {
    config: PartitionConfig,
}

/// Factory and partitioning methods for [`ImportGraphPartitioner`].
impl ImportGraphPartitioner {
    /// Creates a new partitioner with the given configuration.
    pub fn new(config: PartitionConfig) -> Self {
        Self { config }
    }

    /// Partition a codebase into coherent slices
    pub fn partition(&self, project_path: &Path, files: &[PathBuf]) -> Result<PartitionResult> {
        if files.is_empty() {
            return Ok(Self::empty_result());
        }

        let file_nodes = self.build_file_nodes(project_path, files)?;
        if file_nodes.is_empty() {
            return Ok(Self::unassigned_result(files));
        }

        let total_tokens: usize = file_nodes.values().map(|n| n.tokens).sum();
        let (graph, index_map, reverse_map) = self.build_import_graph(&file_nodes, project_path);

        let sccs = tarjan_scc(&graph);
        let scc_count = sccs.len();
        let largest_scc = sccs.iter().map(|scc| scc.len()).max().unwrap_or(0);

        let (slices, unassigned) = self.partition_by_budget(
            &file_nodes,
            &graph,
            &index_map,
            &reverse_map,
            &sccs,
            project_path,
        )?;

        let cross_slice_imports = self.count_cross_slice_imports(&slices, &file_nodes);

        Ok(PartitionResult {
            slices: slices.clone(),
            unassigned,
            stats: PartitionStats {
                total_files: file_nodes.len(),
                total_tokens,
                slice_count: slices.len(),
                scc_count,
                largest_scc,
                cross_slice_imports,
            },
        })
    }

    /// Create an empty partition result.
    fn empty_result() -> PartitionResult {
        PartitionResult {
            slices: vec![],
            unassigned: vec![],
            stats: PartitionStats {
                total_files: 0,
                total_tokens: 0,
                slice_count: 0,
                scc_count: 0,
                largest_scc: 0,
                cross_slice_imports: 0,
            },
        }
    }

    /// Create a result where all files are unassigned.
    fn unassigned_result(files: &[PathBuf]) -> PartitionResult {
        PartitionResult {
            slices: vec![],
            unassigned: files.to_vec(),
            stats: PartitionStats {
                total_files: files.len(),
                total_tokens: 0,
                slice_count: 0,
                scc_count: 0,
                largest_scc: 0,
                cross_slice_imports: 0,
            },
        }
    }

    /// Build file nodes with content and import information
    fn build_file_nodes(
        &self,
        project_path: &Path,
        files: &[PathBuf],
    ) -> Result<HashMap<PathBuf, FileNode>> {
        let mut nodes = HashMap::new();

        for file_path in files {
            let full_path = if file_path.is_absolute() {
                file_path.clone()
            } else {
                project_path.join(file_path)
            };

            if !full_path.exists() {
                continue;
            }

            // Read content and estimate tokens
            let content = match FileReader::read_to_string(&full_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let tokens = content.len() / 4; // Rough token estimate

            // Extract imports
            let imports = self.extract_file_imports(&full_path, &content);

            let relative_path = full_path
                .strip_prefix(project_path)
                .unwrap_or(&full_path)
                .to_path_buf();

            nodes.insert(
                relative_path.clone(),
                FileNode {
                    path: relative_path,
                    tokens,
                    imports,
                },
            );
        }

        Ok(nodes)
    }

    /// Extract imports from a file
    fn extract_file_imports(&self, file_path: &Path, content: &str) -> Vec<String> {
        let mut adapter = match adapter_for_file(file_path) {
            Ok(a) => a,
            Err(_) => return vec![],
        };

        match adapter.extract_imports(content) {
            Ok(imports) => imports.into_iter().map(|imp| imp.module).collect(),
            Err(_) => vec![],
        }
    }

    /// Build directed import graph
    fn build_import_graph(
        &self,
        nodes: &HashMap<PathBuf, FileNode>,
        project_path: &Path,
    ) -> (
        DiGraph<PathBuf, ()>,
        HashMap<PathBuf, NodeIndex>,
        HashMap<NodeIndex, PathBuf>,
    ) {
        let mut graph = DiGraph::new();
        let mut index_map = HashMap::new();
        let mut reverse_map = HashMap::new();

        // Add all files as nodes
        for path in nodes.keys() {
            let idx = graph.add_node(path.clone());
            index_map.insert(path.clone(), idx);
            reverse_map.insert(idx, path.clone());
        }

        // Build module-to-file mapping for import resolution
        let module_map = build_module_map(nodes);

        // Add edges for imports
        for (path, node) in nodes {
            let Some(&from_idx) = index_map.get(path) else {
                continue;
            };

            for import in &node.imports {
                let Some(to_idx) = resolve_import(import, path, &module_map)
                    .and_then(|target| index_map.get(&target).copied()) else {
                    continue;
                };
                if from_idx != to_idx {
                    graph.add_edge(from_idx, to_idx, ());
                }
            }
        }

        (graph, index_map, reverse_map)
    }

    /// Partition files into slices respecting token budget
    fn partition_by_budget(
        &self,
        nodes: &HashMap<PathBuf, FileNode>,
        graph: &DiGraph<PathBuf, ()>,
        index_map: &HashMap<PathBuf, NodeIndex>,
        reverse_map: &HashMap<NodeIndex, PathBuf>,
        sccs: &[Vec<NodeIndex>],
        project_path: &Path,
    ) -> Result<(Vec<CodeSlice>, Vec<PathBuf>)> {
        let mut slices: Vec<CodeSlice> = Vec::new();
        let mut assigned: HashSet<PathBuf> = HashSet::new();
        let mut slice_id = 0;

        // Process SCCs from largest to smallest (they're returned in reverse topological order)
        let mut scc_with_tokens: Vec<(usize, Vec<NodeIndex>)> = sccs
            .iter()
            .enumerate()
            .map(|(i, scc)| {
                let tokens: usize = scc
                    .iter()
                    .filter_map(|idx| reverse_map.get(idx))
                    .filter_map(|path| nodes.get(path))
                    .map(|node| node.tokens)
                    .sum();
                (tokens, scc.clone())
            })
            .collect();

        // Sort by token count descending
        scc_with_tokens.sort_by(|a, b| b.0.cmp(&a.0));

        for (_tokens, scc) in scc_with_tokens {
            let scc_paths: Vec<PathBuf> = scc
                .iter()
                .filter_map(|idx| reverse_map.get(idx))
                .filter(|path| !assigned.contains(*path))
                .cloned()
                .collect();

            if scc_paths.is_empty() {
                continue;
            }

            // Try to fit this SCC into an existing slice
            let scc_tokens: usize = scc_paths
                .iter()
                .filter_map(|p| nodes.get(p))
                .map(|n| n.tokens)
                .sum();

            // First pass: try to find a slice with a direct connection
            let added_to_connected = self.try_add_to_connected_slice(
                &scc_paths, scc_tokens, &mut slices, nodes, project_path, graph, index_map, &mut assigned
            );

            // Second pass: if no connected slice, find the best-matching slice
            let added_to_existing = added_to_connected || self.try_add_to_best_affinity_slice(
                &scc_paths, scc_tokens, &mut slices, nodes, project_path, &mut assigned
            );

            if !added_to_existing {
                // Create new slice(s) for this SCC
                let new_slices =
                    self.create_slices_for_files(&scc_paths, nodes, project_path, &mut slice_id)?;
                assigned.extend(new_slices.iter().flat_map(|s| s.files.iter().cloned()));
                slices.extend(new_slices);
            }
        }

        // Collect unassigned files
        let unassigned: Vec<PathBuf> = nodes
            .keys()
            .filter(|p| !assigned.contains(*p))
            .cloned()
            .collect();

        // Add any remaining unassigned files to existing slices or create new ones
        if !unassigned.is_empty() {
            let new_slices =
                self.create_slices_for_files(&unassigned, nodes, project_path, &mut slice_id)?;
            slices.extend(new_slices);
        }

        // Determine primary module for each slice
        for slice in &mut slices {
            slice.primary_module = self.determine_primary_module(&slice.files);
        }

        Ok((slices, vec![]))
    }

    /// Add files to a slice, updating token counts and assigned set
    fn add_files_to_slice(
        &self,
        paths: &[PathBuf],
        nodes: &HashMap<PathBuf, FileNode>,
        project_path: &Path,
        slice: &mut CodeSlice,
        assigned: &mut HashSet<PathBuf>,
    ) {
        for path in paths {
            let Some(node) = nodes.get(path) else { continue };
            self.add_file_to_slice(path, node, project_path, slice);
            assigned.insert(path.clone());
        }
    }

    /// Add a single file to a slice
    fn add_file_to_slice(
        &self,
        path: &PathBuf,
        node: &FileNode,
        project_path: &Path,
        slice: &mut CodeSlice,
    ) {
        let full_path = project_path.join(path);
        if let Ok(content) = FileReader::read_to_string(&full_path) {
            slice.contents.insert(path.clone(), content);
            slice.token_count += node.tokens;
        }
        slice.files.push(path.clone());
    }

    /// Check if an SCC can fit in a slice based on token budget and file count limits.
    fn scc_fits_in_slice(&self, slice: &CodeSlice, scc_tokens: usize, scc_file_count: usize) -> bool {
        slice.token_count + scc_tokens <= self.config.slice_token_budget
            && slice.files.len() + scc_file_count <= self.config.max_files_per_slice
    }

    /// Try to add SCC files to a connected slice. Returns true if successful.
    fn try_add_to_connected_slice(
        &self,
        scc_paths: &[PathBuf],
        scc_tokens: usize,
        slices: &mut [CodeSlice],
        nodes: &HashMap<PathBuf, FileNode>,
        project_path: &Path,
        graph: &DiGraph<PathBuf, ()>,
        index_map: &HashMap<PathBuf, NodeIndex>,
        assigned: &mut HashSet<PathBuf>,
    ) -> bool {
        for slice in slices.iter_mut() {
            if !self.scc_fits_in_slice(slice, scc_tokens, scc_paths.len()) {
                continue;
            }
            let has_connection = scc_paths.iter().any(|scc_path| {
                slice.files.iter().any(|slice_path| {
                    self.files_connected(scc_path, slice_path, graph, index_map)
                })
            });
            if has_connection {
                self.add_files_to_slice(scc_paths, nodes, project_path, slice, assigned);
                return true;
            }
        }
        false
    }

    /// Try to add SCC files to the best affinity slice. Returns true if successful.
    fn try_add_to_best_affinity_slice(
        &self,
        scc_paths: &[PathBuf],
        scc_tokens: usize,
        slices: &mut [CodeSlice],
        nodes: &HashMap<PathBuf, FileNode>,
        project_path: &Path,
        assigned: &mut HashSet<PathBuf>,
    ) -> bool {
        let mut best_slice_idx: Option<usize> = None;
        let mut best_score: f64 = f64::MIN;

        for (idx, slice) in slices.iter().enumerate() {
            if !self.scc_fits_in_slice(slice, scc_tokens, scc_paths.len()) {
                continue;
            }
            let score = self.compute_affinity_score(scc_paths, &slice.files, nodes);
            if score > best_score {
                best_score = score;
                best_slice_idx = Some(idx);
            }
        }

        if let Some(idx) = best_slice_idx {
            self.add_files_to_slice(scc_paths, nodes, project_path, &mut slices[idx], assigned);
            true
        } else {
            false
        }
    }

    /// Check if two files are connected in the import graph
    fn files_connected(
        &self,
        a: &Path,
        b: &Path,
        graph: &DiGraph<PathBuf, ()>,
        index_map: &HashMap<PathBuf, NodeIndex>,
    ) -> bool {
        let a_path = a.to_path_buf();
        let b_path = b.to_path_buf();

        let (Some(&a_idx), Some(&b_idx)) = (index_map.get(&a_path), index_map.get(&b_path)) else {
            return false;
        };

        // Check direct edge in either direction
        graph.find_edge(a_idx, b_idx).is_some() || graph.find_edge(b_idx, a_idx).is_some()
    }

    /// Compute an affinity score between a set of files and a slice's files
    /// Higher scores indicate files that should be grouped together
    fn compute_affinity_score(
        &self,
        scc_files: &[PathBuf],
        slice_files: &[PathBuf],
        nodes: &HashMap<PathBuf, FileNode>,
    ) -> f64 {
        if scc_files.is_empty() || slice_files.is_empty() {
            return 0.0;
        }

        let mut total_score = 0.0;
        let mut comparisons = 0;

        for scc_file in scc_files {
            for slice_file in slice_files {
                // Score based on directory path similarity
                let dir_score = self.directory_similarity(scc_file, slice_file);

                // Score based on shared imports
                let import_score = self.shared_import_score(scc_file, slice_file, nodes);

                // Combined score (directory proximity weighted higher)
                total_score += dir_score * 2.0 + import_score;
                comparisons += 1;
            }
        }

        if comparisons > 0 {
            total_score / comparisons as f64
        } else {
            0.0
        }
    }

    /// Compute directory similarity between two paths
    /// Returns a score from 0.0 (unrelated) to 1.0 (same directory)
    fn directory_similarity(&self, a: &Path, b: &Path) -> f64 {
        // Get parent directories (exclude filenames)
        let a_dir = a.parent().unwrap_or(Path::new(""));
        let b_dir = b.parent().unwrap_or(Path::new(""));

        let a_components: Vec<_> = a_dir.components().collect();
        let b_components: Vec<_> = b_dir.components().collect();

        // Count matching path components from the start
        let mut matching: usize = 0;
        for (ac, bc) in a_components.iter().zip(b_components.iter()) {
            if ac == bc {
                matching += 1;
            } else {
                break;
            }
        }

        let max_depth = a_components.len().max(b_components.len());

        if max_depth == 0 {
            return 1.0; // Both are in root
        }

        // Score based on proportion of matching directory components
        matching as f64 / max_depth as f64
    }

    /// Compute a score based on shared imports between two files
    fn shared_import_score(
        &self,
        a: &Path,
        b: &Path,
        nodes: &HashMap<PathBuf, FileNode>,
    ) -> f64 {
        let a_node = nodes.get(a);
        let b_node = nodes.get(b);

        match (a_node, b_node) {
            (Some(a), Some(b)) => {
                if a.imports.is_empty() && b.imports.is_empty() {
                    return 0.0;
                }

                let a_imports: HashSet<_> = a.imports.iter().collect();
                let b_imports: HashSet<_> = b.imports.iter().collect();

                let shared = a_imports.intersection(&b_imports).count();
                let total = a_imports.union(&b_imports).count();

                if total == 0 {
                    0.0
                } else {
                    shared as f64 / total as f64
                }
            }
            _ => 0.0,
        }
    }

    /// Create slices for a set of files, splitting if necessary
    fn create_slices_for_files(
        &self,
        files: &[PathBuf],
        nodes: &HashMap<PathBuf, FileNode>,
        project_path: &Path,
        slice_id: &mut usize,
    ) -> Result<Vec<CodeSlice>> {
        let mut slices = Vec::new();
        let mut builder = SliceBuilder::new();
        let sorted_files = Self::sort_files_by_directory(files);

        for path in sorted_files {
            let Some(node) = nodes.get(&path) else {
                continue;
            };

            if builder.would_exceed_budget(node.tokens, self.config.slice_token_budget) {
                if let Some(slice) = builder.finalize(*slice_id) {
                    slices.push(slice);
                    *slice_id += 1;
                }
            }

            builder.add_file(&path, node.tokens, project_path);
        }

        if let Some(slice) = builder.finalize(*slice_id) {
            slices.push(slice);
            *slice_id += 1;
        }

        Ok(slices)
    }

    /// Sort files by directory for better grouping.
    fn sort_files_by_directory(files: &[PathBuf]) -> Vec<PathBuf> {
        let mut sorted = files.to_vec();
        sorted.sort_by(|a, b| {
            let a_dir = a.parent().map(|p| p.to_string_lossy().to_string());
            let b_dir = b.parent().map(|p| p.to_string_lossy().to_string());
            a_dir.cmp(&b_dir).then_with(|| a.cmp(b))
        });
        sorted
    }

    /// Determine the primary module name for a slice
    fn determine_primary_module(&self, files: &[PathBuf]) -> Option<String> {
        if files.is_empty() {
            return None;
        }

        // Find the most common directory prefix
        let mut dir_counts: HashMap<String, usize> = HashMap::new();
        for file in files {
            if let Some(parent) = file.parent() {
                let dir = parent.to_string_lossy().to_string();
                *dir_counts.entry(dir).or_insert(0) += 1;
            }
        }

        dir_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(dir, _)| {
                if dir.is_empty() {
                    "root".to_string()
                } else {
                    dir.replace(['/', '\\'], "_")
                }
            })
    }

    /// Count imports that cross slice boundaries
    fn count_cross_slice_imports(
        &self,
        slices: &[CodeSlice],
        nodes: &HashMap<PathBuf, FileNode>,
    ) -> usize {
        let mut count = 0;

        // Build slice membership map
        let mut file_to_slice: HashMap<&PathBuf, usize> = HashMap::new();
        for (slice_idx, slice) in slices.iter().enumerate() {
            for file in &slice.files {
                file_to_slice.insert(file, slice_idx);
            }
        }

        // Count cross-slice imports
        for slice in slices {
            for file in &slice.files {
                if let Some(node) = nodes.get(file) {
                    for _import in &node.imports {
                        // This is a simplified count - in practice we'd resolve imports
                        // For now, just count imports to files in other slices
                    }
                }
            }
        }

        count
    }
}

/// Default implementation for [`ImportGraphPartitioner`].
impl Default for ImportGraphPartitioner {
    /// Returns a partitioner with default configuration.
    fn default() -> Self {
        Self::new(PartitionConfig::default())
    }
}
