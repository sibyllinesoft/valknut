//! Pipeline services for orchestrating code analysis stages.
//!
//! This module defines the service traits and default implementations used
//! by the analysis pipeline to discover files, read contents, run analysis
//! stages, and aggregate results.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future;

use crate::core::pipeline::results::result_types::AnalysisSummary;
use crate::core::arena_analysis::ArenaAnalysisResult;
use crate::core::config::ValknutConfig;
use crate::core::errors::{Result, ValknutError};
use crate::core::pipeline::results::pipeline_results::{
    ComplexityAnalysisResults, ComprehensiveAnalysisResult, CoverageAnalysisResults, HealthMetrics,
    ImpactAnalysisResults, LshAnalysisResults, RefactoringAnalysisResults,
    StructureAnalysisResults,
};
use crate::detectors::bundled::{BundledDetectionConfig, BundledFileDetector};
use crate::detectors::cohesion::CohesionAnalysisResults;
use crate::core::pipeline::{QualityGateResult, QualityGateViolation};
use serde::{Deserialize, Serialize};

use super::file_discovery;
use crate::core::pipeline::pipeline_config::{AnalysisConfig, QualityGateConfig};

/// Service responsible for translating requested roots into concrete files.
///
/// Implementations traverse directory trees, respect include/exclude patterns,
/// and may integrate with version control systems to filter files.
pub trait FileDiscoverer: Send + Sync {
    /// Discovers analyzable files from the given root paths.
    ///
    /// # Arguments
    /// * `roots` - Directory or file paths to search
    /// * `pipeline_config` - Analysis configuration with include/exclude patterns
    /// * `valknut_config` - Optional global configuration for additional filtering
    ///
    /// # Returns
    /// A list of file paths that should be analyzed.
    fn discover(
        &self,
        roots: &[PathBuf],
        pipeline_config: &AnalysisConfig,
        valknut_config: Option<&ValknutConfig>,
    ) -> Result<Vec<PathBuf>>;
}

/// Default git-aware file discovery implementation.
///
/// Uses git's tracked file index when available for fast file enumeration,
/// falling back to filesystem traversal otherwise. Respects `.gitignore`
/// patterns and configured include/exclude filters.
#[derive(Default, Debug)]
pub struct GitAwareFileDiscoverer;

/// [`FileDiscoverer`] implementation for [`GitAwareFileDiscoverer`].
impl FileDiscoverer for GitAwareFileDiscoverer {
    /// Discovers files using git-aware traversal.
    fn discover(
        &self,
        roots: &[PathBuf],
        pipeline_config: &AnalysisConfig,
        valknut_config: Option<&ValknutConfig>,
    ) -> Result<Vec<PathBuf>> {
        file_discovery::discover_files(roots, pipeline_config, valknut_config)
    }
}

/// Factory method for [`GitAwareFileDiscoverer`].
impl GitAwareFileDiscoverer {
    /// Returns a shared reference to the default file discoverer.
    pub fn shared() -> Arc<dyn FileDiscoverer> {
        Arc::new(Self::default())
    }
}

/// Service responsible for reading file contents in a controlled, batched manner.
///
/// Batching prevents overwhelming the filesystem with concurrent reads and
/// allows for memory-efficient processing of large file sets.
#[async_trait]
pub trait FileBatchReader: Send + Sync {
    /// Reads the contents of all specified files.
    ///
    /// Files are read asynchronously in batches to balance throughput and resource usage.
    ///
    /// # Returns
    /// A vector of (path, content) tuples for successfully read files.
    async fn read_files(&self, files: &[PathBuf]) -> Result<Vec<(PathBuf, String)>>;
}

/// Default implementation that processes files in fixed batches using Tokio async I/O.
///
/// Files within each batch are read concurrently, while batches are processed
/// sequentially to limit memory usage. Optionally filters out bundled JS/TS files.
#[derive(Debug, Default)]
pub struct BatchedFileReader {
    batch_size: usize,
    bundled_detector: Option<BundledFileDetector>,
}

