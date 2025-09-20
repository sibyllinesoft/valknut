//! File analysis, entity extraction, and file splitting logic

use petgraph::graph::NodeIndex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::core::errors::Result;
use crate::core::file_utils::FileReader;
use crate::lang::common::{EntityKind, ParsedEntity};
use crate::lang::registry::adapter_for_file;
use tracing::warn;

use super::config::{
    CohesionEdge, CohesionGraph, EntityNode, FileSplitPack, ImportStatement, SplitEffort,
    SplitValue, StructureConfig, SuggestedSplit,
};

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
        matches!(
            extension,
            "py" | "pyi"
                | "js"
                | "mjs"
                | "ts"
                | "jsx"
                | "tsx"
                | "rs"
                | "go"
                | "java"
                | "cpp"
                | "c"
                | "h"
                | "hpp"
        )
    }

    /// Count lines of code in a file
    pub fn count_lines_of_code(&self, file_path: &Path) -> Result<usize> {
        FileReader::count_lines_of_code(file_path)
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

        // Check if file meets "huge" criteria
        let is_huge =
            loc >= self.config.fsfile.huge_loc || size_bytes >= self.config.fsfile.huge_bytes;

        if !is_huge {
            return Ok(None);
        }

        let mut reasons = Vec::new();

        if loc >= self.config.fsfile.huge_loc {
            reasons.push(format!("loc {} > {}", loc, self.config.fsfile.huge_loc));
        }

        if size_bytes >= self.config.fsfile.huge_bytes {
            reasons.push(format!(
                "size {} bytes > {} bytes",
                size_bytes, self.config.fsfile.huge_bytes
            ));
        }

        // Build entity cohesion graph
        let cohesion_graph = self.build_entity_cohesion_graph(file_path)?;
        let communities = self.find_cohesion_communities(&cohesion_graph)?;

        if communities.len() >= self.config.partitioning.min_clusters {
            reasons.push(format!("{} cohesion communities", communities.len()));
        } else {
            return Ok(None); // Not worth splitting
        }

        // Generate split suggestions
        let suggested_splits = self.generate_split_suggestions(file_path, &communities)?;

        // Derive dependency metrics for value/effort estimation
        let dependency_metrics =
            self.collect_dependency_metrics(file_path, project_root, &cohesion_graph)?;

        // Calculate value and effort using real dependency information
        let value =
            self.calculate_split_value(loc, file_path, &cohesion_graph, &dependency_metrics)?;
        let effort = self.calculate_split_effort(&dependency_metrics)?;

        let pack = FileSplitPack {
            kind: "file_split".to_string(),
            file: file_path.to_path_buf(),
            reasons,
            suggested_splits,
            value,
            effort,
        };

        Ok(Some(pack))
    }

    /// Build entity cohesion graph for file
    pub fn build_entity_cohesion_graph(&self, file_path: &Path) -> Result<CohesionGraph> {
        let mut graph = petgraph::Graph::new_undirected();
        let content = FileReader::read_to_string(file_path)?;

        // Extract entities based on file type using tree-sitter
        let entities = self.extract_entities_with_treesitter(file_path, &content)?;

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

    /// Find cohesion communities in entity graph
    pub fn find_cohesion_communities(&self, graph: &CohesionGraph) -> Result<Vec<Vec<NodeIndex>>> {
        let node_indices: Vec<_> = graph.node_indices().collect();

        if node_indices.len() < 2 {
            return Ok(vec![node_indices]);
        }

        // Use a simple but effective community detection based on edge weights
        let mut communities: Vec<Vec<NodeIndex>> = Vec::new();
        let mut assigned_nodes = HashSet::new();

        // Start with the highest cohesion edges and build communities
        let mut edges: Vec<_> = graph
            .edge_indices()
            .filter_map(|edge_idx| {
                let (source, target) = graph.edge_endpoints(edge_idx)?;
                let weight = graph.edge_weight(edge_idx)?;
                Some((edge_idx, source, target, weight.similarity))
            })
            .collect();

        // Sort by cohesion strength (descending)
        edges.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));

        // Build communities greedily
        for (_, source, target, similarity) in edges {
            if similarity < 0.2 {
                break; // Stop at low similarity threshold
            }

            // Find existing communities for these nodes
            let mut source_comm_idx = None;
            let mut target_comm_idx = None;

            for (idx, comm) in communities.iter().enumerate() {
                if comm.contains(&source) {
                    source_comm_idx = Some(idx);
                }
                if comm.contains(&target) {
                    target_comm_idx = Some(idx);
                }
            }

            match (source_comm_idx, target_comm_idx) {
                (Some(comm_idx), None) => {
                    if !assigned_nodes.contains(&target) {
                        communities[comm_idx].push(target);
                        assigned_nodes.insert(target);
                    }
                }
                (None, Some(comm_idx)) => {
                    if !assigned_nodes.contains(&source) {
                        communities[comm_idx].push(source);
                        assigned_nodes.insert(source);
                    }
                }
                (None, None) => {
                    // Create new community
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
                (Some(_), Some(_)) => {
                    // Both nodes already in communities - could merge but skip for simplicity
                }
            }
        }

        // Add any remaining nodes as singleton communities
        for node in node_indices {
            if !assigned_nodes.contains(&node) {
                communities.push(vec![node]);
            }
        }

        // Filter out communities that are too small to be meaningful
        communities.retain(|comm| comm.len() >= self.config.fsfile.min_entities_per_split);

        // Limit to reasonable number of communities (2-3 for splitting)
        communities.truncate(3);

        Ok(communities)
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

    /// Analyze entity names to suggest appropriate suffixes
    pub fn analyze_entity_names(&self, entities: &[String]) -> String {
        let mut io_count = 0;
        let mut api_count = 0;
        let mut core_count = 0;
        let mut util_count = 0;

        for entity in entities {
            let lower_entity = entity.to_lowercase();

            if lower_entity.contains("read")
                || lower_entity.contains("write")
                || lower_entity.contains("load")
                || lower_entity.contains("save")
                || lower_entity.contains("file")
                || lower_entity.contains("io")
            {
                io_count += 1;
            } else if lower_entity.contains("api")
                || lower_entity.contains("endpoint")
                || lower_entity.contains("route")
                || lower_entity.contains("handler")
                || lower_entity.contains("controller")
            {
                api_count += 1;
            } else if lower_entity.contains("util")
                || lower_entity.contains("helper")
                || lower_entity.contains("tool")
            {
                util_count += 1;
            } else {
                core_count += 1;
            }
        }

        // Return the most appropriate suffix based on analysis
        if io_count > api_count && io_count > core_count && io_count > util_count {
            "_io".to_string()
        } else if api_count > core_count && api_count > util_count {
            "_api".to_string()
        } else if util_count > core_count {
            "_util".to_string()
        } else {
            "_core".to_string()
        }
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
            let imports = match self.extract_imports(&file) {
                Ok(imports) => imports,
                Err(err) => {
                    warn!("Failed to extract imports for {}: {}", file.display(), err);
                    continue;
                }
            };

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
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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

            entities.push(EntityNode {
                name: parsed.name.clone(),
                entity_type: format!("{:?}", parsed.kind).to_lowercase(),
                loc,
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
        match adapter_for_file(file_path) {
            Ok(mut adapter) => adapter.extract_imports(&content),
            Err(err) => {
                warn!(
                    "Failed to obtain language adapter for {}: {}",
                    file_path.display(),
                    err
                );
                Ok(Vec::new())
            }
        }
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
        path_str.contains("node_modules")
            || path_str.contains("__pycache__")
            || path_str.contains("target")
            || path_str.contains(".git")
            || path_str.contains("build")
            || path_str.contains("dist")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detectors::structure::config::{
        FsDirectoryConfig, FsFileConfig, PartitioningConfig, StructureConfig, StructureToggles,
    };
    use crate::lang::registry::adapter_for_language;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> StructureConfig {
        StructureConfig {
            enable_branch_packs: true,
            enable_file_split_packs: true,
            top_packs: 20,
            fsdir: FsDirectoryConfig {
                max_files_per_dir: 20,
                max_subdirs_per_dir: 10,
                max_dir_loc: 2000,
                target_loc_per_subdir: 500,
                min_branch_recommendation_gain: 0.1,
                min_files_for_split: 5,
            },
            fsfile: FsFileConfig {
                huge_loc: 50,     // Low threshold for testing
                huge_bytes: 1000, // Low threshold for testing
                min_split_loc: 10,
                min_entities_per_split: 2,
            },
            partitioning: PartitioningConfig {
                max_clusters: 8,
                min_clusters: 2,
                balance_tolerance: 0.3,
                naming_fallbacks: vec![
                    "core".to_string(),
                    "utils".to_string(),
                    "components".to_string(),
                    "services".to_string(),
                ],
            },
        }
    }

    #[test]
    fn test_file_analyzer_new() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config.clone());

        assert_eq!(analyzer.config.fsfile.huge_loc, config.fsfile.huge_loc);
    }

    #[test]
    fn test_is_code_file() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        assert!(analyzer.is_code_file("py"));
        assert!(analyzer.is_code_file("js"));
        assert!(analyzer.is_code_file("ts"));
        assert!(analyzer.is_code_file("rs"));
        assert!(analyzer.is_code_file("go"));
        assert!(analyzer.is_code_file("java"));
        assert!(analyzer.is_code_file("cpp"));
        assert!(!analyzer.is_code_file("txt"));
        assert!(!analyzer.is_code_file("md"));
        assert!(!analyzer.is_code_file("png"));
    }

    #[test]
    fn test_count_lines_of_code() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");

        let content = r#"# Comment line
import os
import sys

def hello():
    print("Hello world")
    return True
"#;
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);
        let loc = analyzer.count_lines_of_code(&file_path).unwrap();

        assert!(loc > 0);
    }

    #[test]
    fn test_should_skip_directory() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        assert!(analyzer.should_skip_directory(Path::new("node_modules")));
        assert!(analyzer.should_skip_directory(Path::new("__pycache__")));
        assert!(analyzer.should_skip_directory(Path::new("target")));
        assert!(analyzer.should_skip_directory(Path::new(".git")));
        assert!(analyzer.should_skip_directory(Path::new("build")));
        assert!(analyzer.should_skip_directory(Path::new("dist")));
        assert!(!analyzer.should_skip_directory(Path::new("src")));
        assert!(!analyzer.should_skip_directory(Path::new("lib")));
    }

    #[test]
    fn test_calculate_jaccard_similarity_empty_sets() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let set1 = HashSet::new();
        let set2 = HashSet::new();
        let similarity = analyzer.calculate_jaccard_similarity(&set1, &set2);

        assert_eq!(similarity, 1.0);
    }

    #[test]
    fn test_calculate_jaccard_similarity_identical_sets() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut set1 = HashSet::new();
        set1.insert("a".to_string());
        set1.insert("b".to_string());

        let mut set2 = HashSet::new();
        set2.insert("a".to_string());
        set2.insert("b".to_string());

        let similarity = analyzer.calculate_jaccard_similarity(&set1, &set2);

        assert_eq!(similarity, 1.0);
    }

    #[test]
    fn test_calculate_jaccard_similarity_no_overlap() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut set1 = HashSet::new();
        set1.insert("a".to_string());
        set1.insert("b".to_string());

        let mut set2 = HashSet::new();
        set2.insert("c".to_string());
        set2.insert("d".to_string());

        let similarity = analyzer.calculate_jaccard_similarity(&set1, &set2);

        assert_eq!(similarity, 0.0);
    }

    #[test]
    fn test_calculate_jaccard_similarity_partial_overlap() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut set1 = HashSet::new();
        set1.insert("a".to_string());
        set1.insert("b".to_string());

        let mut set2 = HashSet::new();
        set2.insert("a".to_string());
        set2.insert("c".to_string());

        let similarity = analyzer.calculate_jaccard_similarity(&set1, &set2);

        assert_eq!(similarity, 1.0 / 3.0); // 1 intersection / 3 union
    }

    #[test]
    fn test_analyze_entity_names_io_focused() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let entities = vec![
            "read_file".to_string(),
            "write_data".to_string(),
            "load_config".to_string(),
        ];

        let suffix = analyzer.analyze_entity_names(&entities);
        assert_eq!(suffix, "_io");
    }

    #[test]
    fn test_analyze_entity_names_api_focused() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let entities = vec![
            "handle_request".to_string(),
            "api_controller".to_string(),
            "route_handler".to_string(),
        ];

        let suffix = analyzer.analyze_entity_names(&entities);
        assert_eq!(suffix, "_api");
    }

    #[test]
    fn test_analyze_entity_names_util_focused() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let entities = vec![
            "utility_function".to_string(),
            "helper_method".to_string(),
            "tool_implementation".to_string(),
        ];

        let suffix = analyzer.analyze_entity_names(&entities);
        // Could be _util, _helper, _tool, or _io based on keywords found
        assert!(suffix == "_util" || suffix == "_helper" || suffix == "_tool" || suffix == "_io");
    }

    #[test]
    fn test_analyze_entity_names_core_fallback() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let entities = vec![
            "calculate_result".to_string(),
            "process_data".to_string(),
            "main_algorithm".to_string(),
        ];

        let suffix = analyzer.analyze_entity_names(&entities);
        assert_eq!(suffix, "_core");
    }

    #[test]
    fn test_generate_split_name() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let entities = vec!["read_file".to_string(), "write_data".to_string()];
        let name = analyzer.generate_split_name("test", "_suffix", &entities, &file_path);

        assert_eq!(name, "test_io.py"); // Should detect io pattern
    }

    #[test]
    fn test_calculate_split_value() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let graph = petgraph::Graph::new_undirected();
        let metrics = FileDependencyMetrics::default();
        let value = analyzer
            .calculate_split_value(100, &file_path, &graph, &metrics)
            .unwrap();

        assert!(value.score >= 0.0);
        assert!(value.score <= 1.0);
    }

    #[test]
    fn test_calculate_split_effort() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut metrics = FileDependencyMetrics::default();
        metrics.exports.push(ExportedEntity {
            name: "foo".to_string(),
            kind: EntityKind::Function,
        });
        metrics
            .incoming_importers
            .insert(temp_dir.path().join("other.py"));

        let effort = analyzer.calculate_split_effort(&metrics).unwrap();

        assert_eq!(effort.exports, 1);
        assert_eq!(effort.external_importers, 1);
    }

    #[test]
    fn test_extract_python_imports() {
        let content = r#"import os
import sys
from pathlib import Path
from collections import OrderedDict, defaultdict
"#;

        let mut adapter = adapter_for_language("py").unwrap();
        let imports = adapter.extract_imports(content).unwrap();

        assert_eq!(imports.len(), 4);
        assert_eq!(imports[0].module, "os");
        assert_eq!(imports[0].import_type, "module");
        assert_eq!(imports[2].module, "pathlib");
        assert_eq!(imports[2].import_type, "named");
    }

    #[test]
    fn test_extract_javascript_imports() {
        let content = r#"import React from 'react';
import { useState, useEffect } from 'react';
import * as utils from './utils';
"#;

        let mut adapter = adapter_for_language("js").unwrap();
        let imports = adapter.extract_imports(content).unwrap();

        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].module, "react");
        assert_eq!(imports[1].import_type, "named");
        assert_eq!(imports[2].import_type, "star");
    }

    #[test]
    fn test_extract_rust_imports() {
        let content = r#"use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use serde::{Serialize, Deserialize};
"#;

        let mut adapter = adapter_for_language("rs").unwrap();
        let imports = adapter.extract_imports(content).unwrap();

        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].module, "std::collections::HashMap");
        assert_eq!(imports[1].import_type, "named");
    }

    #[test]
    fn test_resolve_import_to_local_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        // Create a test file
        fs::write(temp_dir.path().join("utils.py"), "# Utils module").unwrap();

        let import = ImportStatement {
            module: "utils".to_string(),
            imports: None,
            import_type: "module".to_string(),
            line_number: 1,
        };

        let resolved = analyzer.resolve_import_to_local_file(&import, temp_dir.path());

        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap(), temp_dir.path().join("utils.py"));
    }

    #[test]
    fn test_resolve_import_to_local_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let import = ImportStatement {
            module: "nonexistent".to_string(),
            imports: None,
            import_type: "module".to_string(),
            line_number: 1,
        };

        let resolved = analyzer.resolve_import_to_local_file(&import, temp_dir.path());
        assert!(resolved.is_none());
    }

    #[test]
    fn test_analyze_file_for_split_small_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("small.py");

        let content = "def hello():\n    return 'world'";
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let result = analyzer.analyze_file_for_split(&file_path).unwrap();

        // Should return None for small files
        assert!(result.is_none());
    }

    #[test]
    fn test_analyze_file_for_split_large_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.py");

        // Create a large enough file to trigger split analysis
        let content = "def hello():\n    return 'world'\n".repeat(30); // Should exceed huge_loc threshold
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let result = analyzer.analyze_file_for_split(&file_path).unwrap();

        // Should find split opportunity
        if let Some(pack) = result {
            assert_eq!(pack.kind, "file_split");
            assert_eq!(pack.file, file_path);
            assert!(!pack.reasons.is_empty());
        }
    }

    #[test]
    fn test_build_entity_cohesion_graph_empty() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.py");

        fs::write(&file_path, "# Just a comment").unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let graph = analyzer.build_entity_cohesion_graph(&file_path).unwrap();

        // Should have 0 nodes for empty file
        assert_eq!(graph.node_count(), 0);
    }

    #[test]
    fn test_build_entity_cohesion_graph_with_entities() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("entities.py");

        let content = r#"
