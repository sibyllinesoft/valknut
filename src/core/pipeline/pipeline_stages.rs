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
use crate::detectors::cohesion::{CohesionAnalysisResults, CohesionExtractor};
use crate::core::arena_analysis::{ArenaAnalysisResult, ArenaBatchAnalyzer, ArenaFileAnalyzer};
use crate::core::ast_service::{AstService, CachedTree};
use crate::core::config::{CoverageConfig, ValknutConfig};
use crate::core::dependency::{ModuleGraph, ProjectDependencyAnalysis};
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
    pub cohesion_extractor: Option<tokio::sync::Mutex<CohesionExtractor>>,
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

/// Results from collecting entities for LSH analysis.
struct LshEntityCollection {
    entities: Vec<crate::core::featureset::CodeEntity>,
    entity_index: HashMap<String, crate::core::featureset::CodeEntity>,
    ast_cache: HashMap<String, Arc<CachedTree>>,
}

impl LshEntityCollection {
    fn new() -> Self {
        Self {
            entities: Vec::new(),
            entity_index: HashMap::new(),
            ast_cache: HashMap::new(),
        }
    }
}

impl CloneEndpoint {
    fn from_entity(entity: &crate::core::featureset::CodeEntity) -> Self {
        Self {
            id: entity.id.clone(),
            name: entity.name.clone(),
            path: entity.file_path.clone(),
            range: entity.line_range,
        }
    }
}

/// Parameters for LSH clone detection
struct LshDetectionParams {
    candidate_limit: Option<usize>,
    min_ast_nodes: usize,
    lsh_threshold: f64,
    verify_with_apted: bool,
    apted_limit: Option<usize>,
    apted_max_nodes: usize,
}

/// Statistics collected during clone detection
#[derive(Default)]
struct CloneDetectionStats {
    max_similarity: f64,
    similarity_total: f64,
    similarity_count: usize,
    apted_similarity_total: f64,
    apted_similarity_count: usize,
    apted_pairs_requested: usize,
    apted_pairs_scored: usize,
}

impl CloneDetectionStats {
    fn record_similarity(&mut self, similarity: f64) {
        self.max_similarity = self.max_similarity.max(similarity);
        self.similarity_total += similarity;
        self.similarity_count += 1;
    }

    fn record_verification(&mut self, detail: &Option<CloneVerificationDetail>, apted_evaluated: &mut usize) {
        if let Some(ref d) = detail {
            if d.node_counts.is_some() {
                *apted_evaluated += 1;
            }
            if let Some(sim) = d.similarity {
                self.apted_pairs_scored += 1;
                self.apted_similarity_total += sim;
                self.apted_similarity_count += 1;
            }
        }
    }

    fn avg_similarity(&self) -> f64 {
        if self.similarity_count > 0 {
            self.similarity_total / self.similarity_count as f64
        } else {
            0.0
        }
    }

    fn verification_summary(&self, enabled: bool) -> Option<CloneVerificationResults> {
        if !enabled {
            return None;
        }
        Some(CloneVerificationResults {
            method: "apted".to_string(),
            pairs_considered: self.similarity_count,
            pairs_evaluated: self.apted_pairs_requested,
            pairs_scored: self.apted_pairs_scored,
            avg_similarity: if self.apted_similarity_count > 0 {
                Some(self.apted_similarity_total / self.apted_similarity_count as f64)
            } else {
                None
            },
        })
    }
}

fn compute_apted_limit(settings: &crate::core::config::LshConfig) -> Option<usize> {
    let limit = if settings.apted_max_pairs_per_entity == 0 {
        settings.max_candidates
    } else if settings.max_candidates == 0 {
        settings.apted_max_pairs_per_entity
    } else {
        settings.apted_max_pairs_per_entity.min(settings.max_candidates)
    };
    if limit == 0 { None } else { Some(limit) }
}

