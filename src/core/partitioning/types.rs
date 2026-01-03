//! Types for import graph partitioning.
//!
//! This module contains configuration, result, and internal data types
//! used by the partitioner.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

/// Default implementation for [`PartitionConfig`].
impl Default for PartitionConfig {
    /// Returns the default partitioning configuration.
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

/// Builder methods for [`PartitionConfig`].
impl PartitionConfig {
    /// Sets the token budget and returns the modified config.
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

/// Query methods for [`CodeSlice`].
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
pub(crate) struct FileNode {
    pub(crate) path: PathBuf,
    pub(crate) tokens: usize,
    pub(crate) imports: Vec<String>,
}
