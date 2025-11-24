//! Main pipeline executor that orchestrates the comprehensive analysis.

use chrono::Utc;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;
use walkdir;

use crate::core::ast_service::AstService;
use crate::core::config::{ScoringConfig, ValknutConfig};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::FeatureVector;
use crate::core::scoring::{FeatureScorer, ScoringResult};
use crate::detectors::complexity::{ComplexityAnalyzer, ComplexityConfig, ComplexitySeverity};
use crate::detectors::coverage::{CoverageConfig as CoverageDetectorConfig, CoverageExtractor};
use crate::detectors::refactoring::{RefactoringAnalyzer, RefactoringConfig};
use crate::detectors::structure::{StructureConfig, StructureExtractor};
use std::sync::Arc;

use super::pipeline_config::{AnalysisConfig, QualityGateConfig, QualityGateResult};
use super::pipeline_results::{
    ComprehensiveAnalysisResult, CoverageAnalysisResults, HealthMetrics, MemoryStats,
    PipelineResults, PipelineStatistics, PipelineStatus, ScoringResults,
};
use super::pipeline_stages::AnalysisStages;
use super::services::{
    BatchedFileReader, DefaultResultAggregator, FileBatchReader, FileDiscoverer,
    GitAwareFileDiscoverer, ResultAggregator, StageOrchestrator,
};
use crate::core::pipeline::AnalysisSummary;

/// Progress callback function type
pub type ProgressCallback = Box<dyn Fn(&str, f64) + Send + Sync>;

