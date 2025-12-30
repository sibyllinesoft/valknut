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

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use petgraph::algo::{kosaraju_scc, tarjan_scc};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;

use crate::core::errors::{Result, ValknutError};
use crate::core::file_utils::FileReader;
use crate::detectors::structure::config::ImportStatement;
use crate::lang::adapter_for_file;

#[cfg(test)]
mod tests;

/// Configuration for codebase partitioning
#[derive(Debug, Clone)]
pub struct PartitionConfig {
    /// Maximum tokens per slice (default: 200k)
    pub slice_token_budget: usize,
    /// Minimum files per slice (avoid tiny slices)
    pub min_files_per_slice: usize,
    /// Maximum files per slice (avoid giant slices)
    pub max_files_per_slice: usize,
    /// Whether to include shared dependencies in multiple slices
    pub allow_overlap: bool,
    /// Overlap budget as fraction of slice_token_budget (0.0-0.3)
    pub overlap_fraction: f64,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            slice_token_budget: 200_000,
            min_files_per_slice: 3,
            max_files_per_slice: 100,
            allow_overlap: true,
            overlap_fraction: 0.15,
        }
    }
}

impl PartitionConfig {
    pub fn with_token_budget(mut self, budget: usize) -> Self {
        self.slice_token_budget = budget;
        self
    }
}

/// A coherent slice of the codebase
#[derive(Debug, Clone)]
pub struct CodeSlice {
    /// Unique slice identifier
    pub id: usize,
    /// Files in this slice (paths relative to project root)
    pub files: Vec<PathBuf>,
    /// File contents mapped by path
    pub contents: HashMap<PathBuf, String>,
    /// Estimated token count for this slice
    pub token_count: usize,
    /// Files that are "bridge" dependencies (imported by this slice but belong to another)
    pub bridge_dependencies: Vec<PathBuf>,
    /// Primary module/directory this slice represents (for naming)
    pub primary_module: Option<String>,
}

impl CodeSlice {
    /// Get all file paths including bridge dependencies
    pub fn all_files(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.iter().chain(self.bridge_dependencies.iter())
    }

    /// Check if this slice contains a specific file
    pub fn contains(&self, path: &Path) -> bool {
        self.files.iter().any(|f| f == path) || self.bridge_dependencies.iter().any(|f| f == path)
    }
}

/// Result of partitioning a codebase
#[derive(Debug)]
pub struct PartitionResult {
    /// The computed slices
    pub slices: Vec<CodeSlice>,
    /// Files that couldn't be assigned to any slice
    pub unassigned: Vec<PathBuf>,
    /// Import graph statistics
    pub stats: PartitionStats,
}

/// Statistics about the partitioning
#[derive(Debug, Clone)]
pub struct PartitionStats {
    /// Total files processed
    pub total_files: usize,
    /// Total tokens across all files
    pub total_tokens: usize,
    /// Number of slices created
    pub slice_count: usize,
    /// Number of strongly connected components found
    pub scc_count: usize,
    /// Largest SCC size (files)
    pub largest_scc: usize,
    /// Number of cross-slice imports
    pub cross_slice_imports: usize,
}

/// Node in the file import graph
#[derive(Debug, Clone)]
struct FileNode {
    path: PathBuf,
    tokens: usize,
    imports: Vec<String>,
}

/// Import graph partitioner
pub struct ImportGraphPartitioner {
    config: PartitionConfig,
}

impl ImportGraphPartitioner {
    pub fn new(config: PartitionConfig) -> Self {
        Self { config }
    }

    /// Partition a codebase into coherent slices
    pub fn partition(&self, project_path: &Path, files: &[PathBuf]) -> Result<PartitionResult> {
        if files.is_empty() {
            return Ok(PartitionResult {
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
            });
        }

        // Build file nodes with import information
        let file_nodes = self.build_file_nodes(project_path, files)?;
        if file_nodes.is_empty() {
            return Ok(PartitionResult {
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
            });
        }

        let total_tokens: usize = file_nodes.values().map(|n| n.tokens).sum();

        // Build import graph
        let (graph, index_map, reverse_map) = self.build_import_graph(&file_nodes, project_path);

        // Find strongly connected components
        let sccs = tarjan_scc(&graph);
        let scc_count = sccs.len();
        let largest_scc = sccs.iter().map(|scc| scc.len()).max().unwrap_or(0);

        // Partition based on SCCs and token budget
        let (slices, unassigned) = self.partition_by_budget(
            &file_nodes,
            &graph,
            &index_map,
            &reverse_map,
            &sccs,
            project_path,
        )?;

        // Count cross-slice imports
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
        let module_map = self.build_module_map(nodes, project_path);

        // Add edges for imports
        for (path, node) in nodes {
            let Some(&from_idx) = index_map.get(path) else {
                continue;
            };

            for import in &node.imports {
                // Try to resolve import to a file in our codebase
                if let Some(target_path) = self.resolve_import(import, path, &module_map) {
                    if let Some(&to_idx) = index_map.get(&target_path) {
                        if from_idx != to_idx {
                            graph.add_edge(from_idx, to_idx, ());
                        }
                    }
                }
            }
        }

        (graph, index_map, reverse_map)
    }