/// Constructor and utility methods for [`BatchedFileReader`].
impl BatchedFileReader {
    /// Creates a new batched file reader with the specified batch size.
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size,
            bundled_detector: None,
        }
    }

    /// Enables bundled file detection with the given configuration.
    ///
    /// When enabled, JavaScript/TypeScript files that appear to be bundler
    /// output (webpack, rollup, esbuild, parcel) will be skipped.
    pub fn with_bundled_detection(mut self, config: BundledDetectionConfig) -> Self {
        if config.enabled {
            self.bundled_detector = Some(BundledFileDetector::new(config));
        }
        self
    }

    /// Returns the effective batch size, ensuring at least 1.
    fn effective_batch_size(&self) -> usize {
        self.batch_size.max(1)
    }
}

/// [`FileBatchReader`] implementation for [`BatchedFileReader`].
#[async_trait]
impl FileBatchReader for BatchedFileReader {
    /// Reads files in batches using async I/O.
    ///
    /// If bundled detection is enabled, JavaScript/TypeScript files that
    /// appear to be bundler output will be skipped.
    async fn read_files(&self, files: &[PathBuf]) -> Result<Vec<(PathBuf, String)>> {
        let mut file_contents = Vec::with_capacity(files.len());
        for batch in files.chunks(self.effective_batch_size()) {
            let batch_results = self.read_batch(batch).await;
            self.collect_batch_results(batch_results, &mut file_contents).await?;
        }
        Ok(file_contents)
    }
}

/// Private helper methods for [`BatchedFileReader`].
impl BatchedFileReader {
    async fn read_batch(&self, batch: &[PathBuf]) -> Vec<impl std::future::Future<Output = Result<(PathBuf, String)>>> {
        batch.iter().map(|file_path| {
            let path = file_path.clone();
            async move { Self::read_single_file(path).await }
        }).collect()
    }

    async fn read_single_file(path: PathBuf) -> Result<(PathBuf, String)> {
        let bytes = tokio::fs::read(&path).await.map_err(|e| {
            ValknutError::io(format!("Failed to read file {}", path.display()), e)
        })?;
        let content = Self::bytes_to_string(&path, bytes);
        Ok((path, content))
    }

    fn bytes_to_string(path: &PathBuf, bytes: Vec<u8>) -> String {
        match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("File {} contains invalid UTF-8, using lossy conversion", path.display());
                String::from_utf8_lossy(e.as_bytes()).into_owned()
            }
        }
    }

    async fn collect_batch_results<F>(
        &self,
        batch_results: Vec<F>,
        file_contents: &mut Vec<(PathBuf, String)>,
    ) -> Result<()>
    where
        F: std::future::Future<Output = Result<(PathBuf, String)>>,
    {
        for result in future::join_all(batch_results).await {
            let (path, content) = result?;
            if !self.should_skip_bundled(&path, &content) {
                file_contents.push((path, content));
            }
        }
        Ok(())
    }

    fn should_skip_bundled(&self, path: &PathBuf, content: &str) -> bool {
        if let Some(ref detector) = self.bundled_detector {
            if detector.should_check(path) && detector.is_bundled(content) {
                tracing::debug!("Skipping bundled file: {}", path.display());
                return true;
            }
        }
        false
    }
}

/// Factory method for [`BatchedFileReader`].
impl BatchedFileReader {
    /// Returns a shared reference to a default batched reader (batch size: 200).
    pub fn default_shared() -> Arc<dyn FileBatchReader> {
        Arc::new(Self::new(200))
    }

    /// Returns a shared reference with bundled file detection enabled.
    pub fn shared_with_bundled_detection(config: BundledDetectionConfig) -> Arc<dyn FileBatchReader> {
        Arc::new(Self::new(200).with_bundled_detection(config))
    }
}

/// Aggregated results from all enabled analysis stages.
///
/// Collects the output from each analysis pass (structure, coverage, complexity, etc.)
/// into a single bundle for downstream processing and report generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResultsBundle {
    /// Results from directory and file structure analysis.
    pub structure: StructureAnalysisResults,
    /// Results from test coverage gap analysis.
    pub coverage: CoverageAnalysisResults,
    /// Results from cyclomatic and cognitive complexity analysis.
    pub complexity: ComplexityAnalysisResults,
    /// Results from refactoring opportunity detection.
    pub refactoring: RefactoringAnalysisResults,
    /// Results from dependency impact analysis.
    pub impact: ImpactAnalysisResults,
    /// Results from locality-sensitive hashing clone detection.
    pub lsh: LshAnalysisResults,
    /// Results from semantic cohesion analysis.
    #[serde(default)]
    pub cohesion: CohesionAnalysisResults,
}

