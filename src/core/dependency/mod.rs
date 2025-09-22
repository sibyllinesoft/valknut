use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::Direction;

use crate::core::errors::Result;
use crate::core::file_utils::FileReader;
use crate::lang::{adapter_for_file, EntityKind, ParseIndex, ParsedEntity};

#[derive(Debug, Clone)]
pub struct FunctionNode {
    pub unique_id: String,
    pub name: String,
    pub qualified_name: String,
    pub namespace: Vec<String>,
    pub file_path: PathBuf,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub calls: Vec<String>,
}

#[derive(Debug, Clone)]
struct CallIdentifier {
    segments: Vec<String>,
}

impl CallIdentifier {
    fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        let mut segments = Vec::with_capacity(4); // Typical call has 2-4 segments
        let mut buffer = String::new();
        let mut chars = trimmed.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch.is_alphanumeric() || ch == '_' {
                buffer.push(ch);
            } else if ch == '.' || ch == ':' {
                if !buffer.is_empty() {
                    segments.push(buffer.to_lowercase());
                    buffer.clear();
                }
                while matches!(chars.peek(), Some(':')) {
                    chars.next();
                }
            } else if ch == '(' {
                if !buffer.is_empty() {
                    segments.push(buffer.to_lowercase());
                    buffer.clear();
                }
                break;
            } else if ch.is_whitespace() {
                if !buffer.is_empty() {
                    segments.push(buffer.to_lowercase());
                    buffer.clear();
                }
            } else {
                if !buffer.is_empty() {
                    segments.push(buffer.to_lowercase());
                    buffer.clear();
                }
            }
        }

        if !buffer.is_empty() {
            segments.push(buffer.to_lowercase());
        }

        while matches!(segments.first(), Some(segment) if matches!(segment.as_str(), "self" | "this" | "cls" | "super"))
        {
            segments.remove(0);
        }

        if segments.is_empty() {
            return None;
        }

        Some(Self { segments })
    }

    fn base(&self) -> &str {
        self.segments.last().map(|s| s.as_str()).unwrap_or("")
    }

    fn namespace(&self) -> &[String] {
        if self.segments.len() <= 1 {
            &self.segments[..0]
        } else {
            &self.segments[..self.segments.len() - 1]
        }
    }

    fn candidate_keys(&self) -> Vec<String> {
        let mut keys = Vec::with_capacity(self.segments.len()); // Pre-allocate based on segments
        for start in 0..self.segments.len() {
            let candidate = self.segments[start..].join("::");
            if !keys.contains(&candidate) {
                keys.push(candidate);
            }
        }
        keys
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityKey {
    file_path: PathBuf,
    name: String,
    qualified_name: String,
    start_line: Option<usize>,
}

impl EntityKey {
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

    pub fn from_node(node: &FunctionNode) -> Self {
        Self {
            file_path: node.file_path.clone(),
            name: node.name.clone(),
            qualified_name: node.qualified_name.clone(),
            start_line: node.start_line,
        }
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn qualified_name(&self) -> &str {
        &self.qualified_name
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

        Ok(Self {
            nodes,
            metrics,
            cycles,
            chokepoints,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

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

    pub fn cycles(&self) -> &[Vec<FunctionNode>] {
        &self.cycles
    }

    pub fn chokepoints(&self) -> &[Chokepoint] {
        &self.chokepoints
    }

    pub fn metrics_iter(&self) -> impl Iterator<Item = (&EntityKey, &DependencyMetrics)> {
        self.metrics.iter()
    }
}

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

fn build_graph(nodes: &HashMap<EntityKey, FunctionNode>) -> (DependencyGraph, IndexMap) {
    let node_count = nodes.len();
    let mut graph = DependencyGraph::with_capacity(node_count, node_count * 2); // Estimate 2 edges per node
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
            if let Some(call_id) = CallIdentifier::parse(raw_call) {
                let candidate_keys = call_id.candidate_keys();
                let mut matched_target: Option<&EntityKey> = None;

                for candidate_name in &candidate_keys {
                    if let Some(candidates) = name_lookup.get(candidate_name) {
                        if let Some(target_key) = select_target(
                            candidates.as_slice(),
                            node,
                            nodes,
                            &call_id,
                            &candidate_keys,
                        ) {
                            matched_target = Some(target_key);
                            break;
                        }
                    }
                }

                if let Some(target_key) = matched_target {
                    if let Some(&target_index) = index_map.get(target_key) {
                        if seen_targets.insert(target_index) {
                            graph.add_edge(from_index, target_index, ());
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

fn select_target<'a>(
    candidates: &'a [&'a EntityKey],
    source: &FunctionNode,
    nodes: &HashMap<EntityKey, FunctionNode>,
    call: &CallIdentifier,
    candidate_keys: &[String],
) -> Option<&'a EntityKey> {
    let mut best: Option<&EntityKey> = None;
    let mut best_score = i32::MIN;

    for &candidate_key in candidates {
        let Some(candidate_node) = nodes.get(candidate_key) else {
            continue;
        };

        let is_self_call = candidate_node.unique_id == source.unique_id;

        if is_self_call
            && !call
                .base()
                .eq_ignore_ascii_case(candidate_node.name.as_str())
        {
            continue;
        }

        let mut score = 0;

        if is_self_call {
            score += 120;
        }
        let candidate_qualified_lower = candidate_node.qualified_name.to_lowercase();

        if !candidate_keys.is_empty() && candidate_qualified_lower == candidate_keys[0] {
            score += 100;
        } else if candidate_keys
            .iter()
            .any(|candidate| candidate == &candidate_qualified_lower)
        {
            score += 75;
        } else if candidate_node.name.eq_ignore_ascii_case(call.base()) {
            score += 40;
        }

        if namespace_matches(call.namespace(), &candidate_node.namespace) {
            score += 50;
        }

        if candidate_node.file_path == source.file_path {
            score += 20;
        }

        if namespace_equals(&source.namespace, &candidate_node.namespace) {
            score += 15;
        } else if namespace_shares_tail(&source.namespace, &candidate_node.namespace) {
            score += 8;
        }

        if let (Some(src_line), Some(dst_line)) = (source.start_line, candidate_node.start_line) {
            let distance = if src_line >= dst_line {
                src_line - dst_line
            } else {
                dst_line - src_line
            };
            let capped = distance.min(400);
            score += 15 - (capped as i32 / 25);
        }

        if score > best_score {
            best_score = score;
            best = Some(candidate_key);
        }
    }

    best
}

fn namespace_matches(call_ns: &[String], candidate_ns: &[String]) -> bool {
    if call_ns.is_empty() || call_ns.len() > candidate_ns.len() {
        return false;
    }

    let offset = candidate_ns.len() - call_ns.len();
    for (idx, segment) in call_ns.iter().enumerate() {
        if !candidate_ns[offset + idx].eq_ignore_ascii_case(segment) {
            return false;
        }
    }

    true
}

fn namespace_equals(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.iter()
        .zip(b.iter())
        .all(|(lhs, rhs)| lhs.eq_ignore_ascii_case(rhs))
}

fn namespace_shares_tail(a: &[String], b: &[String]) -> bool {
    match (a.last(), b.last()) {
        (Some(lhs), Some(rhs)) => lhs.eq_ignore_ascii_case(rhs),
        _ => false,
    }
}

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

fn identify_cycles(
    graph: &DependencyGraph,
    index_map: &IndexMap,
    nodes: &HashMap<EntityKey, FunctionNode>,
) -> (Vec<Vec<FunctionNode>>, HashSet<EntityKey>) {
    let sccs = kosaraju_scc(graph);
    
    let mut cycles = Vec::with_capacity(sccs.len() / 4); // Estimate ~25% of SCCs are cycles  
    let mut members = HashSet::with_capacity(nodes.len() / 10); // Estimate ~10% of nodes in cycles

    for component in sccs {
        if component.len() > 1 {
            let mut cycle_nodes = Vec::with_capacity(component.len());
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
                    canonical.strip_prefix(&current_dir)
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