    /// Build mapping from module names to file paths
    /// Creates multiple keys for each file to enable flexible resolution
    fn build_module_map(
        &self,
        nodes: &HashMap<PathBuf, FileNode>,
        _project_path: &Path,
    ) -> HashMap<String, PathBuf> {
        let mut map = HashMap::new();

        for path in nodes.keys() {
            let path_str = path.to_string_lossy();

            // Get the path without extension
            let without_ext = self.strip_extension(&path_str);

            // For Rust: handle mod.rs specially
            // e.g., "src/core/pipeline/mod.rs" -> "core::pipeline" and "core.pipeline"
            if path_str.ends_with("mod.rs") {
                if let Some(parent) = path.parent() {
                    let parent_str = parent.to_string_lossy();
                    let rust_module = self.path_to_rust_module(&parent_str);
                    let dot_module = rust_module.replace("::", ".");
                    map.insert(rust_module.clone(), path.clone());
                    map.insert(dot_module.clone(), path.clone());
                    // Also add crate:: prefixed version
                    map.insert(format!("crate::{}", rust_module), path.clone());
                    map.insert(format!("crate.{}", dot_module), path.clone());
                }
            }

            // Standard module path: "src/core/config.rs" -> "core::config", "core.config", "config"
            let rust_module = self.path_to_rust_module(&without_ext);
            let dot_module = rust_module.replace("::", ".");
            map.insert(rust_module.clone(), path.clone());
            map.insert(dot_module.clone(), path.clone());

            // Add crate:: prefixed version
            map.insert(format!("crate::{}", rust_module), path.clone());
            map.insert(format!("crate.{}", dot_module), path.clone());

            // Add just the file stem for simple resolution
            if let Some(stem) = path.file_stem() {
                let stem_str = stem.to_string_lossy().to_string();
                if stem_str != "mod" && stem_str != "lib" && stem_str != "main" {
                    map.insert(stem_str, path.clone());
                }
            }

            // For TypeScript/JavaScript: handle relative paths
            // e.g., "./foo" or "../bar"
            if path_str.ends_with(".ts")
                || path_str.ends_with(".tsx")
                || path_str.ends_with(".js")
                || path_str.ends_with(".jsx")
            {
                // Add the path without src prefix
                let no_src = without_ext
                    .strip_prefix("src/")
                    .unwrap_or(&without_ext);
                map.insert(format!("./{}", no_src), path.clone());
            }

            // For Python: handle dot-separated module paths
            if path_str.ends_with(".py") {
                let py_module = without_ext
                    .replace('/', ".")
                    .replace('\\', ".");
                map.insert(py_module, path.clone());
            }

            // For Go: the import path is typically the full package path
            if path_str.ends_with(".go") {
                // Just use the directory as the package
                if let Some(parent) = path.parent() {
                    let parent_str = parent.to_string_lossy().to_string();
                    map.insert(parent_str, path.clone());
                }
            }
        }

        map
    }

