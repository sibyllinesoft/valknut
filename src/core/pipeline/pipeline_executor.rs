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
use crate::detectors::complexity::{ComplexityAnalyzer, ComplexityConfig, ComplexitySeverity};
use crate::detectors::coverage::{CoverageConfig as CoverageDetectorConfig, CoverageExtractor};
use crate::detectors::refactoring::{RefactoringAnalyzer, RefactoringConfig};
use crate::detectors::structure::{StructureConfig, StructureExtractor};
use crate::doc_audit::{run_audit, DocAuditConfig};
use std::collections::HashMap;
use std::sync::Arc;

use super::pipeline_config::{AnalysisConfig, QualityGateConfig, QualityGateResult};
use super::pipeline_results::{
    ComprehensiveAnalysisResult, CoverageAnalysisResults, DocumentationAnalysisResults,
    HealthMetrics, MemoryStats, PipelineResults, PipelineStatistics, PipelineStatus,
    ScoringResults,
};
use crate::detectors::cohesion::CohesionAnalysisResults;
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

        let stage_runner: Arc<dyn StageOrchestrator> = if analysis_config.enable_lsh_analysis {
            use crate::detectors::lsh::config::DedupeConfig;
            use crate::detectors::lsh::LshExtractor;

            // Build a single LSH extractor; always honor dedupe thresholds, toggle denoise flag
            let mut dedupe_config = DedupeConfig::default();
            dedupe_config.min_function_tokens = valknut_config.denoise.min_function_tokens;
            dedupe_config.min_ast_nodes = valknut_config.dedupe.min_ast_nodes; // honor dedupe default (20) unless overridden
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
        if let Some((
            doc_score,
            doc_issue_count,
            file_issues,
            dir_scores,
            dir_issue_counts,
            file_health,
        )) = Self::compute_doc_health(
            paths,
            &files,
            self.valknut_config
                .as_ref()
                .map(|c| &c.docs)
                .unwrap_or(&crate::core::config::DocHealthConfig::default()),
        ) {
            health_metrics.doc_health_score = doc_score;
            health_metrics.overall_health_score = (health_metrics.maintainability_score * 0.28
                + health_metrics.structure_quality_score * 0.25
                + (100.0 - health_metrics.complexity_score) * 0.18
                + (100.0 - health_metrics.technical_debt_ratio) * 0.19
                + health_metrics.doc_health_score * 0.10)
                .clamp(0.0, 100.0);

            summary.doc_health_score = (doc_score / 100.0).clamp(0.0, 1.0);
            summary.apply_doc_issues(doc_issue_count);

            documentation_results.enabled = true;
            documentation_results.issues_count = doc_issue_count;
            documentation_results.doc_health_score = doc_score;
            documentation_results.file_doc_issues = file_issues;
            documentation_results.file_doc_health = file_health;
            documentation_results.directory_doc_health = dir_scores;
            documentation_results.directory_doc_issues = dir_issue_counts;
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

    /// Compute documentation health using doc_audit with directory-aware aggregation and eligibility thresholds.
    /// Returns (score 0-100, doc_issue_count, per-file issues, per-directory scores, per-directory issue counts, per-file health scores).
    fn compute_doc_health(
        paths: &[PathBuf],
        analyzed_files: &[PathBuf],
        cfg: &DocHealthConfig,
    ) -> Option<(
        f64,
        usize,
        HashMap<String, usize>,
        HashMap<String, f64>,
        HashMap<String, usize>,
        HashMap<String, f64>,
    )> {
        let root = paths.iter().find(|p| p.is_dir())?.clone();
        let audit_cfg = DocAuditConfig::new(root);
        let result = run_audit(&audit_cfg).ok()?;

        // Per-file issue counts
        let mut file_gaps: HashMap<PathBuf, usize> = HashMap::new();
        for issue in result.documentation_issues.iter() {
            *file_gaps.entry(issue.path.clone()).or_insert(0) += 1;
        }

        let mut file_issue_out: HashMap<String, usize> = HashMap::new();

        // Aggregate per-file eligibility and scores
        let mut eligible_files = 0usize;
        let mut files_with_gaps = 0usize;

        // Directory aggregation buckets
        let mut dir_eligible: HashMap<PathBuf, usize> = HashMap::new();
        let mut dir_gaps: HashMap<PathBuf, usize> = HashMap::new();

        for (path, gaps) in file_gaps.iter() {
            // Paths from doc_audit are relative to audit root, so join them
            let full_path = if path.is_absolute() {
                path.clone()
            } else {
                audit_cfg.root.join(path)
            };
            let loc = std::fs::read_to_string(&full_path)
                .map(|c| c.lines().count())
                .unwrap_or(0);
            if loc < cfg.min_file_nodes {
                continue;
            }
            eligible_files += 1;
            if *gaps > 0 {
                files_with_gaps += 1;
            }

            file_issue_out.insert(path.display().to_string(), *gaps);

            let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            *dir_eligible.entry(dir.clone()).or_insert(0) += 1;
            if *gaps > 0 {
                *dir_gaps.entry(dir.clone()).or_insert(0) += 1;
            }
        }

        // README gaps counted as project-level penalties
        let readme_gap_files = result.missing_readmes.len() + result.stale_readmes.len();
        eligible_files += readme_gap_files;
        files_with_gaps += readme_gap_files;
        let total_doc_issues = files_with_gaps;

        // Directory-level doc health (only if enough files)
        let mut dir_scores = Vec::new();
        let mut dir_score_map: HashMap<String, f64> = HashMap::new();
        let mut dir_issue_map: HashMap<String, usize> = HashMap::new();
        let mut file_health_map: HashMap<String, f64> = HashMap::new();
        for (dir, eligible) in dir_eligible.iter() {
            if *eligible < cfg.min_files_per_dir {
                dir_scores.push(100.0);
                dir_score_map.insert(dir.display().to_string(), 100.0);
                dir_issue_map.insert(dir.display().to_string(), 0);
                continue;
            }
            let gaps = *dir_gaps.get(dir).unwrap_or(&0);
            let coverage = 1.0 - (gaps as f64 / *eligible as f64);
            let score = (coverage * 100.0).clamp(0.0, 100.0);
            dir_scores.push(score);
            dir_score_map.insert(dir.display().to_string(), score);
            dir_issue_map.insert(dir.display().to_string(), gaps);
        }

        // Helper to insert path variants into file_health_map
        let insert_path_variants =
            |file_health_map: &mut HashMap<String, f64>, path: &Path, score: f64| {
                let abs = if path.is_absolute() {
                    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
                } else {
                    let joined = audit_cfg.root.join(path);
                    joined.canonicalize().unwrap_or(joined)
                };
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let rel_cwd = abs
                    .strip_prefix(&cwd)
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|_| abs.clone());
                let rel_root = abs
                    .strip_prefix(&audit_cfg.root)
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|_| rel_cwd.clone());

                let mut keys: Vec<PathBuf> = Vec::new();
                keys.push(abs.clone());
                keys.push(rel_cwd.clone());
                keys.push(rel_root.clone());
                if abs.starts_with(&audit_cfg.root) {
                    let rel_to_root = abs.strip_prefix(&audit_cfg.root).unwrap_or(&abs);
                    keys.push(rel_to_root.to_path_buf());
                }
                if rel_root.starts_with("src") {
                    if let Ok(stripped) = rel_root.strip_prefix("src") {
                        keys.push(stripped.to_path_buf());
                        keys.push(PathBuf::from("src").join(stripped));
                    }
                }
                keys.push(PathBuf::from("src").join(rel_root.clone()));
                if let Some(file_name) = abs.file_name() {
                    keys.push(PathBuf::from(file_name));
                }

                for k in keys {
                    let kstr = k.to_string_lossy().replace('\\', "/");
                    file_health_map.insert(kstr.clone(), score);
                    if !kstr.starts_with("./") {
                        file_health_map.insert(format!("./{}", kstr), score);
                    }
                }
            };

        // File-level doc health: files with issues get scaled score, files without issues get 100
        // Use logarithmic scaling so files with many issues don't all collapse to 0
        // Formula: health = 100 * (1 - log10(gaps + 1) / log10(max_gaps + 1))
        // This gives a gentler curve: 1 issue ~= 85, 10 issues ~= 50, 100 issues ~= 15
        let max_gaps = file_gaps.values().copied().max().unwrap_or(1).max(1) as f64;
        let log_max = (max_gaps + 1.0).log10();

        for (path, gaps) in file_gaps.iter() {
            let score = if *gaps == 0 {
                100.0
            } else {
                let log_gaps = (*gaps as f64 + 1.0).log10();
                let scaled = 1.0 - (log_gaps / log_max);
                (scaled * 100.0).clamp(0.0, 100.0)
            };
            insert_path_variants(&mut file_health_map, path, score);
        }

        // Then add all analyzed source files that don't have issues (score = 100.0)
        // We need to normalize paths before comparing since file_gaps paths may differ
        // file_gaps paths are relative to audit_cfg.root, so we need to join them
        let file_gaps_canonical: std::collections::HashSet<PathBuf> = file_gaps
            .keys()
            .filter_map(|p| {
                let full_path = if p.is_absolute() {
                    p.clone()
                } else {
                    audit_cfg.root.join(p)
                };
                full_path.canonicalize().ok()
            })
            .collect();
        for path in analyzed_files.iter() {
            if path.is_file() {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if !file_gaps_canonical.contains(&canonical) {
                    insert_path_variants(&mut file_health_map, path, 100.0);
                }
            }
        }

        // Project score preference: directory weighted average if any eligible dirs; else file coverage; else 100.
        if !dir_scores.is_empty() {
            let avg = dir_scores.iter().sum::<f64>() / dir_scores.len() as f64;
            return Some((
                avg.clamp(0.0, 100.0),
                total_doc_issues,
                file_issue_out,
                dir_score_map,
                dir_issue_map,
                file_health_map,
            ));
        }

        if eligible_files == 0 {
            return Some((
                100.0,
                0,
                file_issue_out,
                dir_score_map,
                dir_issue_map,
                file_health_map,
            ));
        }

        let coverage = 1.0 - (files_with_gaps as f64 / eligible_files as f64);
        Some((
            (coverage * 100.0).clamp(0.0, 100.0),
            total_doc_issues,
            file_issue_out,
            dir_score_map,
            dir_issue_map,
            file_health_map,
        ))
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
            doc_health_score: 1.0,
            doc_issue_count: 0,
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
                doc_health_score: 100.0,
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
        let doc_health_score = 100.0; // placeholder until doc analysis contributes

        HealthMetrics {
            overall_health_score: overall_health,
            maintainability_score: maintainability,
            technical_debt_ratio: technical_debt,
            complexity_score: complexity,
            structure_quality_score: structure_quality,
            doc_health_score,
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

        // Helper: logistic mapping that trends to 1.0 as value grows past mid
        fn logistic_over(value: f64, mid: f64, steepness: f64) -> f64 {
            let k = if steepness <= 0.0 { 1.0 } else { steepness };
            let exponent = -((value - mid) / k);
            let denom = 1.0 + exponent.exp();
            (1.0 / denom).clamp(0.0, 1.0)
        }

        // Collect per-file aggregates for structure metrics
        use std::collections::HashSet;
        let mut files: HashSet<String> = HashSet::new();
        let mut func_counts: HashMap<String, usize> = HashMap::new();
        let mut class_counts: HashMap<String, usize> = HashMap::new();

        for c in &results.complexity.detailed_results {
            files.insert(c.file_path.clone());
            match c.entity_type.as_str() {
                "function" => *func_counts.entry(c.file_path.clone()).or_insert(0) += 1,
                "class" => *class_counts.entry(c.file_path.clone()).or_insert(0) += 1,
                _ => {}
            }
        }
        for r in &results.refactoring.detailed_results {
            files.insert(r.file_path.clone());
        }

        // LOC per file from disk (best-effort)
        let mut file_loc: HashMap<String, f64> = HashMap::new();
        for file in &files {
            let loc = std::fs::read_to_string(file)
                .map(|c| c.lines().count() as f64)
                .unwrap_or(0.0);
            file_loc.insert(file.clone(), loc);
        }

        // Files per directory
        let mut files_per_dir: HashMap<String, usize> = HashMap::new();
        for file in &files {
            let dir = std::path::Path::new(file)
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| ".".to_string());
            *files_per_dir.entry(dir).or_insert(0) += 1;
        }

        // Create per-file feature vectors with normalized structure metrics
        for file in &files {
            let loc = *file_loc.get(file).unwrap_or(&0.0);
            let funcs = *func_counts.get(file).unwrap_or(&0) as f64;
            let classes = *class_counts.get(file).unwrap_or(&0) as f64;
            let dir = std::path::Path::new(file)
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| ".".to_string());
            let dir_files = *files_per_dir.get(&dir).unwrap_or(&1) as f64;

            let mut feature_vector = FeatureVector::new(format!("{}:file", file));
            feature_vector.add_feature("lines_of_code", loc);
            feature_vector.add_feature("functions_per_file", funcs);
            feature_vector.add_feature("classes_per_file", classes);
            feature_vector.add_feature("files_per_directory", dir_files);

            // Normalized severities (higher is worse)
            feature_vector
                .normalized_features
                .insert("lines_of_code".to_string(), logistic_over(loc, 300.0, 75.0));
            feature_vector.normalized_features.insert(
                "functions_per_file".to_string(),
                logistic_over(funcs, 12.0, 4.0),
            );
            feature_vector.normalized_features.insert(
                "classes_per_file".to_string(),
                logistic_over(classes, 2.0, 1.0),
            );
            feature_vector.normalized_features.insert(
                "files_per_directory".to_string(),
                logistic_over(dir_files, 7.0, 2.0),
            );

            feature_vector
                .add_metadata("entity_type", serde_json::Value::String("file".to_string()));
            feature_vector.add_metadata("file_path", serde_json::Value::String(file.clone()));

            feature_vectors.push(feature_vector);
        }

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
#[path = "pipeline_executor_tests.rs"]
mod tests;
