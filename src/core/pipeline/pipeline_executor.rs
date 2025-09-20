//! Main pipeline executor that orchestrates the comprehensive analysis.

use chrono::Utc;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;

use crate::core::ast_service::AstService;
use crate::core::config::{ScoringConfig, ValknutConfig};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::FeatureVector;
use crate::core::scoring::{FeatureScorer, ScoringResult};
use crate::detectors::complexity::{ComplexityAnalyzer, ComplexityConfig, ComplexitySeverity};
use crate::detectors::refactoring::{RefactoringAnalyzer, RefactoringConfig};
use crate::detectors::structure::{StructureConfig, StructureExtractor};
use std::sync::Arc;

use super::pipeline_config::{AnalysisConfig, QualityGateConfig, QualityGateResult, QualityGateViolation};
use super::pipeline_results::{
    ComprehensiveAnalysisResult, CoverageAnalysisResults, HealthMetrics, MemoryStats, PipelineResults,
    PipelineStatistics, PipelineStatus, ScoringResults,
};
use crate::core::pipeline::AnalysisSummary;
use super::pipeline_stages::AnalysisStages;

/// Progress callback function type
pub type ProgressCallback = Box<dyn Fn(&str, f64) + Send + Sync>;

/// Main analysis pipeline that orchestrates all analyzers
pub struct AnalysisPipeline {
    config: AnalysisConfig,
    valknut_config: Option<ValknutConfig>,
    stages: AnalysisStages,
    feature_scorer: FeatureScorer,
}

impl AnalysisPipeline {
    /// Create new analysis pipeline with configuration
    pub fn new(config: AnalysisConfig) -> Self {
        let complexity_config = ComplexityConfig::default();
        let structure_config = StructureConfig::default();
        let refactoring_config = RefactoringConfig::default();
        let ast_service = Arc::new(AstService::new());

        let refactoring_analyzer =
            RefactoringAnalyzer::new(refactoring_config, ast_service.clone());

        let stages = AnalysisStages::new(
            StructureExtractor::with_config(structure_config),
            ComplexityAnalyzer::new(complexity_config, ast_service.clone()),
            refactoring_analyzer,
            ast_service,
        );

        let feature_scorer = FeatureScorer::new(ScoringConfig::default());

        Self {
            config,
            valknut_config: None,
            stages,
            feature_scorer,
        }
    }

    /// Create new analysis pipeline with full ValknutConfig support
    pub fn new_with_config(analysis_config: AnalysisConfig, valknut_config: ValknutConfig) -> Self {
        // Debug output removed - LSH integration is working

        let ast_service = Arc::new(AstService::new());

        let stages = if valknut_config.denoise.enabled && analysis_config.enable_lsh_analysis {
            use crate::detectors::lsh::config::DedupeConfig;
            use crate::detectors::lsh::LshExtractor;

            // Create LSH extractor with denoising configuration
            let mut dedupe_config = DedupeConfig::default();
            dedupe_config.min_function_tokens = valknut_config.denoise.min_function_tokens;
            dedupe_config.min_ast_nodes = valknut_config.denoise.min_match_tokens; // Mapping to closest field
            dedupe_config.shingle_k = valknut_config.lsh.shingle_size;
            dedupe_config.threshold_s = valknut_config.denoise.similarity;

            let lsh_extractor =
                LshExtractor::with_dedupe_config(dedupe_config).with_denoise_enabled(true);

            info!(
                "LSH extractor configured with denoising enabled (k={})",
                valknut_config.lsh.shingle_size
            );

            let structure_extractor = StructureExtractor::with_config(StructureConfig::default());
            let complexity_analyzer =
                ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
            let refactoring_analyzer =
                RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());

            AnalysisStages::new_with_lsh(
                structure_extractor,
                complexity_analyzer,
                refactoring_analyzer,
                lsh_extractor,
                ast_service.clone(),
            )
        } else if analysis_config.enable_lsh_analysis {
            use crate::detectors::lsh::LshExtractor;

            // Create LSH extractor without denoising
            let lsh_extractor = LshExtractor::new();
            info!("LSH extractor configured without denoising");

            let structure_extractor = StructureExtractor::with_config(StructureConfig::default());
            let complexity_analyzer =
                ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
            let refactoring_analyzer =
                RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());

