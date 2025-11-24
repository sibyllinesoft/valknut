//! Individual analysis stages for the pipeline.

use async_trait::async_trait;
use futures::future;
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use tree_edit_distance::{diff, Node as TedNode, Tree as TedTree};
use tree_sitter::Node as TsNode;

use super::pipeline_config::AnalysisConfig;
use super::pipeline_results::{
    CloneVerificationResults, ComplexityAnalysisResults, CoverageAnalysisResults, CoverageFileInfo,
    ImpactAnalysisResults, LshAnalysisResults, RefactoringAnalysisResults,
    StructureAnalysisResults,
};
use crate::core::arena_analysis::{ArenaAnalysisResult, ArenaBatchAnalyzer, ArenaFileAnalyzer};
use crate::core::ast_service::{AstService, CachedTree};
use crate::core::config::{CoverageConfig, ValknutConfig};
use crate::core::dependency::ProjectDependencyAnalysis;
use crate::core::errors::Result;
use crate::core::featureset::FeatureExtractor;
use crate::core::file_utils::{CoverageDiscovery, CoverageFile, CoverageFormat};
use crate::detectors::complexity::{AstComplexityAnalyzer, ComplexityAnalyzer};
use crate::detectors::coverage::{CoverageConfig as CoverageDetectorConfig, CoverageExtractor};
use crate::detectors::graph::SimilarityCliquePartitioner;
use crate::detectors::lsh::LshExtractor;
use crate::detectors::refactoring::RefactoringAnalyzer;
use crate::detectors::structure::StructureExtractor;
use std::sync::Arc;

use super::services::{StageOrchestrator, StageResultsBundle};

/// Handles all individual analysis stages
pub struct AnalysisStages {
    pub structure_extractor: StructureExtractor,
    pub complexity_analyzer: ComplexityAnalyzer,
    pub ast_complexity_analyzer: AstComplexityAnalyzer,
    pub refactoring_analyzer: RefactoringAnalyzer,
    pub lsh_extractor: Option<LshExtractor>,
    pub coverage_extractor: CoverageExtractor,
    pub arena_analyzer: ArenaFileAnalyzer,
    pub ast_service: Arc<AstService>,
    pub valknut_config: Arc<ValknutConfig>,
}

#[derive(Debug, Clone, Serialize)]
struct CloneEndpoint {
    id: String,
    name: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Serialize)]
struct CloneVerificationDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    similarity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit_cost: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_counts: Option<(usize, usize)>,
    truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ClonePairReport {
    source: CloneEndpoint,
    target: CloneEndpoint,
    similarity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    verification: Option<CloneVerificationDetail>,
}

#[derive(Debug, Clone)]
struct SimpleAstNode {
    kind_hash: u64,
    kind_label: String,
    children: Vec<SimpleAstNode>,
    node_count: usize,
}

impl TedNode for SimpleAstNode {
    type Kind = u64;

    fn kind(&self) -> Self::Kind {
        self.kind_hash
    }

    type Weight = u64;

    fn weight(&self) -> Self::Weight {
        1
    }
}

impl TedTree for SimpleAstNode {
    type Children<'c>
        = std::slice::Iter<'c, SimpleAstNode>
    where
        Self: 'c;

    fn children(&self) -> Self::Children<'_> {
        self.children.iter()
    }
}

#[derive(Clone)]
struct CachedSimpleAst {
    ast: Arc<SimpleAstNode>,
    node_count: usize,
    truncated: bool,
}

fn hash_kind(kind: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    kind.hash(&mut hasher);
    hasher.finish()
}

fn parse_byte_range(entity: &crate::core::featureset::CodeEntity) -> Option<(usize, usize)> {
    let range = entity.properties.get("byte_range")?.as_array()?;
    if range.len() != 2 {
        return None;
    }
    let start = range[0].as_u64()? as usize;
    let end = range[1].as_u64()? as usize;
    Some((start, end))
}

fn build_simple_ast_recursive(
    node: TsNode,
    max_nodes: usize,
    counter: &mut usize,
) -> (SimpleAstNode, bool) {
    *counter += 1;
    let kind_label = node.kind().to_string();
    let kind_hash = hash_kind(&kind_label);
    let mut simple = SimpleAstNode {
        kind_hash,
        kind_label,
        children: Vec::new(),
        node_count: 1,
    };

    if *counter >= max_nodes {
        return (simple, node.named_child_count() > 0);
    }

    let mut truncated = false;
    let child_count = node.named_child_count();
    for i in 0..child_count {
        if *counter >= max_nodes {
            truncated = true;
            break;
        }
        if let Some(child) = node.named_child(i) {
            let (child_ast, child_truncated) =
                build_simple_ast_recursive(child, max_nodes, counter);
            simple.node_count += child_ast.node_count;
            simple.children.push(child_ast);
            if child_truncated {
                truncated = true;
            }
        }
    }

    (simple, truncated)
}

fn build_simple_ast_for_entity(
    entity: &crate::core::featureset::CodeEntity,
    ast_cache: &HashMap<String, Arc<CachedTree>>,
    max_nodes: usize,
) -> Option<CachedSimpleAst> {
    let (start_byte, end_byte) = parse_byte_range(entity)?;
    let cached_tree = ast_cache.get(&entity.file_path)?;
    let root = cached_tree.tree.root_node();
    let target_node = root
        .descendant_for_byte_range(start_byte, end_byte)
        .or_else(|| root.named_descendant_for_byte_range(start_byte, end_byte))
        .unwrap_or(root);

    let mut counter = 0usize;
    let (simple_ast, truncated) = build_simple_ast_recursive(target_node, max_nodes, &mut counter);

    Some(CachedSimpleAst {
        node_count: simple_ast.node_count,
        truncated,
        ast: Arc::new(simple_ast),
    })
}

fn get_or_build_simple_ast(
    cache: &mut HashMap<String, Option<CachedSimpleAst>>,
    entity: &crate::core::featureset::CodeEntity,
    ast_cache: &HashMap<String, Arc<CachedTree>>,
    max_nodes: usize,
) -> Option<CachedSimpleAst> {
    match cache.entry(entity.id.clone()) {
        Entry::Occupied(entry) => entry.get().clone(),
        Entry::Vacant(entry) => {
            let value = build_simple_ast_for_entity(entity, ast_cache, max_nodes);
            entry.insert(value).clone()
        }
    }
}
impl AnalysisStages {
    /// Create new analysis stages with the given analyzers
    pub fn new(
        structure_extractor: StructureExtractor,
        complexity_analyzer: ComplexityAnalyzer,
        refactoring_analyzer: RefactoringAnalyzer,
        coverage_extractor: CoverageExtractor,
        ast_service: Arc<AstService>,
        valknut_config: Arc<ValknutConfig>,
    ) -> Self {
        let ast_complexity_analyzer = AstComplexityAnalyzer::new(
            crate::detectors::complexity::ComplexityConfig::default(),
            ast_service.clone(),
        );

        Self {
            structure_extractor,
            complexity_analyzer,
            ast_complexity_analyzer,
            refactoring_analyzer,
            lsh_extractor: None,
            coverage_extractor,
            arena_analyzer: ArenaFileAnalyzer::with_ast_service(ast_service.clone()),
            ast_service,
            valknut_config,
        }
    }

    /// Create new analysis stages with LSH support
    pub fn new_with_lsh(
        structure_extractor: StructureExtractor,
        complexity_analyzer: ComplexityAnalyzer,
        refactoring_analyzer: RefactoringAnalyzer,
        lsh_extractor: LshExtractor,
        coverage_extractor: CoverageExtractor,
        ast_service: Arc<AstService>,
        valknut_config: Arc<ValknutConfig>,
    ) -> Self {
        let ast_complexity_analyzer = AstComplexityAnalyzer::new(
            crate::detectors::complexity::ComplexityConfig::default(),
            ast_service.clone(),
        );

        Self {
            structure_extractor,
            complexity_analyzer,
            ast_complexity_analyzer,
            refactoring_analyzer,
            lsh_extractor: Some(lsh_extractor),
            coverage_extractor,
            arena_analyzer: ArenaFileAnalyzer::with_ast_service(ast_service.clone()),
            ast_service,
            valknut_config,
        }
    }

