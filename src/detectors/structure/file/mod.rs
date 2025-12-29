//! File analysis, entity extraction, and file splitting logic

use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Directories to skip during file analysis
const SKIP_DIRECTORIES: &[&str] = &[
    "node_modules", "__pycache__", "target", ".git", "build", "dist",
];

/// Code file extensions recognized for analysis
const CODE_EXTENSIONS: &[&str] = &[
    "py", "pyi", "js", "mjs", "ts", "jsx", "tsx", "rs", "go", "java", "cpp", "c", "h", "hpp",
];

use crate::core::ast_utils::count_named_nodes;
use crate::core::errors::Result;
use crate::core::file_utils::FileReader;
use crate::lang::common::{EntityKind, ParsedEntity};
use crate::lang::registry::{adapter_for_file, get_tree_sitter_language};

use super::config::{
    CohesionEdge, CohesionGraph, EntityNode, FileEntityHealth, FileMetrics, FileSplitPack,
    ImportStatement, SplitEffort, SplitValue, StructureConfig, SuggestedSplit,
};
use super::health::HealthScorer;
use super::PrecomputedFileMetrics;

pub struct FileAnalyzer {
    config: StructureConfig,
    project_import_cache: Arc<RwLock<HashMap<PathBuf, Arc<ProjectImportSnapshot>>>>,
}

#[derive(Default, Debug)]
struct ProjectImportSnapshot {
    imports_by_file: HashMap<PathBuf, Vec<PathBuf>>,
    reverse_imports: HashMap<PathBuf, HashSet<PathBuf>>,
}

#[derive(Default, Debug, Clone)]
struct FileDependencyMetrics {
    exports: Vec<ExportedEntity>,
    outgoing_dependencies: HashSet<PathBuf>,
    incoming_importers: HashSet<PathBuf>,
}

#[derive(Debug, Clone)]
struct ExportedEntity {
    name: String,
    kind: EntityKind,
}