/// Factory methods for [`StageResultsBundle`].
impl StageResultsBundle {
    /// Creates a bundle with all stages marked as disabled.
    ///
    /// Used when analysis is skipped or for placeholder initialization.
    pub fn disabled() -> Self {
        StageResultsBundle {
            structure: StructureAnalysisResults {
                enabled: false,
                directory_recommendations: Vec::new(),
                file_splitting_recommendations: Vec::new(),
                issues_count: 0,
            },
            coverage: CoverageAnalysisResults {
                enabled: false,
                coverage_files_used: Vec::new(),
                coverage_gaps: Vec::new(),
                gaps_count: 0,
                overall_coverage_percentage: None,
                analysis_method: "disabled".to_string(),
            },
            complexity: ComplexityAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                average_cyclomatic_complexity: 0.0,
                average_cognitive_complexity: 0.0,
                average_technical_debt_score: 0.0,
                average_maintainability_index: 100.0,
                issues_count: 0,
            },
            refactoring: RefactoringAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                opportunities_count: 0,
            },
            impact: ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            },
            lsh: LshAnalysisResults {
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
            cohesion: CohesionAnalysisResults::default(),
        }
    }
}

/// Default implementation for [`StageResultsBundle`].
impl Default for StageResultsBundle {
    /// Returns a disabled results bundle.
    fn default() -> Self {
        Self::disabled()
    }
}

/// Orchestrates the execution of analysis stages.
///
/// Coordinates arena-based AST analysis and runs all enabled analysis
/// stages (structure, coverage, complexity, etc.) to produce a complete
/// results bundle.
#[async_trait(?Send)]
pub trait StageOrchestrator: Send + Sync {
    /// Runs arena-based AST analysis on pre-read file contents.
    ///
    /// This is the first analysis phase that extracts entities and builds
    /// parse indices for downstream stages.
    async fn run_arena_analysis_with_content(
        &self,
        file_contents: &[(PathBuf, String)],
    ) -> Result<Vec<ArenaAnalysisResult>>;

    /// Runs all enabled analysis stages and returns aggregated results.
    ///
    /// # Arguments
    /// * `config` - Analysis configuration controlling which stages run
    /// * `paths` - Original root paths requested for analysis
    /// * `files` - Discovered files to analyze
    /// * `arena_results` - Pre-computed arena analysis results
    async fn run_all_stages(
        &self,
        config: &AnalysisConfig,
        paths: &[PathBuf],
        files: &[PathBuf],
        arena_results: &[ArenaAnalysisResult],
    ) -> Result<StageResultsBundle>;
}

/// Aggregates stage results into summary metrics and evaluates quality gates.
///
/// Provides methods for computing health scores, building analysis summaries,
/// and checking results against configurable quality thresholds.
pub trait ResultAggregator: Send + Sync {
    /// Builds an analysis summary from stage results.
    ///
    /// Computes aggregate statistics like total files, entities, issues,
    /// and language distribution.
    fn build_summary(
        &self,
        files: &[PathBuf],
        structure: &StructureAnalysisResults,
        complexity: &ComplexityAnalysisResults,
        refactoring: &RefactoringAnalysisResults,
        impact: &ImpactAnalysisResults,
    ) -> AnalysisSummary;

    /// Computes health metrics from stage results.
    ///
    /// Calculates overall health score, maintainability, complexity,
    /// and structure quality scores.
    fn build_health_metrics(
        &self,
        complexity: &ComplexityAnalysisResults,
        structure: &StructureAnalysisResults,
        impact: &ImpactAnalysisResults,
    ) -> HealthMetrics;

    /// Evaluates quality gates against analysis results.
    ///
    /// Checks configured thresholds for maintainability, complexity,
    /// technical debt, and issue counts. Returns pass/fail status
    /// with details on any violations.
    fn evaluate_quality_gates(
        &self,
        config: &QualityGateConfig,
        results: &ComprehensiveAnalysisResult,
    ) -> QualityGateResult;
}

/// Default implementation of result aggregation.
///
/// Provides standard algorithms for computing health scores and
/// evaluating quality gates based on configurable thresholds.
#[derive(Default, Debug)]
pub struct DefaultResultAggregator;