    /// Strip file extension
    fn strip_extension<'a>(&self, path: &'a str) -> String {
        let extensions = [".rs", ".py", ".js", ".ts", ".tsx", ".jsx", ".go"];
        for ext in extensions {
            if let Some(stripped) = path.strip_suffix(ext) {
                return stripped.to_string();
            }
        }
        path.to_string()
    }

    /// Convert a file path to a Rust module path
    /// e.g., "src/core/config" -> "core::config"
    fn path_to_rust_module(&self, path: &str) -> String {
        path.strip_prefix("src/")
            .or_else(|| path.strip_prefix("src\\"))
            .unwrap_or(path)
            .replace(['/', '\\'], "::")
    }

    /// Try to resolve an import string to a file path
    fn resolve_import(
        &self,
        import: &str,
        from_file: &Path,
        module_map: &HashMap<String, PathBuf>,
    ) -> Option<PathBuf> {
        // Handle Rust special prefixes
        let normalized = if import.starts_with("crate::") {
            // crate:: refers to the current crate root
            import.to_string()
        } else if import.starts_with("super::") {
            // super:: refers to parent module - resolve relative to from_file
            if let Some(parent) = from_file.parent() {
                if let Some(grandparent) = parent.parent() {
                    let rest = import.strip_prefix("super::").unwrap();
                    let resolved = format!(
                        "{}::{}",
                        self.path_to_rust_module(&grandparent.to_string_lossy()),
                        rest
                    );
                    resolved
                } else {
                    import.to_string()
                }
            } else {
                import.to_string()
            }
        } else if import.starts_with("self::") {
            // self:: refers to current module
            if let Some(parent) = from_file.parent() {
                let rest = import.strip_prefix("self::").unwrap();
                format!(
                    "{}::{}",
                    self.path_to_rust_module(&parent.to_string_lossy()),
                    rest
                )
            } else {
                import.to_string()
            }
        } else {
            import.to_string()
        };

        // Normalize separators
        let normalized = normalized
            .replace("::", ".")
            .replace('/', ".")
            .trim_start_matches('.')
            .to_string();

        // Try exact match first
        if let Some(path) = module_map.get(&normalized) {
            return Some(path.clone());
        }

        // Try with :: separator
        let rust_style = normalized.replace('.', "::");
        if let Some(path) = module_map.get(&rust_style) {
            return Some(path.clone());
        }

        // Try progressively shorter prefixes
        let parts: Vec<&str> = normalized.split('.').collect();
        for end in (1..=parts.len()).rev() {
            let prefix = parts[..end].join(".");
            if let Some(path) = module_map.get(&prefix) {
                return Some(path.clone());
            }
            let rust_prefix = parts[..end].join("::");
            if let Some(path) = module_map.get(&rust_prefix) {
                return Some(path.clone());
            }
        }

        // For mod declarations in Rust, try resolving relative to the file
        // e.g., `mod foo;` in `src/lib.rs` -> `src/foo.rs` or `src/foo/mod.rs`
        if parts.len() == 1 {
            if let Some(parent) = from_file.parent() {
                let parent_str = parent.to_string_lossy();
                let mod_name = parts[0];

                // Try sibling file: parent/mod_name.rs
                let sibling_path = format!("{}.{}", parent_str, mod_name);
                if let Some(path) = module_map.get(&sibling_path) {
                    return Some(path.clone());
                }

                // Try nested module: parent/mod_name/mod.rs -> mapped as parent::mod_name
                let nested_path = format!("{}::{}", self.path_to_rust_module(&parent_str), mod_name);
                if let Some(path) = module_map.get(&nested_path) {
                    return Some(path.clone());
                }
            }
        }

        // Try just the last component as a fallback
        if let Some(last) = parts.last() {
            if let Some(path) = module_map.get(*last) {
                return Some(path.clone());
            }
        }

        None
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
                for slice in new_slices {
                    for path in &slice.files {
                        assigned.insert(path.clone());
                    }
                    slices.push(slice);
                }
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
            if let Some(node) = nodes.get(path) {
                let full_path = project_path.join(path);
                if let Ok(content) = FileReader::read_to_string(&full_path) {
                    slice.contents.insert(path.clone(), content);
                    slice.token_count += node.tokens;
                }
                slice.files.push(path.clone());
                assigned.insert(path.clone());
            }
        }
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
        let mut current_files = Vec::new();
        let mut current_contents = HashMap::new();
        let mut current_tokens = 0;

        // Sort files by directory for better grouping
        let mut sorted_files = files.to_vec();
        sorted_files.sort_by(|a, b| {
            let a_dir = a.parent().map(|p| p.to_string_lossy().to_string());
            let b_dir = b.parent().map(|p| p.to_string_lossy().to_string());
            a_dir.cmp(&b_dir).then_with(|| a.cmp(b))
        });

        for path in sorted_files {
            let Some(node) = nodes.get(&path) else {
                continue;
            };

            // Check if adding this file would exceed budget
            if current_tokens + node.tokens > self.config.slice_token_budget
                && !current_files.is_empty()
            {
                // Finalize current slice
                slices.push(CodeSlice {
                    id: *slice_id,
                    files: current_files.clone(),
                    contents: current_contents.clone(),
                    token_count: current_tokens,
                    bridge_dependencies: vec![],
                    primary_module: None,
                });
                *slice_id += 1;
                current_files.clear();
                current_contents.clear();
                current_tokens = 0;
            }

            // Add file to current slice
            let full_path = project_path.join(&path);
            if let Ok(content) = FileReader::read_to_string(&full_path) {
                current_contents.insert(path.clone(), content);
                current_tokens += node.tokens;
                current_files.push(path);
            }
        }

        // Don't forget the last slice
        if !current_files.is_empty() {
            slices.push(CodeSlice {
                id: *slice_id,
                files: current_files,
                contents: current_contents,
                token_count: current_tokens,
                bridge_dependencies: vec![],
                primary_module: None,
            });
            *slice_id += 1;
        }

        Ok(slices)
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

impl Default for ImportGraphPartitioner {
    fn default() -> Self {
        Self::new(PartitionConfig::default())
    }
}