/// Main analysis pipeline that orchestrates all analyzers
pub struct AnalysisPipeline {
    config: AnalysisConfig,
    valknut_config: Option<ValknutConfig>,
    feature_scorer: FeatureScorer,
    file_discoverer: Arc<dyn FileDiscoverer>,
    file_reader: Arc<dyn FileBatchReader>,
    stage_runner: Arc<dyn StageOrchestrator>,
    result_aggregator: Arc<dyn ResultAggregator>,
}

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

        let stage_runner: Arc<dyn StageOrchestrator> =
            if valknut_config.denoise.enabled && analysis_config.enable_lsh_analysis {
                use crate::detectors::lsh::config::DedupeConfig;
                use crate::detectors::lsh::LshExtractor;

                // Create LSH extractor with denoising configuration
                let mut dedupe_config = DedupeConfig::default();
                dedupe_config.min_function_tokens = valknut_config.denoise.min_function_tokens;
                dedupe_config.min_ast_nodes = valknut_config.denoise.min_match_tokens; // Mapping to closest field
                dedupe_config.shingle_k = valknut_config.lsh.shingle_size;
                dedupe_config.threshold_s = valknut_config.denoise.similarity;

                let lsh_extractor = LshExtractor::with_dedupe_config(dedupe_config)
                    .with_lsh_config(valknut_config.lsh.clone().into())
                    .with_denoise_enabled(true);

                info!(
                    "LSH extractor configured with denoising enabled (k={})",
                    valknut_config.lsh.shingle_size
                );

                let structure_extractor = StructureExtractor::with_config(structure_config.clone());
                let complexity_analyzer =
                    ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
                let refactoring_analyzer =
                    RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());
                let coverage_extractor =
                    CoverageExtractor::new(coverage_detector_config.clone(), ast_service.clone());

                Arc::new(AnalysisStages::new_with_lsh(
                    structure_extractor,
                    complexity_analyzer,
                    refactoring_analyzer,
                    lsh_extractor,
                    coverage_extractor,
                    ast_service.clone(),
                    Arc::clone(&config_arc),
                ))
            } else if analysis_config.enable_lsh_analysis {
                use crate::detectors::lsh::LshExtractor;

                // Create LSH extractor without denoising
                let lsh_extractor =
                    LshExtractor::new().with_lsh_config(valknut_config.lsh.clone().into());
                info!("LSH extractor configured without denoising");

                let structure_extractor = StructureExtractor::with_config(structure_config.clone());
                let complexity_analyzer =
                    ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
                let refactoring_analyzer =
                    RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());
                let coverage_extractor =
                    CoverageExtractor::new(coverage_detector_config.clone(), ast_service.clone());

                Arc::new(AnalysisStages::new_with_lsh(
                    structure_extractor,
                    complexity_analyzer,
                    refactoring_analyzer,
                    lsh_extractor,
                    coverage_extractor,
                    ast_service.clone(),
                    Arc::clone(&config_arc),
                ))
            } else {
                // No LSH analysis
                let structure_extractor = StructureExtractor::with_config(structure_config);
                let complexity_analyzer =
                    ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
                let refactoring_analyzer =
                    RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());
                let coverage_extractor =
                    CoverageExtractor::new(coverage_detector_config, ast_service.clone());

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

        if let Some(ref callback) = progress_callback {
            callback("Calculating health metrics...", 90.0);
        }

        // Stage 8: Calculate summary and health metrics
        let summary = self.result_aggregator.build_summary(
            &files,
            &structure_results,
            &complexity_results,
            &refactoring_results,
            &impact_results,
        );
        let health_metrics = self.result_aggregator.build_health_metrics(
            &complexity_results,
            &structure_results,
            &impact_results,
        );

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
            health_metrics,
        })
    }

    /// Discover files to analyze using git-aware file discovery
    async fn discover_files(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
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
    async fn read_files_batched(&self, files: &[PathBuf]) -> Result<Vec<(PathBuf, String)>> {
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

        let health_metrics = Self::health_from_scores(&scoring_files);
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
        };

        let placeholder = ComprehensiveAnalysisResult {
            analysis_id: analysis_id.clone(),
            timestamp,
            processing_time: 0.0,
            config: self.config.clone(),
            summary,
            structure: super::pipeline_results::StructureAnalysisResults {
                enabled: false,
                directory_recommendations: Vec::new(),
                file_splitting_recommendations: Vec::new(),
                issues_count: 0,
            },
            complexity: super::pipeline_results::ComplexityAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                average_cyclomatic_complexity: 0.0,
                average_cognitive_complexity: 0.0,
                average_technical_debt_score: 0.0,
                average_maintainability_index: 100.0,
                issues_count: 0,
            },
            refactoring: super::pipeline_results::RefactoringAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                opportunities_count: priority_counts,
            },
            impact: super::pipeline_results::ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            },
            lsh: super::pipeline_results::LshAnalysisResults {
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

    pub fn wrap_results(&self, results: ComprehensiveAnalysisResult) -> PipelineResults {
        let scoring_files = Self::convert_to_scoring_results(&results);

        // Create feature vectors that correspond to the scoring results
        let feature_vectors = Self::create_feature_vectors_from_results(&results);

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

    fn health_from_scores(scoring: &[ScoringResult]) -> HealthMetrics {
        if scoring.is_empty() {
            return HealthMetrics {
                overall_health_score: 100.0,
                maintainability_score: 100.0,
                technical_debt_ratio: 0.0,
                complexity_score: 0.0,
                structure_quality_score: 100.0,
            };
        }

        let avg_abs_score = scoring
            .iter()
            .map(|result| result.overall_score.abs())
            .sum::<f64>()
            / scoring.len() as f64;

        let overall_health = (100.0 - avg_abs_score * 20.0).clamp(0.0, 100.0);
        let maintainability = (100.0 - avg_abs_score * 18.0).clamp(0.0, 100.0);
        let technical_debt = (avg_abs_score * 25.0).clamp(0.0, 100.0);
        let complexity = (avg_abs_score * 30.0).clamp(0.0, 100.0);
        let structure_quality = (100.0 - avg_abs_score * 12.0).clamp(0.0, 100.0);

        HealthMetrics {
            overall_health_score: overall_health,
            maintainability_score: maintainability,
            technical_debt_ratio: technical_debt,
            complexity_score: complexity,
            structure_quality_score: structure_quality,
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

    /// Convert comprehensive analysis results to scoring results
    fn convert_to_scoring_results(
        results: &ComprehensiveAnalysisResult,
    ) -> Vec<crate::core::scoring::ScoringResult> {
        use crate::core::scoring::{Priority, ScoringResult};
        use std::collections::HashMap;

        let mut scoring_results = Vec::new();

        // Helper closure to clamp values into scoring range
        let clamp_score = |value: f64| value.clamp(0.0, 100.0);

        // Convert complexity analysis results to scoring results
        for complexity_result in &results.complexity.detailed_results {
            let entity_id = format!(
                "{}:{}:{}",
                complexity_result.file_path,
                complexity_result.entity_type,
                complexity_result.entity_name
            );

            let metrics = &complexity_result.metrics;

            // Normalise metrics against reasonable thresholds
            let cyclomatic_score = clamp_score((metrics.cyclomatic() / 10.0) * 40.0);
            let cognitive_score = clamp_score((metrics.cognitive() / 15.0) * 30.0);
            let nesting_score = clamp_score(metrics.max_nesting_depth * 6.0);
            let debt_score = clamp_score(metrics.technical_debt_score);
            let maintainability_penalty = clamp_score(100.0 - metrics.maintainability_index);

            let mut category_scores = HashMap::new();
            category_scores.insert("complexity".to_string(), cyclomatic_score);
            category_scores.insert("cognitive".to_string(), cognitive_score);
            category_scores.insert("structure".to_string(), nesting_score);
            category_scores.insert("debt".to_string(), debt_score);
            category_scores.insert("maintainability".to_string(), maintainability_penalty);

            let mut feature_contributions = HashMap::new();
            feature_contributions.insert("cyclomatic_complexity".to_string(), metrics.cyclomatic());
            feature_contributions.insert("cognitive_complexity".to_string(), metrics.cognitive());
            feature_contributions
                .insert("max_nesting_depth".to_string(), metrics.max_nesting_depth);
            feature_contributions.insert("lines_of_code".to_string(), metrics.lines_of_code);
            feature_contributions.insert(
                "technical_debt_score".to_string(),
                metrics.technical_debt_score,
            );
            feature_contributions.insert(
                "maintainability_index".to_string(),
                metrics.maintainability_index,
            );

            let weighted_overall = clamp_score(
                cyclomatic_score * 0.30
                    + cognitive_score * 0.25
                    + nesting_score * 0.15
                    + debt_score * 0.20
                    + maintainability_penalty * 0.10,
            );

            let mut priority = {
                use crate::detectors::complexity::ComplexitySeverity;
                match complexity_result.severity {
                    ComplexitySeverity::Critical => Priority::Critical,
                    ComplexitySeverity::VeryHigh => Priority::High,
                    ComplexitySeverity::High => Priority::High,
                    ComplexitySeverity::Medium => Priority::Medium,
                    ComplexitySeverity::Moderate => Priority::Medium,
                    ComplexitySeverity::Low => Priority::Low,
                }
            };

            if complexity_result.issues.is_empty() {
                priority = if weighted_overall >= 70.0 {
                    Priority::Critical
                } else if weighted_overall >= 55.0 {
                    Priority::High
                } else if weighted_overall >= 35.0 {
                    Priority::Medium
                } else if weighted_overall >= 20.0 {
                    Priority::Low
                } else {
                    Priority::None
                };
            }

            let confidence = if metrics.lines_of_code >= 30.0 {
                0.95
            } else if metrics.lines_of_code >= 15.0 {
                0.85
            } else if metrics.lines_of_code >= 5.0 {
                0.7
            } else {
                0.5
            };

            let feature_count = feature_contributions.len();
            scoring_results.push(ScoringResult {
                entity_id,
                overall_score: weighted_overall,
                priority,
                category_scores,
                feature_contributions,
                normalized_feature_count: feature_count,
                confidence,
            });
        }

        // Convert refactoring analysis results to scoring results
        for refactoring_result in &results.refactoring.detailed_results {
            let entity_id = format!(
                "{}:refactoring:{}",
                refactoring_result.file_path,
                refactoring_result.recommendations.len()
            );

            // Map refactoring metrics to scoring categories
            let mut category_scores = HashMap::new();
            let refactoring_score = refactoring_result.refactoring_score;
            category_scores.insert("refactoring".to_string(), refactoring_score);

            // Map individual features to contributions
            let mut feature_contributions = HashMap::new();
            feature_contributions.insert("refactoring_score".to_string(), refactoring_score);
            feature_contributions.insert(
                "refactoring_recommendations".to_string(),
                refactoring_result.recommendations.len() as f64,
            );

            // Calculate overall score based on refactoring needs
            let overall_score = clamp_score(refactoring_score);

            let priority = if overall_score >= 75.0 {
                Priority::Critical
            } else if overall_score >= 55.0 {
                Priority::High
            } else if overall_score >= 35.0 {
                Priority::Medium
            } else if overall_score >= 20.0 {
                Priority::Low
            } else {
                Priority::None
            };

            // High confidence for refactoring analysis
            let confidence = 0.85;

            if priority != Priority::None {
                let feature_count = feature_contributions.len();
                scoring_results.push(ScoringResult {
                    entity_id,
                    overall_score,
                    priority,
                    category_scores,
                    feature_contributions,
                    normalized_feature_count: feature_count,
                    confidence,
                });
            }
        }

        scoring_results
    }

    /// Create feature vectors from comprehensive analysis results
    fn create_feature_vectors_from_results(
        results: &ComprehensiveAnalysisResult,
    ) -> Vec<FeatureVector> {
        let mut feature_vectors = Vec::new();

        // Create feature vectors from complexity analysis results
        for complexity_result in &results.complexity.detailed_results {
            let entity_id = format!(
                "{}:{}:{}",
                complexity_result.file_path,
                complexity_result.entity_type,
                complexity_result.entity_name
            );

            let metrics = &complexity_result.metrics;

            // Create feature vector with features and their values
            let mut feature_vector = FeatureVector::new(entity_id.clone());

            // Add raw feature values
            feature_vector.add_feature("cyclomatic_complexity", metrics.cyclomatic());
            feature_vector.add_feature("cognitive_complexity", metrics.cognitive());
            feature_vector.add_feature("max_nesting_depth", metrics.max_nesting_depth);
            feature_vector.add_feature("lines_of_code", metrics.lines_of_code);
            feature_vector.add_feature("technical_debt_score", metrics.technical_debt_score);
            feature_vector.add_feature("maintainability_index", metrics.maintainability_index);

            // Add normalized versions (simple normalization for now)
            feature_vector.normalized_features.insert(
                "cyclomatic_complexity".to_string(),
                (metrics.cyclomatic() / 10.0).min(1.0),
            );
            feature_vector.normalized_features.insert(
                "cognitive_complexity".to_string(),
                (metrics.cognitive() / 15.0).min(1.0),
            );
            feature_vector.normalized_features.insert(
                "max_nesting_depth".to_string(),
                (metrics.max_nesting_depth / 5.0).min(1.0),
            );
            feature_vector.normalized_features.insert(
                "lines_of_code".to_string(),
                (metrics.lines_of_code / 100.0).min(1.0),
            );
            feature_vector.normalized_features.insert(
                "technical_debt_score".to_string(),
                metrics.technical_debt_score / 100.0,
            );
            feature_vector.normalized_features.insert(
                "maintainability_index".to_string(),
                metrics.maintainability_index / 100.0,
            );

            // Set metadata
            feature_vector.add_metadata(
                "entity_type",
                serde_json::Value::String(complexity_result.entity_type.clone()),
            );
            feature_vector.add_metadata(
                "file_path",
                serde_json::Value::String(complexity_result.file_path.clone()),
            );
            feature_vector
                .add_metadata("language", serde_json::Value::String("Python".to_string()));
            feature_vector.add_metadata(
                "line_number",
                serde_json::Value::Number(complexity_result.start_line.into()),
            );
            // Note: end_line not available in ComplexityAnalysisResult

            feature_vectors.push(feature_vector);
        }

        // Create feature vectors from refactoring analysis results
        for refactoring_result in &results.refactoring.detailed_results {
            let entity_id = format!(
                "{}:refactoring:{}",
                refactoring_result.file_path,
                refactoring_result.recommendations.len()
            );

            let mut feature_vector = FeatureVector::new(entity_id.clone());

            // Add refactoring-specific features
            feature_vector.add_feature("refactoring_score", refactoring_result.refactoring_score);
            feature_vector.add_feature(
                "refactoring_recommendations",
                refactoring_result.recommendations.len() as f64,
            );

            // Add normalized versions
            feature_vector.normalized_features.insert(
                "refactoring_score".to_string(),
                refactoring_result.refactoring_score / 100.0,
            );
            feature_vector.normalized_features.insert(
                "refactoring_recommendations".to_string(),
                (refactoring_result.recommendations.len() as f64 / 10.0).min(1.0),
            );

            // Set metadata
            feature_vector.add_metadata(
                "entity_type",
                serde_json::Value::String("refactoring".to_string()),
            );
            feature_vector.add_metadata(
                "file_path",
                serde_json::Value::String(refactoring_result.file_path.clone()),
            );
            feature_vector
                .add_metadata("language", serde_json::Value::String("Python".to_string()));

            feature_vectors.push(feature_vector);
        }

        feature_vectors
    }
}

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

impl ExtractorRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn get_all_extractors(&self) -> std::iter::Empty<()> {
        std::iter::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::ValknutConfig;
    use crate::core::featureset::FeatureVector;
    use crate::core::pipeline::pipeline_config::{AnalysisConfig, QualityGateConfig};
    use crate::core::pipeline::pipeline_results;
    use crate::core::pipeline::pipeline_results::{
        CoverageAnalysisResults, CoverageFileInfo, HealthMetrics, ImpactAnalysisResults,
        LshAnalysisResults, RefactoringAnalysisResults, StructureAnalysisResults,
    };
    use crate::core::pipeline::result_types::AnalysisSummary;
    use crate::core::pipeline::DefaultResultAggregator;
    use crate::core::scoring::{Priority, ScoringResult};
    use crate::detectors::complexity::{
        ComplexityAnalysisResult, ComplexityIssue, ComplexityMetrics, ComplexitySeverity,
        HalsteadMetrics,
    };
    use crate::detectors::refactoring::{
        RefactoringAnalysisResult, RefactoringRecommendation, RefactoringType,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn sample_complexity_result(
        file_path: &str,
        cyclomatic: f64,
        technical_debt: f64,
        maintainability: f64,
        severity: ComplexitySeverity,
    ) -> ComplexityAnalysisResult {
        ComplexityAnalysisResult {
            entity_id: format!("{file_path}::sample_fn"),
            file_path: file_path.to_string(),
            line_number: 1,
            start_line: 1,
            entity_name: "sample_fn".to_string(),
            entity_type: "function".to_string(),
            metrics: ComplexityMetrics {
                cyclomatic_complexity: cyclomatic,
                cognitive_complexity: cyclomatic + 5.0,
                max_nesting_depth: 3.0,
                parameter_count: 2.0,
                lines_of_code: 24.0,
                statement_count: 12.0,
                halstead: HalsteadMetrics::default(),
                technical_debt_score: technical_debt,
                maintainability_index: maintainability,
                decision_points: Vec::new(),
            },
            issues: vec![ComplexityIssue {
                entity_id: format!("{file_path}:sample_fn"),
                issue_type: "cyclomatic_complexity".to_string(),
                severity: "High".to_string(),
                description: "Cyclomatic complexity exceeds threshold".to_string(),
                recommendation: "Split the function into smaller helpers".to_string(),
                location: "src/lib.rs:1-10".to_string(),
                metric_value: cyclomatic,
                threshold: 20.0,
            }],
            severity,
            recommendations: vec!["Reduce branches".to_string()],
        }
    }

    fn build_sample_results() -> ComprehensiveAnalysisResult {
        let complexity_entries = vec![
            sample_complexity_result("src/lib.rs", 28.0, 72.0, 48.0, ComplexitySeverity::Critical),
            sample_complexity_result("src/utils.rs", 22.0, 65.0, 55.0, ComplexitySeverity::High),
        ];

        let recommendation = RefactoringRecommendation {
            refactoring_type: RefactoringType::ExtractMethod,
            description: "Extract helper to simplify branching".to_string(),
            estimated_impact: 8.0,
            estimated_effort: 3.0,
            priority_score: 2.6,
            location: (5, 25),
        };

        let refactoring_entry = RefactoringAnalysisResult {
            file_path: "src/lib.rs".to_string(),
            recommendations: vec![recommendation],
            refactoring_score: 82.0,
        };

        let summary = AnalysisSummary {
            files_processed: 2,
            entities_analyzed: 2,
            refactoring_needed: 2,
            high_priority: 3,
            critical: 2,
            avg_refactoring_score: 78.0,
            code_health_score: 0.45,
            total_files: 2,
            total_entities: 2,
            total_lines_of_code: 400,
            languages: vec!["Rust".to_string()],
            total_issues: 6,
            high_priority_issues: 4,
            critical_issues: 3,
        };

        ComprehensiveAnalysisResult {
            analysis_id: "analysis".to_string(),
            timestamp: Utc::now(),
            processing_time: 1.2,
            config: AnalysisConfig::default(),
            summary,
            structure: StructureAnalysisResults {
                enabled: true,
                directory_recommendations: vec![json!({"path": "src", "reason": "Deep tree"})],
                file_splitting_recommendations: vec![],
                issues_count: 1,
            },
            complexity: crate::core::pipeline::pipeline_results::ComplexityAnalysisResults {
                enabled: true,
                detailed_results: complexity_entries.clone(),
                average_cyclomatic_complexity: 25.0,
                average_cognitive_complexity: 30.0,
                average_technical_debt_score: 68.5,
                average_maintainability_index: 51.5,
                issues_count: 4,
            },
            refactoring: RefactoringAnalysisResults {
                enabled: true,
                detailed_results: vec![refactoring_entry.clone()],
                opportunities_count: refactoring_entry.recommendations.len(),
            },
            impact: ImpactAnalysisResults {
                enabled: true,
                dependency_cycles: vec![json!({"module": "core", "depth": 3})],
                chokepoints: vec![],
                clone_groups: vec![],
                issues_count: 1,
            },
            lsh: LshAnalysisResults {
                enabled: false,
                clone_pairs: vec![],
                max_similarity: 0.85,
                avg_similarity: 0.6,
                duplicate_count: 1,
                apted_verification_enabled: false,
                verification: None,
                denoising_enabled: false,
                tfidf_stats: None,
            },
            coverage: CoverageAnalysisResults {
                enabled: true,
                coverage_files_used: vec![CoverageFileInfo {
                    path: "coverage.lcov".to_string(),
                    format: "lcov".to_string(),
                    size: 256,
                    modified: "2024-01-01T00:00:00Z".to_string(),
                }],
                coverage_gaps: vec![],
                gaps_count: 0,
                overall_coverage_percentage: Some(74.0),
                analysis_method: "lcov".to_string(),
            },
            health_metrics: HealthMetrics {
                overall_health_score: 58.0,
                maintainability_score: 52.0,
                technical_debt_ratio: 71.0,
                complexity_score: 83.0,
                structure_quality_score: 45.0,
            },
        }
    }

    #[test]
    fn should_include_for_dedupe_respects_patterns() {
        let pipeline = AnalysisPipeline::default();
        let mut config = ValknutConfig::default();
        config.dedupe.include = vec!["src/**".to_string()];
        config.dedupe.exclude = vec!["src/generated/**".to_string()];

        assert!(pipeline.should_include_for_dedupe(Path::new("src/lib.rs"), &config));
        assert!(!pipeline.should_include_for_dedupe(Path::new("src/generated/mod.rs"), &config));
        assert!(!pipeline.should_include_for_dedupe(Path::new("tests/integration.rs"), &config));
    }

    #[test]
    fn health_from_scores_handles_empty_and_weighted_values() {
        let empty_health = AnalysisPipeline::health_from_scores(&[]);
        assert_eq!(empty_health.overall_health_score, 100.0);
        assert_eq!(empty_health.structure_quality_score, 100.0);

        let mut category_scores = HashMap::new();
        category_scores.insert("complexity".to_string(), 1.5);
        let mut feature_contributions = HashMap::new();
        feature_contributions.insert("cyclomatic_complexity".to_string(), 1.5);

        let populated = vec![
            ScoringResult {
                entity_id: "a".to_string(),
                overall_score: 1.5,
                priority: Priority::High,
                category_scores: category_scores.clone(),
                feature_contributions: feature_contributions.clone(),
                normalized_feature_count: 3,
                confidence: 0.9,
            },
            ScoringResult {
                entity_id: "b".to_string(),
                overall_score: 0.75,
                priority: Priority::Medium,
                category_scores,
                feature_contributions,
                normalized_feature_count: 2,
                confidence: 0.8,
            },
        ];

        let derived = AnalysisPipeline::health_from_scores(&populated);
        assert!(derived.overall_health_score < 100.0);
        assert!(derived.technical_debt_ratio > 0.0);
        assert!(derived.maintainability_score <= 100.0);
    }

    #[test]
    fn converts_analysis_results_into_scoring_entries() {
        let results = build_sample_results();

        let scoring = AnalysisPipeline::convert_to_scoring_results(&results);
        assert!(scoring
            .iter()
            .any(|result| result.entity_id == "src/lib.rs:function:sample_fn"));
        assert!(scoring
            .iter()
            .any(|result| result.entity_id == "src/lib.rs:refactoring:1"));

        let complexity_entry = scoring
            .iter()
            .find(|s| s.entity_id == "src/lib.rs:function:sample_fn")
            .unwrap();
        assert!(complexity_entry.overall_score > 0.0);
        assert!(complexity_entry.category_scores.contains_key("complexity"));

        let refactoring_entry = scoring
            .iter()
            .find(|s| s.entity_id == "src/lib.rs:refactoring:1")
            .unwrap();
        assert_eq!(refactoring_entry.priority, Priority::Critical);
        assert!(refactoring_entry.overall_score >= 80.0);
    }

    #[test]
    fn creates_feature_vectors_from_analysis_results() {
        let results = build_sample_results();
        let vectors = AnalysisPipeline::create_feature_vectors_from_results(&results);

        let complexity_vector = vectors
            .iter()
            .find(|v| v.entity_id == "src/lib.rs:function:sample_fn")
            .expect("expected complexity feature vector");
        assert_eq!(
            complexity_vector
                .get_feature("technical_debt_score")
                .unwrap(),
            72.0
        );
        assert!(
            complexity_vector
                .get_normalized_feature("lines_of_code")
                .unwrap()
                <= 1.0
        );

        let refactoring_vector = vectors
            .iter()
            .find(|v| v.entity_id == "src/lib.rs:refactoring:1")
            .expect("expected refactoring feature vector");
        assert_eq!(
            refactoring_vector
                .get_feature("refactoring_recommendations")
                .unwrap(),
            1.0
        );
        assert!(
            refactoring_vector
                .get_normalized_feature("refactoring_score")
                .unwrap()
                > 0.0
        );
    }

    #[tokio::test]
    async fn analyze_vectors_scores_and_wraps_results() {
        let pipeline = AnalysisPipeline::default();
        let mut vector = FeatureVector::new("entity-1");
        vector.add_feature("cyclomatic_complexity", 4.0);
        vector.add_feature("cognitive_complexity", 3.0);
        vector.add_feature("max_nesting_depth", 2.0);
        vector.add_feature("maintainability_index", 70.0);

        let results = pipeline.analyze_vectors(vec![vector]).await.unwrap();

        assert_eq!(results.scoring_results.files.len(), 1);
        assert_eq!(results.feature_vectors.len(), 1);
        assert_eq!(results.results.summary.total_entities, 1);
        assert!(results.results.health_metrics.overall_health_score <= 100.0);
    }

    #[test]
    fn evaluate_quality_gates_reports_violations() {
        let pipeline = AnalysisPipeline::default();
        let results = build_sample_results();
        let mut config = QualityGateConfig::default();
        config.enabled = true;
        config.max_complexity_score = 60.0;
        config.max_technical_debt_ratio = 50.0;
        config.min_maintainability_score = 60.0;
        config.max_critical_issues = 1;
        config.max_high_priority_issues = 2;

        let evaluation = pipeline.evaluate_quality_gates(&config, &results);
        assert!(!evaluation.passed);
        assert!(evaluation.violations.len() >= 4);
        assert!(
            evaluation.overall_score <= results.health_metrics.overall_health_score,
            "penalties should not improve overall score"
        );
    }

    #[test]
    fn evaluate_quality_gates_handles_disabled_and_permissive_configs() {
        let pipeline = AnalysisPipeline::default();
        let results = build_sample_results();

        let disabled = QualityGateConfig::default();
        let disabled_eval = pipeline.evaluate_quality_gates(&disabled, &results);
        assert!(disabled_eval.passed);
        assert!(disabled_eval.violations.is_empty());
        assert_eq!(
            disabled_eval.overall_score,
            results.health_metrics.overall_health_score
        );

        let mut permissive = QualityGateConfig::default();
        permissive.enabled = true;
        permissive.max_complexity_score = 200.0;
        permissive.max_technical_debt_ratio = 200.0;
        permissive.min_maintainability_score = 0.0;
        permissive.max_critical_issues = usize::MAX;
        permissive.max_high_priority_issues = usize::MAX;

        let permissive_eval = pipeline.evaluate_quality_gates(&permissive, &results);
        assert!(permissive_eval.passed);
        assert!(permissive_eval.violations.is_empty());
        assert_eq!(
            permissive_eval.overall_score,
            results.health_metrics.overall_health_score
        );
    }

    #[test]
    fn new_with_config_enables_lsh_variants() {
        let mut analysis_config = AnalysisConfig::default();
        analysis_config.enable_lsh_analysis = true;

        let mut valknut_config = ValknutConfig::default();
        valknut_config.denoise.enabled = true;
        valknut_config.denoise.min_function_tokens = 4;
        valknut_config.denoise.min_match_tokens = 6;
        valknut_config.lsh.similarity_threshold = 0.4;

        let pipeline_with_denoise =
            AnalysisPipeline::new_with_config(analysis_config.clone(), valknut_config.clone());
        assert!(pipeline_with_denoise.valknut_config.is_some());

        let mut no_denoise_config = valknut_config;
        no_denoise_config.denoise.enabled = false;
        let _pipeline_without_denoise =
            AnalysisPipeline::new_with_config(analysis_config.clone(), no_denoise_config);

        let mut disabled_analysis = analysis_config;
        disabled_analysis.enable_lsh_analysis = false;
        let _pipeline_disabled =
            AnalysisPipeline::new_with_config(disabled_analysis, ValknutConfig::default());
    }

    #[tokio::test]
    async fn discover_files_respects_max_file_limit() {
        let temp = tempdir().expect("temp dir");
        let root = temp.path();
        for idx in 0..3 {
            let file_path = root.join(format!("file_{idx}.rs"));
            tokio::fs::write(&file_path, "pub fn demo() {}")
                .await
                .unwrap();
        }

        let mut config = AnalysisConfig::default();
        config.max_files = 1;
        let pipeline = AnalysisPipeline::new(config);

        let files = pipeline
            .discover_files(&[root.to_path_buf()])
            .await
            .expect("discover files");
        assert_eq!(files.len(), 1, "max_files should limit the result set");
    }

    #[tokio::test]
    async fn read_files_batched_returns_error_for_missing_file() {
        let pipeline = AnalysisPipeline::default();
        let temp = tempdir().expect("temp dir");
        let missing_path = temp.path().join("missing.rs");

        let result = pipeline.read_files_batched(&[missing_path]).await;
        assert!(
            matches!(result, Err(ValknutError::Io { .. })),
            "expected I/O error for missing files"
        );
    }

    #[test]
    fn calculate_health_metrics_handles_disabled_modules() {
        let aggregator = DefaultResultAggregator::default();
        let complexity = pipeline_results::ComplexityAnalysisResults {
            enabled: false,
            detailed_results: Vec::new(),
            average_cyclomatic_complexity: 0.0,
            average_cognitive_complexity: 0.0,
            average_technical_debt_score: 0.0,
            average_maintainability_index: 100.0,
            issues_count: 0,
        };
        let structure = StructureAnalysisResults {
            enabled: false,
            directory_recommendations: Vec::new(),
            file_splitting_recommendations: Vec::new(),
            issues_count: 0,
        };
        let impact = ImpactAnalysisResults {
            enabled: false,
            dependency_cycles: Vec::new(),
            chokepoints: Vec::new(),
            clone_groups: Vec::new(),
            issues_count: 0,
        };

        let metrics = aggregator.build_health_metrics(&complexity, &structure, &impact);
        assert_eq!(metrics.complexity_score, 0.0);
        assert_eq!(metrics.technical_debt_ratio, 0.0);
        assert_eq!(metrics.maintainability_score, 100.0);
        assert_eq!(metrics.structure_quality_score, 100.0);
        assert!(metrics.overall_health_score >= 60.0);
    }

    #[test]
    fn calculate_summary_extracts_languages_and_counts_issues() {
        let aggregator = DefaultResultAggregator::default();
        let files = vec![
            PathBuf::from("src/lib.rs"),
            PathBuf::from("scripts/main.py"),
            PathBuf::from("README.md"),
        ];

        let structure = StructureAnalysisResults {
            enabled: true,
            directory_recommendations: Vec::new(),
            file_splitting_recommendations: Vec::new(),
            issues_count: 2,
        };

        let complexity_entry =
            sample_complexity_result("src/lib.rs", 12.0, 20.0, 80.0, ComplexitySeverity::High);
        let complexity = pipeline_results::ComplexityAnalysisResults {
            enabled: true,
            detailed_results: vec![complexity_entry],
            average_cyclomatic_complexity: 12.0,
            average_cognitive_complexity: 14.0,
            average_technical_debt_score: 20.0,
            average_maintainability_index: 80.0,
            issues_count: 1,
        };

        let recommendation = RefactoringRecommendation {
            refactoring_type: RefactoringType::ExtractMethod,
            description: "Simplify logic".to_string(),
            estimated_impact: 5.0,
            estimated_effort: 2.0,
            priority_score: 1.5,
            location: (3, 10),
        };

        let refactoring = RefactoringAnalysisResults {
            enabled: true,
            detailed_results: vec![RefactoringAnalysisResult {
                file_path: "src/lib.rs".to_string(),
                recommendations: vec![recommendation],
                refactoring_score: 90.0,
            }],
            opportunities_count: 1,
        };

        let impact = ImpactAnalysisResults {
            enabled: false,
            dependency_cycles: Vec::new(),
            chokepoints: Vec::new(),
            clone_groups: Vec::new(),
            issues_count: 0,
        };

        let summary =
            aggregator.build_summary(&files, &structure, &complexity, &refactoring, &impact);

        assert_eq!(summary.files_processed, 3);
        assert!(summary.languages.contains(&"Rust".to_string()));
        assert!(summary.languages.contains(&"Python".to_string()));
        assert_eq!(summary.high_priority_issues, 1);
        assert!(summary.total_lines_of_code > 0);
    }
}
