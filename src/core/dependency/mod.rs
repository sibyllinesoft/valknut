//! Dependency analysis for function-level call graphs.
//!
//! This module provides analysis of function dependencies within a codebase,
//! including:
//!
//! - **Call graph construction**: Builds directed graphs of function calls
//! - **Cycle detection**: Identifies strongly connected components using Kosaraju's algorithm
//! - **Chokepoint analysis**: Finds functions with high fan-in × fan-out products
//! - **Closeness centrality**: Measures how central each function is in the call graph
//! - **Module graph**: Aggregates function-level data to file-level visualization
//!
//! # Example
//!
//! ```ignore
//! let files = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")];
//! let analysis = ProjectDependencyAnalysis::analyze(&files)?;
//!
//! // Check for dependency cycles
//! for cycle in analysis.cycles() {
//!     println!("Cycle detected: {:?}", cycle);
//! }
//!
//! // Find chokepoint functions
//! for chokepoint in analysis.chokepoints() {
//!     println!("{} (score: {})", chokepoint.node.name, chokepoint.score);
//! }
//! ```

mod call_resolution;
pub mod types;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;

use crate::core::errors::Result;
use crate::core::file_utils::FileReader;
use crate::lang::{adapter_for_file, EntityKind, ParseIndex, ParsedEntity};

pub use types::{
    Chokepoint, DependencyMetrics, EntityKey, FunctionNode, ModuleGraph, ModuleGraphEdge,
    ModuleGraphNode,
};
use call_resolution::{select_target, CallIdentifier};

/// Results of dependency analysis for a project.
///
/// Contains the complete call graph analysis including function nodes,
/// computed metrics, detected cycles, and identified chokepoints.
#[derive(Debug, Default)]
pub struct ProjectDependencyAnalysis {
    /// All function nodes indexed by their unique key.
    nodes: HashMap<EntityKey, FunctionNode>,
    /// Computed dependency metrics for each function.
    metrics: HashMap<EntityKey, DependencyMetrics>,
    /// Detected dependency cycles (strongly connected components).
    cycles: Vec<Vec<FunctionNode>>,
    /// Functions identified as chokepoints (high coupling).
    chokepoints: Vec<Chokepoint>,
    /// Module-level aggregation of the dependency graph.
    module_graph: ModuleGraph,
}

/// Analysis and query methods for [`ProjectDependencyAnalysis`].
impl ProjectDependencyAnalysis {
    /// Creates an empty analysis result with no data.
    pub fn empty() -> Self {
        Self {
            nodes: HashMap::new(),
            metrics: HashMap::new(),
            cycles: Vec::new(),
            chokepoints: Vec::new(),
            module_graph: ModuleGraph::default(),
        }
    }

    /// Analyzes the given files to build a complete dependency graph.
    ///
    /// Parses each file to extract function definitions and their calls,
    /// then constructs a graph and computes metrics including fan-in,
    /// fan-out, closeness centrality, cycle membership, and chokepoint scores.
    pub fn analyze(files: &[PathBuf]) -> Result<Self> {
        let mut nodes = HashMap::with_capacity(files.len() * 10); // Estimate ~10 functions per file

        for path in files {
            let canonical = canonicalize_path(path);
            let mut functions = collect_function_nodes(&canonical)?;
            if functions.is_empty() {
                continue;
            }

            for function in functions.drain(..) {
                let key = EntityKey::from_node(&function);
                nodes.insert(key, function);
            }
        }

        if nodes.is_empty() {
            return Ok(Self::empty());
        }

        let (graph, index_map) = build_graph(&nodes);
        let mut metrics = compute_metrics(&graph, &index_map, &nodes);
        let (cycles, cycle_members) = identify_cycles(&graph, &index_map, &nodes);
        mark_cycle_members(&mut metrics, &cycle_members);
        let chokepoints = compute_chokepoints(&metrics, &nodes, 10);
        let module_graph = build_module_graph(&graph, &nodes, &metrics);

        Ok(Self {
            nodes,
            metrics,
            cycles,
            chokepoints,
            module_graph,
        })
    }