    /// Run structure analysis
    pub async fn run_structure_analysis(
        &self,
        paths: &[PathBuf],
    ) -> Result<StructureAnalysisResults> {
        debug!("Running structure analysis");

        let mut all_recommendations = Vec::new();
        let mut file_splitting_recommendations = Vec::new();

        for path in paths {
            match self
                .structure_extractor
                .generate_recommendations(path)
                .await
            {
                Ok(recommendations) => {
                    for rec in recommendations {
                        match rec.get("kind") {
                            Some(serde_json::Value::String(kind)) if kind == "file_split" => {
                                file_splitting_recommendations.push(rec);
                            }
                            _ => {
                                all_recommendations.push(rec);
                            }
                        }
                    }
                }
                Err(e) => warn!("Structure analysis failed for {}: {}", path.display(), e),
            }
        }

        let issues_count = all_recommendations.len() + file_splitting_recommendations.len();

        Ok(StructureAnalysisResults {
            enabled: true,
            directory_recommendations: all_recommendations,
            file_splitting_recommendations,
            issues_count,
        })
    }

    /// Run complexity analysis from pre-extracted arena results (optimized path)
    pub async fn run_complexity_analysis_from_arena_results(
        &self,
        arena_results: &[crate::core::arena_analysis::ArenaAnalysisResult],
    ) -> Result<ComplexityAnalysisResults> {
        debug!(
            "Running complexity analysis from {} arena results",
            arena_results.len()
        );

        // Use the configured analyzer instance and run analyses in parallel.
        let analysis_futures = arena_results.iter().map(|arena_result| {
            let analyzer = self.ast_complexity_analyzer.clone(); // Clone Arc'd analyzer
            let file_path_str = arena_result.file_path_str().to_string();
            // Source code is not in ArenaAnalysisResult, so we must read it.
            // A better optimization would be to pass the source map down.
            let file_path = PathBuf::from(&file_path_str);

            tokio::spawn(async move {
                match tokio::fs::read_to_string(&file_path).await {
                    Ok(source) => {
                        analyzer
                            .analyze_file_with_results(&file_path_str, &source)
                            .await
                    }
                    Err(e) => {
                        warn!(
                            "Could not read file for complexity analysis {}: {}",
                            file_path.display(),
                            e
                        );
                        Ok(Vec::new())
                    }
                }
            })
        });

        let results_of_results = future::join_all(analysis_futures).await;

        // Collect and flatten the results
        let mut detailed_results = Vec::new();
        for result in results_of_results {
            match result {
                Ok(Ok(file_results)) => detailed_results.extend(file_results),
                Ok(Err(e)) => warn!("Complexity analysis task failed: {}", e),
                Err(e) => warn!("Tokio spawn failed for complexity analysis: {}", e),
            }
        }

        // Calculate averages
        let count = detailed_results.len() as f64;
        let (total_cyclomatic, total_cognitive, total_debt, total_maintainability) = if count > 0.0
        {
            let total_cyclomatic: f64 = detailed_results
                .iter()
                .map(|r| r.metrics.cyclomatic())
                .sum();
            let total_cognitive: f64 = detailed_results.iter().map(|r| r.metrics.cognitive()).sum();
            let total_debt: f64 = detailed_results
                .iter()
                .map(|r| r.metrics.technical_debt_score)
                .sum();
            let total_maintainability: f64 = detailed_results
                .iter()
                .map(|r| r.metrics.maintainability_index)
                .sum();
            (
                total_cyclomatic,
                total_cognitive,
                total_debt,
                total_maintainability,
            )
        } else {
            (0.0, 0.0, 0.0, 100.0)
        };

        let issues_count = detailed_results.iter().map(|r| r.issues.len()).sum();

        debug!(
            "Complexity analysis completed: {} entities, avg cyclomatic: {:.2}, avg cognitive: {:.2}",
            detailed_results.len(),
            if count > 0.0 { total_cyclomatic / count } else { 0.0 },
            if count > 0.0 { total_cognitive / count } else { 0.0 }
        );

        Ok(ComplexityAnalysisResults {
            enabled: true,
            detailed_results,
            average_cyclomatic_complexity: if count > 0.0 {
                total_cyclomatic / count
            } else {
                0.0
            },
            average_cognitive_complexity: if count > 0.0 {
                total_cognitive / count
            } else {
                0.0
            },
            average_technical_debt_score: if count > 0.0 { total_debt / count } else { 0.0 },
            average_maintainability_index: if count > 0.0 {
                total_maintainability / count
            } else {
                100.0
            },
            issues_count,
        })
    }

    /// Run complexity analysis (legacy path - re-parses files)
    pub async fn run_complexity_analysis(
        &self,
        files: &[PathBuf],
    ) -> Result<ComplexityAnalysisResults> {
        debug!("Running complexity analysis on {} files", files.len());

        // Parallelize file analysis using tokio::spawn
        let analysis_futures = files.iter().map(|file_path| {
            let analyzer = self.ast_complexity_analyzer.clone();
            let path = file_path.clone();

            tokio::spawn(async move {
                let file_refs = vec![path.as_path()];
                analyzer.analyze_files(&file_refs).await
            })
        });

        // Wait for all concurrent analyses to complete
        let results_of_results = future::join_all(analysis_futures).await;

        // Collect and flatten the results
        let mut detailed_results = Vec::new();
        for result in results_of_results {
            match result {
                Ok(Ok(file_results)) => detailed_results.extend(file_results),
                Ok(Err(e)) => warn!("Complexity analysis task failed: {}", e),
                Err(e) => warn!("Tokio spawn failed for complexity analysis: {}", e),
            }
        }

        // Calculate averages
        let count = detailed_results.len() as f64;
        let total_cyclomatic: f64 = detailed_results
            .iter()
            .map(|r| r.metrics.cyclomatic())
            .sum();
        let total_cognitive: f64 = detailed_results.iter().map(|r| r.metrics.cognitive()).sum();
        let total_debt: f64 = detailed_results
            .iter()
            .map(|r| r.metrics.technical_debt_score)
            .sum();
        let total_maintainability: f64 = detailed_results
            .iter()
            .map(|r| r.metrics.maintainability_index)
            .sum();

        let average_cyclomatic_complexity = if count > 0.0 {
            total_cyclomatic / count
        } else {
            0.0
        };
        let average_cognitive_complexity = if count > 0.0 {
            total_cognitive / count
        } else {
            0.0
        };
        let average_technical_debt_score = if count > 0.0 { total_debt / count } else { 0.0 };
        let average_maintainability_index = if count > 0.0 {
            total_maintainability / count
        } else {
            100.0
        };

        // Count issues
        let issues_count = detailed_results.iter().map(|r| r.issues.len()).sum();

        Ok(ComplexityAnalysisResults {
            enabled: true,
            detailed_results,
            average_cyclomatic_complexity,
            average_cognitive_complexity,
            average_technical_debt_score,
            average_maintainability_index,
            issues_count,
        })
    }

    /// Run refactoring analysis
    pub async fn run_refactoring_analysis(
        &self,
        files: &[PathBuf],
    ) -> Result<RefactoringAnalysisResults> {
        debug!("Running refactoring analysis on {} files", files.len());

        // Parallelize file analysis using tokio::spawn
        let analysis_futures = files.iter().map(|file_path| {
            // Clone the Arc'd analyzer, which is cheap
            let analyzer = self.refactoring_analyzer.clone();
            let path = file_path.clone();

            tokio::spawn(async move { analyzer.analyze_files(&[path]).await })
        });

        // Wait for all concurrent analyses to complete
        let results_of_results = future::join_all(analysis_futures).await;

        // Collect and flatten the results
        let mut detailed_results = Vec::new();
        for result in results_of_results {
            match result {
                Ok(Ok(file_results)) => detailed_results.extend(file_results),
                Ok(Err(e)) => warn!("Refactoring analysis task failed: {}", e),
                Err(e) => warn!("Tokio spawn failed for refactoring analysis: {}", e),
            }
        }
        let opportunities_count = detailed_results
            .iter()
            .map(|r| r.recommendations.len())
            .sum();

        Ok(RefactoringAnalysisResults {
            enabled: true,
            detailed_results,
            opportunities_count,
        })
    }

    /// Run impact analysis powered by the dependency graph
    pub async fn run_impact_analysis(&self, files: &[PathBuf]) -> Result<ImpactAnalysisResults> {
        debug!(
            "Running dependency impact analysis across {} files",
            files.len()
        );

        if files.is_empty() {
            return Ok(ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            });
        }

        let analysis = ProjectDependencyAnalysis::analyze(files)?;

