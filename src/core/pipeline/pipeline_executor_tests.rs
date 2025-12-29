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
            doc_health_score: 1.0,
            doc_issue_count: 0,
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
            documentation: DocumentationAnalysisResults::default(),
            cohesion: CohesionAnalysisResults::default(),
            health_metrics: HealthMetrics {
                overall_health_score: 58.0,
                maintainability_score: 52.0,
                technical_debt_ratio: 71.0,
                complexity_score: 83.0,
                structure_quality_score: 45.0,
                doc_health_score: 100.0,
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