impl FileAnalyzer {
    pub fn new(config: StructureConfig) -> Self {
        Self {
            config,
            project_import_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if file extension indicates a code file
    pub fn is_code_file(&self, extension: &str) -> bool {
        CODE_EXTENSIONS.contains(&extension)
    }

    /// Count lines of code in a file
    pub fn count_lines_of_code(&self, file_path: &Path) -> Result<usize> {
        FileReader::count_lines_of_code(file_path)
    }

    /// Calculate a lognormal distribution-based score for file size.
    ///
    /// Returns a score in [0, 1] where 1.0 means the value equals the mode (optimal),
    /// and the score decreases as the value deviates from optimal. Uses an asymmetric
    /// lognormal distribution which penalizes large files more gradually than small ones.
    ///
    /// The distribution parameters are derived from:
    /// - `optimal`: The mode (peak) of the distribution
    /// - `percentile_95`: The value at the 95th percentile
    ///
    /// For a lognormal distribution:
    /// - mode = exp(μ - σ²)
    /// - 95th percentile = exp(μ + 1.645σ)
    ///
    /// We solve for μ and σ from these two equations.
    pub fn calculate_lognormal_score(&self, value: usize, optimal: usize, percentile_95: usize) -> f64 {
        if value == 0 || optimal == 0 || percentile_95 <= optimal {
            return if value == optimal { 1.0 } else { 0.0 };
        }

        let value = value as f64;
        let optimal = optimal as f64;
        let p95 = percentile_95 as f64;

        // For lognormal: mode = exp(μ - σ²), so ln(mode) = μ - σ²
        // 95th percentile: exp(μ + 1.645σ) = p95, so ln(p95) = μ + 1.645σ
        //
        // From these: ln(p95) - ln(mode) = σ² + 1.645σ
        // Let's solve: σ² + 1.645σ - (ln(p95) - ln(mode)) = 0
        let log_ratio = p95.ln() - optimal.ln();

        // Quadratic formula: σ = (-1.645 + sqrt(1.645² + 4*log_ratio)) / 2
        let discriminant = 1.645_f64 * 1.645_f64 + 4.0 * log_ratio;
        if discriminant < 0.0 {
            return 0.0;
        }

        let sigma = (-1.645 + discriminant.sqrt()) / 2.0;
        if sigma <= 0.0 {
            return if (value - optimal).abs() < 0.001 { 1.0 } else { 0.0 };
        }

        // μ = ln(mode) + σ²
        let mu = optimal.ln() + sigma * sigma;

        // Lognormal PDF: f(x) = (1 / (x * σ * sqrt(2π))) * exp(-((ln(x) - μ)² / (2σ²)))
        // Score = f(value) / f(mode), which simplifies since the normalization cancels:
        // score = (mode / value) * exp(-((ln(value) - μ)² - (ln(mode) - μ)²) / (2σ²))
        //
        // Since ln(mode) = μ - σ², we have (ln(mode) - μ)² = σ⁴
        // So: score = (mode / value) * exp(-((ln(value) - μ)² - σ⁴) / (2σ²))

        let log_value = value.ln();
        let log_value_centered = log_value - mu;

        // PDF ratio: f(value) / f(mode)
        // = (mode/value) * exp(-0.5 * ((ln(value) - μ)² - (ln(mode) - μ)²) / σ²)
        // = (mode/value) * exp(-0.5 * ((ln(value) - μ)² - σ⁴) / σ²)
        let exponent = -0.5 * (log_value_centered * log_value_centered - sigma.powi(4)) / (sigma * sigma);
        let score = (optimal / value) * exponent.exp();

        score.clamp(0.0, 1.0)
    }

    /// Calculate file size score using AST node count
    pub fn calculate_file_size_score(&self, ast_nodes: usize) -> f64 {
        self.calculate_lognormal_score(
            ast_nodes,
            self.config.fsfile.optimal_ast_nodes,
            self.config.fsfile.ast_nodes_95th_percentile,
        )
    }

    /// Calculate file metrics including AST-based size scoring.
    ///
    /// Uses tree-sitter to parse the file and count named AST nodes,
    /// then scores the file size against a lognormal distribution.
    pub fn calculate_file_metrics(&self, file_path: &Path) -> Result<FileMetrics> {
        let content = FileReader::read_to_string(file_path)?;
        let loc = content.lines().filter(|line| !line.trim().is_empty()).count();

        // Get tree-sitter language for this file
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let ast_nodes = match get_tree_sitter_language(extension) {
            Ok(language) => {
                let mut parser = tree_sitter::Parser::new();
                parser.set_language(&language).ok();
                match parser.parse(&content, None) {
                    Some(tree) => count_named_nodes(&tree.root_node()),
                    None => self.estimate_ast_nodes_from_loc(loc),
                }
            }
            Err(_) => self.estimate_ast_nodes_from_loc(loc),
        };

        let size_score = self.calculate_file_size_score(ast_nodes);

        // Calculate entity health metrics
        let entity_health = self.calculate_entity_health(file_path, &content).ok();

        Ok(FileMetrics {
            path: file_path.to_path_buf(),
            ast_nodes,
            loc,
            size_score,
            entity_health,
        })
    }

    /// Calculate aggregated entity health metrics for a file.
    ///
    /// Extracts all entities, scores them using the health scorer,
    /// and computes an AST-weighted average health score.
    pub fn calculate_entity_health(
        &self,
        file_path: &Path,
        content: &str,
    ) -> Result<FileEntityHealth> {
        let entities = self.extract_entities_with_treesitter(file_path, content)?;
        let scorer = HealthScorer::new(self.config.clone());

        if entities.is_empty() {
            return Ok(FileEntityHealth {
                entity_count: 0,
                total_ast_nodes: 0,
                health: 1.0,
                min_health: 1.0,
            });
        }

        let mut total_ast_nodes = 0usize;
        let mut weighted_health_sum = 0.0;
        let mut min_health = 1.0f64;

        // Score each entity and compute AST-weighted average
        for entity in &entities {
            let health = match entity.entity_type.as_str() {
                "class" | "struct" | "interface" | "enum" => scorer.score_class(entity.ast_nodes),
                _ => scorer.score_function(entity.ast_nodes), // function, method, etc.
            };
            let weight = entity.ast_nodes as f64;

            total_ast_nodes += entity.ast_nodes;
            weighted_health_sum += health.health * weight;
            min_health = min_health.min(health.health);
        }

        let health = if total_ast_nodes > 0 {
            weighted_health_sum / total_ast_nodes as f64
        } else {
            1.0
        };

        Ok(FileEntityHealth {
            entity_count: entities.len(),
            total_ast_nodes,
            health,
            min_health,
        })
    }

    /// Estimate AST nodes from LOC when tree-sitter parsing isn't available.
    /// Uses the empirical heuristic of ~10 nodes per line of code.
    fn estimate_ast_nodes_from_loc(&self, loc: usize) -> usize {
        loc * 10
    }

    /// Analyze file for split potential
    pub fn analyze_file_for_split(&self, file_path: &Path) -> Result<Option<FileSplitPack>> {
        self.analyze_file_for_split_internal(file_path, None)
    }

    /// Analyze file for split potential with explicit project root context
    pub fn analyze_file_for_split_with_root(
        &self,
        file_path: &Path,
        project_root: &Path,
    ) -> Result<Option<FileSplitPack>> {
        self.analyze_file_for_split_internal(file_path, Some(project_root))
    }

    fn analyze_file_for_split_internal(
        &self,
        file_path: &Path,
        project_root: Option<&Path>,
    ) -> Result<Option<FileSplitPack>> {
        let metadata = std::fs::metadata(file_path)?;
        let size_bytes = metadata.len() as usize;
        let loc = self.count_lines_of_code(file_path)?;
        let cohesion_graph = self.build_entity_cohesion_graph(file_path)?;

        self.build_split_pack(file_path, loc, size_bytes, &cohesion_graph, project_root)
    }

    /// Analyze file for splitting using pre-computed metrics (avoids file I/O)
    pub fn analyze_file_for_split_with_metrics(
        &self,
        metrics: &PrecomputedFileMetrics,
        project_root: &Path,
    ) -> Result<Option<FileSplitPack>> {
        let file_path = &metrics.path;
        let loc = metrics.loc;
        let size_bytes = metrics.source.len();
        let cohesion_graph = self.build_entity_cohesion_graph_from_source(file_path, &metrics.source)?;

        self.build_split_pack(file_path, loc, size_bytes, &cohesion_graph, Some(project_root))
    }

    /// Common logic for building a FileSplitPack from analysis data
    fn build_split_pack(
        &self,
        file_path: &Path,
        loc: usize,
        size_bytes: usize,
        cohesion_graph: &CohesionGraph,
        project_root: Option<&Path>,
    ) -> Result<Option<FileSplitPack>> {
        if !self.is_huge_file(loc, size_bytes) {
            return Ok(None);
        }

        let mut reasons = self.collect_size_reasons(loc, size_bytes);
        let communities = self.find_cohesion_communities(cohesion_graph)?;

        if communities.len() < self.config.partitioning.min_clusters {
            return Ok(None);
        }
        reasons.push(format!("{} cohesion communities", communities.len()));

        let suggested_splits = self.generate_split_suggestions(file_path, &communities)?;
        let dependency_metrics = self.collect_dependency_metrics(file_path, project_root, cohesion_graph)?;
        let value = self.calculate_split_value(loc, file_path, cohesion_graph, &dependency_metrics)?;
        let effort = self.calculate_split_effort(&dependency_metrics)?;

        Ok(Some(FileSplitPack {
            kind: "file_split".to_string(),
            file: file_path.to_path_buf(),
            reasons,
            suggested_splits,
            value,
            effort,
        }))
    }

    /// Check if file exceeds "huge" thresholds
    fn is_huge_file(&self, loc: usize, size_bytes: usize) -> bool {
        loc >= self.config.fsfile.huge_loc || size_bytes >= self.config.fsfile.huge_bytes
    }

    /// Collect reasons for why a file is considered huge
    fn collect_size_reasons(&self, loc: usize, size_bytes: usize) -> Vec<String> {
        let mut reasons = Vec::new();
        if loc >= self.config.fsfile.huge_loc {
            reasons.push(format!("loc {} > {}", loc, self.config.fsfile.huge_loc));
        }
        if size_bytes >= self.config.fsfile.huge_bytes {
            reasons.push(format!("size {} bytes > {} bytes", size_bytes, self.config.fsfile.huge_bytes));
        }
        reasons
    }

    /// Build entity cohesion graph from pre-loaded source (avoids file I/O)
    pub fn build_entity_cohesion_graph_from_source(
        &self,
        file_path: &Path,
        source: &str,
    ) -> Result<CohesionGraph> {
        let mut graph = Graph::new_undirected();

        // Extract entities based on file type using tree-sitter
        let entities = self.extract_entities_with_treesitter(file_path, source)?;

        if entities.len() < 2 {
            return Ok(graph); // Need at least 2 entities for cohesion analysis
        }

        // Add entity nodes to graph
        let mut entity_nodes = Vec::new();
        for entity in entities {
            let node_idx = graph.add_node(entity);
            entity_nodes.push(node_idx);
        }

        // Calculate cohesion between all pairs of entities
        for i in 0..entity_nodes.len() {
            for j in i + 1..entity_nodes.len() {
                let entity_a = &graph[entity_nodes[i]];
                let entity_b = &graph[entity_nodes[j]];

                let jaccard_similarity =
                    self.calculate_jaccard_similarity(&entity_a.symbols, &entity_b.symbols);

                // Only add edges for significant cohesion
                if jaccard_similarity > 0.1 {
                    let shared_symbols = entity_a.symbols.intersection(&entity_b.symbols).count();
                    let edge = CohesionEdge {
                        similarity: jaccard_similarity,
                        shared_symbols,
                    };
                    graph.add_edge(entity_nodes[i], entity_nodes[j], edge);
                }
            }
        }

        Ok(graph)
    }

    /// Build entity cohesion graph for file
    pub fn build_entity_cohesion_graph(&self, file_path: &Path) -> Result<CohesionGraph> {
        let content = FileReader::read_to_string(file_path)?;
        self.build_entity_cohesion_graph_from_source(file_path, &content)
    }

    /// Find cohesion communities in entity graph
    pub fn find_cohesion_communities(&self, graph: &CohesionGraph) -> Result<Vec<Vec<NodeIndex>>> {
        let node_indices: Vec<_> = graph.node_indices().collect();

        if node_indices.len() < 2 {
            return Ok(vec![node_indices]);
        }

        let edges = self.collect_sorted_edges(graph);
        let (mut communities, mut assigned_nodes) = self.build_communities_from_edges(&edges);

        // Add remaining nodes as singleton communities
        for node in node_indices {
            if !assigned_nodes.contains(&node) {
                communities.push(vec![node]);
            }
        }

        // Filter and limit communities
        communities.retain(|comm| comm.len() >= self.config.fsfile.min_entities_per_split);
        communities.truncate(3);

        Ok(communities)
    }

    /// Collect edges sorted by cohesion strength (descending)
    fn collect_sorted_edges(&self, graph: &CohesionGraph) -> Vec<(NodeIndex, NodeIndex, f64)> {
        let mut edges: Vec<_> = graph
            .edge_indices()
            .filter_map(|edge_idx| {
                let (source, target) = graph.edge_endpoints(edge_idx)?;
                let weight = graph.edge_weight(edge_idx)?;
                Some((source, target, weight.similarity))
            })
            .collect();

        edges.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        edges
    }

    /// Build communities greedily from sorted edges
    fn build_communities_from_edges(
        &self,
        edges: &[(NodeIndex, NodeIndex, f64)],
    ) -> (Vec<Vec<NodeIndex>>, HashSet<NodeIndex>) {
        let mut communities: Vec<Vec<NodeIndex>> = Vec::new();
        let mut assigned_nodes = HashSet::new();

        for &(source, target, similarity) in edges {
            if similarity < 0.2 {
                break;
            }

            let source_comm = self.find_node_community(&communities, source);
            let target_comm = self.find_node_community(&communities, target);

            self.assign_nodes_to_community(
                source, target, source_comm, target_comm,
                &mut communities, &mut assigned_nodes,
            );
        }

        (communities, assigned_nodes)
    }

    /// Find which community a node belongs to
    fn find_node_community(&self, communities: &[Vec<NodeIndex>], node: NodeIndex) -> Option<usize> {
        communities.iter().position(|comm| comm.contains(&node))
    }

    /// Assign nodes to communities based on their current membership
    fn assign_nodes_to_community(
        &self,
        source: NodeIndex,
        target: NodeIndex,
        source_comm: Option<usize>,
        target_comm: Option<usize>,
        communities: &mut Vec<Vec<NodeIndex>>,
        assigned_nodes: &mut HashSet<NodeIndex>,
    ) {
        match (source_comm, target_comm) {
            (Some(idx), None) => self.add_to_community(target, idx, communities, assigned_nodes),
            (None, Some(idx)) => self.add_to_community(source, idx, communities, assigned_nodes),
            (None, None) => self.create_new_community(source, target, communities, assigned_nodes),
            (Some(_), Some(_)) => {} // Both already assigned
        }
    }

    /// Add a node to an existing community
    fn add_to_community(
        &self,
        node: NodeIndex,
        comm_idx: usize,
        communities: &mut [Vec<NodeIndex>],
        assigned_nodes: &mut HashSet<NodeIndex>,
    ) {
        if !assigned_nodes.contains(&node) {
            communities[comm_idx].push(node);
            assigned_nodes.insert(node);
        }
    }

    /// Create a new community with two nodes
    fn create_new_community(
        &self,
        source: NodeIndex,
        target: NodeIndex,
        communities: &mut Vec<Vec<NodeIndex>>,
        assigned_nodes: &mut HashSet<NodeIndex>,
    ) {
        let mut new_community = Vec::new();
        if !assigned_nodes.contains(&source) {
            new_community.push(source);
            assigned_nodes.insert(source);
        }
        if !assigned_nodes.contains(&target) {
            new_community.push(target);
            assigned_nodes.insert(target);
        }
        if !new_community.is_empty() {
            communities.push(new_community);
        }
    }

    /// Generate split file suggestions
    pub fn generate_split_suggestions(
        &self,
        file_path: &Path,
        communities: &[Vec<NodeIndex>],
    ) -> Result<Vec<SuggestedSplit>> {
        let cohesion_graph = self.build_entity_cohesion_graph(file_path)?;

        let base_name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");

        let suffixes = ["_core", "_io", "_api"];
        let mut splits = Vec::new();

        for (community_idx, community) in communities.iter().enumerate().take(3) {
            let suffix = suffixes.get(community_idx).unwrap_or(&"_part");

            let mut entities = Vec::new();
            let mut total_loc = 0;

            // Extract entity information from the community
            for &node_idx in community {
                if let Some(entity) = cohesion_graph.node_weight(node_idx) {
                    entities.push(entity.name.clone());
                    total_loc += entity.loc;
                }
            }

            // Generate meaningful name based on entity analysis
            let split_name = self.generate_split_name(base_name, suffix, &entities, file_path);

            splits.push(SuggestedSplit {
                name: split_name,
                entities,
                loc: total_loc,
            });
        }

        // If no communities found, create default splits
        if splits.is_empty() {
            for (i, suffix) in suffixes.iter().enumerate().take(2) {
                splits.push(SuggestedSplit {
                    name: format!(
                        "{}{}.{}",
                        base_name,
                        suffix,
                        file_path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("py")
                    ),
                    entities: vec![format!("Entity{}", i + 1)],
                    loc: 400, // Rough estimate
                });
            }
        }

        Ok(splits)
    }

    /// Generate a meaningful name for a split file based on entity analysis
    pub fn generate_split_name(
        &self,
        base_name: &str,
        suffix: &str,
        entities: &[String],
        file_path: &Path,
    ) -> String {
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("py");

        // Analyze entity names to suggest better suffixes
        let entity_analysis = self.analyze_entity_names(entities);

        let final_suffix = if !entity_analysis.is_empty() {
            entity_analysis
        } else {
            suffix.to_string()
        };

        format!("{}{}.{}", base_name, final_suffix, extension)
    }

    /// Check if a lowercased entity name matches any pattern in the list.
    fn matches_patterns(name: &str, patterns: &[&str]) -> bool {
        patterns.iter().any(|p| name.contains(p))
    }

    /// Analyze entity names to suggest appropriate suffixes
    pub fn analyze_entity_names(&self, entities: &[String]) -> String {
        const IO_PATTERNS: &[&str] = &["read", "write", "load", "save", "file", "io"];
        const API_PATTERNS: &[&str] = &["api", "endpoint", "route", "handler", "controller"];
        const UTIL_PATTERNS: &[&str] = &["util", "helper", "tool"];

        let (io_count, api_count, util_count, core_count) = entities.iter().fold(
            (0, 0, 0, 0),
            |(io, api, util, core), entity| {
                let lower = entity.to_lowercase();
                if Self::matches_patterns(&lower, IO_PATTERNS) {
                    (io + 1, api, util, core)
                } else if Self::matches_patterns(&lower, API_PATTERNS) {
                    (io, api + 1, util, core)
                } else if Self::matches_patterns(&lower, UTIL_PATTERNS) {
                    (io, api, util + 1, core)
                } else {
                    (io, api, util, core + 1)
                }
            },
        );

        // Return suffix for the category with highest count
        let counts = [(io_count, "_io"), (api_count, "_api"), (util_count, "_util"), (core_count, "_core")];
        counts.iter().max_by_key(|(count, _)| count).map(|(_, suffix)| *suffix).unwrap_or("_core").to_string()
    }

    /// Calculate value score for file splitting
    pub fn calculate_split_value(
        &self,
        loc: usize,
        _file_path: &Path,
        cohesion_graph: &CohesionGraph,
        metrics: &FileDependencyMetrics,
    ) -> Result<SplitValue> {
        let size_factor = (loc as f64 / self.config.fsfile.huge_loc as f64).min(1.0);

        let cycle_factor = if metrics.outgoing_dependencies.is_empty() {
            0.0
        } else {
            let mutual = metrics
                .outgoing_dependencies
                .intersection(&metrics.incoming_importers)
                .count();
            let denominator = metrics
                .outgoing_dependencies
                .union(&metrics.incoming_importers)
                .count()
                .max(1);
            (mutual as f64 / denominator as f64).min(1.0)
        };

        let clone_factor = self.estimate_clone_factor(cohesion_graph);

        let score = 0.6 * size_factor + 0.3 * cycle_factor + 0.1 * clone_factor;

        Ok(SplitValue { score })
    }

    /// Calculate effort required for file splitting
    pub fn calculate_split_effort(&self, metrics: &FileDependencyMetrics) -> Result<SplitEffort> {
        Ok(SplitEffort {
            exports: metrics.exports.len(),
            external_importers: metrics.incoming_importers.len(),
        })
    }

    fn estimate_clone_factor(&self, graph: &CohesionGraph) -> f64 {
        let node_count = graph.node_count();
        if node_count < 2 {
            return 0.0;
        }

        let mut heavy_edges = 0usize;
        for edge_idx in graph.edge_indices() {
            if let Some(edge) = graph.edge_weight(edge_idx) {
                if edge.similarity >= 0.75 && edge.shared_symbols >= 3 {
                    heavy_edges += 1;
                }
            }
        }

        if heavy_edges == 0 {
            return 0.0;
        }

        let max_edges = (node_count.saturating_sub(1) * node_count) / 2;
        if max_edges == 0 {
            return 0.0;
        }

        (heavy_edges as f64 / max_edges as f64).min(1.0)
    }

    fn collect_dependency_metrics(
        &self,
        file_path: &Path,
        project_root: Option<&Path>,
        _cohesion_graph: &CohesionGraph,
    ) -> Result<FileDependencyMetrics> {
        let mut metrics = FileDependencyMetrics::default();
        let content = FileReader::read_to_string(file_path)?;

        if let Ok(mut adapter) = adapter_for_file(file_path) {
            if let Ok(parse_index) = adapter.parse_source(&content, &file_path.to_string_lossy()) {
                metrics.exports = self.extract_exported_entities(file_path, &parse_index, &content);
            }
        }

        if let Some(root) = project_root {
            let snapshot = self.get_project_import_snapshot(root)?;
            let canonical_file = self.canonicalize_path(file_path);

            if let Some(targets) = snapshot.imports_by_file.get(&canonical_file) {
                metrics
                    .outgoing_dependencies
                    .extend(targets.iter().cloned());
            }

            if let Some(importers) = snapshot.reverse_imports.get(&canonical_file) {
                metrics.incoming_importers.extend(importers.iter().cloned());
            }
        }

        Ok(metrics)
    }

    fn extract_exported_entities(
        &self,
        file_path: &Path,
        parse_index: &crate::lang::common::ParseIndex,
        content: &str,
    ) -> Vec<ExportedEntity> {
        let file_key = file_path.to_string_lossy();
        parse_index
            .get_entities_in_file(&file_key)
            .into_iter()
            .filter(|entity| entity.parent.is_none())
            .filter(|entity| {
                matches!(
                    entity.kind,
                    EntityKind::Function
                        | EntityKind::Class
                        | EntityKind::Struct
                        | EntityKind::Enum
                        | EntityKind::Interface
                )
            })
            .filter(|entity| self.is_entity_exported(entity, file_path, content))
            .map(|entity| ExportedEntity {
                name: entity.name.clone(),
                kind: entity.kind,
            })
            .collect()
    }

    fn is_entity_exported(&self, entity: &ParsedEntity, file_path: &Path, content: &str) -> bool {
        let ext = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default();

        match ext {
            "rs" => entity
                .metadata
                .get("visibility")
                .and_then(|value| value.as_str())
                .map(|vis| vis.contains("pub"))
                .unwrap_or(false),
            "py" | "pyi" => {
                if entity.name.starts_with('_') {
                    return false;
                }
                entity.parent.is_none()
            }
            "go" => entity
                .name
                .chars()
                .next()
                .map(|ch| ch.is_ascii_uppercase())
                .unwrap_or(false),
            "ts" | "tsx" | "js" | "jsx" => {
                self.line_has_export_keyword(content, entity.location.start_line)
            }
            "java" => self.line_has_keyword(content, entity.location.start_line, "public"),
            _ => entity.parent.is_none(),
        }
    }

    fn line_has_export_keyword(&self, content: &str, start_line: usize) -> bool {
        self.line_has_keyword(content, start_line, "export")
    }

    fn line_has_keyword(&self, content: &str, start_line: usize, keyword: &str) -> bool {
        if start_line == 0 {
            return false;
        }

        let lines: Vec<&str> = content.lines().collect();
        let line_idx = start_line.saturating_sub(1);

        if let Some(line) = lines.get(line_idx) {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                // Skip comment-only lines
                return false;
            }
            if trimmed.starts_with(keyword) || trimmed.contains(&format!("{keyword} ")) {
                return true;
            }
        }

        if line_idx > 0 {
            if let Some(previous) = lines.get(line_idx - 1) {
                if previous.trim_end().ends_with(keyword) {
                    return true;
                }
            }
        }

        false
    }

    fn get_project_import_snapshot(
        &self,
        project_root: &Path,
    ) -> Result<Arc<ProjectImportSnapshot>> {
        let canonical_root = self.canonicalize_path(project_root);

        if let Some(snapshot) = self
            .project_import_cache
            .read()
            .unwrap()
            .get(&canonical_root)
            .cloned()
        {
            return Ok(snapshot);
        }

        let snapshot = Arc::new(self.build_project_import_snapshot(&canonical_root)?);
        self.project_import_cache
            .write()
            .unwrap()
            .insert(canonical_root, snapshot.clone());

        Ok(snapshot)
    }

    fn build_project_import_snapshot(&self, project_root: &Path) -> Result<ProjectImportSnapshot> {
        let mut snapshot = ProjectImportSnapshot::default();
        for file in self.collect_project_code_files(project_root)? {
            let canonical_file = self.canonicalize_path(&file);
            let imports = self.extract_imports(&file)?;

            for import in imports {
                if let Some(resolved) =
                    self.resolve_import_to_project_file(&import, &file, project_root)
                {
                    let canonical_target = self.canonicalize_path(&resolved);
                    snapshot
                        .imports_by_file
                        .entry(canonical_file.clone())
                        .or_default()
                        .push(canonical_target.clone());
                    snapshot
                        .reverse_imports
                        .entry(canonical_target)
                        .or_default()
                        .insert(canonical_file.clone());
                }
            }
        }

        Ok(snapshot)
    }

    fn collect_project_code_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_project_code_files_recursive(root, &mut files)?;
        Ok(files)
    }

