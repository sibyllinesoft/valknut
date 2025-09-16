//! Main pipeline executor that orchestrates the comprehensive analysis.

use chrono::Utc;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;

use crate::core::config::ValknutConfig;
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::FeatureVector;
use crate::detectors::complexity::{ComplexityAnalyzer, ComplexityConfig, ComplexitySeverity};
use crate::detectors::refactoring::{RefactoringAnalyzer, RefactoringConfig};
use crate::detectors::structure::{StructureConfig, StructureExtractor};

use super::pipeline_config::{AnalysisConfig, QualityGateConfig, QualityGateResult};
use super::pipeline_results::{
    AnalysisSummary, ComprehensiveAnalysisResult, CoverageAnalysisResults, HealthMetrics,
    MemoryStats, PipelineResults, PipelineStatistics, PipelineStatus, ScoringResults,
};
use super::pipeline_stages::AnalysisStages;

/// Progress callback function type
pub type ProgressCallback = Box<dyn Fn(&str, f64) + Send + Sync>;

/// Main analysis pipeline that orchestrates all analyzers
pub struct AnalysisPipeline {
    config: AnalysisConfig,
    valknut_config: Option<ValknutConfig>,
    stages: AnalysisStages,
}

impl AnalysisPipeline {
    /// Create new analysis pipeline with configuration
    pub fn new(config: AnalysisConfig) -> Self {
        let complexity_config = ComplexityConfig::default();
        let structure_config = StructureConfig::default();
        let refactoring_config = RefactoringConfig::default();

        let stages = AnalysisStages::new(
            StructureExtractor::with_config(structure_config),
            ComplexityAnalyzer::new(complexity_config),
            RefactoringAnalyzer::new(refactoring_config),
        );

        Self {
            config,
            valknut_config: None,
            stages,
        }
    }