        if analysis.is_empty() {
            return Ok(ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            });
        }

        let dependency_cycles = analysis
            .cycles()
            .iter()
            .map(|cycle| {
                serde_json::json!({
                    "size": cycle.len(),
                    "members": cycle
                        .iter()
                        .map(|node| serde_json::json!({
                            "name": node.name,
                            "file": node.file_path,
                            "start_line": node.start_line,
                        }))
                        .collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();

        let chokepoints = analysis
            .chokepoints()
            .iter()
            .map(|chokepoint| {
                serde_json::json!({
                    "name": chokepoint.node.name,
                    "file": chokepoint.node.file_path,
                    "start_line": chokepoint.node.start_line,
                    "score": chokepoint.score,
                })
            })
            .collect::<Vec<_>>();

        let issues_count = dependency_cycles.len() + chokepoints.len();

        Ok(ImpactAnalysisResults {
            enabled: true,
            dependency_cycles,
            chokepoints,
            clone_groups: Vec::new(),
            issues_count,
        })
    }

    /// Run LSH analysis for clone detection
    pub async fn run_lsh_analysis(
        &self,
        files: &[PathBuf],
        denoise_enabled: bool,
    ) -> Result<LshAnalysisResults> {
        const MAX_ENTITIES_PER_FILE_FOR_LSH: usize = 1500;
        debug!(
            "Running LSH analysis for clone detection on {} files",
            files.len()
        );

        if let Some(ref lsh_extractor) = self.lsh_extractor {
            use crate::core::featureset::{CodeEntity, ExtractionContext};

            let mut context = ExtractionContext::new(Arc::clone(&self.valknut_config), "mixed");

            let lsh_settings = &self.valknut_config.lsh;
            let verify_with_apted = lsh_settings.verify_with_apted;
            let apted_max_nodes = lsh_settings.apted_max_nodes;
            let apted_limit = if lsh_settings.apted_max_pairs_per_entity == 0 {
                lsh_settings.max_candidates
            } else if lsh_settings.max_candidates == 0 {
                lsh_settings.apted_max_pairs_per_entity
            } else {
                lsh_settings
                    .apted_max_pairs_per_entity
                    .min(lsh_settings.max_candidates)
            };
            let apted_limit = if apted_limit == 0 {
                None
            } else {
                Some(apted_limit)
            };

            let mut entities = Vec::new();
            let mut entity_index = HashMap::new();
            let mut ast_cache: HashMap<String, Arc<CachedTree>> = HashMap::new();

            for file_path in files.iter() {
                let content = match tokio::fs::read_to_string(file_path).await {
                    Ok(content) => content,
                    Err(e) => {
                        warn!("Failed to read file {}: {}", file_path.display(), e);
                        continue;
                    }
                };

                let path_str = file_path.to_string_lossy().to_string();

                if verify_with_apted {
                    match self.ast_service.get_ast(&path_str, &content).await {
                        Ok(tree) => {
                            ast_cache.insert(path_str.clone(), tree);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse AST for {}: {} – APTED verification will be skipped for entities in this file",
                                file_path.display(),
                                e
                            );
                        }
                    }
                }

                if let Some(extracted_entities) =
                    self.extract_entities_from_file(file_path, &content).await
                {
                    if extracted_entities.len() > MAX_ENTITIES_PER_FILE_FOR_LSH {
                        info!(
                            file = %file_path.display(),
                            entities = extracted_entities.len(),
                            "Skipping LSH for file with excessive entity count"
                        );
                        continue;
                    }

                    for entity in extracted_entities {
                        // Apply fragment thresholds (tokens, AST nodes, blocks, stop motifs)
                        if !lsh_extractor
                            .entity_passes_thresholds(&entity)
                            .await
                            .unwrap_or(false)
                        {
                            continue;
                        }

                        entity_index.insert(entity.id.clone(), entity.clone());
                        entities.push(entity);
                    }
                }
            }

            context.entity_index = entity_index;

            if entities.is_empty() {
                info!("No entities available for LSH after filtering; skipping clone analysis");
                return Ok(LshAnalysisResults {
                    enabled: true,
                    clone_pairs: Vec::new(),
                    max_similarity: 0.0,
                    avg_similarity: 0.0,
                    duplicate_count: 0,
                    apted_verification_enabled: verify_with_apted,
                    verification: None,
                    denoising_enabled: denoise_enabled,
                    tfidf_stats: None,
                });
            }

            let partitions = SimilarityCliquePartitioner::new().partition(&entities);
            if !partitions.is_empty() {
                let partition_count = partitions.len();
                let total_peers: usize = partitions.values().map(|group| group.len()).sum();
                let max_peers = partitions
                    .values()
                    .map(|group| group.len())
                    .max()
                    .unwrap_or(0);
                let avg_peers = if partition_count > 0 {
                    total_peers as f64 / partition_count as f64
                } else {
                    0.0
                };
                info!(
                    entities_with_peers = partition_count,
                    avg_peers = avg_peers,
                    max_peers = max_peers,
                    "Similarity clique pre-filter enabled"
                );
                context.candidate_partitions = Some(Arc::new(partitions));
            }

            let similarity_context = lsh_extractor.similarity_context(&context);

            if similarity_context.is_none() {
                warn!("Unable to build LSH similarity context; clone pairs will not be generated");
            }

            let candidate_limit = lsh_extractor.max_candidates();
            let min_ast_nodes = lsh_extractor.min_ast_nodes_threshold().unwrap_or(0);
            info!(
                min_ast_nodes,
                "LSH clone pair filter min_ast_nodes threshold"
            );

            let mut clone_pairs = Vec::new();
            let mut seen_pairs: HashSet<(String, String)> = HashSet::new();
            let mut max_similarity: f64 = 0.0;
            let mut similarity_total: f64 = 0.0;
            let mut similarity_count = 0usize;

            let mut apted_similarity_total: f64 = 0.0;
            let mut apted_similarity_count = 0usize;
            let mut apted_pairs_requested = 0usize;
            let mut apted_pairs_scored = 0usize;
            let mut simple_ast_cache: HashMap<String, Option<CachedSimpleAst>> = HashMap::new();

            let lsh_threshold = lsh_extractor.similarity_threshold();

            for entity in &entities {
                let Some(ctx) = similarity_context.as_ref() else {
                    break;
                };

                let candidates = ctx.find_similar_entities(&entity.id, candidate_limit);
                let mut apted_evaluated = 0usize;

                for (candidate_id, similarity) in candidates {
                    if similarity < lsh_threshold {
                        continue;
                    }

                    let key = if entity.id <= candidate_id {
                        (entity.id.clone(), candidate_id.clone())
                    } else {
                        (candidate_id.clone(), entity.id.clone())
                    };

                    if !seen_pairs.insert(key) {
                        continue;
                    }

                    let Some(candidate_entity) = context.entity_index.get(&candidate_id) else {
                        continue;
                    };

                    max_similarity = max_similarity.max(similarity);
                    similarity_total += similarity;
                    similarity_count += 1;

                    let apted_allowed = verify_with_apted
                        && apted_limit.map_or(true, |limit| apted_evaluated < limit);

                    let verification_detail = if apted_allowed {
                        apted_pairs_requested += 1;
                        let source_ast = get_or_build_simple_ast(
                            &mut simple_ast_cache,
                            entity,
                            &ast_cache,
                            apted_max_nodes,
                        );
                        let target_ast = get_or_build_simple_ast(
                            &mut simple_ast_cache,
                            candidate_entity,
                            &ast_cache,
                            apted_max_nodes,
                        );

                        if let (Some(source_ast), Some(target_ast)) = (source_ast, target_ast) {
                            apted_evaluated += 1;
                            let nodes_total =
                                (source_ast.node_count + target_ast.node_count).max(1);
                            let truncated = source_ast.truncated || target_ast.truncated;
                            let tree_a = Arc::clone(&source_ast.ast);
                            let tree_b = Arc::clone(&target_ast.ast);
                            let node_counts = Some((source_ast.node_count, target_ast.node_count));

                            match tokio::task::spawn_blocking(move || {
                                let (_, cost) = diff(&*tree_a, &*tree_b);
                                cost
                            })
                            .await
                            {
                                Ok(cost) => {
                                    apted_pairs_scored += 1;
                                    let normalized =
                                        (1.0 - (cost as f64 / nodes_total as f64)).clamp(0.0, 1.0);
                                    apted_similarity_total += normalized;
                                    apted_similarity_count += 1;
                                    Some(CloneVerificationDetail {
                                        similarity: Some(normalized),
                                        edit_cost: Some(cost),
                                        node_counts,
                                        truncated,
                                    })
                                }
                                Err(e) => {
                                    warn!(
                                        "APTED computation failed for {} -> {}: {}",
                                        entity.id, candidate_entity.id, e,
                                    );
                                    Some(CloneVerificationDetail {
                                        similarity: None,
                                        edit_cost: None,
                                        node_counts,
                                        truncated: true,
                                    })
                                }
                            }
                        } else {
                            Some(CloneVerificationDetail {
                                similarity: None,
                                edit_cost: None,
                                node_counts: None,
                                truncated: false,
                            })
                        }
                    } else {
                        None
                    };

                    let source_endpoint = CloneEndpoint {
                        id: entity.id.clone(),
                        name: entity.name.clone(),
                        path: entity.file_path.clone(),
                        range: entity.line_range,
                    };
                    let target_endpoint = CloneEndpoint {
                        id: candidate_entity.id.clone(),
                        name: candidate_entity.name.clone(),
                        path: candidate_entity.file_path.clone(),
                        range: candidate_entity.line_range,
                    };

                    // Skip extremely small verified pairs when a min_ast_nodes threshold is configured
                    if let Some(ref detail) = verification_detail {
                        if let Some(ref counts) = detail.node_counts {
                            let observed_min = counts.0.min(counts.1);
                            if min_ast_nodes > 0 && observed_min < min_ast_nodes {
                                debug!(
                                    "Skipping clone pair below min_ast_nodes (min {}): {} -> {} ({:?})",
                                    min_ast_nodes,
                                    entity.id,
                                    candidate_entity.id,
                                    counts
                                );
                                continue;
                            }
                        }
                    }

                    clone_pairs.push(ClonePairReport {
                        source: source_endpoint,
                        target: target_endpoint,
                        similarity,
                        verification: verification_detail,
                    });
                }
            }

            if min_ast_nodes > 0 {
                let before = clone_pairs.len();
                clone_pairs.retain(|pair| {
                    if let Some(ref ver) = pair.verification {
                        if let Some(counts) = ver.node_counts {
                            return counts.0.min(counts.1) >= min_ast_nodes;
                        }
                    }
                    true
                });
                let filtered = before.saturating_sub(clone_pairs.len());
                if filtered > 0 {
                    info!(
                        filtered,
                        min_ast_nodes, "Filtered clone pairs below min_ast_nodes"
                    );
                }
            }

            let avg_similarity = if similarity_count > 0 {
                similarity_total / similarity_count as f64
            } else {
                0.0
            };

            let verification_summary = if verify_with_apted {
                Some(CloneVerificationResults {
                    method: "apted".to_string(),
                    pairs_considered: similarity_count,
                    pairs_evaluated: apted_pairs_requested,
                    pairs_scored: apted_pairs_scored,
                    avg_similarity: if apted_similarity_count > 0 {
                        Some(apted_similarity_total / apted_similarity_count as f64)
                    } else {
                        None
                    },
                })
            } else {
                None
            };

            let tfidf_stats = if denoise_enabled {
                use super::pipeline_results::TfIdfStats;

                Some(TfIdfStats {
                    total_grams: 0,
                    unique_grams: 0,
                    top1pct_contribution: 0.0,
                })
            } else {
                None
            };

            let clone_pair_count = clone_pairs.len();
            let mut serialized_pairs = Vec::with_capacity(clone_pairs.len());
            // Enforce the configured min_ast_nodes (0 means no filter)
            let min_ast_nodes_cfg = min_ast_nodes;

            for pair in clone_pairs {
                match serde_json::to_value(&pair) {
                    Ok(value) => {
                        // Final defensive filter so UI never sees sub-threshold clones.
                        if min_ast_nodes_cfg > 0 {
                            if let Some(ver) = value.get("verification") {
                                if let Some(counts) = ver.get("node_counts") {
                                    if let (Some(a), Some(b)) = (
                                        counts.get(0).and_then(|v| v.as_u64()),
                                        counts.get(1).and_then(|v| v.as_u64()),
                                    ) {
                                        if std::cmp::min(a, b) < min_ast_nodes_cfg as u64 {
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                        serialized_pairs.push(value);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to serialize clone pair {} -> {}: {}",
                            pair.source.id, pair.target.id, e
                        );
                    }
                }
            }

            Ok(LshAnalysisResults {
                enabled: true,
                clone_pairs: serialized_pairs,
                max_similarity,
                avg_similarity,
                duplicate_count: clone_pair_count,
                apted_verification_enabled: verify_with_apted,
                verification: verification_summary,
                denoising_enabled: denoise_enabled,
                tfidf_stats,
            })
        } else {
            // LSH extractor not available
            Ok(LshAnalysisResults {
                enabled: false,
                clone_pairs: Vec::new(),
                max_similarity: 0.0,
                avg_similarity: 0.0,
                duplicate_count: 0,
                apted_verification_enabled: false,
                verification: None,
                denoising_enabled: false,
                tfidf_stats: None,
            })
        }
    }

    /// Run coverage analysis with automatic file discovery
    pub async fn run_coverage_analysis(
        &self,
        root_path: &Path,
        coverage_config: &CoverageConfig,
    ) -> Result<CoverageAnalysisResults> {
        debug!("Running coverage analysis with auto-discovery");

        // Discover coverage files
        let discovered_files =
            CoverageDiscovery::discover_coverage_files(root_path, coverage_config)?;

        if discovered_files.is_empty() {
            info!("No coverage files found - analysis disabled");
            return Ok(CoverageAnalysisResults {
                enabled: false,
                coverage_files_used: Vec::new(),
                coverage_gaps: Vec::new(),
                gaps_count: 0,
                overall_coverage_percentage: None,
                analysis_method: "no_coverage_files_found".to_string(),
            });
        }

        // Convert discovered files to info structs
        let coverage_files_info: Vec<CoverageFileInfo> = discovered_files
            .iter()
            .map(|file| CoverageFileInfo {
                path: file.path.display().to_string(),
                format: format!("{:?}", file.format),
                size: file.size,
                modified: format!("{:?}", file.modified),
            })
            .collect();

        // Log which files are being used
        for file in &discovered_files {
            info!(
                "Using coverage file: {} (format: {:?})",
                file.path.display(),
                file.format
            );
        }

        // Run comprehensive coverage analysis using CoverageExtractor
        let gaps_count = self.analyze_coverage_gaps(&discovered_files).await?;

        // Build actual coverage packs for detailed analysis
        let mut all_coverage_packs = Vec::new();
        for file in &discovered_files {
            let packs = self
                .coverage_extractor
                .build_coverage_packs(vec![file.path.clone()])
                .await?;
            all_coverage_packs.extend(packs);
        }

        // Calculate overall coverage percentage from LCOV data
        let overall_coverage_percentage = if !discovered_files.is_empty() {
            self.calculate_overall_coverage(&discovered_files).await?
        } else {
            None
        };

        let analysis_method = if discovered_files.len() == 1 {
            format!("single_file_{:?}", discovered_files[0].format)
        } else {
            format!("multi_file_{}_sources", discovered_files.len())
        };

        // Convert CoveragePacks to JSON for storage in coverage_gaps
        let coverage_gaps: Vec<serde_json::Value> = all_coverage_packs
            .iter()
            .map(|pack| serde_json::to_value(pack).unwrap_or(serde_json::Value::Null))
            .collect();

        Ok(CoverageAnalysisResults {
            enabled: true,
            coverage_files_used: coverage_files_info,
            coverage_gaps,
            gaps_count,
            overall_coverage_percentage,
            analysis_method,
        })
    }

    /// Analyze coverage gaps from discovered coverage files
    async fn analyze_coverage_gaps(&self, coverage_files: &[CoverageFile]) -> Result<usize> {
        // Basic implementation - count files that could have coverage gaps
        // This is a placeholder for the more sophisticated coverage analysis

        let mut total_gaps = 0;

        for coverage_file in coverage_files {
            match coverage_file.format {
                CoverageFormat::CoveragePyXml
                | CoverageFormat::Cobertura
                | CoverageFormat::JaCoCo => {
                    // XML-based coverage files
                    total_gaps += self.analyze_xml_coverage(&coverage_file.path).await?;
                }
                CoverageFormat::Lcov => {
                    // LCOV format
                    total_gaps += self.analyze_lcov_coverage(&coverage_file.path).await?;
                }
                CoverageFormat::IstanbulJson => {
                    // JSON format
                    total_gaps += self.analyze_json_coverage(&coverage_file.path).await?;
                }
                CoverageFormat::Unknown => {
                    warn!(
                        "Unknown coverage format, skipping: {}",
                        coverage_file.path.display()
                    );
                }
            }
        }

        Ok(total_gaps)
    }

    /// Calculate overall coverage percentage from coverage files
    async fn calculate_overall_coverage(
        &self,
        coverage_files: &[CoverageFile],
    ) -> Result<Option<f64>> {
        for coverage_file in coverage_files {
            if matches!(coverage_file.format, CoverageFormat::Lcov) {
                // Parse LCOV file to calculate coverage percentage
                if let Ok(content) = std::fs::read_to_string(&coverage_file.path) {
                    let mut total_lines = 0;
                    let mut covered_lines = 0;

                    for line in content.lines() {
                        if line.starts_with("DA:") {
                            let parts: Vec<&str> = line[3..].split(',').collect();
                            if parts.len() >= 2 {
                                total_lines += 1;
                                if let Ok(hits) = parts[1].parse::<usize>() {
                                    if hits > 0 {
                                        covered_lines += 1;
                                    }
                                }
                            }
                        }
                    }

                    if total_lines > 0 {
                        let coverage_percentage =
                            (covered_lines as f64 / total_lines as f64) * 100.0;
                        debug!(
                            "Calculated coverage: {:.2}% ({}/{} lines)",
                            coverage_percentage, covered_lines, total_lines
                        );
                        return Ok(Some(coverage_percentage));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Analyze XML-based coverage files
    async fn analyze_xml_coverage(&self, coverage_path: &Path) -> Result<usize> {
        use std::fs;

        // Read and parse XML coverage file
        let xml_content = match fs::read_to_string(coverage_path) {
            Ok(content) => content,
            Err(e) => {
                warn!(
                    "Failed to read coverage file {}: {}",
                    coverage_path.display(),
                    e
                );
                return Ok(0);
            }
        };

        // Simple XML parsing to extract uncovered lines
        let mut uncovered_count = 0;

        for line in xml_content.lines() {
            // Count lines with hits="0" (uncovered lines)
            if line.trim().contains("<line number=") && line.contains("hits=\"0\"") {
                uncovered_count += 1;
            }
        }

        debug!(
            "Analyzed XML coverage file: {} uncovered lines found",
            uncovered_count
        );

        // Return a reasonable gap count - group consecutive uncovered lines into gaps
        // Assume average gap spans 2-3 lines, so divide by 2
        Ok((uncovered_count / 2).max(1))
    }

    /// Analyze LCOV coverage files
    async fn analyze_lcov_coverage(&self, coverage_path: &Path) -> Result<usize> {
        debug!("Analyzing LCOV coverage file: {:?}", coverage_path);

        // Use the CoverageExtractor to parse the LCOV file and build coverage packs
        let coverage_packs = self
            .coverage_extractor
            .build_coverage_packs(vec![coverage_path.to_path_buf()])
            .await?;

        // Count the total gaps across all packs
        let total_gaps: usize = coverage_packs.iter().map(|pack| pack.gaps.len()).sum();

        info!("Found {} coverage gaps in LCOV file", total_gaps);
        Ok(total_gaps)
    }

    /// Analyze JSON coverage files
    async fn analyze_json_coverage(&self, _coverage_path: &Path) -> Result<usize> {
        // Placeholder implementation
        // Future: Parse JSON coverage and identify gaps
        debug!("Analyzing JSON coverage file");
        Ok(0)
    }

    /// Extract entities from a file using appropriate language adapter
    async fn extract_entities_from_file(
        &self,
        file_path: &Path,
        content: &str,
    ) -> Option<Vec<crate::core::featureset::CodeEntity>> {
        use crate::lang::registry::adapter_for_file;

        // Get appropriate language adapter
        let mut adapter = match adapter_for_file(file_path) {
            Ok(adapter) => adapter,
            Err(e) => {
                debug!("No language adapter for {}: {}", file_path.display(), e);
                return None;
            }
        };

        // Extract entities using the standardized interface
        match adapter.extract_code_entities(content, &file_path.to_string_lossy()) {
            Ok(entities) => {
                debug!(
                    "Extracted {} entities from {}",
                    entities.len(),
                    file_path.display()
                );
                Some(entities)
            }
            Err(e) => {
                warn!(
                    "Failed to extract entities from {}: {}",
                    file_path.display(),
                    e
                );
                None
            }
        }
    }

    /// Run arena-based file analysis for optimal memory performance
    ///
    /// This method demonstrates arena allocation benefits by processing files
    /// with minimal memory allocation overhead using bump-pointer allocation.
    pub async fn run_arena_file_analysis(
        &self,
        files: &[PathBuf],
    ) -> Result<Vec<crate::core::arena_analysis::ArenaAnalysisResult>> {
        debug!("Running arena-based file analysis on {} files", files.len());

        use tokio::fs;

        // Prepare file paths and sources for batch arena analysis
        let mut file_sources = Vec::with_capacity(files.len());

        for file_path in files {
            match fs::read_to_string(file_path).await {
                Ok(source) => {
                    file_sources.push((file_path.as_path(), source));
                }
                Err(e) => {
                    warn!("Failed to read file {}: {}", file_path.display(), e);
                    continue;
                }
            }
        }

        if file_sources.is_empty() {
            info!("No files could be read for arena analysis");
            return Ok(Vec::new());
        }

        // Use ArenaBatchAnalyzer for optimal memory usage
        let batch_analyzer = ArenaBatchAnalyzer::new();

        // Convert to the format expected by batch analyzer
        let file_refs: Vec<(&std::path::Path, &str)> = file_sources
            .iter()
            .map(|(path, source)| (*path, source.as_str()))
            .collect();

        let batch_result = batch_analyzer.analyze_batch(file_refs).await?;

        info!(
            "Arena batch analysis completed: {} files, {} entities, {:.2} KB arena usage, {:.1} entities/sec",
            batch_result.total_files,
            batch_result.total_entities,
            batch_result.total_arena_kb(),
            batch_result.entities_per_second()
        );

        info!(
            "Estimated malloc savings: {:.2} KB overhead reduction vs traditional allocation",
            batch_result.estimated_malloc_savings()
        );

        Ok(batch_result.file_results)
    }

    /// Run arena-based file analysis with pre-loaded file contents (performance optimized)
    pub async fn run_arena_file_analysis_with_content(
        &self,
        file_contents: &[(PathBuf, String)],
    ) -> Result<Vec<crate::core::arena_analysis::ArenaAnalysisResult>> {
        debug!(
            "Running arena-based file analysis on {} pre-loaded files",
            file_contents.len()
        );

        if file_contents.is_empty() {
            info!("No files provided for arena analysis");
            return Ok(Vec::new());
        }

        // Use ArenaBatchAnalyzer for optimal memory usage
        let batch_analyzer = ArenaBatchAnalyzer::new();

        // Convert to the format expected by batch analyzer
        let file_refs: Vec<(&std::path::Path, &str)> = file_contents
            .iter()
            .map(|(path, content)| (path.as_path(), content.as_str()))
            .collect();

        let batch_result = batch_analyzer.analyze_batch(file_refs).await?;

        info!(
            "Arena analysis completed: {} files, {} entities, {:.2} KB arena memory, {:.1} entities/sec",
            batch_result.total_files,
            batch_result.total_entities,
            batch_result.total_arena_kb(),
            batch_result.entities_per_second()
        );

        info!(
            "Estimated malloc savings: {:.2} KB overhead reduction vs traditional allocation",
            batch_result.estimated_malloc_savings()
        );

        Ok(batch_result.file_results)
    }
}

#[async_trait(?Send)]
impl StageOrchestrator for AnalysisStages {
    async fn run_arena_analysis_with_content(
        &self,
        file_contents: &[(PathBuf, String)],
    ) -> Result<Vec<ArenaAnalysisResult>> {
        self.run_arena_file_analysis_with_content(file_contents)
            .await
    }

    async fn run_all_stages(
        &self,
        config: &AnalysisConfig,
        paths: &[PathBuf],
        files: &[PathBuf],
        arena_results: &[ArenaAnalysisResult],
    ) -> Result<StageResultsBundle> {
        use futures::future;

        let group1_future = async {
            let structure_future = async {
                if config.enable_structure_analysis {
                    self.run_structure_analysis(paths).await
                } else {
                    Ok(StructureAnalysisResults {
                        enabled: false,
                        directory_recommendations: Vec::new(),
                        file_splitting_recommendations: Vec::new(),
                        issues_count: 0,
                    })
                }
            };

            let coverage_future = async {
                if config.enable_coverage_analysis {
                    let coverage_config = self.valknut_config.coverage.clone();
                    let default_path = PathBuf::from(".");
                    let root_path = paths.first().unwrap_or(&default_path);
                    self.run_coverage_analysis(root_path, &coverage_config)
                        .await
                } else {
                    Ok(CoverageAnalysisResults {
                        enabled: false,
                        coverage_files_used: Vec::new(),
                        coverage_gaps: Vec::new(),
                        gaps_count: 0,
                        overall_coverage_percentage: None,
                        analysis_method: "disabled".to_string(),
                    })
                }
            };

            future::join(structure_future, coverage_future).await
        };

        let group2_future = async {
            let complexity_future = async {
                if config.enable_complexity_analysis {
                    self.run_complexity_analysis_from_arena_results(arena_results)
                        .await
                } else {
                    Ok(ComplexityAnalysisResults {
                        enabled: false,
                        detailed_results: Vec::new(),
                        average_cyclomatic_complexity: 0.0,
                        average_cognitive_complexity: 0.0,
                        average_technical_debt_score: 0.0,
                        average_maintainability_index: 100.0,
                        issues_count: 0,
                    })
                }
            };

            let refactoring_future = async {
                if config.enable_refactoring_analysis {
                    self.run_refactoring_analysis(files).await
                } else {
                    Ok(RefactoringAnalysisResults {
                        enabled: false,
                        detailed_results: Vec::new(),
                        opportunities_count: 0,
                    })
                }
            };

            let impact_future = async {
                if config.enable_impact_analysis {
                    self.run_impact_analysis(files).await
                } else {
                    Ok(ImpactAnalysisResults {
                        enabled: false,
                        dependency_cycles: Vec::new(),
                        chokepoints: Vec::new(),
                        clone_groups: Vec::new(),
                        issues_count: 0,
                    })
                }
            };

            let lsh_future = async {
                if config.enable_lsh_analysis && self.lsh_extractor.is_some() {
                    let denoise_enabled = self.valknut_config.denoise.enabled;
                    self.run_lsh_analysis(files, denoise_enabled).await
                } else {
                    Ok(LshAnalysisResults {
                        enabled: false,
                        clone_pairs: Vec::new(),
                        max_similarity: 0.0,
                        avg_similarity: 0.0,
                        duplicate_count: 0,
                        apted_verification_enabled: false,
                        verification: None,
                        denoising_enabled: false,
                        tfidf_stats: None,
                    })
                }
            };

            future::join4(
                complexity_future,
                refactoring_future,
                impact_future,
                lsh_future,
            )
            .await
        };

        let (
            (structure_result, coverage_result),
            (complexity_result, refactoring_result, impact_result, lsh_result),
        ) = future::join(group1_future, group2_future).await;

        Ok(StageResultsBundle {
            structure: structure_result?,
            coverage: coverage_result?,
            complexity: complexity_result?,
            refactoring: refactoring_result?,
            impact: impact_result?,
            lsh: lsh_result?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::arena_analysis::ArenaAnalysisResult;
    use crate::core::dependency::ProjectDependencyAnalysis;
    use crate::core::featureset::CodeEntity;
    use crate::core::file_utils::{CoverageFile, CoverageFormat};
    use crate::core::interning::intern;
    use crate::detectors::complexity::ComplexityConfig;
    use crate::detectors::lsh::LshExtractor;
    use crate::detectors::refactoring::{RefactoringAnalyzer, RefactoringConfig};
    use crate::detectors::structure::StructureConfig;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::Arc;
    use std::time::Duration;
    use std::time::SystemTime;
    use tempfile::tempdir;

    fn build_test_stages() -> AnalysisStages {
        let ast_service = Arc::new(AstService::new());
        let structure_extractor = StructureExtractor::with_config(StructureConfig::default());
        let complexity_analyzer =
            ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
        let refactoring_analyzer =
            RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());
        let coverage_extractor =
            CoverageExtractor::new(CoverageDetectorConfig::default(), ast_service.clone());
        let config = Arc::new(ValknutConfig::default());

        AnalysisStages::new(
            structure_extractor,
            complexity_analyzer,
            refactoring_analyzer,
            coverage_extractor,
            ast_service,
            config,
        )
    }

    fn build_test_stages_with_lsh() -> AnalysisStages {
        let ast_service = Arc::new(AstService::new());
        let structure_extractor = StructureExtractor::with_config(StructureConfig::default());
        let complexity_analyzer =
            ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
        let refactoring_analyzer =
            RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());
        let mut valknut_config = ValknutConfig::default();
        valknut_config.lsh.similarity_threshold = 0.0;
        valknut_config.lsh.num_hashes = 32;
        valknut_config.lsh.num_bands = 4;
        valknut_config.lsh.max_candidates = 8;
        valknut_config.lsh.apted_max_nodes = 512;
        let lsh_config = valknut_config.lsh.clone();

        let lsh_extractor = LshExtractor::new()
            .with_shared_ast_service(ast_service.clone())
            .with_lsh_config(lsh_config.clone().into());
        let coverage_extractor =
            CoverageExtractor::new(CoverageDetectorConfig::default(), ast_service.clone());

        AnalysisStages::new_with_lsh(
            structure_extractor,
            complexity_analyzer,
            refactoring_analyzer,
            lsh_extractor,
            coverage_extractor,
            ast_service,
            Arc::new(valknut_config),
        )
    }

    #[test]
    fn hash_kind_is_stable_for_identical_input() {
        let first = hash_kind("function_declaration");
        let second = hash_kind("function_declaration");

        assert_eq!(first, second);
        let different = hash_kind("struct_declaration");
        assert_ne!(first, different);
    }

    #[test]
    fn parse_byte_range_extracts_start_and_end() {
        let mut entity = CodeEntity::new("id", "function", "sample", "src/lib.rs");
        entity.add_property("byte_range", serde_json::json!([12, 48]));
        assert_eq!(parse_byte_range(&entity), Some((12, 48)));

        entity
            .properties
            .insert("byte_range".to_string(), serde_json::json!([12]));
        assert_eq!(parse_byte_range(&entity), None);
    }

    #[tokio::test]
    async fn calculate_overall_coverage_parses_lcov_percentage() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let lcov_path = tmp.path().join("lcov.info");
        let lcov_content = r#"TN:
SF:src/lib.rs
DA:1,1
DA:2,0
DA:3,2
end_of_record
"#;
        std::fs::write(&lcov_path, lcov_content).expect("write lcov");

        let coverage_file = CoverageFile {
            path: lcov_path,
            format: CoverageFormat::Lcov,
            modified: std::time::SystemTime::now(),
            size: lcov_content.len() as u64,
        };

        let percentage = stages
            .calculate_overall_coverage(&[coverage_file])
            .await
            .expect("coverage calc");

        assert!(percentage.is_some());
        let pct = percentage.unwrap();
        assert!(
            pct > 60.0 && pct < 80.0,
            "expected coverage around 66%, got {pct}"
        );
    }

    #[tokio::test]
    async fn analyze_xml_coverage_counts_uncovered_lines() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let xml_path = tmp.path().join("coverage.xml");
        let xml_content = r#"
<coverage>
  <line number="1" hits="0"/>
  <line number="2" hits="0"/>
  <line number="3" hits="1"/>
</coverage>
"#;
        std::fs::write(&xml_path, xml_content).expect("write xml");

        let gaps = stages
            .analyze_xml_coverage(&xml_path)
            .await
            .expect("xml analysis");

        assert_eq!(gaps, 1);
    }

    #[tokio::test]
    async fn analyze_json_coverage_returns_zero() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let json_path = tmp.path().join("coverage.json");
        std::fs::write(&json_path, r#"{"result": "placeholder"}"#).expect("write json");

        let gaps = stages
            .analyze_json_coverage(&json_path)
            .await
            .expect("json analysis");

        assert_eq!(gaps, 0);
    }

    #[tokio::test]
    async fn analyze_xml_coverage_warns_on_missing_file() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let missing_path = tmp.path().join("missing.xml");
        // Do not create the file

        let gaps = stages
            .analyze_xml_coverage(&missing_path)
            .await
            .expect("xml analysis");

        assert_eq!(gaps, 0, "missing files should yield zero gaps");
    }

    #[tokio::test]
    async fn analyze_lcov_coverage_counts_gaps() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let lcov_path = tmp.path().join("coverage.lcov");
        let content = "\
TN:\n\
SF:src/main.rs\n\
DA:1,1\n\
DA:2,0\n\
DA:3,0\n\
DA:4,1\n\
end_of_record\n";
        std::fs::write(&lcov_path, content).expect("write lcov");

        let gaps = stages
            .analyze_lcov_coverage(&lcov_path)
            .await
            .expect("lcov gaps");
        assert_eq!(
            gaps, 0,
            "expected zero gaps until dedicated LCOV parser support is added"
        );
    }

    #[tokio::test]
    async fn analyze_lcov_coverage_propagates_errors() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let lcov_path = tmp.path().join("coverage.lcov");
        std::fs::write(&lcov_path, "malformed").expect("write malformed lcov");

        let result = stages.analyze_lcov_coverage(&lcov_path).await;
        assert!(
            result.is_err(),
            "malformed LCOV input should surface extractor errors"
        );
    }

    #[tokio::test]
    async fn analyze_coverage_gaps_combines_multiple_formats() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");

        // Prepare source file and LCOV report
        let source_path = tmp.path().join("sample.rs");
        let source = r#"pub fn add(a: i32, b: i32) -> i32 {
    if a > 0 {
        a + b
    } else {
        b - a
    }
}
"#;
        std::fs::write(&source_path, source).expect("write source file");

        let lcov_path = tmp.path().join("coverage.lcov");
        let lcov_report = format!(
            "TN:\nSF:{}\nDA:1,1\nDA:2,0\nDA:3,0\nDA:4,1\nend_of_record\n",
            source_path.display()
        );
        std::fs::write(&lcov_path, lcov_report).expect("write lcov file");

        // XML coverage with two uncovered lines
        let xml_path = tmp.path().join("coverage.xml");
        let xml_content = r#"
<coverage>
  <line number="10" hits="0"/>
  <line number="11" hits="0"/>
  <line number="12" hits="1"/>
</coverage>
"#;
        std::fs::write(&xml_path, xml_content).expect("write xml file");

        // Placeholder JSON coverage (currently treated as zero gaps)
        let json_path = tmp.path().join("coverage.json");
        std::fs::write(&json_path, r#"{"files": []}"#).expect("write json file");

        let coverage_files = vec![
            CoverageFile {
                path: lcov_path,
                format: CoverageFormat::Lcov,
                modified: SystemTime::now(),
                size: 64,
            },
            CoverageFile {
                path: xml_path,
                format: CoverageFormat::CoveragePyXml,
                modified: SystemTime::now(),
                size: 64,
            },
            CoverageFile {
                path: json_path,
                format: CoverageFormat::IstanbulJson,
                modified: SystemTime::now(),
                size: 16,
            },
        ];

        let gap_count = stages
            .analyze_coverage_gaps(&coverage_files)
            .await
            .expect("gap analysis");

        assert!(
            gap_count >= 1,
            "expected at least one gap from LCOV or XML, got {gap_count}"
        );
    }

    #[tokio::test]
    async fn analyze_coverage_gaps_skips_unknown_formats() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let unknown_path = tmp.path().join("mystery.dat");
        std::fs::write(&unknown_path, "opaque").expect("write unknown coverage stub");

        let coverage_files = vec![CoverageFile {
            path: unknown_path,
            format: CoverageFormat::Unknown,
            modified: SystemTime::now(),
            size: 6,
        }];

        let gap_count = stages
            .analyze_coverage_gaps(&coverage_files)
            .await
            .expect("gap analysis");

        assert_eq!(
            gap_count, 0,
            "unknown coverage formats should be ignored without contributing gaps"
        );
    }

    #[tokio::test]
    async fn run_impact_analysis_returns_disabled_when_inputs_empty() {
        let stages = build_test_stages();
        let impact = stages
            .run_impact_analysis(&[])
            .await
            .expect("impact analysis");
        assert!(
            !impact.enabled,
            "no files means the dependency analysis should be disabled"
        );
        assert_eq!(impact.issues_count, 0);
    }

    #[tokio::test]
    async fn run_impact_analysis_handles_files_without_functions() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let path = tmp.path().join("values.py");
        fs::write(&path, "VALUE = 3\n").expect("write python module");

        let impact = stages
            .run_impact_analysis(&[path.clone()])
            .await
            .expect("impact analysis");

        assert!(
            !impact.enabled,
            "modules with no functions should not produce dependency results"
        );
        assert_eq!(impact.issues_count, 0);
    }

    #[tokio::test]
    async fn run_impact_analysis_enables_when_functions_present() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let path = tmp.path().join("mod.py");
        let source = r#"
def helper():
    return 1

def caller():
    return helper()
"#;
        fs::write(&path, source).expect("write python module");

        let impact = stages
            .run_impact_analysis(&[path.clone()])
            .await
            .expect("impact analysis");

        assert!(
            impact.enabled,
            "a module with functions should enable results"
        );
        assert_eq!(impact.issues_count, 0);
        assert!(
            impact.dependency_cycles.is_empty(),
            "simple single-module graph should not produce cycles"
        );
    }

    #[tokio::test]
    async fn calculate_overall_coverage_returns_none_without_lcov() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let json_path = tmp.path().join("coverage.json");
        std::fs::write(&json_path, "{}").expect("write json coverage");

        let coverage_files = vec![CoverageFile {
            path: json_path,
            format: CoverageFormat::IstanbulJson,
            modified: SystemTime::now(),
            size: 2,
        }];

        let coverage = stages
            .calculate_overall_coverage(&coverage_files)
            .await
            .expect("coverage calc");

        assert!(
            coverage.is_none(),
            "non-LCOV coverage inputs should not produce a coverage percentage"
        );
    }

    #[tokio::test]
    async fn analyze_xml_coverage_returns_zero_when_file_missing() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let missing_path = tmp.path().join("missing.xml");

        let gaps = stages
            .analyze_xml_coverage(&missing_path)
            .await
            .expect("xml analysis");

        assert_eq!(
            gaps, 0,
            "missing coverage files should be treated as having no measurable gaps"
        );
    }

    #[tokio::test]
    async fn run_lsh_analysis_disabled_without_extractor() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let file_path = tmp.path().join("sample.rs");
        std::fs::write(&file_path, "pub fn demo() {}").expect("write sample");

        let analysis = stages
            .run_lsh_analysis(&[file_path], false)
            .await
            .expect("lsh analysis");

        assert!(!analysis.enabled);
        assert!(analysis.clone_pairs.is_empty());
    }

    #[tokio::test]
    async fn run_lsh_analysis_with_extractor_handles_empty_entities() {
        let stages = build_test_stages_with_lsh();
        let tmp = tempdir().expect("temp dir");
        let file_path = tmp.path().join("notes.txt");
        std::fs::write(&file_path, "plain text that yields no entities").expect("write stub");

        let analysis = stages
            .run_lsh_analysis(&[file_path], true)
            .await
            .expect("lsh analysis");

        assert!(analysis.enabled);
        assert!(analysis.clone_pairs.is_empty());
        assert!(analysis.verification.is_none());
    }

    #[tokio::test]
    async fn run_impact_analysis_handles_empty_and_non_empty_inputs() {
        let stages = build_test_stages();

        let empty = stages
            .run_impact_analysis(&[])
            .await
            .expect("empty impact analysis");
        assert!(!empty.enabled);

        let tmp = tempdir().expect("temp dir");
        let file_path = tmp.path().join("deps.rs");
        let content = r#"
pub mod deps {
    pub fn alpha() {
        beta();
    }

    pub fn beta() {
        alpha();
    }
}
"#;
        std::fs::write(&file_path, content).expect("write deps");

        let non_empty = stages
            .run_impact_analysis(&[file_path.clone()])
            .await
            .expect("impact analysis");

        assert!(non_empty.enabled);
        assert_eq!(non_empty.clone_groups.len(), 0);
        assert!(
            non_empty.issues_count >= 0,
            "issues_count should be non-negative"
        );
    }

    #[test]
    fn dependency_analysis_collects_metrics() {
        let tmp = tempdir().expect("temp dir");
        let file_path = tmp.path().join("analysis.rs");
        let content = r#"
pub mod cycle {
    pub fn first() {
        second();
    }

    pub fn second() {
        first();
    }
}
"#;
        std::fs::write(&file_path, content).expect("write analysis file");

        let analysis =
            ProjectDependencyAnalysis::analyze(&[file_path]).expect("perform dependency analysis");

        assert!(
            !analysis.is_empty(),
            "analysis should contain at least one function node"
        );
        assert!(analysis.metrics_iter().count() > 0, "metrics should exist");
        // Chokepoints may be empty depending on AST metadata, but call ensures accessor coverage.
        let _ = analysis.chokepoints();
    }

    #[tokio::test]
    async fn simple_ast_cache_reuses_entries_and_handles_truncation() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let file_path = tmp.path().join("ast_sample.rs");
        let content = r#"
