//! File analysis, entity extraction, and file splitting logic

pub(crate) mod cohesion;
pub(crate) mod imports;
pub(crate) mod splitting;

use petgraph::Graph;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::core::ast_utils::count_named_nodes;
use crate::core::errors::Result;
use crate::core::file_utils::FileReader;
use crate::lang::common::EntityKind;
use crate::lang::registry::{adapter_for_file, get_tree_sitter_language};

use super::config::{
    is_code_extension, CohesionGraph, EntityNode, FileEntityHealth, FileMetrics, FileSplitPack,
    StructureConfig, SKIP_DIRECTORIES,
};
use super::health::HealthScorer;
use super::PrecomputedFileMetrics;

use cohesion::{build_cohesion_graph, calculate_jaccard_similarity, CommunityFinder};
use imports::ImportResolver;
use splitting::SplitAnalyzer;

// Re-export for backward compatibility
pub use cohesion::estimate_clone_factor;
pub use imports::{ExportedEntity, FileDependencyMetrics, ProjectImportSnapshot};
pub use splitting::analyze_entity_names;

/// Analyzer for file-level structure metrics and splitting recommendations.
pub struct FileAnalyzer {
    config: StructureConfig,
    import_resolver: ImportResolver,
}

/// Factory, metrics, cohesion, and splitting methods for [`FileAnalyzer`].
impl FileAnalyzer {
    /// Creates a new file analyzer with the given configuration.
    pub fn new(config: StructureConfig) -> Self {
        Self {
            config,
            import_resolver: ImportResolver::new(),
        }
    }

    /// Check if file extension indicates a code file
    pub fn is_code_file(&self, extension: &str) -> bool {
        is_code_extension(extension)
    }

    /// Count lines of code in a file
    pub fn count_lines_of_code(&self, file_path: &Path) -> Result<usize> {
        FileReader::count_lines_of_code(file_path)
    }