fn log_partition_stats(partitions: &crate::detectors::graph::clique::CliquePartitions) {
    let partition_count = partitions.len();
    let total_peers: usize = partitions.values().map(|g| g.len()).sum();
    let max_peers = partitions.values().map(|g| g.len()).max().unwrap_or(0);
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
}

fn ordered_pair_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

fn should_skip_small_pair(
    verification: &Option<CloneVerificationDetail>,
    min_ast_nodes: usize,
    entity: &crate::core::featureset::CodeEntity,
    candidate: &crate::core::featureset::CodeEntity,
) -> bool {
    if min_ast_nodes == 0 {
        return false;
    }
    if let Some(ref detail) = verification {
        if let Some(ref counts) = detail.node_counts {
            let observed_min = counts.0.min(counts.1);
            if observed_min < min_ast_nodes {
                debug!(
                    "Skipping clone pair below min_ast_nodes (min {}): {} -> {} ({:?})",
                    min_ast_nodes, entity.id, candidate.id, counts
                );
                return true;
            }
        }
    }
    false
}

fn filter_small_pairs(mut pairs: Vec<ClonePairReport>, min_ast_nodes: usize) -> Vec<ClonePairReport> {
    if min_ast_nodes == 0 {
        return pairs;
    }
    let before = pairs.len();
    pairs.retain(|pair| {
        if let Some(ref ver) = pair.verification {
            if let Some(counts) = ver.node_counts {
                return counts.0.min(counts.1) >= min_ast_nodes;
            }
        }
        true
    });
    let filtered = before.saturating_sub(pairs.len());
    if filtered > 0 {
        info!(filtered, min_ast_nodes, "Filtered clone pairs below min_ast_nodes");
    }
    pairs
}

