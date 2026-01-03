//! Main pipeline executor that orchestrates the comprehensive analysis.

use chrono::Utc;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;
use walkdir;

use crate::core::ast_service::AstService;
use crate::core::config::{DocHealthConfig, ScoringConfig, ValknutConfig};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::FeatureVector;
use crate::core::scoring::{FeatureScorer, ScoringResult};
use crate::detectors::complexity::{ComplexityAnalyzer, ComplexityConfig};
use crate::detectors::coverage::{CoverageConfig as CoverageDetectorConfig, CoverageExtractor};
use crate::detectors::refactoring::{RefactoringAnalyzer, RefactoringConfig};
use crate::detectors::structure::{StructureConfig, StructureExtractor};
use std::collections::HashMap;
use std::sync::Arc;

use super::health::doc_health::compute_doc_health;
use super::pipeline_config::{AnalysisConfig, QualityGateConfig, QualityGateResult};
use super::results::pipeline_results::{
    ComprehensiveAnalysisResult, CoverageAnalysisResults, DocumentationAnalysisResults,
    HealthMetrics, MemoryStats, PipelineResults, PipelineStatistics, PipelineStatus,
    ScoringResults,
};
use super::pipeline_stages::AnalysisStages;
use super::health::scoring_conversion::{
    convert_to_scoring_results, create_feature_vectors_from_results, health_from_scores,
};
use super::discovery::services::{
    BatchedFileReader, DefaultResultAggregator, FileBatchReader, FileDiscoverer,
    GitAwareFileDiscoverer, ResultAggregator, StageOrchestrator,
};
use crate::core::pipeline::AnalysisSummary;
use crate::detectors::cohesion::CohesionAnalysisResults;

/// Progress callback function type
pub type ProgressCallback = Box<dyn Fn(&str, f64) + Send + Sync>;

/// Main analysis pipeline that orchestrates all analyzers
pub struct AnalysisPipeline {
    config: AnalysisConfig,
    pub(crate) valknut_config: Option<ValknutConfig>,
    feature_scorer: FeatureScorer,
    file_discoverer: Arc<dyn FileDiscoverer>,
    file_reader: Arc<dyn FileBatchReader>,
    stage_runner: Arc<dyn StageOrchestrator>,
    result_aggregator: Arc<dyn ResultAggregator>,
}

/// Factory, configuration, and analysis methods for [`AnalysisPipeline`].
impl AnalysisPipeline {
    /// Create new analysis pipeline with configuration
    pub fn new(config: AnalysisConfig) -> Self {
        let complexity_config = ComplexityConfig::default();
        let structure_config = StructureConfig::default();
        let refactoring_config = RefactoringConfig::default();
        let ast_service = Arc::new(AstService::new());
        let valknut_config = Arc::new(ValknutConfig::default());

        let refactoring_analyzer =
            RefactoringAnalyzer::new(refactoring_config, ast_service.clone());

        let coverage_extractor =
            CoverageExtractor::new(CoverageDetectorConfig::default(), ast_service.clone());

        let stage_runner: Arc<dyn StageOrchestrator> = Arc::new(AnalysisStages::new(
            StructureExtractor::with_config(structure_config),
            ComplexityAnalyzer::new(complexity_config, ast_service.clone()),
            refactoring_analyzer,
            coverage_extractor,
            ast_service,
            valknut_config,
        ));

        let feature_scorer = FeatureScorer::new(ScoringConfig::default());

        Self {
            config,
            valknut_config: None,
            feature_scorer,
            file_discoverer: GitAwareFileDiscoverer::shared(),
            file_reader: BatchedFileReader::default_shared(),
            stage_runner,
            result_aggregator: Arc::new(DefaultResultAggregator::default()),
        }
    }

    /// Create new analysis pipeline with full ValknutConfig support
    pub fn new_with_config(analysis_config: AnalysisConfig, valknut_config: ValknutConfig) -> Self {
        // Debug output removed - LSH integration is working

        let ast_service = Arc::new(AstService::new());
        let config_arc = Arc::new(valknut_config.clone());

        let structure_config = valknut_config.structure.clone();
        let coverage_detector_config = detector_coverage_config(
            &valknut_config.coverage,
            valknut_config.analysis.enable_coverage_analysis,
        );

        // Create common analyzers once
        let structure_extractor = StructureExtractor::with_config(structure_config);
        let complexity_analyzer =
            ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
        let refactoring_analyzer =
            RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());
        let coverage_extractor =
            CoverageExtractor::new(coverage_detector_config, ast_service.clone());