    /// Calculate a lognormal distribution-based score for file size.
    pub fn calculate_lognormal_score(&self, value: usize, optimal: usize, percentile_95: usize) -> f64 {
        if value == 0 || optimal == 0 || percentile_95 <= optimal {
            return if value == optimal { 1.0 } else { 0.0 };
        }

        let value = value as f64;
        let optimal = optimal as f64;
        let p95 = percentile_95 as f64;

        let log_ratio = p95.ln() - optimal.ln();

        let discriminant = 1.645_f64 * 1.645_f64 + 4.0 * log_ratio;
        if discriminant < 0.0 {
            return 0.0;
        }

        let sigma = (-1.645 + discriminant.sqrt()) / 2.0;
        if sigma <= 0.0 {
            return if (value - optimal).abs() < 0.001 { 1.0 } else { 0.0 };
        }

        let mu = optimal.ln() + sigma * sigma;

        let log_value = value.ln();
        let log_value_centered = log_value - mu;

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
    pub fn calculate_file_metrics(&self, file_path: &Path) -> Result<FileMetrics> {
        let content = FileReader::read_to_string(file_path)?;
        let loc = content.lines().filter(|line| !line.trim().is_empty()).count();

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

        for entity in &entities {
            let health = match entity.entity_type.as_str() {
                "class" | "struct" | "interface" | "enum" => scorer.score_class(entity.ast_nodes),
                _ => scorer.score_function(entity.ast_nodes),
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

    /// Internal implementation of file split analysis.
    fn analyze_file_for_split_internal(
        &self,
        file_path: &Path,
        project_root: Option<&Path>,
    ) -> Result<Option<FileSplitPack>> {
        let metadata = std::fs::metadata(file_path)?;
        let size_bytes = metadata.len() as usize;
        let loc = self.count_lines_of_code(file_path)?;
        let cohesion_graph = self.build_entity_cohesion_graph(file_path)?;
        let community_finder = CommunityFinder::new(&self.config);
        let communities = community_finder.find_communities(&cohesion_graph)?;
        let dependency_metrics = self.import_resolver.collect_dependency_metrics(file_path, project_root)?;

        let split_analyzer = SplitAnalyzer::new(&self.config);
        split_analyzer.build_split_pack(
            file_path,
            loc,
            size_bytes,
            &cohesion_graph,
            communities,
            &dependency_metrics,
        )
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
        let community_finder = CommunityFinder::new(&self.config);
        let communities = community_finder.find_communities(&cohesion_graph)?;
        let dependency_metrics = self.import_resolver.collect_dependency_metrics(file_path, Some(project_root))?;

        let split_analyzer = SplitAnalyzer::new(&self.config);
        split_analyzer.build_split_pack(
            file_path,
            loc,
            size_bytes,
            &cohesion_graph,
            communities,
            &dependency_metrics,
        )
    }

    /// Build entity cohesion graph from pre-loaded source (avoids file I/O)
    pub fn build_entity_cohesion_graph_from_source(
        &self,
        file_path: &Path,
        source: &str,
    ) -> Result<CohesionGraph> {
        let entities = self.extract_entities_with_treesitter(file_path, source)?;
        Ok(build_cohesion_graph(entities))
    }

    /// Build entity cohesion graph for file
    pub fn build_entity_cohesion_graph(&self, file_path: &Path) -> Result<CohesionGraph> {
        let content = FileReader::read_to_string(file_path)?;
        self.build_entity_cohesion_graph_from_source(file_path, &content)
    }

    /// Find cohesion communities in entity graph
    pub fn find_cohesion_communities(
        &self,
        graph: &CohesionGraph,
    ) -> Result<Vec<Vec<petgraph::graph::NodeIndex>>> {
        let community_finder = CommunityFinder::new(&self.config);
        community_finder.find_communities(graph)
    }

    /// Generate split file suggestions
    pub fn generate_split_suggestions(
        &self,
        file_path: &Path,
        communities: &[Vec<petgraph::graph::NodeIndex>],
    ) -> Result<Vec<super::config::SuggestedSplit>> {
        let cohesion_graph = self.build_entity_cohesion_graph(file_path)?;
        let split_analyzer = SplitAnalyzer::new(&self.config);
        split_analyzer.generate_split_suggestions(file_path, communities, &cohesion_graph)
    }

    /// Generate a meaningful name for a split file based on entity analysis
    pub fn generate_split_name(
        &self,
        base_name: &str,
        suffix: &str,
        entities: &[String],
        file_path: &Path,
    ) -> String {
        let split_analyzer = SplitAnalyzer::new(&self.config);
        split_analyzer.generate_split_name(base_name, suffix, entities, file_path)
    }

    /// Analyze entity names to suggest appropriate suffixes
    pub fn analyze_entity_names(&self, entities: &[String]) -> String {
        analyze_entity_names(entities)
    }

    /// Calculate value score for file splitting
    pub fn calculate_split_value(
        &self,
        loc: usize,
        _file_path: &Path,
        cohesion_graph: &CohesionGraph,
        metrics: &FileDependencyMetrics,
    ) -> Result<super::config::SplitValue> {
        let split_analyzer = SplitAnalyzer::new(&self.config);
        split_analyzer.calculate_split_value(loc, cohesion_graph, metrics)
    }

    /// Calculate effort required for file splitting
    pub fn calculate_split_effort(
        &self,
        metrics: &FileDependencyMetrics,
    ) -> Result<super::config::SplitEffort> {
        let split_analyzer = SplitAnalyzer::new(&self.config);
        split_analyzer.calculate_split_effort(metrics)
    }

    /// Calculate Jaccard similarity between two symbol sets
    pub fn calculate_jaccard_similarity(&self, a: &HashSet<String>, b: &HashSet<String>) -> f64 {
        calculate_jaccard_similarity(a, b)
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

    /// Extracts entities from a language adapter.
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

    /// Checks if an entity kind is supported for analysis.
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

    /// Extract imports from a file
    pub fn extract_imports(
        &self,
        file_path: &Path,
    ) -> Result<Vec<super::config::ImportStatement>> {
        self.import_resolver.extract_imports(file_path)
    }

    /// Resolve import statement to local file path
    pub fn resolve_import_to_local_file(
        &self,
        import: &super::config::ImportStatement,
        dir_path: &Path,
    ) -> Option<PathBuf> {
        self.import_resolver.resolve_import_to_local_file(import, dir_path)
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
            } else if self.is_large_code_file(&child_path)? {
                files.push(child_path);
            }
        }

        Ok(())
    }

    /// Check if a file is a large code file that should be collected.
    fn is_large_code_file(&self, path: &Path) -> Result<bool> {
        if !path.is_file() {
            return Ok(false);
        }

        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(ext) if self.is_code_file(ext) => ext,
            _ => return Ok(false),
        };
        let _ = ext; // Used in the match guard above

        let metadata = std::fs::metadata(path)?;
        let size_bytes = metadata.len() as usize;

        if size_bytes >= self.config.fsfile.huge_bytes {
            return Ok(true);
        }

        let loc = self.count_lines_of_code(path)?;
        Ok(loc >= self.config.fsfile.huge_loc)
    }

    /// Check if directory should be skipped
    pub fn should_skip_directory(&self, path: &Path) -> bool {
        self.import_resolver.should_skip_directory(path)
    }

    /// Estimate clone factor from cohesion graph
    pub fn estimate_clone_factor(&self, graph: &CohesionGraph) -> f64 {
        estimate_clone_factor(graph)
    }

    /// Check if line has a keyword
    pub fn line_has_keyword(&self, content: &str, start_line: usize, keyword: &str) -> bool {
        self.import_resolver.line_has_keyword(content, start_line, keyword)
    }

    /// Canonicalize a path for consistent comparison
    pub fn canonicalize_path(&self, path: &Path) -> PathBuf {
        self.import_resolver.canonicalize_path(path)
    }

    /// Collect dependency metrics for a file
    pub fn collect_dependency_metrics(
        &self,
        file_path: &Path,
        project_root: Option<&Path>,
        _cohesion_graph: &CohesionGraph,
    ) -> Result<FileDependencyMetrics> {
        self.import_resolver.collect_dependency_metrics(file_path, project_root)
    }

    /// Resolve candidate path to an existing file
    pub fn resolve_candidate_path(&self, candidate: &Path) -> Option<PathBuf> {
        self.import_resolver.resolve_candidate_path(candidate)
    }

    /// Get directory module fallback paths
    pub fn directory_module_fallbacks(&self, dir: &Path) -> Vec<PathBuf> {
        self.import_resolver.directory_module_fallbacks(dir)
    }

    /// Supported file extensions for import resolution
    pub fn supported_extensions() -> &'static [&'static str] {
        ImportResolver::supported_extensions()
    }

    /// Collect all code files in a project
    pub fn collect_project_code_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        self.import_resolver.collect_project_code_files(root)
    }

    /// Check if an entity is exported based on language conventions
    pub fn is_entity_exported(
        &self,
        entity: &crate::lang::common::ParsedEntity,
        file_path: &Path,
        content: &str,
    ) -> bool {
        self.import_resolver.is_entity_exported(entity, file_path, content)
    }
}


#[cfg(test)]
mod tests;