    /// Create new analysis pipeline with full ValknutConfig support
    pub fn new_with_config(analysis_config: AnalysisConfig, valknut_config: ValknutConfig) -> Self {
        // Debug output removed - LSH integration is working

        let complexity_config = ComplexityConfig::default();
        let structure_config = StructureConfig::default();
        let refactoring_config = RefactoringConfig::default();

        // Configure LSH extractor with denoising (enabled by default)
        let stages = if valknut_config.denoise.enabled && analysis_config.enable_lsh_analysis {
            use crate::core::config::DedupeConfig;
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

            AnalysisStages::new_with_lsh(
                StructureExtractor::with_config(structure_config),
                ComplexityAnalyzer::new(complexity_config),
                RefactoringAnalyzer::new(refactoring_config),
                lsh_extractor,
            )
        } else if analysis_config.enable_lsh_analysis {
            use crate::detectors::lsh::LshExtractor;

            // Create LSH extractor without denoising
            let lsh_extractor = LshExtractor::new();
            info!("LSH extractor configured without denoising");

            AnalysisStages::new_with_lsh(
                StructureExtractor::with_config(structure_config),
                ComplexityAnalyzer::new(complexity_config),
                RefactoringAnalyzer::new(refactoring_config),
                lsh_extractor,
            )
        } else {
            // No LSH analysis
            AnalysisStages::new(
                StructureExtractor::with_config(structure_config),
                ComplexityAnalyzer::new(complexity_config),
                RefactoringAnalyzer::new(refactoring_config),
            )
        };

        Self {
            config: analysis_config,
            valknut_config: Some(valknut_config),
            stages,
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

    /// Simple glob pattern matching
    fn matches_glob_pattern(&self, path: &str, pattern: &str) -> bool {
        // Simple implementation - could be enhanced with proper glob matching
        if pattern.ends_with("/**") {
            let prefix = &pattern[..pattern.len() - 3];
            path.starts_with(prefix)
        } else if pattern.contains("**/") {
            let parts: Vec<&str> = pattern.split("**/").collect();
            if parts.len() == 2 {
                path.starts_with(parts[0]) && path.contains(parts[1])
            } else {
                path.contains(&pattern.replace("**/", ""))
            }
        } else if pattern.contains('*') {
            // Simple wildcard matching
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                path.starts_with(parts[0]) && path.ends_with(parts[1])
            } else {
                // More complex patterns - use basic string matching for now
                path.contains(&pattern.replace('*', ""))
            }
        } else {
            path.contains(pattern)
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
                match issue.severity {
                    ComplexitySeverity::High => high_priority_issues += 1,
                    ComplexitySeverity::VeryHigh => high_priority_issues += 1,
                    ComplexitySeverity::Critical => critical_issues += 1,
                    _ => {}
                }
            }
        }

        AnalysisSummary {
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

        // Convert analysis results to scoring results
        let scoring_files = Self::convert_to_scoring_results(&results);

        Ok(PipelineResults {
            analysis_id: results.analysis_id.clone(),
            timestamp: results.timestamp,
            statistics: PipelineStatistics {
                memory_stats: MemoryStats {
                    current_memory_bytes: 0,
                    peak_memory_bytes: 0,
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
        })
    }

    /// Legacy API - analyze feature vectors
    pub async fn analyze_vectors(&self, _vectors: Vec<FeatureVector>) -> Result<PipelineResults> {
        // For now, create empty results
        let results = ComprehensiveAnalysisResult {
            analysis_id: "placeholder".to_string(),
            timestamp: Utc::now(),
            processing_time: 0.0,
            config: self.config.clone(),
            summary: AnalysisSummary {
                total_files: 0,
                total_entities: 0,
                total_lines_of_code: 0,
                languages: Vec::new(),
                total_issues: 0,
                high_priority_issues: 0,
                critical_issues: 0,
            },
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
                opportunities_count: 0,
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
            health_metrics: HealthMetrics {
                overall_health_score: 100.0,
                maintainability_score: 100.0,
                technical_debt_ratio: 0.0,
                complexity_score: 0.0,
                structure_quality_score: 100.0,
            },
        };

        Ok(PipelineResults {
            analysis_id: "placeholder".to_string(),
            timestamp: Utc::now(),
            results,
            statistics: PipelineStatistics {
                memory_stats: MemoryStats {
                    current_memory_bytes: 0,
                    peak_memory_bytes: 0,
                },
                files_processed: 0,
                total_duration_ms: 0,
            },
            errors: Vec::new(),
            scoring_results: ScoringResults { files: Vec::new() },
            feature_vectors: Vec::new(),
        })
    }

    /// Fit the pipeline (legacy API compatibility)
    pub async fn fit(&mut self, _vectors: &[FeatureVector]) -> Result<()> {
        // Legacy API - no-op for now
        Ok(())
    }

    /// Get extractor registry (legacy API compatibility)
    pub fn extractor_registry(&self) -> ExtractorRegistry {
        ExtractorRegistry::new()
    }

    /// Evaluate quality gates against analysis results
    pub fn evaluate_quality_gates(
        &self,
        config: &QualityGateConfig,
        results: &ComprehensiveAnalysisResult,
    ) -> QualityGateResult {
        // Placeholder implementation
        QualityGateResult {
            passed: true,
            violations: Vec::new(),
            overall_score: results.health_metrics.overall_health_score,
        }
    }

    /// Convert comprehensive analysis results to scoring results
    fn convert_to_scoring_results(
        results: &ComprehensiveAnalysisResult,
    ) -> Vec<crate::core::scoring::ScoringResult> {
        use crate::core::scoring::{Priority, ScoringResult};
        use std::collections::HashMap;

        let mut scoring_results = Vec::new();

        // Convert complexity analysis results to scoring results
        for complexity_result in &results.complexity.detailed_results {
            let entity_id = format!(
                "{}:{}:{}",
                complexity_result.file_path,
                "function", // Use generic type since entity_type field doesn't exist
                complexity_result.entity_name
            );

            // Map complexity metrics to scoring categories
            let mut category_scores = HashMap::new();
            category_scores.insert(
                "complexity".to_string(),
                (complexity_result.metrics.cyclomatic + complexity_result.metrics.cognitive) / 2.0,
            );

            if complexity_result.metrics.max_nesting_depth > 0.0 {
                category_scores.insert(
                    "structure".to_string(),
                    complexity_result.metrics.max_nesting_depth,
                );
            }

            // Map individual features to contributions
            let mut feature_contributions = HashMap::new();
            feature_contributions.insert(
                "cyclomatic_complexity".to_string(),
                complexity_result.metrics.cyclomatic,
            );
            feature_contributions.insert(
                "cognitive_complexity".to_string(),
                complexity_result.metrics.cognitive,
            );
            feature_contributions.insert(
                "nesting_depth".to_string(),
                complexity_result.metrics.max_nesting_depth,
            );
            feature_contributions.insert(
                "lines_of_code".to_string(),
                complexity_result.metrics.lines_of_code,
            );
            feature_contributions.insert(
                "technical_debt_score".to_string(),
                complexity_result.metrics.technical_debt_score,
            );
            feature_contributions.insert(
                "maintainability_index".to_string(),
                complexity_result.metrics.maintainability_index,
            );

            // Calculate overall score based on complexity
            let complexity_avg =
                (complexity_result.metrics.cyclomatic + complexity_result.metrics.cognitive) / 2.0;
            let overall_score =
                complexity_avg + (complexity_result.metrics.max_nesting_depth * 0.5);

            // Determine priority based on overall score and issues
            let priority = if !complexity_result.issues.is_empty() {
                use crate::detectors::complexity::ComplexitySeverity;
                // Use the severity of the complexity result itself since we can't easily find max
                match complexity_result.severity {
                    ComplexitySeverity::Critical => Priority::Critical,
                    ComplexitySeverity::VeryHigh => Priority::High,
                    ComplexitySeverity::High => Priority::High,
                    ComplexitySeverity::Moderate => Priority::Medium,
                    ComplexitySeverity::Low => Priority::Low,
                }
            } else if overall_score >= 20.0 {
                Priority::Critical
            } else if overall_score >= 15.0 {
                Priority::High
            } else if overall_score >= 10.0 {
                Priority::Medium
            } else if overall_score >= 5.0 {
                Priority::Low
            } else {
                Priority::None
            };

            // Calculate confidence based on data quality
            let confidence = if complexity_result.metrics.lines_of_code > 10.0 {
                0.9
            } else if complexity_result.metrics.lines_of_code > 5.0 {
                0.7
            } else {
                0.5
            };

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
            let overall_score = refactoring_score;

            // Determine priority based on refactoring score
            let priority = if refactoring_score >= 80.0 {
                Priority::Critical
            } else if refactoring_score >= 60.0 {
                Priority::High
            } else if refactoring_score >= 40.0 {
                Priority::Medium
            } else if refactoring_score >= 20.0 {
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