        let stage_runner: Arc<dyn StageOrchestrator> = if analysis_config.enable_lsh_analysis {
            use crate::detectors::lsh::config::DedupeConfig;
            use crate::detectors::lsh::LshExtractor;

            // Build LSH extractor with dedupe thresholds and denoise flag
            let mut dedupe_config = DedupeConfig::default();
            dedupe_config.min_function_tokens = valknut_config.denoise.min_function_tokens;
            dedupe_config.min_ast_nodes = valknut_config.dedupe.min_ast_nodes;
            dedupe_config.min_match_tokens = valknut_config.denoise.min_match_tokens;
            dedupe_config.require_distinct_blocks = valknut_config.denoise.require_blocks;
            dedupe_config.shingle_k = valknut_config.lsh.shingle_size;
            dedupe_config.threshold_s = valknut_config.denoise.similarity;

            let lsh_extractor = LshExtractor::with_dedupe_config(dedupe_config)
                .with_lsh_config(valknut_config.lsh.clone().into())
                .with_denoise_enabled(valknut_config.denoise.enabled);

            info!(
                "LSH extractor configured (denoise: {}, k={}, min_ast_nodes={}, min_tokens={}, similarity={:.2})",
                valknut_config.denoise.enabled,
                valknut_config.lsh.shingle_size,
                valknut_config.dedupe.min_ast_nodes,
                valknut_config.denoise.min_function_tokens,
                valknut_config.denoise.similarity
            );

            Arc::new(AnalysisStages::new_with_lsh(
                structure_extractor,
                complexity_analyzer,
                refactoring_analyzer,
                lsh_extractor,
                coverage_extractor,
                ast_service,
                Arc::clone(&config_arc),
            ))
        } else {
            Arc::new(AnalysisStages::new(
                structure_extractor,
                complexity_analyzer,
                refactoring_analyzer,
                coverage_extractor,
                ast_service,
                Arc::clone(&config_arc),
            ))
        };

        let scoring_config = valknut_config.scoring.clone();
        let feature_scorer = FeatureScorer::new(scoring_config);

        Self {
            config: analysis_config,
            valknut_config: Some(valknut_config),
            feature_scorer,
            file_discoverer: GitAwareFileDiscoverer::shared(),
            file_reader: BatchedFileReader::default_shared(),
            stage_runner,
            result_aggregator: Arc::new(DefaultResultAggregator::default()),
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(AnalysisConfig::default())
    }

    /// Override the file ingestion services (useful for tests or custom environments).
    pub fn with_file_services(
        mut self,
        discoverer: Arc<dyn FileDiscoverer>,
        reader: Arc<dyn FileBatchReader>,
    ) -> Self {
        self.file_discoverer = discoverer;
        self.file_reader = reader;
        self
    }

    /// Override the result aggregator (useful for tests or custom environments).
    pub fn with_result_aggregator(mut self, aggregator: Arc<dyn ResultAggregator>) -> Self {
        self.result_aggregator = aggregator;
        self
    }