/// [`ResultAggregator`] implementation for [`DefaultResultAggregator`].
impl ResultAggregator for DefaultResultAggregator {
    /// Builds an analysis summary from stage results.
    fn build_summary(
        &self,
        files: &[PathBuf],
        structure: &StructureAnalysisResults,
        complexity: &ComplexityAnalysisResults,
        refactoring: &RefactoringAnalysisResults,
        impact: &ImpactAnalysisResults,
    ) -> AnalysisSummary {
        let total_files = files.len();
        let total_entities = complexity.detailed_results.len();
        let total_lines_of_code = Self::count_lines_of_code(complexity);
        let languages = Self::detect_languages(files);
        let total_issues = structure.issues_count + complexity.issues_count + impact.issues_count;
        let (high_priority_issues, critical_issues) = Self::count_priority_issues(complexity);
        let avg_refactoring_score = Self::calculate_avg_refactoring_score(refactoring);
        let code_health_score = Self::calculate_code_health_score(total_entities, total_issues);

        AnalysisSummary {
            files_processed: total_files,
            entities_analyzed: total_entities,
            refactoring_needed: refactoring.opportunities_count,
            high_priority: high_priority_issues,
            critical: critical_issues,
            avg_refactoring_score,
            code_health_score,
            total_files,
            total_entities,
            total_lines_of_code,
            languages,
            total_issues,
            high_priority_issues,
            critical_issues,
            doc_health_score: 1.0,
            doc_issue_count: 0,
        }
    }

    /// Computes overall health metrics from analysis results.
    fn build_health_metrics(
        &self,
        complexity: &ComplexityAnalysisResults,
        structure: &StructureAnalysisResults,
        impact: &ImpactAnalysisResults,
    ) -> HealthMetrics {
        let complexity_score = if complexity.enabled {
            let avg_complexity = (complexity.average_cyclomatic_complexity
                + complexity.average_cognitive_complexity)
                / 2.0;
            (avg_complexity * 4.0).min(100.0)
        } else {
            0.0
        };

        let technical_debt_ratio = if complexity.enabled {
            complexity.average_technical_debt_score
        } else {
            0.0
        };

        let maintainability_score = if complexity.enabled {
            complexity.average_maintainability_index
        } else {
            100.0
        };

        let structure_quality_score = if structure.enabled {
            let issue_penalty = structure.issues_count as f64 * 5.0;
            (100.0 - issue_penalty).max(0.0)
        } else {
            100.0
        };

        // Documentation health currently treated as neutral unless populated by future doc-analysis stage.
        let doc_health_score = 100.0;

        let overall_health_score = (maintainability_score * 0.28
            + structure_quality_score * 0.25
            + (100.0 - complexity_score) * 0.18
            + (100.0 - technical_debt_ratio) * 0.19
            + doc_health_score * 0.10)
            .clamp(0.0, 100.0);

        HealthMetrics {
            overall_health_score,
            maintainability_score,
            technical_debt_ratio,
            complexity_score,
            structure_quality_score,
            doc_health_score,
        }
    }