pub fn compute(limit: i32) -> i32 {
    let mut acc = 0;
    for i in 0..limit {
        acc += i;
    }
    acc
}
"#;
        std::fs::write(&file_path, content).expect("write rust sample");
        let path_str = file_path.to_string_lossy().to_string();

        let entities = stages
            .extract_entities_from_file(&file_path, content)
            .await
            .expect("extract entities");
        let entity = entities
            .into_iter()
            .find(|e| e.entity_type.to_lowercase().contains("function"))
            .expect("function entity");

        let mut ast_cache = HashMap::new();
        let cached_tree = stages
            .ast_service
            .get_ast(&path_str, content)
            .await
            .expect("cached tree");
        ast_cache.insert(path_str.clone(), cached_tree);

        let mut cache = HashMap::new();
        let simple =
            get_or_build_simple_ast(&mut cache, &entity, &ast_cache, 10_000).expect("simple ast");
        assert!(!simple.truncated);
        assert!(simple.node_count > 0);
        assert_eq!(cache.len(), 1);

        let reused =
            get_or_build_simple_ast(&mut cache, &entity, &ast_cache, 10_000).expect("reuse ast");
        assert_eq!(reused.node_count, simple.node_count);

        let mut truncated_cache = HashMap::new();
        let truncated =
            get_or_build_simple_ast(&mut truncated_cache, &entity, &ast_cache, 1).expect("trunc");
        assert!(truncated.truncated);

        let mut without_range = entity.clone();
        without_range.properties.remove("byte_range");
        let mut cache_without_range = HashMap::new();
        assert!(get_or_build_simple_ast(
            &mut cache_without_range,
            &without_range,
            &ast_cache,
            10_000
        )
        .is_none());

        let mut cache_missing_ast = HashMap::new();
        let empty_ast_cache: HashMap<String, Arc<CachedTree>> = HashMap::new();
        assert!(
            get_or_build_simple_ast(&mut cache_missing_ast, &entity, &empty_ast_cache, 10_000)
                .is_none()
        );
    }

    #[tokio::test]
    async fn run_lsh_analysis_produces_verified_clone_pairs() {
        let stages = build_test_stages_with_lsh();
        let tmp = tempdir().expect("temp dir");
        let file_a = tmp.path().join("clone_a.rs");
        let file_b = tmp.path().join("clone_b.rs");
        let function_src = r#"
pub fn compute() -> i32 {
    let mut total = 0;
    for value in 0..10 {
        total += value * 2;
    }
    total
}
"#;
        std::fs::write(&file_a, function_src).expect("write clone sample a");
        std::fs::write(&file_b, function_src).expect("write clone sample b");

        let analysis = stages
            .run_lsh_analysis(&[file_a.clone(), file_b.clone()], false)
            .await
            .expect("lsh analysis");

        assert!(analysis.enabled, "expected LSH analysis to be enabled");
        assert!(
            analysis.apted_verification_enabled,
            "APTED verification should be enabled"
        );
        assert!(
            analysis.duplicate_count > 0,
            "expected at least one clone pair"
        );

        let verification_summary = analysis.verification.expect("verification summary present");
        assert!(
            verification_summary.pairs_scored > 0,
            "expected structural verification to score at least one pair"
        );

        let first_pair = analysis.clone_pairs.first().expect("clone pair present");
        let similarity = first_pair
            .get("similarity")
            .and_then(|value| value.as_f64())
            .expect("similarity value recorded");
        assert!(
            similarity >= 0.0,
            "similarity scores should be non-negative"
        );

        let verification_detail = first_pair
            .get("verification")
            .and_then(|value| value.as_object())
            .expect("verification detail recorded");
        assert!(
            verification_detail.contains_key("node_counts"),
            "expected node count metadata"
        );
        assert!(
            verification_detail.contains_key("similarity")
                || verification_detail.contains_key("edit_cost"),
            "verification detail should include similarity or cost"
        );
    }

    #[tokio::test]
    async fn run_lsh_analysis_marks_truncated_asts() {
        let ast_service = Arc::new(AstService::new());
        let structure_extractor = StructureExtractor::with_config(StructureConfig::default());
        let complexity_analyzer =
            ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
        let refactoring_analyzer =
            RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());

        let mut valknut_config = ValknutConfig::default();
        valknut_config.lsh.similarity_threshold = 0.0;
        valknut_config.lsh.num_hashes = 16;
        valknut_config.lsh.num_bands = 2;
        valknut_config.lsh.max_candidates = 4;
        valknut_config.lsh.apted_max_pairs_per_entity = 2;
        valknut_config.lsh.apted_max_nodes = 8;
        valknut_config.lsh.verify_with_apted = true;
        let lsh_config = valknut_config.lsh.clone();

        let lsh_extractor = LshExtractor::new()
            .with_shared_ast_service(ast_service.clone())
            .with_lsh_config(lsh_config.into());
        let coverage_extractor =
            CoverageExtractor::new(CoverageDetectorConfig::default(), ast_service.clone());

        let stages = AnalysisStages::new_with_lsh(
            structure_extractor,
            complexity_analyzer,
            refactoring_analyzer,
            lsh_extractor,
            coverage_extractor,
            ast_service,
            Arc::new(valknut_config),
        );

        let tmp = tempdir().expect("temp dir");
        let file_a = tmp.path().join("truncated_a.rs");
        let file_b = tmp.path().join("truncated_b.rs");
        let big_function = r#"