/// Serialize clone pairs to JSON values, filtering by min_ast_nodes threshold.
fn serialize_clone_pairs(
    clone_pairs: Vec<ClonePairReport>,
    min_ast_nodes: usize,
) -> Vec<serde_json::Value> {
    let mut serialized = Vec::with_capacity(clone_pairs.len());

    for pair in clone_pairs {
        match serde_json::to_value(&pair) {
            Ok(value) => {
                // Filter pairs below min_ast_nodes threshold
                if min_ast_nodes > 0 {
                    if let Some(ver) = value.get("verification") {
                        if let Some(counts) = ver.get("node_counts") {
                            if let (Some(a), Some(b)) = (
                                counts.get(0).and_then(|v| v.as_u64()),
                                counts.get(1).and_then(|v| v.as_u64()),
                            ) {
                                if std::cmp::min(a, b) < min_ast_nodes as u64 {
                                    continue;
                                }
                            }
                        }
                    }
                }
                serialized.push(value);
            }
            Err(e) => {
                warn!(
                    "Failed to serialize clone pair {} -> {}: {}",
                    pair.source.id, pair.target.id, e
                );
            }
        }
    }

    serialized
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

/// Compute APTED tree-edit-distance verification for a clone pair.
///
/// Returns `Some(CloneVerificationDetail)` if verification was attempted,
/// `None` if verification was not allowed (limit reached).
async fn compute_apted_verification(
    source_entity: &crate::core::featureset::CodeEntity,
    target_entity: &crate::core::featureset::CodeEntity,
    simple_ast_cache: &mut HashMap<String, Option<CachedSimpleAst>>,
    ast_cache: &HashMap<String, Arc<CachedTree>>,
    apted_max_nodes: usize,
) -> Option<CloneVerificationDetail> {
    let source_ast = get_or_build_simple_ast(simple_ast_cache, source_entity, ast_cache, apted_max_nodes);
    let target_ast = get_or_build_simple_ast(simple_ast_cache, target_entity, ast_cache, apted_max_nodes);

    let (source_ast, target_ast) = match (source_ast, target_ast) {
        (Some(s), Some(t)) => (s, t),
        _ => {
            return Some(CloneVerificationDetail {
                similarity: None,
                edit_cost: None,
                node_counts: None,
                truncated: false,
            });
        }
    };

    let nodes_total = (source_ast.node_count + target_ast.node_count).max(1);
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
            let normalized = (1.0 - (cost as f64 / nodes_total as f64)).clamp(0.0, 1.0);
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
                source_entity.id, target_entity.id, e,
            );
            Some(CloneVerificationDetail {
                similarity: None,
                edit_cost: None,
                node_counts,
                truncated: true,
            })
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

        // Initialize cohesion extractor if enabled in config
        let cohesion_extractor = if valknut_config.cohesion.enabled {
            Some(tokio::sync::Mutex::new(CohesionExtractor::with_config(valknut_config.cohesion.clone())))
        } else {
            None
        };

        Self {
            structure_extractor,
            complexity_analyzer,
            ast_complexity_analyzer,
            refactoring_analyzer,
            lsh_extractor: None,
            coverage_extractor,
            cohesion_extractor,
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

        // Initialize cohesion extractor if enabled in config
        let cohesion_extractor = if valknut_config.cohesion.enabled {
            Some(tokio::sync::Mutex::new(CohesionExtractor::with_config(valknut_config.cohesion.clone())))
        } else {
            None
        };

        Self {
            structure_extractor,
            complexity_analyzer,
            ast_complexity_analyzer,
            refactoring_analyzer,
            lsh_extractor: Some(lsh_extractor),
            coverage_extractor,
            cohesion_extractor,
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

    /// Run structure analysis using pre-computed arena results (optimized path - avoids re-reading files)
    pub async fn run_structure_analysis_with_arena_results(
        &self,
        paths: &[PathBuf],
        arena_results: &[crate::core::arena_analysis::ArenaAnalysisResult],
    ) -> Result<StructureAnalysisResults> {
        debug!(
            "Running optimized structure analysis with {} pre-computed file metrics",
            arena_results.len()
        );

        // Convert arena results to pre-computed metrics
        let metrics: Vec<crate::detectors::structure::PrecomputedFileMetrics> = arena_results
            .iter()
            .map(crate::detectors::structure::PrecomputedFileMetrics::from_arena_result)
            .collect();

        let mut all_recommendations = Vec::new();
        let mut file_splitting_recommendations = Vec::new();

        for path in paths {
            match self
                .structure_extractor
                .generate_recommendations_with_metrics(path, &metrics)
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

    /// Run cohesion analysis using pre-computed arena results (uses source code from arena)
    pub async fn run_cohesion_analysis_with_arena_results(
        &self,
        paths: &[PathBuf],
        arena_results: &[crate::core::arena_analysis::ArenaAnalysisResult],
    ) -> Result<CohesionAnalysisResults> {
        // Check if cohesion analysis is enabled
        let cohesion_mutex = match &self.cohesion_extractor {
            Some(m) => m,
            None => return Ok(CohesionAnalysisResults::default()),
        };

        info!("Running cohesion analysis with {} pre-computed sources", arena_results.len());

        // Build file sources from arena results (reusing already-read source code)
        let file_sources: Vec<(PathBuf, String)> = arena_results
            .iter()
            .map(|r| (PathBuf::from(r.file_path_str()), r.source_code.clone()))
            .collect();

        let root_path = paths.first().cloned().unwrap_or_else(|| PathBuf::from("."));

        // Run cohesion analysis with mutex lock
        let mut cohesion_extractor = cohesion_mutex.lock().await;
        cohesion_extractor.analyze_with_sources(&file_sources, &root_path).await
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

    /// Lightweight Fruchterman–Reingold layout tuned for up to a few hundred nodes.
    fn force_directed_layout(node_count: usize, edges: &[(usize, usize, f64)]) -> Vec<(f64, f64)> {
        if node_count == 0 {
            return Vec::new();
        }

        let golden_angle = std::f64::consts::PI * (3.0 - (5.0_f64).sqrt());
        let mut positions: Vec<(f64, f64)> = (0..node_count)
            .map(|i| {
                let r = ((i + 1) as f64 / node_count as f64).sqrt();
                let theta = i as f64 * golden_angle;
                (r * theta.cos(), r * theta.sin())
            })
            .collect();

        let k = (1.0 / node_count as f64).sqrt().max(0.02);
        let mut temperature = 0.8;
        let iterations = if node_count < 40 { 30 } else { 45 };

        for _ in 0..iterations {
            let mut disp = vec![(0.0_f64, 0.0_f64); node_count];

            for i in 0..node_count {
                for j in (i + 1)..node_count {
                    let dx = positions[i].0 - positions[j].0;
                    let dy = positions[i].1 - positions[j].1;
                    let dist_sq = dx * dx + dy * dy + 1e-9;
                    let dist = dist_sq.sqrt();
                    let rep = (k * k) / dist;
                    let rx = dx / dist * rep;
                    let ry = dy / dist * rep;
                    disp[i].0 += rx;
                    disp[i].1 += ry;
                    disp[j].0 -= rx;
                    disp[j].1 -= ry;
                }
            }

            for (src, dst, weight) in edges {
                let dx = positions[*src].0 - positions[*dst].0;
                let dy = positions[*src].1 - positions[*dst].1;
                let dist_sq = dx * dx + dy * dy + 1e-9;
                let dist = dist_sq.sqrt();
                let strength = weight.ln_1p() + 1.0;
                let attr = (dist_sq / k) * strength;
                let ax = dx / dist * attr;
                let ay = dy / dist * attr;
                disp[*src].0 -= ax;
                disp[*src].1 -= ay;
                disp[*dst].0 += ax;
                disp[*dst].1 += ay;
            }

            for i in 0..node_count {
                let (dx, dy) = disp[i];
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                positions[i].0 += (dx / len) * temperature;
                positions[i].1 += (dy / len) * temperature;
            }

            temperature *= 0.9;
        }

        let mut max_mag = positions
            .iter()
            .fold(0.0_f64, |acc, (x, y)| acc.max(x.abs()).max(y.abs()));
        if max_mag < 1e-6 {
            max_mag = 1.0;
        }

        for pos in &mut positions {
            pos.0 /= max_mag;
            pos.1 /= max_mag;
        }

        positions
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

        let Some(ref lsh_extractor) = self.lsh_extractor else {
            return Ok(LshAnalysisResults::disabled());
        };

        use crate::core::featureset::ExtractionContext;

        let lsh_settings = &self.valknut_config.lsh;
        let verify_with_apted = lsh_settings.verify_with_apted;
        let apted_max_nodes = lsh_settings.apted_max_nodes;
        let apted_limit = compute_apted_limit(lsh_settings);

        let collection = self
            .collect_entities_for_lsh(
                files,
                lsh_extractor,
                verify_with_apted,
                MAX_ENTITIES_PER_FILE_FOR_LSH,
            )
            .await;

        let LshEntityCollection {
            entities,
            entity_index,
            ast_cache,
        } = collection;

        if entities.is_empty() {
            info!("No entities available for LSH after filtering; skipping clone analysis");
            return Ok(LshAnalysisResults::empty_with_settings(
                verify_with_apted,
                denoise_enabled,
            ));
        }

        let mut context = ExtractionContext::new(Arc::clone(&self.valknut_config), "mixed");
        context.entity_index = entity_index;

        let partitions = SimilarityCliquePartitioner::new().partition(&entities);
        if !partitions.is_empty() {
            log_partition_stats(&partitions);
            context.candidate_partitions = Some(Arc::new(partitions));
        }

        let similarity_context = lsh_extractor.similarity_context(&context);
        if similarity_context.is_none() {
            warn!("Unable to build LSH similarity context; clone pairs will not be generated");
        }

        let candidate_limit = lsh_extractor.max_candidates();
        let min_ast_nodes = lsh_extractor.min_ast_nodes_threshold().unwrap_or(0);
        info!(min_ast_nodes, "LSH clone pair filter min_ast_nodes threshold");

        let lsh_threshold = lsh_extractor.similarity_threshold();

        let (clone_pairs, stats) = self
            .detect_clone_pairs(
                &entities,
                &context,
                similarity_context.as_deref(),
                &ast_cache,
                LshDetectionParams {
                    candidate_limit,
                    min_ast_nodes,
                    lsh_threshold,
                    verify_with_apted,
                    apted_limit,
                    apted_max_nodes,
                },
            )
            .await;

        let clone_pairs = filter_small_pairs(clone_pairs, min_ast_nodes);
        let clone_pair_count = clone_pairs.len();
        let serialized_pairs = serialize_clone_pairs(clone_pairs, min_ast_nodes);

        Ok(LshAnalysisResults {
            enabled: true,
            clone_pairs: serialized_pairs,
            max_similarity: stats.max_similarity,
            avg_similarity: stats.avg_similarity(),
            duplicate_count: clone_pair_count,
            apted_verification_enabled: verify_with_apted,
            verification: stats.verification_summary(verify_with_apted),
            denoising_enabled: denoise_enabled,
            tfidf_stats: if denoise_enabled {
                Some(super::pipeline_results::TfIdfStats::default())
            } else {
                None
            },
        })
    }

    async fn detect_clone_pairs(
        &self,
        entities: &[crate::core::featureset::CodeEntity],
        context: &crate::core::featureset::ExtractionContext,
        similarity_context: Option<&crate::detectors::lsh::LshSimilarityContext>,
        ast_cache: &HashMap<String, Arc<CachedTree>>,
        params: LshDetectionParams,
    ) -> (Vec<ClonePairReport>, CloneDetectionStats) {
        let mut clone_pairs = Vec::new();
        let mut seen_pairs: HashSet<(String, String)> = HashSet::new();
        let mut stats = CloneDetectionStats::default();
        let mut simple_ast_cache: HashMap<String, Option<CachedSimpleAst>> = HashMap::new();

        let Some(ctx) = similarity_context else {
            return (clone_pairs, stats);
        };

        for entity in entities {
            let candidates = ctx.find_similar_entities(&entity.id, params.candidate_limit);
            let mut apted_evaluated = 0usize;

            for (candidate_id, similarity) in candidates {
                if similarity < params.lsh_threshold {
                    continue;
                }

                let key = ordered_pair_key(&entity.id, &candidate_id);
                if !seen_pairs.insert(key) {
                    continue;
                }

                let Some(candidate_entity) = context.entity_index.get(&candidate_id) else {
                    continue;
                };

                stats.record_similarity(similarity);

                let apted_allowed = params.verify_with_apted
                    && params.apted_limit.map_or(true, |limit| apted_evaluated < limit);

                let verification_detail = if apted_allowed {
                    stats.apted_pairs_requested += 1;
                    let detail = compute_apted_verification(
                        entity,
                        candidate_entity,
                        &mut simple_ast_cache,
                        ast_cache,
                        params.apted_max_nodes,
                    )
                    .await;
                    stats.record_verification(&detail, &mut apted_evaluated);
                    detail
                } else {
                    None
                };

                if should_skip_small_pair(&verification_detail, params.min_ast_nodes, entity, candidate_entity) {
                    continue;
                }

                clone_pairs.push(ClonePairReport {
                    source: CloneEndpoint::from_entity(entity),
                    target: CloneEndpoint::from_entity(candidate_entity),
                    similarity,
                    verification: verification_detail,
                });
            }
        }

        (clone_pairs, stats)
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
                CoverageFormat::IstanbulJson | CoverageFormat::Tarpaulin => {
                    // JSON format (Istanbul or Tarpaulin)
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

    /// Collect entities from files for LSH clone detection analysis.
    ///
    /// This method reads files, extracts entities, and optionally builds an AST cache
    /// for APTED verification. Entities are filtered through the LSH extractor's thresholds.
    async fn collect_entities_for_lsh(
        &self,
        files: &[PathBuf],
        lsh_extractor: &LshExtractor,
        verify_with_apted: bool,
        max_entities_per_file: usize,
    ) -> LshEntityCollection {
        let mut collection = LshEntityCollection::new();

        for file_path in files.iter() {
            let content = match tokio::fs::read_to_string(file_path).await {
                Ok(content) => content,
                Err(e) => {
                    warn!("Failed to read file {}: {}", file_path.display(), e);
                    continue;
                }
            };

            let path_str = file_path.to_string_lossy().to_string();

            // Build AST cache if APTED verification is enabled
            if verify_with_apted {
                match self.ast_service.get_ast(&path_str, &content).await {
                    Ok(tree) => {
                        collection.ast_cache.insert(path_str.clone(), tree);
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

            // Extract entities from the file
            let Some(extracted_entities) = self.extract_entities_from_file(file_path, &content).await else {
                continue;
            };

            if extracted_entities.len() > max_entities_per_file {
                info!(
                    file = %file_path.display(),
                    entities = extracted_entities.len(),
                    "Skipping LSH for file with excessive entity count"
                );
                continue;
            }

            // Filter entities through LSH thresholds
            for entity in extracted_entities {
                if !lsh_extractor
                    .entity_passes_thresholds(&entity)
                    .await
                    .unwrap_or(false)
                {
                    continue;
                }

                collection.entity_index.insert(entity.id.clone(), entity.clone());
                collection.entities.push(entity);
            }
        }

        collection
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

        info!("Starting run_all_stages with {} paths, {} files, {} arena results",
              paths.len(), files.len(), arena_results.len());

        let group1_future = async {
            let structure_future = async {
                if config.enable_structure_analysis {
                    info!("Starting structure analysis...");
                    let result = self.run_structure_analysis_with_arena_results(paths, arena_results).await;
                    info!("Structure analysis completed");
                    result
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
                    info!("Starting coverage analysis...");
                    let coverage_config = self.valknut_config.coverage.clone();
                    let default_path = PathBuf::from(".");
                    let root_path = paths.first().unwrap_or(&default_path);
                    let result = self.run_coverage_analysis(root_path, &coverage_config)
                        .await;
                    info!("Coverage analysis completed");
                    result
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
                    info!("Starting complexity analysis...");
                    let result = self.run_complexity_analysis_from_arena_results(arena_results)
                        .await;
                    info!("Complexity analysis completed");
                    result
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
                    info!("Starting refactoring analysis...");
                    let result = self.run_refactoring_analysis(files).await;
                    info!("Refactoring analysis completed");
                    result
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
                    info!("Starting impact analysis...");
                    let result = self.run_impact_analysis(files).await;
                    info!("Impact analysis completed");
                    result
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
                    info!("Starting LSH analysis...");
                    let denoise_enabled = self.valknut_config.denoise.enabled;
                    let result = self.run_lsh_analysis(files, denoise_enabled).await;
                    info!("LSH analysis completed");
                    result
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

        info!("Waiting for all analysis stages to complete...");
        let (
            (structure_result, coverage_result),
            (complexity_result, refactoring_result, impact_result, lsh_result),
        ) = future::join(group1_future, group2_future).await;

        info!("All analysis stages completed");

        // Run cohesion analysis (requires mutable access via mutex, so run after other stages)
        let cohesion_result = if self.cohesion_extractor.is_some() {
            info!("Starting cohesion analysis...");
            let result = self.run_cohesion_analysis_with_arena_results(paths, arena_results).await;
            info!("Cohesion analysis completed");
            result?
        } else {
            CohesionAnalysisResults::default()
        };

        info!("Building results bundle");
        Ok(StageResultsBundle {
            structure: structure_result?,
            coverage: coverage_result?,
            complexity: complexity_result?,
            refactoring: refactoring_result?,
            impact: impact_result?,
            lsh: lsh_result?,
            cohesion: cohesion_result,
        })
    }
}


#[cfg(test)]
#[path = "pipeline_stages_tests.rs"]
mod tests;