    /// Run comprehensive analysis on the given paths
    pub async fn analyze_paths(
        &self,
        paths: &[PathBuf],
        progress_callback: Option<ProgressCallback>,
    ) -> Result<ComprehensiveAnalysisResult> {
        let start_time = Instant::now();
        let analysis_id = Uuid::new_v4().to_string();

        info!(
            "Starting comprehensive analysis {} for {} paths",
            analysis_id,
            paths.len()
        );

        // Update progress
        if let Some(ref callback) = progress_callback {
            callback("Discovering files...", 0.0);
        }

        // Stage 1: File discovery
        let files = self.discover_files(paths).await?;
        info!("Discovered {} files for analysis", files.len());

        if let Some(ref callback) = progress_callback {
            callback("Running arena-based entity extraction...", 5.0);
        }

        // Stage 1.5: Batched file reading for performance
        if let Some(ref callback) = progress_callback {
            callback("Reading file contents in batches...", 7.5);
        }

        let file_contents = self.read_files_batched(&files).await?;
        info!("Read {} files in batches", file_contents.len());

        // Stage 1.6: Arena-based entity extraction (performance optimization)
        let arena_results = self
            .stage_runner
            .run_arena_analysis_with_content(&file_contents)
            .await?;
        info!(
            "Arena analysis completed: {} files processed with {:.2} KB total arena usage",
            arena_results.len(),
            arena_results.iter().map(|r| r.arena_kb_used()).sum::<f64>()
        );

        if let Some(ref callback) = progress_callback {
            callback("Running parallel analysis stages...", 10.0);
        }

        let stage_results_bundle = self
            .stage_runner
            .run_all_stages(&self.config, paths, &files, &arena_results)
            .await?;
        let structure_results = stage_results_bundle.structure;
        let coverage_results = stage_results_bundle.coverage;
        let complexity_results = stage_results_bundle.complexity;
        let refactoring_results = stage_results_bundle.refactoring;
        let impact_results = stage_results_bundle.impact;
        let lsh_results = stage_results_bundle.lsh;
        let cohesion_results = stage_results_bundle.cohesion;

        if let Some(ref callback) = progress_callback {
            callback("Calculating health metrics...", 90.0);
        }

        // Stage 8: Calculate summary and health metrics
        let mut summary = self.result_aggregator.build_summary(
            &files,
            &structure_results,
            &complexity_results,
            &refactoring_results,
            &impact_results,
        );
        let mut health_metrics = self.result_aggregator.build_health_metrics(
            &complexity_results,
            &structure_results,
            &impact_results,
        );

        let mut documentation_results = DocumentationAnalysisResults::default();

        // Compute documentation health (project-level for now)
        if let Some(doc_health_result) = compute_doc_health(
            paths,
            &files,
            self.valknut_config
                .as_ref()
                .map(|c| &c.docs)
                .unwrap_or(&crate::core::config::DocHealthConfig::default()),
        ) {
            health_metrics.doc_health_score = doc_health_result.score;
            health_metrics.overall_health_score = (health_metrics.maintainability_score * 0.28
                + health_metrics.structure_quality_score * 0.25
                + (100.0 - health_metrics.complexity_score) * 0.18
                + (100.0 - health_metrics.technical_debt_ratio) * 0.19
                + health_metrics.doc_health_score * 0.10)
                .clamp(0.0, 100.0);

            summary.doc_health_score = (doc_health_result.score / 100.0).clamp(0.0, 1.0);
            summary.apply_doc_issues(doc_health_result.issue_count);

            documentation_results.enabled = true;
            documentation_results.issues_count = doc_health_result.issue_count;
            documentation_results.doc_health_score = doc_health_result.score;
            documentation_results.file_doc_issues = doc_health_result.file_issues;
            documentation_results.file_doc_health = doc_health_result.file_health;
            documentation_results.directory_doc_health = doc_health_result.dir_scores;
            documentation_results.directory_doc_issues = doc_health_result.dir_issues;
        }

        if let Some(ref callback) = progress_callback {
            callback("Analysis complete", 100.0);
        }

        let processing_time = start_time.elapsed().as_secs_f64();

        info!(
            "Comprehensive analysis completed in {:.2}s",
            processing_time
        );
        info!("Total issues found: {}", summary.total_issues);
        info!(
            "Overall health score: {:.1}",
            health_metrics.overall_health_score
        );

        Ok(ComprehensiveAnalysisResult {
            analysis_id,
            timestamp: Utc::now(),
            processing_time,
            config: self.config.clone(),
            summary,
            structure: structure_results,
            complexity: complexity_results,
            refactoring: refactoring_results,
            impact: impact_results,
            lsh: lsh_results,
            coverage: coverage_results,
            documentation: documentation_results,
            cohesion: cohesion_results,
            health_metrics,
        })
    }

    /// Discover files to analyze using git-aware file discovery
    pub(crate) async fn discover_files(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let start_time = std::time::Instant::now();
        let mut files =
            self.file_discoverer
                .discover(paths, &self.config, self.valknut_config.as_ref())?;

        let discovery_time = start_time.elapsed();

        if self.config.max_files > 0 && files.len() > self.config.max_files {
            warn!(
                "Limiting analysis to {} files (found {} in {:?})",
                self.config.max_files,
                files.len(),
                discovery_time
            );
            files.truncate(self.config.max_files);
        } else {
            info!("Discovered {} files in {:?}", files.len(), discovery_time);
        }

        Ok(files)
    }

    /// Read multiple files in batches for optimal I/O performance
    pub(crate) async fn read_files_batched(&self, files: &[PathBuf]) -> Result<Vec<(PathBuf, String)>> {
        let start_time = std::time::Instant::now();
        let file_contents = self.file_reader.read_files(files).await?;
        let read_time = start_time.elapsed();
        let total_size_mb = file_contents
            .iter()
            .map(|(_, content)| content.len())
            .sum::<usize>() as f64
            / (1024.0 * 1024.0);

        info!(
            "Read {} files ({:.2} MB) in {:?}",
            file_contents.len(),
            total_size_mb,
            read_time
        );

        Ok(file_contents)
    }