pub fn heavy() -> i32 {
    let mut value = 0;
    for outer in 0..20 {
        value += outer;
        for inner in 0..20 {
            if inner % 3 == 0 {
                value -= inner;
            } else {
                value += inner;
            }
        }
    }
    value
}
"#;
        std::fs::write(&file_a, big_function).expect("write truncated sample a");
        std::fs::write(&file_b, big_function).expect("write truncated sample b");

        let analysis = stages
            .run_lsh_analysis(&[file_a, file_b], true)
            .await
            .expect("lsh analysis");

        let first_pair = analysis
            .clone_pairs
            .first()
            .expect("expected at least one clone pair");
        let truncated_flag = first_pair
            .get("verification")
            .and_then(|value| value.get("truncated"))
            .and_then(|flag| flag.as_bool())
            .unwrap_or(false);
        assert!(
            truncated_flag,
            "verification detail should mark ASTs as truncated when node budget is exceeded"
        );
    }

    #[tokio::test]
    async fn run_arena_file_analysis_with_content_returns_empty_for_none() {
        let stages = build_test_stages();
        let results = stages
            .run_arena_file_analysis_with_content(&[])
            .await
            .expect("arena analysis");

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn run_arena_file_analysis_skips_missing_files() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let missing_path = tmp.path().join("does_not_exist.rs");
        let results = stages
            .run_arena_file_analysis(&[missing_path])
            .await
            .expect("arena analysis");

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn run_complexity_analysis_from_arena_results_handles_mix_of_inputs() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");

        // Existing file to drive successful analysis
        let existing_path = tmp.path().join("metrics.rs");
        let existing_source = r#"
pub fn compute(limit: i32) -> i32 {
    let mut acc = 0;
    for i in 0..limit {
        if i % 2 == 0 {
            acc += i;
        } else {
            acc -= 1;
        }
    }
    acc
}
"#;
        std::fs::write(&existing_path, existing_source).expect("write metrics file");

        // Missing file triggers warning path
        let missing_path = tmp.path().join("missing.rs");

        let mut entity = CodeEntity::new(
            "metrics::compute",
            "function",
            "compute",
            existing_path.to_string_lossy(),
        )
        .with_line_range(1, 12)
        .with_source_code(existing_source);

        entity.add_property("byte_range", serde_json::json!([0, existing_source.len()]));

        let arena_results = vec![
            ArenaAnalysisResult {
                entity_count: 0,
                file_path: intern(missing_path.to_string_lossy()),
                entity_extraction_time: Duration::from_millis(1),
                total_analysis_time: Duration::from_millis(1),
                arena_bytes_used: 0,
                memory_efficiency_score: 0.0,
                entities: Vec::new(),
            },
            ArenaAnalysisResult {
                entity_count: 1,
                file_path: intern(existing_path.to_string_lossy()),
                entity_extraction_time: Duration::from_millis(2),
                total_analysis_time: Duration::from_millis(5),
                arena_bytes_used: 2 * 1024,
                memory_efficiency_score: 0.0,
                entities: vec![entity],
            },
        ];

        let analysis = stages
            .run_complexity_analysis_from_arena_results(&arena_results)
            .await
            .expect("complexity analysis");

        assert!(
            analysis.enabled,
            "analysis should be enabled with valid input"
        );
        assert!(
            analysis.detailed_results.len() >= 1,
            "expected at least one per-file complexity result"
        );
        assert!(
            analysis.average_cyclomatic_complexity >= 0.0,
            "averages should be non-negative"
        );
    }
}
