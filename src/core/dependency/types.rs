//! Types for dependency analysis.
//!
//! This module contains the core data structures for representing
//! function nodes, dependency metrics, and module graphs.

use std::path::{Path, PathBuf};

/// A function or method node in the dependency graph.
#[derive(Debug, Clone)]
pub struct FunctionNode {
    /// Unique identifier combining file path, name, and line number.
    pub unique_id: String,
    /// Simple function name without namespace.
    pub name: String,
    /// Fully qualified name including parent class/module.
    pub qualified_name: String,
    /// Parent namespace components (class, module names).
    pub namespace: Vec<String>,
    /// Source file path.
    pub file_path: PathBuf,
    /// Starting line number in the source file.
    pub start_line: Option<usize>,
    /// Ending line number in the source file.
    pub end_line: Option<usize>,
    /// Raw function call strings extracted from AST.
    pub calls: Vec<String>,
}

/// Unique key identifying an entity in the dependency graph.
///
/// Used as a hash key for looking up function nodes and their metrics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityKey {
    /// Path to the source file containing this entity.
    pub file_path: PathBuf,
    /// Simple function name.
    pub name: String,
    /// Fully qualified name including namespace.
    pub qualified_name: String,
    /// Line number where the entity starts.
    pub start_line: Option<usize>,
}

/// Factory and query methods for [`EntityKey`].
impl EntityKey {
    /// Creates a new entity key from components.
    pub fn new(
        path: PathBuf,
        name: String,
        qualified_name: String,
        start_line: Option<usize>,
    ) -> Self {
        Self {
            file_path: path,
            name,
            qualified_name,
            start_line,
        }
    }

    /// Creates an entity key from a function node.
    pub fn from_node(node: &FunctionNode) -> Self {
        Self {
            file_path: node.file_path.clone(),
            name: node.name.clone(),
            qualified_name: node.qualified_name.clone(),
            start_line: node.start_line,
        }
    }

    /// Returns the file path for this entity.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Returns the simple name of this entity.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the fully qualified name of this entity.
    pub fn qualified_name(&self) -> &str {
        &self.qualified_name
    }

    /// Returns the starting line number, if known.
    pub fn start_line(&self) -> Option<usize> {
        self.start_line
    }
}

/// Metrics computed for an entity in the dependency graph.
///
/// These metrics help identify coupling issues and architectural concerns.
#[derive(Debug, Clone)]
pub struct DependencyMetrics {
    /// Number of functions that call this function (incoming edges).
    pub fan_in: f64,
    /// Number of functions this function calls (outgoing edges).
    pub fan_out: f64,
    /// Closeness centrality (inverse of average path length to all nodes).
    pub closeness: f64,
    /// Chokepoint score (fan_in × fan_out) indicating coupling bottleneck.
    pub choke_score: f64,
    /// Whether this function is part of a dependency cycle.
    pub in_cycle: bool,
}

/// A chokepoint in the dependency graph (high fan-in × fan-out).
///
/// Chokepoints indicate functions that are coupling bottlenecks,
/// often good candidates for refactoring or special attention.
#[derive(Debug, Clone)]
pub struct Chokepoint {
    /// The function node identified as a chokepoint.
    pub node: FunctionNode,
    /// Chokepoint score (fan_in × fan_out).
    pub score: f64,
}

/// Module-level dependency graph for visualization.
///
/// Aggregates function-level dependencies to the file level for
/// higher-level architectural visualization.
#[derive(Debug, Clone, Default)]
pub struct ModuleGraph {
    /// File-level nodes in the graph.
    pub nodes: Vec<ModuleGraphNode>,
    /// Edges representing cross-file dependencies.
    pub edges: Vec<ModuleGraphEdge>,
}

/// A node in the module-level dependency graph.
///
/// Represents a single source file with aggregated metrics.
#[derive(Debug, Clone)]
pub struct ModuleGraphNode {
    /// Unique identifier (normalized file path).
    pub id: String,
    /// Path to the source file.
    pub path: PathBuf,
    /// Number of functions defined in this file.
    pub functions: usize,
    /// Total incoming dependencies from other files.
    pub fan_in: usize,
    /// Total outgoing dependencies to other files.
    pub fan_out: usize,
    /// Maximum chokepoint score among functions in this file.
    pub chokepoint_score: f64,
    /// Whether any function in this file is in a cycle.
    pub in_cycle: bool,
}

/// An edge in the module-level dependency graph.
///
/// Represents a dependency relationship between two files.
#[derive(Debug, Clone)]
pub struct ModuleGraphEdge {
    /// Index of the source node in the nodes vector.
    pub source: usize,
    /// Index of the target node in the nodes vector.
    pub target: usize,
    /// Number of function calls from source to target.
    pub weight: usize,
}