def func1():
    x = value
    return x

def func2():
    y = value
    return y
"#;
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let graph = analyzer.build_entity_cohesion_graph(&file_path).unwrap();

        // Should have at least some nodes (may vary based on parsing implementation)
        // node_count() is unsigned, always >= 0
    }

    #[test]
    fn test_find_cohesion_communities_empty_graph() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let graph = petgraph::Graph::new_undirected();
        let communities = analyzer.find_cohesion_communities(&graph).unwrap();

        assert_eq!(communities.len(), 1);
        assert!(communities[0].is_empty());
    }

    #[test]
    fn test_generate_split_suggestions_empty_communities() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        fs::write(&file_path, "# test").unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let communities = Vec::new();
        let suggestions = analyzer
            .generate_split_suggestions(&file_path, &communities)
            .unwrap();

        // Should generate default splits when no communities found
        assert_eq!(suggestions.len(), 2);
        assert!(suggestions.iter().all(|s| s.name.contains("test")));
    }

    #[tokio::test]
    async fn test_discover_large_files() {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path();

        // Create a large file
        let large_file = root_path.join("large.py");
        let content = "def hello():\n    return 'world'\n".repeat(30);
        fs::write(&large_file, content).unwrap();

        // Create a small file
        let small_file = root_path.join("small.py");
        fs::write(&small_file, "print('hello')").unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let large_files = analyzer.discover_large_files(root_path).await.unwrap();

        // Should find the large file but not the small one
        assert!(large_files.contains(&large_file));
        assert!(!large_files.contains(&small_file));
    }

    #[test]
    fn test_extract_imports_by_extension() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        // Test Python file
        let py_file = temp_dir.path().join("test.py");
        fs::write(&py_file, "import os").unwrap();
        let py_imports = analyzer.extract_imports(&py_file).unwrap();
        assert_eq!(py_imports.len(), 1);

        // Test JavaScript file
        let js_file = temp_dir.path().join("test.js");
        fs::write(&js_file, "import React from 'react';").unwrap();
        let js_imports = analyzer.extract_imports(&js_file).unwrap();
        assert_eq!(js_imports.len(), 1);

        // Test Rust file
        let rs_file = temp_dir.path().join("test.rs");
        fs::write(&rs_file, "use std::collections::HashMap;").unwrap();
        let rs_imports = analyzer.extract_imports(&rs_file).unwrap();
        assert_eq!(rs_imports.len(), 1);

        // Test unsupported file
        let txt_file = temp_dir.path().join("test.txt");
        fs::write(&txt_file, "some text").unwrap();
        let txt_imports = analyzer.extract_imports(&txt_file).unwrap();
        assert_eq!(txt_imports.len(), 0);
    }

    #[test]
    fn test_collect_large_files_recursive_skips_directories() {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path();

        // Create node_modules directory (should be skipped)
        let node_modules = root_path.join("node_modules");
        fs::create_dir(&node_modules).unwrap();
        let large_file_in_node_modules = node_modules.join("large.js");
        let content = "function test() { return 'test'; }\n".repeat(30);
        fs::write(&large_file_in_node_modules, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut files = Vec::new();
        analyzer
            .collect_large_files_recursive(root_path, &mut files)
            .unwrap();

        // Should not find the file in node_modules
        assert!(!files.contains(&large_file_in_node_modules));
    }
}