            AnalysisStages::new_with_lsh(
                structure_extractor,
                complexity_analyzer,
                refactoring_analyzer,
                lsh_extractor,
                ast_service.clone(),
            )
        } else {
            // No LSH analysis
            let structure_extractor = StructureExtractor::with_config(StructureConfig::default());
            let complexity_analyzer =
                ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
            let refactoring_analyzer =
                RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());

            AnalysisStages::new(
                structure_extractor,
                complexity_analyzer,
                refactoring_analyzer,
                ast_service,
            )
        };

        let scoring_config = valknut_config.scoring.clone();
        let feature_scorer = FeatureScorer::new(scoring_config);

        Self {
            config: analysis_config,
            valknut_config: Some(valknut_config),
            stages,
            feature_scorer,
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(AnalysisConfig::default())
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
            callback("Analyzing file structure...", 10.0);
        }

        // Stage 2: Structure analysis
        let structure_results = if self.config.enable_structure_analysis {
            self.stages.run_structure_analysis(paths).await?
        } else {
            super::pipeline_results::StructureAnalysisResults {
                enabled: false,
                directory_recommendations: Vec::new(),
                file_splitting_recommendations: Vec::new(),
                issues_count: 0,
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Analyzing code complexity...", 30.0);
        }

        // Stage 3: Complexity analysis
        let complexity_results = if self.config.enable_complexity_analysis {
            self.stages.run_complexity_analysis(&files).await?
        } else {
            super::pipeline_results::ComplexityAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                average_cyclomatic_complexity: 0.0,
                average_cognitive_complexity: 0.0,
                average_technical_debt_score: 0.0,
                average_maintainability_index: 100.0,
                issues_count: 0,
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Analyzing refactoring opportunities...", 50.0);
        }

        // Stage 4: Refactoring analysis
        let refactoring_results = if self.config.enable_refactoring_analysis {
            self.stages.run_refactoring_analysis(&files).await?
        } else {
            super::pipeline_results::RefactoringAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                opportunities_count: 0,
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Analyzing dependencies and impact...", 80.0);
        }

        // Stage 5: Impact analysis
        let impact_results = if self.config.enable_impact_analysis {
            self.stages.run_impact_analysis(&files).await?
        } else {
            super::pipeline_results::ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Analyzing code clones and duplicates...", 75.0);
        }

        // Stage 6: LSH analysis for clone detection
        let lsh_results = if self.config.enable_lsh_analysis {
            let denoise_enabled = self
                .valknut_config
                .as_ref()
                .map(|config| config.denoise.enabled)
                .unwrap_or(false);
            self.stages
                .run_lsh_analysis(&files, denoise_enabled)
                .await?
        } else {
            super::pipeline_results::LshAnalysisResults {
                enabled: false,
                clone_pairs: Vec::new(),
                max_similarity: 0.0,
                avg_similarity: 0.0,
                duplicate_count: 0,
                denoising_enabled: false,
                tfidf_stats: None,
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Running coverage analysis...", 85.0);
        }

        // Stage 7: Coverage analysis with automatic file discovery
        let coverage_results = if self.config.enable_coverage_analysis {
            let coverage_config = self
                .valknut_config
                .as_ref()
                .map(|config| &config.coverage)
                .cloned()
                .unwrap_or_default();

            // Use the first analysis path as root for coverage discovery
            let default_path = PathBuf::from(".");
            let root_path = paths.first().unwrap_or(&default_path);
            self.stages
                .run_coverage_analysis(root_path, &coverage_config)
                .await?
        } else {
            CoverageAnalysisResults {
                enabled: false,
                coverage_files_used: Vec::new(),
                coverage_gaps: Vec::new(),
                gaps_count: 0,
                overall_coverage_percentage: None,
                analysis_method: "disabled".to_string(),
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Calculating health metrics...", 90.0);
        }

        // Stage 8: Calculate summary and health metrics
        let summary = self.calculate_summary(
            &files,
            &structure_results,
            &complexity_results,
            &refactoring_results,
            &impact_results,
        );
        let health_metrics =
            self.calculate_health_metrics(&complexity_results, &structure_results, &impact_results);

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

    /// Discover files to analyze
    async fn discover_files(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for path in paths {
            if path.is_file() {
                if self.should_include_file(path) {
                    files.push(path.clone());
                }
            } else if path.is_dir() {
                self.discover_files_recursive(path, &mut files).await?;
            }
        }

        // Limit files if configured
        if self.config.max_files > 0 && files.len() > self.config.max_files {
            warn!(
                "Limiting analysis to {} files (found {})",
                self.config.max_files,
                files.len()
            );
            files.truncate(self.config.max_files);
        }

        Ok(files)
    }

    /// Recursively discover files in a directory
    fn discover_files_recursive<'a>(
        &'a self,
        dir: &'a Path,
        files: &'a mut Vec<PathBuf>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = fs::read_dir(dir).await.map_err(|e| {
                ValknutError::io(
                    format!("Failed to read directory {}: {}", dir.display(), e),
                    e,
                )
            })?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| ValknutError::io("Failed to read directory entry".to_string(), e))?
            {
                let path = entry.path();

                if path.is_file() && self.should_include_file(&path) {
                    files.push(path);
                } else if path.is_dir() && self.should_include_directory(&path) {
                    self.discover_files_recursive(&path, files).await?;
                }
            }

            Ok(())
        })
    }

    /// Check if a file should be included in analysis
    fn should_include_file(&self, file: &Path) -> bool {
        if let Some(extension) = file.extension().and_then(|ext| ext.to_str()) {
            self.config.file_extensions.contains(&extension.to_string())
        } else {
            false
        }
    }

    /// Check if a directory should be included in analysis
    fn should_include_directory(&self, dir: &Path) -> bool {
        if let Some(dir_name) = dir.file_name().and_then(|name| name.to_str()) {
            !self
                .config
                .exclude_directories
                .contains(&dir_name.to_string())
        } else {
            true
        }
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

    /// Calculate analysis summary
    fn calculate_summary(
        &self,
        files: &[PathBuf],
        structure: &super::pipeline_results::StructureAnalysisResults,
        complexity: &super::pipeline_results::ComplexityAnalysisResults,
        refactoring: &super::pipeline_results::RefactoringAnalysisResults,
        impact: &super::pipeline_results::ImpactAnalysisResults,
    ) -> AnalysisSummary {
        let total_files = files.len();
        let total_entities = complexity.detailed_results.len(); // Approximate
        let total_lines_of_code = complexity
            .detailed_results
            .iter()
            .map(|r| r.metrics.lines_of_code as usize)
            .sum();

        // Extract languages from file extensions
        let mut languages = HashSet::new();
        for file in files {
            if let Some(extension) = file.extension().and_then(|ext| ext.to_str()) {
                let language = match extension {
                    "py" => "Python",
                    "js" | "jsx" => "JavaScript",
                    "ts" | "tsx" => "TypeScript",
                    "rs" => "Rust",
                    "go" => "Go",
                    "java" => "Java",
                    _ => continue,
                };
                languages.insert(language.to_string());
            }
        }

        let total_issues = structure.issues_count + complexity.issues_count + impact.issues_count;

        // Count high-priority and critical issues from complexity analysis
        let mut high_priority_issues = 0;
        let mut critical_issues = 0;

        for result in &complexity.detailed_results {
            for issue in &result.issues {
                match issue.severity.as_str() {
                    "High" => high_priority_issues += 1,
                    "VeryHigh" => high_priority_issues += 1,
                    "Critical" => critical_issues += 1,
                    _ => {}
                }
            }
        }

        let files_processed = total_files;
        let entities_analyzed = total_entities;
        let refactoring_needed = refactoring.opportunities_count;
        let high_priority = high_priority_issues;
        let critical = critical_issues;
        let avg_refactoring_score = if refactoring_needed > 0 {
            refactoring
                .detailed_results
                .iter()
                .map(|result| result.refactoring_score)
                .sum::<f64>()
                / refactoring_needed as f64
        } else {
            0.0
        };

        let code_health_score = if total_entities > 0 {
            let penalty = (total_issues as f64 / total_entities as f64).min(1.0);
            (1.0 - penalty).clamp(0.0, 1.0)
        } else {
            1.0
        };

        AnalysisSummary {
            files_processed,
            entities_analyzed,
            refactoring_needed,
            high_priority,
            critical,
            avg_refactoring_score,
            code_health_score,
            total_files,
            total_entities,
            total_lines_of_code,
            languages: languages.into_iter().collect(),
            total_issues,
            high_priority_issues,
            critical_issues,
        }
    }

    /// Calculate overall health metrics
    fn calculate_health_metrics(
        &self,
        complexity: &super::pipeline_results::ComplexityAnalysisResults,
        structure: &super::pipeline_results::StructureAnalysisResults,
        impact: &super::pipeline_results::ImpactAnalysisResults,
    ) -> HealthMetrics {
        // Complexity score (0-100, lower is better)
        let complexity_score = if complexity.enabled {
            let avg_complexity = (complexity.average_cyclomatic_complexity
                + complexity.average_cognitive_complexity)
                / 2.0;
            (avg_complexity * 4.0).min(100.0) // Scale to 0-100
        } else {
            0.0
        };

        // Technical debt ratio (average of technical debt scores)
        let technical_debt_ratio = if complexity.enabled {
            complexity.average_technical_debt_score
        } else {
            0.0
        };

        // Maintainability score (average maintainability index)
        let maintainability_score = if complexity.enabled {
            complexity.average_maintainability_index
        } else {
            100.0
        };

        // Structure quality score (based on issues found)
        let structure_quality_score = if structure.enabled {
            let issue_penalty = structure.issues_count as f64 * 5.0;
            (100.0 - issue_penalty).max(0.0)
        } else {
            100.0
        };

        // Overall health score (weighted average)
        let overall_health_score = (maintainability_score * 0.3
            + structure_quality_score * 0.3
            + (100.0 - complexity_score) * 0.2
            + (100.0 - technical_debt_ratio) * 0.2)
            .max(0.0)
            .min(100.0);

        HealthMetrics {
            overall_health_score,
            maintainability_score,
            technical_debt_ratio,
            complexity_score,
            structure_quality_score,
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
            high_priority: high_priority,
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
            feature_vectors: Vec::new(),
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
        if !config.enabled {
            return QualityGateResult {
                passed: true,
                violations: Vec::new(),
                overall_score: results.health_metrics.overall_health_score,
            };
        }

        let mut violations = Vec::new();
        let mut penalty = 0.0;

        // Helper closure to map ratio to severity labels
        let severity_from_ratio = |ratio: f64| -> String {
            if ratio >= 0.5 {
                "critical".to_string()
            } else if ratio >= 0.25 {
                "high".to_string()
            } else if ratio >= 0.1 {
                "medium".to_string()
            } else {
                "low".to_string()
            }
        };

        let pick_top_files = |paths: Vec<String>| {
            paths
                .into_iter()
                .map(PathBuf::from)
                .take(5)
                .collect::<Vec<PathBuf>>()
        };

        // Complexity score gate (lower is better)
        if results.health_metrics.complexity_score > config.max_complexity_score {
            let delta = results.health_metrics.complexity_score - config.max_complexity_score;
            let ratio = delta / config.max_complexity_score.max(1.0);
            penalty += (ratio * 15.0).min(25.0);

            let mut offenders: Vec<_> = results.complexity.detailed_results.iter().collect();
            offenders.sort_by(|a, b| {
                b.metrics
                    .cyclomatic()
                    .partial_cmp(&a.metrics.cyclomatic())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let affected_files = pick_top_files(
                offenders
                    .into_iter()
                    .map(|entry| entry.file_path.clone())
                    .collect(),
            );

            violations.push(QualityGateViolation {
                rule_name: "complexity_score".to_string(),
                description: format!(
                    "Average complexity {:.1} exceeds allowed {:.1}",
                    results.health_metrics.complexity_score,
                    config.max_complexity_score
                ),
                current_value: results.health_metrics.complexity_score,
                threshold: config.max_complexity_score,
                severity: severity_from_ratio(ratio),
                affected_files,
                recommended_actions: vec![
                    "Refactor the highest-complexity functions to smaller, cohesive units".to_string(),
                    "Introduce helper methods to reduce cyclomatic paths".to_string(),
                ],
            });
        }

        // Technical debt ratio gate (lower is better)
        if results.health_metrics.technical_debt_ratio > config.max_technical_debt_ratio {
            let delta =
                results.health_metrics.technical_debt_ratio - config.max_technical_debt_ratio;
            let ratio = delta / config.max_technical_debt_ratio.max(1.0);
            penalty += (ratio * 10.0).min(20.0);

            let mut high_debt: Vec<_> = results.complexity.detailed_results.iter().collect();
            high_debt.sort_by(|a, b| {
                b.metrics
                    .technical_debt_score
                    .partial_cmp(&a.metrics.technical_debt_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let affected_files = pick_top_files(
                high_debt
                    .into_iter()
                    .map(|entry| entry.file_path.clone())
                    .collect(),
            );

            violations.push(QualityGateViolation {
                rule_name: "technical_debt_ratio".to_string(),
                description: format!(
                    "Technical debt ratio {:.1}% exceeds allowed {:.1}%",
                    results.health_metrics.technical_debt_ratio,
                    config.max_technical_debt_ratio
                ),
                current_value: results.health_metrics.technical_debt_ratio,
                threshold: config.max_technical_debt_ratio,
                severity: severity_from_ratio(ratio),
                affected_files,
                recommended_actions: vec![
                    "Schedule debt-repayment tasks for the modules with the highest debt score".to_string(),
                    "Add regression tests before refactoring debt-heavy code".to_string(),
                ],
            });
        }

        // Maintainability gate (higher is better)
        if results.health_metrics.maintainability_score < config.min_maintainability_score {
            let delta = config.min_maintainability_score - results.health_metrics.maintainability_score;
            let ratio = delta / config.min_maintainability_score.max(1.0);
            penalty += (ratio * 12.0).min(20.0);

            let mut low_maint: Vec<_> = results.complexity.detailed_results.iter().collect();
            low_maint.sort_by(|a, b| {
                a.metrics
                    .maintainability_index
                    .partial_cmp(&b.metrics.maintainability_index)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let affected_files = pick_top_files(
                low_maint
                    .into_iter()
                    .map(|entry| entry.file_path.clone())
                    .collect(),
            );

            violations.push(QualityGateViolation {
                rule_name: "maintainability_score".to_string(),
                description: format!(
                    "Maintainability score {:.1} fell below required {:.1}",
                    results.health_metrics.maintainability_score,
                    config.min_maintainability_score
                ),
                current_value: results.health_metrics.maintainability_score,
                threshold: config.min_maintainability_score,
                severity: severity_from_ratio(ratio),
                affected_files,
                recommended_actions: vec![
                    "Document complex modules and add unit tests to stabilise behaviour".to_string(),
                    "Break large files into well-scoped components".to_string(),
                ],
            });
        }

        let critical_issues = results.summary.critical_issues.max(results.summary.critical);
        if critical_issues as usize > config.max_critical_issues {
            let delta = critical_issues as f64 - config.max_critical_issues as f64;
            let ratio = delta / config.max_critical_issues.max(1) as f64;
            penalty += (ratio * 8.0).min(15.0);

            let mut critical_files: Vec<_> = results.refactoring.detailed_results.iter().collect();
            critical_files.sort_by(|a, b| {
                b.refactoring_score
                    .partial_cmp(&a.refactoring_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let affected_files = pick_top_files(
                critical_files
                    .into_iter()
                    .map(|entry| entry.file_path.clone())
                    .collect(),
            );

            violations.push(QualityGateViolation {
                rule_name: "critical_issues".to_string(),
                description: format!(
                    "{} critical issues exceed allowed {}",
                    critical_issues, config.max_critical_issues
                ),
                current_value: critical_issues as f64,
                threshold: config.max_critical_issues as f64,
                severity: severity_from_ratio(ratio),
                affected_files,
                recommended_actions: vec![
                    "Prioritise fixes for critical refactoring recommendations".to_string(),
                    "Pull the highest scoring files into an immediate remediation sprint".to_string(),
                ],
            });
        }

        let high_priority_issues = results.summary.high_priority_issues.max(results.summary.high_priority);
        if high_priority_issues as usize > config.max_high_priority_issues {
            let delta = high_priority_issues as f64 - config.max_high_priority_issues as f64;
            let ratio = delta / config.max_high_priority_issues.max(1) as f64;
            penalty += (ratio * 5.0).min(12.0);

            let mut high_priority_files: Vec<_> = results.refactoring.detailed_results.iter().collect();
            high_priority_files.sort_by(|a, b| {
                b.refactoring_score
                    .partial_cmp(&a.refactoring_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let affected_files = pick_top_files(
                high_priority_files
                    .into_iter()
                    .map(|entry| entry.file_path.clone())
                    .collect(),
            );

            violations.push(QualityGateViolation {
                rule_name: "high_priority_issues".to_string(),
                description: format!(
                    "{} high-priority issues exceed allowed {}",
                    high_priority_issues, config.max_high_priority_issues
                ),
                current_value: high_priority_issues as f64,
                threshold: config.max_high_priority_issues as f64,
                severity: severity_from_ratio(ratio),
                affected_files,
                recommended_actions: vec![
                    "Schedule remediation tasks for the highest scoring files".to_string(),
                    "Pair with senior maintainers to reduce backlog of high-priority fixes".to_string(),
                ],
            });
        }

        let mut overall_score = results.health_metrics.overall_health_score - penalty;
        if overall_score < 0.0 {
            overall_score = 0.0;
        }

        QualityGateResult {
            passed: violations.is_empty(),
            violations,
            overall_score,
        }
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
            feature_contributions.insert("max_nesting_depth".to_string(), metrics.max_nesting_depth);
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
