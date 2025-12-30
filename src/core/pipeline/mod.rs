//! Analysis Pipeline Module
//!
//! This module provides the core analysis pipeline for valknut, which orchestrates
//! the entire code analysis process through multiple stages.
//!
//! ## Key Components
//!
//! - **AnalysisPipeline**: Main orchestrator that coordinates all analysis stages
//! - **ExtractorRegistry**: Manages and organizes feature extractors
//! - **Quality Gates**: Configurable thresholds for CI/CD integration
//! - **Pipeline Results**: Comprehensive analysis results and metrics
//!
//! ## Pipeline Stages
//!
//! 1. **File Discovery**: Identify source files to analyze
//! 2. **Feature Extraction**: Extract features using specialized detectors
//! 3. **Normalization**: Apply statistical normalization to features
//! 4. **Scoring**: Calculate health metrics and technical debt scores
//! 5. **Results Aggregation**: Combine all analysis results
//!
//! ## Usage
//!
//! ```ignore
//! use valknut_rs::core::pipeline::AnalysisPipeline;
//!
//! let pipeline = AnalysisPipeline::default();
//! let results = pipeline.analyze_directory("./src").await?;
//! println!("Health score: {}", results.health_metrics.overall_health_score);
//! ```

pub use code_dictionary::*;
pub use pipeline_config::{
    AnalysisConfig, QualityGateConfig, QualityGateResult, QualityGateViolation,
};
pub use pipeline_executor::{AnalysisPipeline, ExtractorRegistry, ProgressCallback};
pub use pipeline_results::{
    CloneVerificationResults, ComplexityAnalysisResults, ComprehensiveAnalysisResult,
    CoverageAnalysisResults, FileScore, HealthMetrics, ImpactAnalysisResults, PipelineResults,
    PipelineStatistics, PipelineStatus, RefactoringAnalysisResults, ResultSummary, ScoringResults,
    StructureAnalysisResults,
};
pub use pipeline_stages::AnalysisStages;
pub use result_conversions::*;
pub use result_types::*;
pub use services::{
    BatchedFileReader, DefaultResultAggregator, FileBatchReader, FileDiscoverer,
    GitAwareFileDiscoverer, ResultAggregator, StageOrchestrator, StageResultsBundle,
};

mod clone_detection;
mod code_dictionary;
mod coverage_stage;
mod file_discovery;
mod lsh_stage;
mod pipeline_config;
mod pipeline_executor;
mod pipeline_results;
mod pipeline_stages;
mod result_conversions;
mod result_types;
mod services;

// Re-export stage modules
pub use coverage_stage::CoverageStage;
pub use lsh_stage::LshStage;

/// Additional tests for pipeline modules to improve coverage

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_pipeline_fit_legacy_api() {
        let pipeline = AnalysisPipeline::default();
        let mut pipeline = pipeline;
        let result = pipeline.fit(&[]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pipeline_extractor_registry() {
        let pipeline = AnalysisPipeline::default();
        let registry = pipeline.extractor_registry();
        let extractors: Vec<_> = registry.get_all_extractors().collect();
        assert_eq!(extractors.len(), 0);
    }

    #[tokio::test]
    async fn test_pipeline_analyze_vectors_legacy() {
        let pipeline = AnalysisPipeline::default();
        let result = pipeline.analyze_vectors(vec![]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pipeline_status() {
        let pipeline = AnalysisPipeline::default();
        let status = pipeline.get_status();
        assert!(status.ready);
        assert!(status.is_ready);
        assert!(status.config_valid);
    }

    #[tokio::test]
    async fn test_quality_gates_evaluation() {
        let pipeline = AnalysisPipeline::default();
        let config = QualityGateConfig::default();
        let results = pipeline_results::ComprehensiveAnalysisResult {
            analysis_id: "test".to_string(),
            timestamp: chrono::Utc::now(),
            processing_time: 1.0,
            config: pipeline_config::AnalysisConfig::default(),
            summary: AnalysisSummary {
                files_processed: 1,
                entities_analyzed: 1,
                refactoring_needed: 0,
                high_priority: 0,
                critical: 0,
                avg_refactoring_score: 0.0,
                code_health_score: 1.0,
                total_files: 1,
                total_entities: 1,
                total_lines_of_code: 100,
                languages: vec!["Rust".to_string()],
                total_issues: 0,
                high_priority_issues: 0,
                critical_issues: 0,
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
            structure: pipeline_results::StructureAnalysisResults {
                enabled: true,
                directory_recommendations: vec![],
                file_splitting_recommendations: vec![],
                issues_count: 0,
            },
            complexity: pipeline_results::ComplexityAnalysisResults {
                enabled: true,
                detailed_results: vec![],
                average_cyclomatic_complexity: 2.0,
                average_cognitive_complexity: 1.5,
                average_technical_debt_score: 10.0,
                average_maintainability_index: 85.0,
                issues_count: 0,
            },
            refactoring: pipeline_results::RefactoringAnalysisResults {
                enabled: true,
                detailed_results: vec![],
                opportunities_count: 0,
            },
            impact: pipeline_results::ImpactAnalysisResults {
                enabled: true,
                dependency_cycles: vec![],
                chokepoints: vec![],
                clone_groups: vec![],
                issues_count: 0,
            },
            lsh: pipeline_results::LshAnalysisResults {
                enabled: false,
                clone_pairs: vec![],
                max_similarity: 0.0,
                avg_similarity: 0.0,
                duplicate_count: 0,
                apted_verification_enabled: false,
                verification: None,
                denoising_enabled: false,
                tfidf_stats: None,
            },
            coverage: pipeline_results::CoverageAnalysisResults {
                enabled: false,
                coverage_files_used: vec![],
                coverage_gaps: vec![],
                gaps_count: 0,
                overall_coverage_percentage: None,
                analysis_method: "none".to_string(),
            },
            documentation: pipeline_results::DocumentationAnalysisResults::default(),
            cohesion: crate::detectors::cohesion::CohesionAnalysisResults::default(),
            health_metrics: pipeline_results::HealthMetrics {
                overall_health_score: 88.0,
                maintainability_score: 85.0,
                technical_debt_ratio: 10.0,
                complexity_score: 15.0,
                structure_quality_score: 90.0,
                doc_health_score: 100.0,
            },
        };

        let gate_result = pipeline.evaluate_quality_gates(&config, &results);
        assert!(gate_result.passed);
    }

    #[tokio::test]
    async fn test_analyze_directory_integration() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn main() { println!(\"Hello\"); }").unwrap();

        let pipeline = AnalysisPipeline::default();
        let result = pipeline.analyze_directory(temp_dir.path()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_analyze_paths_with_progress() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn main() { println!(\"Hello\"); }").unwrap();

        let pipeline = AnalysisPipeline::default();
        let paths = vec![temp_dir.path().to_path_buf()];

        let progress_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let progress_called_clone = progress_called.clone();
        let progress_callback = Some(Box::new(move |_msg: &str, _progress: f64| {
            progress_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        }) as ProgressCallback);

        let result = pipeline.analyze_paths(&paths, progress_callback).await;
        assert!(result.is_ok());
        assert!(progress_called.load(std::sync::atomic::Ordering::SeqCst));
    }
}