    /// Evaluates quality gates against analysis results.
    fn evaluate_quality_gates(
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

        if results.health_metrics.overall_health_score < config.min_maintainability_score {
            violations.push(QualityGateViolation {
                rule_name: "Minimum maintainability score".to_string(),
                description: format!(
                    "Maintainability {:.1} is below minimum {:.1}",
                    results.health_metrics.overall_health_score, config.min_maintainability_score
                ),
                current_value: results.health_metrics.overall_health_score,
                threshold: config.min_maintainability_score,
                severity: "high".to_string(),
                affected_files: Vec::new(),
                recommended_actions: vec![
                    "Address high-impact structure or complexity findings first".to_string(),
                ],
            });
        }

        if results.health_metrics.complexity_score > config.max_complexity_score {
            violations.push(QualityGateViolation {
                rule_name: "Maximum complexity score".to_string(),
                description: format!(
                    "Complexity {:.1} exceeds maximum {:.1}",
                    results.health_metrics.complexity_score, config.max_complexity_score
                ),
                current_value: results.health_metrics.complexity_score,
                threshold: config.max_complexity_score,
                severity: "medium".to_string(),
                affected_files: Vec::new(),
                recommended_actions: vec!["Refactor high-complexity entities".to_string()],
            });
        }

        if results.health_metrics.technical_debt_ratio > config.max_technical_debt_ratio {
            violations.push(QualityGateViolation {
                rule_name: "Technical debt ratio".to_string(),
                description: format!(
                    "Debt ratio {:.1}% exceeds {:.1}%",
                    results.health_metrics.technical_debt_ratio, config.max_technical_debt_ratio
                ),
                current_value: results.health_metrics.technical_debt_ratio,
                threshold: config.max_technical_debt_ratio,
                severity: "medium".to_string(),
                affected_files: Vec::new(),
                recommended_actions: vec![
                    "Prioritize high-impact issues surfaced in reports".to_string()
                ],
            });
        }

        if results.summary.critical_issues > config.max_critical_issues {
            violations.push(QualityGateViolation {
                rule_name: "Critical issues".to_string(),
                description: format!(
                    "Critical issues {} exceed maximum {}",
                    results.summary.critical_issues, config.max_critical_issues
                ),
                current_value: results.summary.critical_issues as f64,
                threshold: config.max_critical_issues as f64,
                severity: "blocker".to_string(),
                affected_files: Vec::new(),
                recommended_actions: vec!["Resolve critical impact issues".to_string()],
            });
        }

        if results.summary.high_priority_issues > config.max_high_priority_issues {
            violations.push(QualityGateViolation {
                rule_name: "High-priority issues".to_string(),
                description: format!(
                    "High-priority issues {} exceed maximum {}",
                    results.summary.high_priority_issues, config.max_high_priority_issues
                ),
                current_value: results.summary.high_priority_issues as f64,
                threshold: config.max_high_priority_issues as f64,
                severity: "high".to_string(),
                affected_files: Vec::new(),
                recommended_actions: vec!["Focus on high-priority refactoring".to_string()],
            });
        }

        if results.health_metrics.doc_health_score < config.min_doc_health_score {
            violations.push(QualityGateViolation {
                rule_name: "Minimum documentation health".to_string(),
                description: format!(
                    "Documentation health {:.1} is below minimum {:.1}",
                    results.health_metrics.doc_health_score, config.min_doc_health_score
                ),
                current_value: results.health_metrics.doc_health_score,
                threshold: config.min_doc_health_score,
                severity: "medium".to_string(),
                affected_files: Vec::new(),
                recommended_actions: vec![
                    "Add or update documentation for eligible files".to_string()
                ],
            });
        }

        QualityGateResult {
            passed: violations.is_empty(),
            violations,
            overall_score: results.health_metrics.overall_health_score,
        }
    }
}

/// Helper methods for [`DefaultResultAggregator`].
impl DefaultResultAggregator {
    fn count_lines_of_code(complexity: &ComplexityAnalysisResults) -> usize {
        complexity
            .detailed_results
            .iter()
            .map(|r| r.metrics.lines_of_code as usize)
            .sum()
    }

    fn detect_languages(files: &[PathBuf]) -> Vec<String> {
        let mut languages = HashSet::new();
        for file in files {
            if let Some(ext) = file.extension().and_then(|e| e.to_str()) {
                let lang = match ext {
                    "py" => "Python",
                    "js" | "jsx" => "JavaScript",
                    "ts" | "tsx" => "TypeScript",
                    "rs" => "Rust",
                    "go" => "Go",
                    "java" => "Java",
                    _ => continue,
                };
                languages.insert(lang.to_string());
            }
        }
        languages.into_iter().collect()
    }

    fn count_priority_issues(complexity: &ComplexityAnalysisResults) -> (usize, usize) {
        let mut high = 0;
        let mut critical = 0;
        for result in &complexity.detailed_results {
            for issue in &result.issues {
                match issue.severity.as_str() {
                    "High" | "VeryHigh" => high += 1,
                    "Critical" => critical += 1,
                    _ => {}
                }
            }
        }
        (high, critical)
    }

    fn calculate_avg_refactoring_score(refactoring: &RefactoringAnalysisResults) -> f64 {
        if refactoring.opportunities_count > 0 {
            refactoring
                .detailed_results
                .iter()
                .map(|r| r.refactoring_score)
                .sum::<f64>()
                / refactoring.opportunities_count as f64
        } else {
            0.0
        }
    }

    fn calculate_code_health_score(total_entities: usize, total_issues: usize) -> f64 {
        if total_entities > 0 {
            let penalty = (total_issues as f64 / total_entities as f64).min(1.0);
            (1.0 - penalty).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}