    /// Returns true if no function nodes were found.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Retrieves dependency metrics for a specific entity.
    ///
    /// Performs exact key lookup first, then falls back to fuzzy matching
    /// by file path and name (case-insensitive) if exact match fails.
    pub fn metrics_for(&self, key: &EntityKey) -> Option<&DependencyMetrics> {
        if let Some(metrics) = self.metrics.get(key) {
            return Some(metrics);
        }

        self.metrics.iter().find_map(|(candidate, metrics)| {
            if candidate.file_path == key.file_path
                && (candidate
                    .qualified_name
                    .eq_ignore_ascii_case(key.qualified_name())
                    || candidate.name.eq_ignore_ascii_case(key.name()))
            {
                Some(metrics)
            } else {
                None
            }
        })
    }

    /// Returns detected dependency cycles as groups of function nodes.
    ///
    /// Each cycle represents a strongly connected component where functions
    /// mutually depend on each other, potentially indicating tight coupling.
    pub fn cycles(&self) -> &[Vec<FunctionNode>] {
        &self.cycles
    }

    /// Returns the top chokepoint functions ranked by score.
    ///
    /// Chokepoints are functions with high fan-in × fan-out products,
    /// indicating they are bottlenecks in the dependency graph.
    pub fn chokepoints(&self) -> &[Chokepoint] {
        &self.chokepoints
    }

    /// Returns the module-level dependency graph for visualization.
    ///
    /// Aggregates function-level dependencies to the file level,
    /// useful for understanding high-level architecture.
    pub fn module_graph(&self) -> &ModuleGraph {
        &self.module_graph
    }

    /// Iterates over all entity keys and their dependency metrics.
    pub fn metrics_iter(&self) -> impl Iterator<Item = (&EntityKey, &DependencyMetrics)> {
        self.metrics.iter()
    }
}