    /// Check if a file should be included for dedupe analysis based on scope filtering
    pub fn should_include_for_dedupe(&self, file: &Path, valknut_config: &ValknutConfig) -> bool {
        let file_path_str = file.to_string_lossy();

        // Check dedupe exclude patterns first
        for exclude_pattern in &valknut_config.dedupe.exclude {
            if self.matches_glob_pattern(&file_path_str, exclude_pattern) {
                return false;
            }
        }

        // Check dedupe include patterns
        for include_pattern in &valknut_config.dedupe.include {
            if self.matches_glob_pattern(&file_path_str, include_pattern) {
                return true;
            }
        }

        // Default to false if no include pattern matches
        false
    }

    /// Glob pattern matching using the `glob` crate
    fn matches_glob_pattern(&self, path: &str, pattern: &str) -> bool {
        match glob::Pattern::new(pattern) {
            Ok(glob) => glob.matches(path),
            Err(_) => false,
        }
    }

    /// Get pipeline status for API layer
    pub fn get_status(&self) -> PipelineStatus {
        let is_ready = self.is_ready();
        PipelineStatus {
            ready: is_ready,
            status: if is_ready {
                "Ready".to_string()
            } else {
                "Not initialized".to_string()
            },
            errors: Vec::new(),
            issues: Vec::new(),
            is_ready,
            config_valid: true,
        }
    }

    /// Check if pipeline is ready for analysis
    pub fn is_ready(&self) -> bool {
        true // Always ready with current implementation
    }

    /// Legacy API - analyze a directory and wrap in PipelineResults
    pub async fn analyze_directory(&self, path: &Path) -> Result<PipelineResults> {
        let paths = vec![path.to_path_buf()];
        let results = self.analyze_paths(&paths, None).await?;
        Ok(self.wrap_results(results))
    }

    /// Legacy API - analyze feature vectors
    pub async fn analyze_vectors(&self, vectors: Vec<FeatureVector>) -> Result<PipelineResults> {
        let analysis_id = Uuid::new_v4().to_string();
        let timestamp = Utc::now();
        let mut feature_vectors = vectors;
        let scoring_files: Vec<ScoringResult> = if feature_vectors.is_empty() {
            Vec::new()
        } else {
            self.feature_scorer
                .score(&mut feature_vectors)
                .map_err(|err| {
                    ValknutError::internal(format!("Failed to score feature vectors: {}", err))
                })?
        };

        let health_metrics = health_from_scores(&scoring_files);
        let total_entities = scoring_files.len();
        let priority_counts = scoring_files
            .iter()
            .filter(|result| result.priority != crate::core::scoring::Priority::None)
            .count();
        let high_priority = scoring_files
            .iter()
            .filter(|result| {
                matches!(
                    result.priority,
                    crate::core::scoring::Priority::High | crate::core::scoring::Priority::Critical
                )
            })
            .count();

        let critical_issues = scoring_files
            .iter()
            .filter(|result| result.priority == crate::core::scoring::Priority::Critical)
            .count();

        let code_health_score = if total_entities > 0 {
            let penalty = (priority_counts as f64 / total_entities as f64).min(1.0);
            (1.0 - penalty).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let summary = AnalysisSummary {
            files_processed: total_entities,
            entities_analyzed: total_entities,
            refactoring_needed: priority_counts,
            high_priority,
            critical: critical_issues,
            avg_refactoring_score: 0.0,
            code_health_score,
            total_files: total_entities,
            total_entities,
            total_lines_of_code: 0,
            languages: Vec::new(),
            total_issues: priority_counts,
            high_priority_issues: high_priority,
            critical_issues,
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        let placeholder = ComprehensiveAnalysisResult {
            analysis_id: analysis_id.clone(),
            timestamp,
            processing_time: 0.0,
            config: self.config.clone(),
            summary,
            structure: super::results::pipeline_results::StructureAnalysisResults {
                enabled: false,
                directory_recommendations: Vec::new(),
                file_splitting_recommendations: Vec::new(),
                issues_count: 0,
            },
            complexity: super::results::pipeline_results::ComplexityAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                average_cyclomatic_complexity: 0.0,
                average_cognitive_complexity: 0.0,
                average_technical_debt_score: 0.0,
                average_maintainability_index: 100.0,
                issues_count: 0,
            },
            refactoring: super::results::pipeline_results::RefactoringAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                opportunities_count: priority_counts,
            },
            impact: super::results::pipeline_results::ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            },
            lsh: super::results::pipeline_results::LshAnalysisResults {
                enabled: false,
                clone_pairs: Vec::new(),
                max_similarity: 0.0,
                avg_similarity: 0.0,
                duplicate_count: 0,
                apted_verification_enabled: false,
                verification: None,
                denoising_enabled: false,
                tfidf_stats: None,
            },
            coverage: CoverageAnalysisResults {
                enabled: false,
                coverage_files_used: Vec::new(),
                coverage_gaps: Vec::new(),
                gaps_count: 0,
                overall_coverage_percentage: None,
                analysis_method: "disabled".to_string(),
            },
            documentation: DocumentationAnalysisResults::default(),
            cohesion: CohesionAnalysisResults::default(),
            health_metrics,
        };

