use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::Direction;
use tracing::{debug, warn};

use crate::core::errors::Result;
use crate::lang::{adapter_for_file, EntityKind};

#[derive(Debug, Clone)]
pub struct FunctionNode {
    pub unique_id: String,
    pub name: String,
    pub file_path: PathBuf,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub calls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityKey {
    file_path: PathBuf,
    name: String,
    start_line: Option<usize>,
}

impl EntityKey {
    pub fn new(path: PathBuf, name: String, start_line: Option<usize>) -> Self {
        Self {
            file_path: path,
            name,
            start_line,
        }
    }

    pub fn from_node(node: &FunctionNode) -> Self {
        Self {
            file_path: node.file_path.clone(),
            name: node.name.clone(),
            start_line: node.start_line,
        }
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start_line(&self) -> Option<usize> {
        self.start_line
    }
}

#[derive(Debug, Clone)]
pub struct DependencyMetrics {
    pub fan_in: f64,
    pub fan_out: f64,
    pub closeness: f64,
    pub choke_score: f64,
    pub in_cycle: bool,
}

#[derive(Debug, Clone)]
pub struct Chokepoint {
    pub node: FunctionNode,
    pub score: f64,
}

#[derive(Debug, Default)]
pub struct ProjectDependencyAnalysis {
    nodes: HashMap<EntityKey, FunctionNode>,
    metrics: HashMap<EntityKey, DependencyMetrics>,
    cycles: Vec<Vec<FunctionNode>>,
    chokepoints: Vec<Chokepoint>,
}

impl ProjectDependencyAnalysis {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn analyze(files: &[PathBuf]) -> Result<Self> {
        let mut nodes = HashMap::new();

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

        Ok(Self {
            nodes,
            metrics,
            cycles,
            chokepoints,
        })
    }

    pub fn metrics_for(&self, key: &EntityKey) -> Option<&DependencyMetrics> {
        if let Some(metrics) = self.metrics.get(key) {
            return Some(metrics);
        }

        self.metrics.iter().find_map(|(candidate, metrics)| {
            if candidate.file_path == key.file_path && candidate.name == key.name {
                Some(metrics)
            } else {
                None
            }
        })
    }

    pub fn cycles(&self) -> &[Vec<FunctionNode>] {
        &self.cycles
    }

    pub fn chokepoints(&self) -> &[Chokepoint] {
        &self.chokepoints
    }
}

fn collect_function_nodes(path: &Path) -> Result<Vec<FunctionNode>> {
    let mut adapter = match adapter_for_file(path) {
        Ok(adapter) => adapter,
        Err(err) => {
            debug!(
                "Skipping dependency analysis for {}: {}",
                path.display(),
                err
            );
            return Ok(Vec::new());
        }
    };

    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => {
            warn!(
                "Failed to read file {} for dependency analysis: {}",
                path.display(),
                err
            );
            return Ok(Vec::new());
        }
    };

    let path_str = path.to_string_lossy().to_string();
    let parse_index = match adapter.parse_source(&source, &path_str) {
        Ok(index) => index,
        Err(err) => {
            warn!(
                "Failed to parse {} for dependency analysis: {}",
                path.display(),
                err
            );
            return Ok(Vec::new());
        }
    };

    let mut functions = Vec::new();

    for entity in parse_index.entities.values() {
        if !matches!(entity.kind, EntityKind::Function | EntityKind::Method) {
            continue;
        }

        let file_path = canonicalize_path(Path::new(&entity.location.file_path));
        let start_line = Some(entity.location.start_line);
        let end_line = Some(entity.location.end_line);

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
            file_path,
            start_line,
            end_line,
            calls,
        });
    }

    Ok(functions)
}

type DependencyGraph = Graph<EntityKey, (), petgraph::Directed>;
type IndexMap = HashMap<EntityKey, NodeIndex>;

fn build_graph(nodes: &HashMap<EntityKey, FunctionNode>) -> (DependencyGraph, IndexMap) {
    let mut graph = DependencyGraph::new();
    let mut index_map = HashMap::new();

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
            if let Some(call_name) = normalize_call_name(raw_call) {
                let call_key = call_name.to_lowercase();
                if let Some(candidates) = name_lookup.get(&call_key) {
                    let target = select_target(candidates.as_slice(), key);
                    if let Some(target_key) = target {
                        if let Some(&target_index) = index_map.get(target_key) {
                            if seen_targets.insert(target_index) {
                                graph.add_edge(from_index, target_index, ());
                            }
                        }
                    }
                }
            }
        }
    }

    (graph, index_map)
}

fn build_name_lookup<'a>(
    nodes: &'a HashMap<EntityKey, FunctionNode>,
) -> HashMap<String, Vec<&'a EntityKey>> {
    let mut map: HashMap<String, Vec<&EntityKey>> = HashMap::new();

    for key in nodes.keys() {
        map.entry(key.name.to_lowercase()).or_default().push(key);
    }

    map
}

fn select_target<'a>(candidates: &'a [&'a EntityKey], source: &EntityKey) -> Option<&'a EntityKey> {
    for &candidate in candidates {
        if candidate.file_path == source.file_path && candidate != source {
            return Some(candidate);
        }
    }

    for &candidate in candidates {
        if candidate != source {
            return Some(candidate);
        }
    }

    candidates.first().copied()
}

fn compute_metrics(
    graph: &DependencyGraph,
    index_map: &IndexMap,
    nodes: &HashMap<EntityKey, FunctionNode>,
) -> HashMap<EntityKey, DependencyMetrics> {
    let mut metrics = HashMap::new();

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

fn compute_closeness(graph: &DependencyGraph, start: NodeIndex) -> f64 {
    let mut visited: HashMap<NodeIndex, usize> = HashMap::new();
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

fn identify_cycles(
    graph: &DependencyGraph,
    index_map: &IndexMap,
    nodes: &HashMap<EntityKey, FunctionNode>,
) -> (Vec<Vec<FunctionNode>>, HashSet<EntityKey>) {
    let mut cycles = Vec::new();
    let mut members = HashSet::new();

    let sccs = kosaraju_scc(graph);

    for component in sccs {
        if component.len() > 1 {
            let mut cycle_nodes = Vec::new();
            for index in component {
                if let Some(key) = graph.node_weight(index) {
                    if let Some(node) = nodes.get(key) {
                        cycle_nodes.push(node.clone());
                        members.insert(key.clone());
                    }
                }
            }
            cycles.push(cycle_nodes);
        } else if let Some(&index) = component.first() {
            if graph.find_edge(index, index).is_some() {
                if let Some(key) = graph.node_weight(index) {
                    if let Some(node) = nodes.get(key) {
                        cycles.push(vec![node.clone()]);
                        members.insert(key.clone());
                    }
                }
            }
        }
    }

    (cycles, members)
}

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

fn normalize_call_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut candidate = trimmed;
    for prefix in ["self.", "cls.", "this."] {
        if candidate.starts_with(prefix) {
            candidate = &candidate[prefix.len()..];
            break;
        }
    }

    let parts: Vec<&str> = candidate
        .split(|c: char| c == '.' || c == ':' || c == ' ')
        .filter(|segment| !segment.is_empty())
        .collect();

    let Some(last) = parts.last() else {
        return None;
    };

    let clean = last.trim_matches(|c: char| c == '(' || c == ')' || c == '[' || c == ']');
    if clean.is_empty() {
        None
    } else {
        Some(clean.to_string())
    }
}

pub fn canonicalize_path(path: &Path) -> PathBuf {
    match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(_) => path.to_path_buf(),
    }
}