/// Parses a file and extracts function nodes with their call information.
fn collect_function_nodes(path: &Path) -> Result<Vec<FunctionNode>> {
    let mut adapter = adapter_for_file(path)?;
    let source = FileReader::read_to_string(path)?;

    let path_str = path.to_string_lossy().to_string();
    let parse_index = adapter.parse_source(&source, &path_str)?;

    let mut functions = Vec::with_capacity(parse_index.entities.len()); // Pre-allocate based on parsed entities

    for entity in parse_index.entities.values() {
        if !matches!(entity.kind, EntityKind::Function | EntityKind::Method) {
            continue;
        }

        let file_path = canonicalize_path(Path::new(&entity.location.file_path));
        let start_line = Some(entity.location.start_line);
        let end_line = Some(entity.location.end_line);

        let namespace = build_namespace(entity, &parse_index);
        let qualified_name = if namespace.is_empty() {
            entity.name.clone()
        } else {
            format!("{}::{}", namespace.join("::"), entity.name)
        };

        let calls = entity
            .metadata
            .get("function_calls")
            .and_then(|value| value.as_array())
            .map(|array| {
                array
                    .iter()
                    .filter_map(|value| value.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let unique_id = format!(
            "{}::{}:{}",
            file_path.display(),
            entity.name,
            start_line.unwrap_or_default()
        );

        functions.push(FunctionNode {
            unique_id,
            name: entity.name.clone(),
            qualified_name,
            namespace,
            file_path,
            start_line,
            end_line,
            calls,
        });
    }

    Ok(functions)
}

/// Builds the namespace path by traversing parent entities.
fn build_namespace(entity: &ParsedEntity, index: &ParseIndex) -> Vec<String> {
    let mut namespace = Vec::with_capacity(3); // Typical nesting depth is 1-3 levels
    let mut current = entity.parent.clone();

    while let Some(parent_id) = current {
        if let Some(parent) = index.entities.get(&parent_id) {
            match parent.kind {
                EntityKind::Class
                | EntityKind::Interface
                | EntityKind::Struct
                | EntityKind::Enum
                | EntityKind::Module => namespace.push(parent.name.clone()),
                _ => {}
            }
            current = parent.parent.clone();
        } else {
            break;
        }
    }

    namespace.reverse();
    namespace
}

type DependencyGraph = Graph<EntityKey, (), petgraph::Directed>;
type IndexMap = HashMap<EntityKey, NodeIndex>;

/// Find the matching target key for a call identifier by searching candidate names.
fn find_target_for_call<'a>(
    call_id: &CallIdentifier,
    name_lookup: &'a HashMap<String, Vec<&'a EntityKey>>,
    node: &FunctionNode,
    nodes: &HashMap<EntityKey, FunctionNode>,
) -> Option<&'a EntityKey> {
    let candidate_keys = call_id.candidate_keys();
    for candidate_name in &candidate_keys {
        let Some(candidates) = name_lookup.get(candidate_name) else {
            continue;
        };
        if let Some(target_key) = select_target(candidates.as_slice(), node, nodes, call_id, &candidate_keys) {
            return Some(target_key);
        }
    }
    None
}

/// Try to add an edge from source to target if target exists and not already connected.
fn try_add_edge(
    graph: &mut DependencyGraph,
    index_map: &IndexMap,
    seen_targets: &mut HashSet<NodeIndex>,
    from_index: NodeIndex,
    target_key: &EntityKey,
) {
    let Some(&target_index) = index_map.get(target_key) else {
        return;
    };
    if seen_targets.insert(target_index) {
        graph.add_edge(from_index, target_index, ());
    }
}

/// Builds a directed graph from function nodes and their call relationships.
fn build_graph(nodes: &HashMap<EntityKey, FunctionNode>) -> (DependencyGraph, IndexMap) {
    let node_count = nodes.len();
    let mut graph = DependencyGraph::with_capacity(node_count, node_count * 2);
    let mut index_map = HashMap::with_capacity(node_count);

    for key in nodes.keys() {
        let index = graph.add_node(key.clone());
        index_map.insert(key.clone(), index);
    }

    let name_lookup = build_name_lookup(nodes);

    for (key, node) in nodes {
        let Some(&from_index) = index_map.get(key) else {
            continue;
        };

        let mut seen_targets = HashSet::new();

        for raw_call in &node.calls {
            let Some(call_id) = CallIdentifier::parse(raw_call) else {
                continue;
            };
            if let Some(target_key) = find_target_for_call(&call_id, &name_lookup, node, nodes) {
                try_add_edge(&mut graph, &index_map, &mut seen_targets, from_index, target_key);
            }
        }
    }

    (graph, index_map)
}

/// Builds a lookup table from function names to their entity keys.
fn build_name_lookup<'a>(
    nodes: &'a HashMap<EntityKey, FunctionNode>,
) -> HashMap<String, Vec<&'a EntityKey>> {
    let mut map: HashMap<String, Vec<&EntityKey>> = HashMap::with_capacity(nodes.len());

    for (key, node) in nodes {
        map.entry(node.name.to_lowercase()).or_default().push(key);

        let qualified_lower = node.qualified_name.to_lowercase();
        map.entry(qualified_lower.clone()).or_default().push(key);

        let mut segments: Vec<String> = node
            .qualified_name
            .split("::")
            .map(|segment| segment.to_lowercase())
            .collect();
        while segments.len() > 1 {
            segments.remove(0);
            map.entry(segments.join("::")).or_default().push(key);
        }
    }

    for values in map.values_mut() {
        values.sort_by(|a, b| {
            let path_cmp = a.file_path().cmp(b.file_path());
            if path_cmp != std::cmp::Ordering::Equal {
                path_cmp
            } else {
                a.start_line().cmp(&b.start_line())
            }
        });
        values.dedup();
    }

    map
}

/// Computes dependency metrics (fan-in, fan-out, closeness) for all nodes.
fn compute_metrics(
    graph: &DependencyGraph,
    index_map: &IndexMap,
    nodes: &HashMap<EntityKey, FunctionNode>,
) -> HashMap<EntityKey, DependencyMetrics> {
    let mut metrics = HashMap::with_capacity(index_map.len());

    for (key, &index) in index_map {
        let fan_out = graph.neighbors_directed(index, Direction::Outgoing).count() as f64;
        let fan_in = graph.neighbors_directed(index, Direction::Incoming).count() as f64;
        let closeness = compute_closeness(graph, index);
        let choke_score = fan_in * fan_out;

        metrics.insert(
            key.clone(),
            DependencyMetrics {
                fan_in,
                fan_out,
                closeness,
                choke_score,
                in_cycle: false,
            },
        );
    }

    for key in nodes.keys() {
        metrics.entry(key.clone()).or_insert(DependencyMetrics {
            fan_in: 0.0,
            fan_out: 0.0,
            closeness: 0.0,
            choke_score: 0.0,
            in_cycle: false,
        });
    }

    metrics
}