    fn collect_project_code_files_recursive(
        &self,
        path: &Path,
        files: &mut Vec<PathBuf>,
    ) -> Result<()> {
        if self.should_skip_directory(path) {
            return Ok(());
        }

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let child_path = entry.path();

            if child_path.is_dir() {
                self.collect_project_code_files_recursive(&child_path, files)?;
            } else if child_path.is_file() {
                if let Some(ext) = child_path.extension().and_then(|e| e.to_str()) {
                    if self.is_code_file(ext) {
                        files.push(child_path);
                    }
                }
            }
        }

        Ok(())
    }

    fn resolve_import_to_project_file(
        &self,
        import: &ImportStatement,
        current_file: &Path,
        project_root: &Path,
    ) -> Option<PathBuf> {
        let module = import.module.trim();
        if module.is_empty() {
            return None;
        }

        let current_dir = current_file.parent().unwrap_or(project_root);
        let mut candidates: Vec<PathBuf> = Vec::new();

        if module.starts_with("./") || module.starts_with("../") {
            candidates.push(current_dir.join(module));
        } else if module.starts_with('.') {
            candidates.extend(self.resolve_python_relative_module(
                current_dir,
                project_root,
                module,
            ));
        } else {
            if module.contains('/') {
                candidates.push(project_root.join(module));
                candidates.push(current_dir.join(module));
            }

            if module.contains('.') {
                let mut from_root = project_root.to_path_buf();
                for part in module.split('.') {
                    if part.is_empty() {
                        continue;
                    }
                    from_root.push(part);
                }
                candidates.push(from_root);
            }

            candidates.push(current_dir.join(module));
        }

        for candidate in candidates {
            if let Some(resolved) = self.resolve_candidate_path(&candidate) {
                return Some(resolved);
            }
        }

        None
    }

    fn resolve_python_relative_module(
        &self,
        current_dir: &Path,
        project_root: &Path,
        module: &str,
    ) -> Vec<PathBuf> {
        let mut base = current_dir.to_path_buf();
        let mut parts = Vec::new();
        for part in module.split('.') {
            if part.is_empty() {
                if let Some(parent) = base.parent() {
                    base = parent.to_path_buf();
                } else {
                    base = project_root.to_path_buf();
                }
            } else {
                parts.push(part);
            }
        }

        if parts.is_empty() {
            vec![base]
        } else {
            let mut path = base;
            for part in parts {
                path.push(part);
            }
            vec![path]
        }
    }

    fn resolve_candidate_path(&self, candidate: &Path) -> Option<PathBuf> {
        let mut targets = Vec::new();

        if candidate.exists() {
            if candidate.is_file() {
                targets.push(candidate.to_path_buf());
            } else if candidate.is_dir() {
                targets.extend(self.directory_module_fallbacks(candidate));
            }
        }

        if candidate.extension().is_none() {
            for ext in Self::supported_extensions() {
                let candidate_with_ext = candidate.with_extension(ext);
                if candidate_with_ext.exists() {
                    targets.push(candidate_with_ext);
                }
            }
        }

        targets.into_iter().find(|path| path.exists())
    }

    fn directory_module_fallbacks(&self, dir: &Path) -> Vec<PathBuf> {
        [
            "mod.rs",
            "lib.rs",
            "__init__.py",
            "index.ts",
            "index.tsx",
            "index.js",
            "index.jsx",
        ]
        .iter()
        .map(|candidate| dir.join(candidate))
        .collect()
    }

    fn supported_extensions() -> &'static [&'static str] {
        &[
            "py", "pyi", "js", "mjs", "jsx", "ts", "tsx", "rs", "go", "java", "cpp", "c", "h",
            "hpp",
        ]
    }

    fn canonicalize_path(&self, path: &Path) -> PathBuf {
        // Use relative paths instead of absolute canonicalized paths to prevent
        // filesystem hierarchy traversal outside the project
        if path.is_absolute() {
            // If absolute path, try to make it relative to current directory
            if let Ok(current_dir) = std::env::current_dir() {
                if let Ok(relative) = path.strip_prefix(&current_dir) {
                    return relative.to_path_buf();
                }
            }
        }
        path.to_path_buf()
    }

    /// Extract entities using tree-sitter for accurate parsing
    pub fn extract_entities_with_treesitter(
        &self,
        file_path: &Path,
        content: &str,
    ) -> Result<Vec<EntityNode>> {
        let file_path_str = file_path.to_string_lossy().to_string();
        match adapter_for_file(file_path) {
            Ok(mut adapter) => {
                self.extract_entities_from_adapter(adapter.as_mut(), content, &file_path_str)
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    fn extract_entities_from_adapter(
        &self,
        adapter: &mut dyn crate::lang::common::LanguageAdapter,
        content: &str,
        file_path: &str,
    ) -> Result<Vec<EntityNode>> {
        let parse_index = adapter.parse_source(content, file_path)?;
        let parsed_entities = parse_index.get_entities_in_file(file_path);
        let mut entities = Vec::new();

        for parsed in parsed_entities {
            if !self.is_supported_entity_kind(parsed.kind) {
                continue;
            }

            let start_line = parsed.location.start_line;
            let end_line = parsed.location.end_line;
            let loc = if end_line >= start_line {
                end_line - start_line + 1
            } else {
                1
            };

            let entity_source = self.get_entity_lines_from_source(content, start_line, end_line);

            let mut symbols = HashSet::new();
            if !entity_source.is_empty() {
                if let Ok(identifiers) = adapter.extract_identifiers(&entity_source) {
                    for identifier in identifiers {
                        symbols.insert(identifier);
                    }
                }
            }

            // Estimate AST nodes from LOC (~10 nodes per line heuristic)
            let ast_nodes = self.estimate_ast_nodes_from_loc(loc);

            entities.push(EntityNode {
                name: parsed.name.clone(),
                entity_type: format!("{:?}", parsed.kind).to_lowercase(),
                loc,
                ast_nodes,
                symbols,
            });
        }

        Ok(entities)
    }

    fn is_supported_entity_kind(&self, kind: EntityKind) -> bool {
        matches!(
            kind,
            EntityKind::Function
                | EntityKind::Method
                | EntityKind::Class
                | EntityKind::Struct
                | EntityKind::Enum
                | EntityKind::Interface
        )
    }

    fn calculate_jaccard_similarity(&self, a: &HashSet<String>, b: &HashSet<String>) -> f64 {
        if a.is_empty() && b.is_empty() {
            return 1.0;
        }

        let intersection = a.intersection(b).count() as f64;
        let union = a.union(b).count() as f64;

        if union == 0.0 {
            0.0
        } else {
            intersection / union
        }
    }

    /// Helper method to extract lines from source code for an entity
    fn get_entity_lines_from_source(
        &self,
        content: &str,
        start_line: usize,
        end_line: usize,
    ) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let start_idx = (start_line.saturating_sub(1)).min(lines.len());
        let end_idx = end_line.min(lines.len());

        if start_idx >= lines.len() || end_idx <= start_idx {
            return String::new();
        }

        lines[start_idx..end_idx].join("\n")
    }

    // Legacy text-based extraction methods (deprecated - kept for reference)

    pub fn extract_imports(&self, file_path: &Path) -> Result<Vec<ImportStatement>> {
        let content = FileReader::read_to_string(file_path)?;
        let mut adapter = adapter_for_file(file_path)?;
        adapter.extract_imports(&content)
    }

    /// Extract Python import statements
    /// Resolve import statement to local file path
    pub fn resolve_import_to_local_file(
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
        for ext in Self::supported_extensions() {
            let potential_path = dir_path.join(format!("{}.{}", module_name, ext));
            if potential_path.exists() {
                return Some(potential_path);
            }
        }

        None
    }

    /// Discover large files to analyze
    pub async fn discover_large_files(&self, root_path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_large_files_recursive(root_path, &mut files)?;
        Ok(files)
    }

    /// Recursively collect large files
    fn collect_large_files_recursive(&self, path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if self.should_skip_directory(path) {
            return Ok(());
        }

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let child_path = entry.path();

            if child_path.is_dir() {
                self.collect_large_files_recursive(&child_path, files)?;
            } else if child_path.is_file() {
                if let Some(ext) = child_path.extension().and_then(|e| e.to_str()) {
                    if self.is_code_file(ext) {
                        let metadata = std::fs::metadata(&child_path)?;
                        let size_bytes = metadata.len() as usize;

                        if size_bytes >= self.config.fsfile.huge_bytes {
                            files.push(child_path);
                        } else {
                            // Also check LOC for smaller files that might still be huge by line count
                            let loc = self.count_lines_of_code(&child_path)?;
                            if loc >= self.config.fsfile.huge_loc {
                                files.push(child_path);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if directory should be skipped
    fn should_skip_directory(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        // Skip common generated/build/dependency directories
        SKIP_DIRECTORIES.iter().any(|d| path_str.contains(d))
    }
}


#[cfg(test)]
mod tests;