        Ok(PipelineResults {
            analysis_id,
            timestamp,
            results: placeholder,
            statistics: PipelineStatistics {
                memory_stats: MemoryStats {
                    current_memory_bytes: 0,
                    peak_memory_bytes: 0,
                    final_memory_bytes: 0,
                    efficiency_score: 1.0,
                },
                files_processed: total_entities,
                total_duration_ms: 0,
            },
            errors: Vec::new(),
            scoring_results: ScoringResults {
                files: scoring_files,
            },
            feature_vectors,
        })
    }

    /// Fit the pipeline (legacy API compatibility)
    pub async fn fit(&mut self, vectors: &[FeatureVector]) -> Result<()> {
        if vectors.is_empty() {
            return Ok(());
        }

        self.feature_scorer.fit(vectors)?;
        Ok(())
    }

    /// Get extractor registry (legacy API compatibility)
    pub fn extractor_registry(&self) -> ExtractorRegistry {
        ExtractorRegistry::new()
    }

    /// Wrap comprehensive analysis results into the legacy PipelineResults format.
    pub fn wrap_results(&self, results: ComprehensiveAnalysisResult) -> PipelineResults {
        let scoring_files = convert_to_scoring_results(&results);

        // Create feature vectors that correspond to the scoring results
        let feature_vectors = create_feature_vectors_from_results(&results);

        PipelineResults {
            analysis_id: results.analysis_id.clone(),
            timestamp: results.timestamp,
            statistics: PipelineStatistics {
                memory_stats: MemoryStats {
                    current_memory_bytes: 0,
                    peak_memory_bytes: 0,
                    final_memory_bytes: 0,
                    efficiency_score: 1.0,
                },
                files_processed: results.summary.total_files,
                total_duration_ms: (results.processing_time * 1000.0) as u64,
            },
            results,
            errors: Vec::new(),
            scoring_results: ScoringResults {
                files: scoring_files,
            },
            feature_vectors,
        }
    }

    /// Evaluate quality gates against analysis results
    pub fn evaluate_quality_gates(
        &self,
        config: &QualityGateConfig,
        results: &ComprehensiveAnalysisResult,
    ) -> QualityGateResult {
        self.result_aggregator
            .evaluate_quality_gates(config, results)
    }

}

/// Converts core coverage config into the detector-specific coverage config.
fn detector_coverage_config(
    core_config: &crate::core::config::CoverageConfig,
    coverage_enabled: bool,
) -> CoverageDetectorConfig {
    let mut detector_config = CoverageDetectorConfig::default();

    detector_config.auto_discover = core_config.auto_discover;
    detector_config.search_paths = core_config.search_paths.clone();
    detector_config.file_patterns = core_config.file_patterns.clone();
    detector_config.max_age_days = core_config.max_age_days;
    detector_config.coverage_file = core_config.coverage_file.clone();
    detector_config.enabled = coverage_enabled;

    if let Some(path) = &core_config.coverage_file {
        if !detector_config
            .report_paths
            .iter()
            .any(|existing| existing == path)
        {
            detector_config.report_paths.push(path.clone());
        }
    }

    detector_config
}

/// Registry for extractors (legacy compatibility)
pub struct ExtractorRegistry;

/// Factory and query methods for [`ExtractorRegistry`] (legacy compatibility).
impl ExtractorRegistry {
    /// Creates a new empty extractor registry.
    pub fn new() -> Self {
        Self
    }

    /// Returns an empty iterator (placeholder for legacy API).
    pub fn get_all_extractors(&self) -> std::iter::Empty<()> {
        std::iter::empty()
    }
}


#[cfg(test)]
#[path = "pipeline_executor_tests.rs"]
mod tests;