/// Computes closeness centrality for a node using BFS traversal.
fn compute_closeness(graph: &DependencyGraph, start: NodeIndex) -> f64 {
    let mut visited: HashMap<NodeIndex, usize> = HashMap::with_capacity(16); // Typical BFS explores ~10-20 nodes
    let mut queue = VecDeque::new();

    visited.insert(start, 0);
    queue.push_back(start);

    while let Some(node) = queue.pop_front() {
        let depth = visited[&node] + 1;
        for neighbor in graph.neighbors_undirected(node) {
            if !visited.contains_key(&neighbor) {
                visited.insert(neighbor, depth);
                queue.push_back(neighbor);
            }
        }
    }

    if visited.len() <= 1 {
        return 0.0;
    }

    let total_distance: usize = visited.values().sum();
    if total_distance == 0 {
        return 0.0;
    }

    ((visited.len() - 1) as f64) / (total_distance as f64)
}

/// Extract cycle nodes from a multi-node strongly connected component.
fn extract_multi_node_cycle(
    component: &[NodeIndex],
    graph: &DependencyGraph,
    nodes: &HashMap<EntityKey, FunctionNode>,
    members: &mut HashSet<EntityKey>,
) -> Vec<FunctionNode> {
    let mut cycle_nodes = Vec::with_capacity(component.len());
    for &index in component {
        let Some(key) = graph.node_weight(index) else { continue };
        let Some(node) = nodes.get(key) else { continue };
        cycle_nodes.push(node.clone());
        members.insert(key.clone());
    }
    cycle_nodes
}

/// Check if a single-node component has a self-loop and return the cycle if so.
fn check_self_loop_cycle(
    index: NodeIndex,
    graph: &DependencyGraph,
    nodes: &HashMap<EntityKey, FunctionNode>,
    members: &mut HashSet<EntityKey>,
) -> Option<Vec<FunctionNode>> {
    if graph.find_edge(index, index).is_none() {
        return None;
    }
    let key = graph.node_weight(index)?;
    let node = nodes.get(key)?;
    members.insert(key.clone());
    Some(vec![node.clone()])
}

/// Identifies dependency cycles using Kosaraju's algorithm for SCCs.
fn identify_cycles(
    graph: &DependencyGraph,
    _index_map: &IndexMap,
    nodes: &HashMap<EntityKey, FunctionNode>,
) -> (Vec<Vec<FunctionNode>>, HashSet<EntityKey>) {
    let sccs = kosaraju_scc(graph);

    let mut cycles = Vec::with_capacity(sccs.len() / 4);
    let mut members = HashSet::with_capacity(nodes.len() / 10);

    for component in sccs {
        if component.len() > 1 {
            let cycle_nodes = extract_multi_node_cycle(&component, graph, nodes, &mut members);
            cycles.push(cycle_nodes);
        } else if let Some(&index) = component.first() {
            if let Some(cycle) = check_self_loop_cycle(index, graph, nodes, &mut members) {
                cycles.push(cycle);
            }
        }
    }

    (cycles, members)
}

/// Marks entities that are part of dependency cycles in the metrics map.
fn mark_cycle_members(
    metrics: &mut HashMap<EntityKey, DependencyMetrics>,
    members: &HashSet<EntityKey>,
) {
    for member in members {
        if let Some(metric) = metrics.get_mut(member) {
            metric.in_cycle = true;
        }
    }
}

/// Computes the top chokepoint functions ranked by coupling score.
fn compute_chokepoints(
    metrics: &HashMap<EntityKey, DependencyMetrics>,
    nodes: &HashMap<EntityKey, FunctionNode>,
    limit: usize,
) -> Vec<Chokepoint> {
    let mut entries: Vec<(EntityKey, &DependencyMetrics)> = metrics
        .iter()
        .map(|(key, value)| (key.clone(), value))
        .collect();
    entries.sort_by(|a, b| b.1.choke_score.partial_cmp(&a.1.choke_score).unwrap());

    entries
        .into_iter()
        .filter_map(|(key, metrics)| {
            if metrics.choke_score <= 0.0 {
                None
            } else {
                nodes.get(&key).map(|node| Chokepoint {
                    node: node.clone(),
                    score: metrics.choke_score,
                })
            }
        })
        .take(limit)
        .collect()
}

