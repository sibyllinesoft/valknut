//! Individual analysis stages for the pipeline.

use async_trait::async_trait;
use futures::future;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::verification::clone_detection::{
    compute_apted_limit, compute_apted_verification, filter_small_pairs, log_partition_stats,
    ordered_pair_key, serialize_clone_pairs, should_skip_small_pair, CachedSimpleAst,
    CloneDetectionStats, CloneEndpoint, ClonePairReport, CloneVerificationDetail,
    LshDetectionParams, LshEntityCollection,
};
use super::stages::complexity_stage::ComplexityStage;
use super::stages::coverage_stage::CoverageStage;
use super::stages::impact_stage::ImpactStage;
use super::stages::lsh_stage::LshStage;
use super::stages::refactoring_stage::RefactoringStage;
use super::stages::structure_stage::StructureStage;
use super::pipeline_config::AnalysisConfig;
use super::results::pipeline_results::{
    ComplexityAnalysisResults, CoverageAnalysisResults, CoverageFileInfo, ImpactAnalysisResults,
    LshAnalysisResults, RefactoringAnalysisResults, StructureAnalysisResults,
};
use super::discovery::services::{StageOrchestrator, StageResultsBundle};
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

/// Factory and configuration methods for [`AnalysisStages`].
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
        // Wire analysis.exclude_patterns to cohesion config
        let cohesion_extractor = if valknut_config.cohesion.enabled {
            let mut cohesion_config = valknut_config.cohesion.clone();
            for pattern in &valknut_config.analysis.exclude_patterns {
                if !cohesion_config.issues.exclude_patterns.contains(pattern) {
                    cohesion_config.issues.exclude_patterns.push(pattern.clone());
                }
            }
            Some(tokio::sync::Mutex::new(CohesionExtractor::with_config(cohesion_config)))
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
        // Wire analysis.exclude_patterns to cohesion config
        let cohesion_extractor = if valknut_config.cohesion.enabled {
            let mut cohesion_config = valknut_config.cohesion.clone();
            for pattern in &valknut_config.analysis.exclude_patterns {
                if !cohesion_config.issues.exclude_patterns.contains(pattern) {
                    cohesion_config.issues.exclude_patterns.push(pattern.clone());
                }
            }
            Some(tokio::sync::Mutex::new(CohesionExtractor::with_config(cohesion_config)))
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

    /// Run structure analysis.
    /// Delegates to StructureStage for implementation.
    pub async fn run_structure_analysis(
        &self,
        paths: &[PathBuf],
    ) -> Result<StructureAnalysisResults> {
        let structure_stage = StructureStage::new(&self.structure_extractor);
        structure_stage.run_structure_analysis(paths).await
    }

    /// Run structure analysis using pre-computed arena results (optimized path - avoids re-reading files).
    /// Delegates to StructureStage for implementation.
    pub async fn run_structure_analysis_with_arena_results(
        &self,
        paths: &[PathBuf],
        arena_results: &[crate::core::arena_analysis::ArenaAnalysisResult],
    ) -> Result<StructureAnalysisResults> {
        let structure_stage = StructureStage::new(&self.structure_extractor);
        structure_stage.run_structure_analysis_with_arena_results(paths, arena_results).await
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
    /// Delegates to ComplexityStage for implementation.
    pub async fn run_complexity_analysis_from_arena_results(
        &self,
        arena_results: &[crate::core::arena_analysis::ArenaAnalysisResult],
    ) -> Result<ComplexityAnalysisResults> {
        let complexity_stage = ComplexityStage::new(self.ast_complexity_analyzer.clone());
        complexity_stage.run_from_arena_results(arena_results).await
    }

    /// Run complexity analysis (legacy path - re-parses files)
    /// Delegates to ComplexityStage for implementation.
    pub async fn run_complexity_analysis(
        &self,
        files: &[PathBuf],
    ) -> Result<ComplexityAnalysisResults> {
        let complexity_stage = ComplexityStage::new(self.ast_complexity_analyzer.clone());
        complexity_stage.run_from_files(files).await
    }

    /// Run refactoring analysis.
    /// Delegates to RefactoringStage for implementation.
    pub async fn run_refactoring_analysis(
        &self,
        files: &[PathBuf],
    ) -> Result<RefactoringAnalysisResults> {
        let refactoring_stage = RefactoringStage::new(&self.refactoring_analyzer);
        refactoring_stage.run_refactoring_analysis(files).await
    }

    /// Run impact analysis powered by the dependency graph.
    /// Delegates to ImpactStage for implementation.
    pub async fn run_impact_analysis(&self, files: &[PathBuf]) -> Result<ImpactAnalysisResults> {
        let impact_stage = ImpactStage::new();
        impact_stage.run_impact_analysis(files).await
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

/// [`StageOrchestrator`] implementation for [`AnalysisStages`].
#[async_trait(?Send)]
impl StageOrchestrator for AnalysisStages {
    /// Runs arena analysis on provided file contents.
    async fn run_arena_analysis_with_content(
        &self,
        file_contents: &[(PathBuf, String)],
    ) -> Result<Vec<ArenaAnalysisResult>> {
        self.run_arena_file_analysis_with_content(file_contents)
            .await
    }

    /// Runs all analysis stages and returns a bundled result.
    async fn run_all_stages(
        &self,
        config: &AnalysisConfig,
        paths: &[PathBuf],
        files: &[PathBuf],
        arena_results: &[ArenaAnalysisResult],
    ) -> Result<StageResultsBundle> {
        info!(
            "Starting run_all_stages with {} paths, {} files, {} arena results",
            paths.len(),
            files.len(),
            arena_results.len()
        );

        // Run Group 1 (structure + coverage) and Group 2 (complexity + refactoring + impact + lsh) in parallel
        let (group1_results, group2_results) = future::join(
            self.run_stage_group1(config, paths, arena_results),
            self.run_stage_group2(config, files, arena_results),
        )
        .await;

        let (structure_result, coverage_result) = group1_results;
        let (complexity_result, refactoring_result, impact_result, lsh_result) = group2_results;

        info!("All analysis stages completed");

        // Run cohesion analysis separately (requires mutable access via mutex)
        let cohesion_result = self.run_cohesion_stage(paths, arena_results).await?;

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

/// Stage execution helpers for [`AnalysisStages`].
impl AnalysisStages {
    /// Run stage group 1: structure and coverage analysis in parallel.
    async fn run_stage_group1(
        &self,
        config: &AnalysisConfig,
        paths: &[PathBuf],
        arena_results: &[ArenaAnalysisResult],
    ) -> (Result<StructureAnalysisResults>, Result<CoverageAnalysisResults>) {
        let structure_future = self.run_structure_stage(config, paths, arena_results);
        let coverage_future = self.run_coverage_stage(config, paths);
        future::join(structure_future, coverage_future).await
    }

    /// Run stage group 2: complexity, refactoring, impact, and LSH analysis in parallel.
    async fn run_stage_group2(
        &self,
        config: &AnalysisConfig,
        files: &[PathBuf],
        arena_results: &[ArenaAnalysisResult],
    ) -> (
        Result<ComplexityAnalysisResults>,
        Result<RefactoringAnalysisResults>,
        Result<ImpactAnalysisResults>,
        Result<LshAnalysisResults>,
    ) {
        future::join4(
            self.run_complexity_stage(config, arena_results),
            self.run_refactoring_stage(config, files),
            self.run_impact_stage(config, files),
            self.run_lsh_stage(config, files),
        )
        .await
    }

    /// Run structure analysis stage.
    async fn run_structure_stage(
        &self,
        config: &AnalysisConfig,
        paths: &[PathBuf],
        arena_results: &[ArenaAnalysisResult],
    ) -> Result<StructureAnalysisResults> {
        if !config.enable_structure_analysis {
            return Ok(StructureAnalysisResults::disabled());
        }
        info!("Starting structure analysis...");
        let result = self.run_structure_analysis_with_arena_results(paths, arena_results).await;
        info!("Structure analysis completed");
        result
    }

    /// Run coverage analysis stage.
    async fn run_coverage_stage(
        &self,
        config: &AnalysisConfig,
        paths: &[PathBuf],
    ) -> Result<CoverageAnalysisResults> {
        if !config.enable_coverage_analysis {
            return Ok(CoverageAnalysisResults::disabled());
        }
        info!("Starting coverage analysis...");
        let coverage_config = self.valknut_config.coverage.clone();
        let default_path = PathBuf::from(".");
        let root_path = paths.first().unwrap_or(&default_path);
        let result = self.run_coverage_analysis(root_path, &coverage_config).await;
        info!("Coverage analysis completed");
        result
    }

    /// Run complexity analysis stage.
    async fn run_complexity_stage(
        &self,
        config: &AnalysisConfig,
        arena_results: &[ArenaAnalysisResult],
    ) -> Result<ComplexityAnalysisResults> {
        if !config.enable_complexity_analysis {
            return Ok(ComplexityAnalysisResults::disabled());
        }
        info!("Starting complexity analysis...");
        let result = self.run_complexity_analysis_from_arena_results(arena_results).await;
        info!("Complexity analysis completed");
        result
    }

    /// Run refactoring analysis stage.
    async fn run_refactoring_stage(
        &self,
        config: &AnalysisConfig,
        files: &[PathBuf],
    ) -> Result<RefactoringAnalysisResults> {
        if !config.enable_refactoring_analysis {
            return Ok(RefactoringAnalysisResults::disabled());
        }
        info!("Starting refactoring analysis...");
        let result = self.run_refactoring_analysis(files).await;
        info!("Refactoring analysis completed");
        result
    }

    /// Run impact analysis stage.
    async fn run_impact_stage(
        &self,
        config: &AnalysisConfig,
        files: &[PathBuf],
    ) -> Result<ImpactAnalysisResults> {
        if !config.enable_impact_analysis {
            return Ok(ImpactAnalysisResults::disabled());
        }
        info!("Starting impact analysis...");
        let result = self.run_impact_analysis(files).await;
        info!("Impact analysis completed");
        result
    }

    /// Run LSH analysis stage.
    async fn run_lsh_stage(
        &self,
        config: &AnalysisConfig,
        files: &[PathBuf],
    ) -> Result<LshAnalysisResults> {
        if !config.enable_lsh_analysis || self.lsh_extractor.is_none() {
            return Ok(LshAnalysisResults::disabled());
        }
        info!("Starting LSH analysis...");
        let denoise_enabled = self.valknut_config.denoise.enabled;
        let result = self.run_lsh_analysis(files, denoise_enabled).await;
        info!("LSH analysis completed");
        result
    }

    /// Run cohesion analysis stage (requires mutex, so runs separately).
    async fn run_cohesion_stage(
        &self,
        paths: &[PathBuf],
        arena_results: &[ArenaAnalysisResult],
    ) -> Result<CohesionAnalysisResults> {
        if self.cohesion_extractor.is_none() {
            return Ok(CohesionAnalysisResults::default());
        }
        info!("Starting cohesion analysis...");
        let result = self.run_cohesion_analysis_with_arena_results(paths, arena_results).await;
        info!("Cohesion analysis completed");
        result
    }
}


#[cfg(test)]
#[path = "pipeline_stages_tests.rs"]
mod tests;
