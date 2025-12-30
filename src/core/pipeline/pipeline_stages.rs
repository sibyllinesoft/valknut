//! Individual analysis stages for the pipeline.

use async_trait::async_trait;
use futures::future;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::clone_detection::{
    compute_apted_limit, compute_apted_verification, filter_small_pairs, log_partition_stats,
    ordered_pair_key, serialize_clone_pairs, should_skip_small_pair, CachedSimpleAst,
    CloneDetectionStats, CloneEndpoint, ClonePairReport, CloneVerificationDetail,
    LshDetectionParams, LshEntityCollection,
};
use super::coverage_stage::CoverageStage;
use super::lsh_stage::LshStage;
use super::pipeline_config::AnalysisConfig;
use super::pipeline_results::{
    ComplexityAnalysisResults, CoverageAnalysisResults, CoverageFileInfo, ImpactAnalysisResults,
    LshAnalysisResults, RefactoringAnalysisResults, StructureAnalysisResults,
};
use super::services::{StageOrchestrator, StageResultsBundle};
use crate::core::arena_analysis::{ArenaAnalysisResult, ArenaBatchAnalyzer, ArenaFileAnalyzer};
use crate::core::ast_service::{AstService, CachedTree};
use crate::core::config::{CoverageConfig, ValknutConfig};
use crate::core::dependency::{ModuleGraph, ProjectDependencyAnalysis};
use crate::core::errors::Result;
use crate::core::featureset::FeatureExtractor;
use crate::core::file_utils::{CoverageDiscovery, CoverageFile, CoverageFormat};
use crate::detectors::cohesion::{CohesionAnalysisResults, CohesionExtractor};
use crate::detectors::complexity::{AstComplexityAnalyzer, ComplexityAnalyzer};
use crate::detectors::coverage::{CoverageConfig as CoverageDetectorConfig, CoverageExtractor};
use crate::detectors::graph::SimilarityCliquePartitioner;
use crate::detectors::lsh::LshExtractor;
use crate::detectors::refactoring::RefactoringAnalyzer;
use crate::detectors::structure::StructureExtractor;

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

    /// Run LSH analysis for clone detection (delegates to LshStage)
    pub async fn run_lsh_analysis(
        &self,
        files: &[PathBuf],
        denoise_enabled: bool,
    ) -> Result<LshAnalysisResults> {
        let Some(ref lsh_extractor) = self.lsh_extractor else {
            return Ok(LshAnalysisResults::disabled());
        };

        let lsh_stage = LshStage::new(
            lsh_extractor,
            Arc::clone(&self.ast_service),
            Arc::clone(&self.valknut_config),
        );
        lsh_stage.run_lsh_analysis(files, denoise_enabled).await
    }

    /// Run coverage analysis with automatic file discovery (delegates to CoverageStage)
    pub async fn run_coverage_analysis(
        &self,
        root_path: &Path,
        coverage_config: &CoverageConfig,
    ) -> Result<CoverageAnalysisResults> {
        let coverage_stage = CoverageStage::new(&self.coverage_extractor);
        coverage_stage.run_coverage_analysis(root_path, coverage_config).await
    }

    // Entity extraction and collection methods have been moved to LshStage

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