/// Aggregates function-level dependencies into a module-level graph.
fn build_module_graph(
    graph: &DependencyGraph,
    nodes: &HashMap<EntityKey, FunctionNode>,
    metrics: &HashMap<EntityKey, DependencyMetrics>,
) -> ModuleGraph {
    if nodes.is_empty() {
        return ModuleGraph::default();
    }

    /// Temporary aggregation structure for module-level metrics.
    #[derive(Clone)]
    struct ModuleAgg {
        path: String,
        functions: usize,
        fan_in: usize,
        fan_out: usize,
        chokepoint_score: f64,
        in_cycle: bool,
    }

    let mut modules: HashMap<String, ModuleAgg> = HashMap::with_capacity(nodes.len() / 2 + 1);

    for (key, func) in nodes {
        let path_str = normalize_path_string(&func.file_path);
        let entry = modules.entry(path_str.clone()).or_insert(ModuleAgg {
            path: path_str.clone(),
            functions: 0,
            fan_in: 0,
            fan_out: 0,
            chokepoint_score: 0.0,
            in_cycle: false,
        });

        entry.functions += 1;

        if let Some(metric) = metrics.get(key) {
            if metric.choke_score > entry.chokepoint_score {
                entry.chokepoint_score = metric.choke_score;
            }
            entry.in_cycle |= metric.in_cycle;
        }
    }

    let mut edge_weights: HashMap<(String, String), usize> = HashMap::new();
    for edge in graph.edge_references() {
        let Some(src_key) = graph.node_weight(edge.source()) else {
            continue;
        };
        let Some(dst_key) = graph.node_weight(edge.target()) else {
            continue;
        };
        let Some(src_node) = nodes.get(src_key) else {
            continue;
        };
        let Some(dst_node) = nodes.get(dst_key) else {
            continue;
        };

        let src_path = normalize_path_string(&src_node.file_path);
        let dst_path = normalize_path_string(&dst_node.file_path);

        *edge_weights.entry((src_path, dst_path)).or_insert(0) += 1;
    }

    let mut module_vec: Vec<ModuleAgg> = modules.into_values().collect();
    module_vec.sort_by(|a, b| a.path.cmp(&b.path));

    let mut index_lookup: HashMap<String, usize> = HashMap::with_capacity(module_vec.len());
    for (idx, module) in module_vec.iter().enumerate() {
        index_lookup.insert(module.path.clone(), idx);
    }

    let mut edges: Vec<ModuleGraphEdge> = Vec::with_capacity(edge_weights.len());
    for ((src_path, dst_path), weight) in edge_weights {
        if let (Some(&source), Some(&target)) =
            (index_lookup.get(&src_path), index_lookup.get(&dst_path))
        {
            edges.push(ModuleGraphEdge {
                source,
                target,
                weight,
            });
            if let Some(module) = module_vec.get_mut(source) {
                module.fan_out = module.fan_out.saturating_add(weight);
            }
            if let Some(module) = module_vec.get_mut(target) {
                module.fan_in = module.fan_in.saturating_add(weight);
            }
        }
    }

    let nodes = module_vec
        .into_iter()
        .map(|module| ModuleGraphNode {
            id: module.path.clone(),
            path: PathBuf::from(&module.path),
            functions: module.functions,
            fan_in: module.fan_in,
            fan_out: module.fan_out,
            chokepoint_score: module.chokepoint_score,
            in_cycle: module.in_cycle,
        })
        .collect();

    ModuleGraph { nodes, edges }
}

/// Normalizes a path to a forward-slash string for consistent keys.
fn normalize_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Normalizes a path for consistent dependency tracking.
///
/// Preserves relative paths when the file exists to avoid absolute path
/// display issues. Only canonicalizes when necessary for existence checking,
/// and attempts to convert back to relative paths when possible.
pub fn canonicalize_path(path: &Path) -> PathBuf {
    // Preserve relative paths to avoid absolute path display issues
    // Only canonicalize for existence checking if path doesn't exist as-is
    if path.exists() {
        path.to_path_buf()
    } else {
        match path.canonicalize() {
            Ok(canonical) => {
                // Try to convert back to relative if possible
                if let Ok(current_dir) = std::env::current_dir() {
                    canonical
                        .strip_prefix(&current_dir)
                        .map(|p| p.to_path_buf())
                        .unwrap_or(canonical)
                } else {
                    canonical
                }
            }
            Err(_) => path.to_path_buf(),
        }
    }
}
