use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future;

use super::result_types::AnalysisSummary;
use crate::core::arena_analysis::ArenaAnalysisResult;
use crate::core::config::ValknutConfig;
use crate::core::errors::{Result, ValknutError};
use crate::core::pipeline::pipeline_results::{
    ComplexityAnalysisResults, ComprehensiveAnalysisResult, CoverageAnalysisResults, HealthMetrics,
    ImpactAnalysisResults, LshAnalysisResults, RefactoringAnalysisResults,
    StructureAnalysisResults,
};
use crate::core::pipeline::{QualityGateResult, QualityGateViolation};
use serde::{Deserialize, Serialize};

use super::file_discovery;
use super::pipeline_config::{AnalysisConfig, QualityGateConfig};

/// Service responsible for translating requested roots into concrete files.
pub trait FileDiscoverer: Send + Sync {
    fn discover(
        &self,
        roots: &[PathBuf],
        pipeline_config: &AnalysisConfig,
        valknut_config: Option<&ValknutConfig>,
    ) -> Result<Vec<PathBuf>>;
}

/// Default git-aware file discovery implementation that reuses the legacy logic.
#[derive(Default, Debug)]
pub struct GitAwareFileDiscoverer;

impl FileDiscoverer for GitAwareFileDiscoverer {
    fn discover(
        &self,
        roots: &[PathBuf],
        pipeline_config: &AnalysisConfig,
        valknut_config: Option<&ValknutConfig>,
    ) -> Result<Vec<PathBuf>> {
        file_discovery::discover_files(roots, pipeline_config, valknut_config)
    }
}

impl GitAwareFileDiscoverer {
    pub fn shared() -> Arc<dyn FileDiscoverer> {
        Arc::new(Self::default())
    }
}

/// Service responsible for reading file contents in a controlled, batched manner.
#[async_trait]
pub trait FileBatchReader: Send + Sync {
    async fn read_files(&self, files: &[PathBuf]) -> Result<Vec<(PathBuf, String)>>;
}

/// Default implementation that processes files in fixed batches and uses Tokio I/O.
#[derive(Debug, Default)]
pub struct BatchedFileReader {
    batch_size: usize,
}

impl BatchedFileReader {
    pub fn new(batch_size: usize) -> Self {
        Self { batch_size }
    }

    fn effective_batch_size(&self) -> usize {
        self.batch_size.max(1)
    }
}

#[async_trait]
impl FileBatchReader for BatchedFileReader {
    async fn read_files(&self, files: &[PathBuf]) -> Result<Vec<(PathBuf, String)>> {
        let mut file_contents = Vec::with_capacity(files.len());
        for batch in files.chunks(self.effective_batch_size()) {
            let mut batch_results = Vec::with_capacity(batch.len());
            for file_path in batch {
                let path = file_path.clone();
                batch_results.push(async move {
                    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
                        ValknutError::io(format!("Failed to read file {}", path.display()), e)
                    })?;
                    Ok::<_, ValknutError>((path, content))
                });
            }

            for result in future::join_all(batch_results).await {
                let (path, content) = result?;
                file_contents.push((path, content));
            }
        }

        Ok(file_contents)
    }
}

impl BatchedFileReader {
    pub fn default_shared() -> Arc<dyn FileBatchReader> {
        Arc::new(Self::new(200))
    }
}

/// Aggregated results from all enabled analysis stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResultsBundle {
    pub structure: StructureAnalysisResults,
    pub coverage: CoverageAnalysisResults,
    pub complexity: ComplexityAnalysisResults,
    pub refactoring: RefactoringAnalysisResults,
    pub impact: ImpactAnalysisResults,
    pub lsh: LshAnalysisResults,
}

impl StageResultsBundle {
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
                module_force_graph: None,
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
        }
    }
}

impl Default for StageResultsBundle {
    fn default() -> Self {
        Self::disabled()
    }
}

#[async_trait(?Send)]
pub trait StageOrchestrator: Send + Sync {
    async fn run_arena_analysis_with_content(
        &self,
        file_contents: &[(PathBuf, String)],
    ) -> Result<Vec<ArenaAnalysisResult>>;

    async fn run_all_stages(
        &self,
        config: &AnalysisConfig,
        paths: &[PathBuf],
        files: &[PathBuf],
        arena_results: &[ArenaAnalysisResult],
    ) -> Result<StageResultsBundle>;
}

pub trait ResultAggregator: Send + Sync {
    fn build_summary(
        &self,
        files: &[PathBuf],
        structure: &StructureAnalysisResults,
        complexity: &ComplexityAnalysisResults,
        refactoring: &RefactoringAnalysisResults,
        impact: &ImpactAnalysisResults,
    ) -> AnalysisSummary;

    fn build_health_metrics(
        &self,
        complexity: &ComplexityAnalysisResults,
        structure: &StructureAnalysisResults,
        impact: &ImpactAnalysisResults,
    ) -> HealthMetrics;

    fn evaluate_quality_gates(
        &self,
        config: &QualityGateConfig,
        results: &ComprehensiveAnalysisResult,
    ) -> QualityGateResult;
}

#[derive(Default, Debug)]
pub struct DefaultResultAggregator;

impl ResultAggregator for DefaultResultAggregator {
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
        let total_lines_of_code = complexity
            .detailed_results
            .iter()
            .map(|r| r.metrics.lines_of_code as usize)
            .sum();

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
            high_priority_issues: high_priority,
            critical_issues: critical,
            doc_health_score: 1.0,
            doc_issue_count: 0,
        }
    }

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
