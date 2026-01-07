    use super::*;
    use crate::core::featureset::FeatureVector;
    use crate::core::pipeline::coverage_mapping;
    use crate::core::pipeline::results::pipeline_results::{
        LshAnalysisResults as PipelineLshAnalysisResult, MemoryStats as PipelineMemoryStats,
        TfIdfStats,
    };
    use crate::core::pipeline::{
        AnalysisConfig, CloneVerificationResults, ComplexityAnalysisResults,
        ComprehensiveAnalysisResult, CoverageAnalysisResults, HealthMetrics, ImpactAnalysisResults,
        PipelineResults, PipelineStatistics, RefactoringAnalysisResults, ScoringResults,
        StructureAnalysisResults,
    };
    use crate::core::scoring::{Priority, ScoringResult};
    use crate::detectors::coverage::{
        CoverageGap, CoveragePack, FileInfo, GapFeatures, GapMarkers, GapSymbol, PackEffort,
        PackValue, SnippetPreview, SymbolKind, UncoveredSpan,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    fn sample_candidate(
        file_path: &str,
        name: &str,
        priority: Priority,
        category: &str,
        score: f64,
    ) -> RefactoringCandidate {
        RefactoringCandidate {
            entity_id: format!("{}:{}", file_path, name),
            name: name.to_string(),
            file_path: file_path.to_string(),
            line_range: Some((1, 5)),
            priority,
            score,
            confidence: 0.85,
            issues: vec![RefactoringIssue {
                code: format!("{}_CODE", category.to_uppercase()),
                category: category.to_string(),
                severity: 1.2,
                contributing_features: Vec::new(),
            }],
            suggestions: Vec::new(),
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        }
    }

    fn pipeline_results_fixture() -> PipelineResults {
        let summary = AnalysisSummary {
            files_processed: 2,
            entities_analyzed: 3,
            refactoring_needed: 1,
            high_priority: 1,
            critical: 0,
            avg_refactoring_score: 0.75,
            code_health_score: 0.82,
            total_files: 2,
            total_entities: 3,
            total_lines_of_code: 200,
            languages: vec!["rust".to_string()],
            total_issues: 1,
            high_priority_issues: 1,
            critical_issues: 0,
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        let structure = StructureAnalysisResults {
            enabled: true,
            directory_recommendations: Vec::new(),
            file_splitting_recommendations: Vec::new(),
            issues_count: 0,
        };

        let complexity = ComplexityAnalysisResults {
            enabled: true,
            detailed_results: Vec::new(),
            average_cyclomatic_complexity: 10.0,
            average_cognitive_complexity: 8.0,
            average_technical_debt_score: 0.3,
            average_maintainability_index: 0.7,
            issues_count: 1,
        };

        let refactoring = RefactoringAnalysisResults {
            enabled: true,
            detailed_results: Vec::new(),
            opportunities_count: 1,
        };

        let impact = ImpactAnalysisResults {
            enabled: true,
            dependency_cycles: Vec::new(),
            chokepoints: Vec::new(),
            clone_groups: Vec::new(),
            issues_count: 0,
        };

        let lsh = PipelineLshAnalysisResult {
            enabled: false,
            clone_pairs: Vec::new(),
            max_similarity: 0.0,
            avg_similarity: 0.0,
            duplicate_count: 0,
            apted_verification_enabled: false,
            verification: None,
            denoising_enabled: false,
            tfidf_stats: None,
        };

        let coverage = CoverageAnalysisResults {
            enabled: false,
            coverage_files_used: Vec::new(),
            coverage_gaps: Vec::new(),
            gaps_count: 0,
            overall_coverage_percentage: None,
            analysis_method: "none".to_string(),
        };

        let documentation = DocumentationAnalysisResults {
            enabled: false,
            issues_count: 0,
            doc_health_score: 100.0,
            file_doc_health: HashMap::new(),
            file_doc_issues: HashMap::new(),
            directory_doc_health: HashMap::new(),
            directory_doc_issues: HashMap::new(),
        };

        let health_metrics = HealthMetrics {
            overall_health_score: 0.82,
            maintainability_score: 0.78,
            technical_debt_ratio: 0.22,
            complexity_score: 20.0,
            structure_quality_score: 0.7,
            doc_health_score: 1.0,
        };

        let comprehensive = ComprehensiveAnalysisResult {
            analysis_id: "analysis-123".to_string(),
            timestamp: Utc::now(),
            processing_time: 1.25,
            config: AnalysisConfig::default(),
            summary: summary.clone(),
            structure,
            complexity,
            refactoring,
            impact,
            lsh,
            coverage,
            documentation,
            cohesion: crate::detectors::cohesion::CohesionAnalysisResults::default(),
            health_metrics,
        };

        let pipeline_statistics = PipelineStatistics {
            memory_stats: PipelineMemoryStats {
                current_memory_bytes: 750_000,
                peak_memory_bytes: 1_500_000,
                final_memory_bytes: 900_000,
                efficiency_score: 0.8,
            },
            files_processed: summary.files_processed,
            total_duration_ms: 250,
        };

        let mut scoring_result = ScoringResult {
            entity_id: "src/lib.rs:function:process_data".to_string(),
            overall_score: 45.0,
            priority: Priority::High,
            category_scores: HashMap::new(),
            feature_contributions: HashMap::new(),
            normalized_feature_count: 3,
            confidence: 0.9,
        };
        scoring_result
            .category_scores
            .insert("complexity".to_string(), 1.6);
        scoring_result
            .feature_contributions
            .insert("cyclomatic_complexity".to_string(), 1.2);

        let scoring_results = ScoringResults {
            files: vec![scoring_result.clone()],
        };

        let mut vector = FeatureVector::new(&scoring_result.entity_id);
        vector.add_feature("cyclomatic_complexity", 13.0);
        vector.add_metadata("name", json!("process_data"));
        vector.add_metadata("line_range", json!([12, 36]));

        PipelineResults {
            analysis_id: comprehensive.analysis_id.clone(),
            timestamp: comprehensive.timestamp,
            results: comprehensive,
            statistics: pipeline_statistics,
            errors: vec!["engine warning".to_string()],
            scoring_results,
            feature_vectors: vec![vector],
        }
    }

    fn sample_coverage_pack_json() -> serde_json::Value {
        let gap = CoverageGap {
            path: PathBuf::from("src/lib.rs"),
            span: UncoveredSpan {
                path: PathBuf::from("src/lib.rs"),
                start: 10,
                end: 18,
                hits: Some(0),
            },
            file_loc: 200,
            language: "rust".to_string(),
            score: 0.78,
            features: GapFeatures {
                gap_loc: 8,
                cyclomatic_in_gap: 1.2,
                cognitive_in_gap: 1.0,
                fan_in_gap: 3,
                exports_touched: false,
                dependency_centrality_file: 0.4,
                interface_surface: 2,
                docstring_or_comment_present: false,
                exception_density_in_gap: 0.0,
            },
            symbols: vec![GapSymbol {
                kind: SymbolKind::Function,
                name: "process_data".to_string(),
                signature: "fn process_data()".to_string(),
                line_start: 10,
                line_end: 18,
            }],
            preview: SnippetPreview {
                language: "rust".to_string(),
                pre: vec!["fn helper() {}".to_string()],
                head: vec!["fn process_data() {".to_string()],
                tail: vec!["}".to_string()],
                post: vec!["// end".to_string()],
                markers: GapMarkers {
                    start_line: 10,
                    end_line: 18,
                },
                imports: Vec::new(),
            },
        };

        let pack = CoveragePack {
            kind: "hotspot".to_string(),
            pack_id: "pack-1".to_string(),
            path: PathBuf::from("src/lib.rs"),
            file_info: FileInfo {
                loc: 200,
                coverage_before: 42.0,
                coverage_after_if_filled: 64.0,
            },
            gaps: vec![gap],
            value: PackValue {
                file_cov_gain: 12.0,
                repo_cov_gain_est: 2.4,
            },
            effort: PackEffort {
                tests_to_write_est: 2,
                mocks_est: 0,
            },
        };

        serde_json::to_value(pack).expect("pack serializes")
    }

    #[test]
    fn test_code_health_calculation() {
        let summary = crate::core::pipeline::ResultSummary {
            total_files: 10,
            total_issues: 5,
            health_score: 0.8,
            processing_time: 1.5,
            total_entities: 100,
            refactoring_needed: 20,
            avg_score: 0.5,
        };

        let health_score = AnalysisResults::calculate_code_health_score(&summary);
        assert!(health_score > 0.0);
        assert!(health_score <= 1.0);
    }

    #[test]
    fn test_refactoring_candidate_creation() {
        let mut scoring_result = ScoringResult {
            entity_id: "test_entity".to_string(),
            overall_score: 2.0,
            priority: Priority::High,
            category_scores: HashMap::new(),
            feature_contributions: HashMap::new(),
            normalized_feature_count: 5,
            confidence: 0.8,
        };

        scoring_result
            .category_scores
            .insert("complexity".to_string(), 1.5);
        scoring_result
            .feature_contributions
            .insert("cyclomatic".to_string(), 1.2);

        let candidate = RefactoringCandidate::from_scoring_result(&scoring_result, &[], std::path::Path::new(""));

        assert_eq!(candidate.entity_id, "test_entity");
        assert_eq!(candidate.priority, Priority::High);
        assert!(!candidate.issues.is_empty());
        assert!(!candidate.suggestions.is_empty());
    }

    #[test]
    fn test_analysis_summary_default() {
        let summary = AnalysisSummary {
            files_processed: 10,
            entities_analyzed: 50,
            refactoring_needed: 5,
            high_priority: 2,
            critical: 1,
            avg_refactoring_score: 1.2,
            code_health_score: 0.85,
            total_files: 10,
            total_entities: 50,
            total_lines_of_code: 1_000,
            languages: vec!["Rust".to_string()],
            total_issues: 3,
            high_priority_issues: 2,
            critical_issues: 1,
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        assert_eq!(summary.files_processed, 10);
        assert_eq!(summary.entities_analyzed, 50);
        assert_eq!(summary.refactoring_needed, 5);
        assert_eq!(summary.high_priority, 2);
        assert_eq!(summary.critical, 1);
        assert!((summary.code_health_score - 0.85).abs() < f64::EPSILON);
        assert_eq!(summary.total_files, 10);
        assert_eq!(summary.total_entities, 50);
        assert_eq!(summary.total_lines_of_code, 1_000);
        assert_eq!(summary.languages, vec!["Rust".to_string()]);
        assert_eq!(summary.total_issues, 3);
        assert_eq!(summary.high_priority_issues, 2);
        assert_eq!(summary.critical_issues, 1);
    }

    #[test]
    fn group_candidates_by_file_sorts_by_priority_and_score() {
        let candidates = vec![
            sample_candidate("src/lib.rs", "High", Priority::High, "complexity", 2.0),
            sample_candidate("src/lib.rs", "Low", Priority::Low, "structure", 0.3),
            sample_candidate(
                "src/critical.rs",
                "CriticalOne",
                Priority::Critical,
                "architecture",
                4.0,
            ),
        ];

        let groups = AnalysisResults::group_candidates_by_file(&candidates);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].file_path, "src/critical.rs");
        assert_eq!(groups[0].highest_priority, Priority::Critical);
        assert_eq!(groups[1].file_path, "src/lib.rs");
        assert_eq!(groups[1].entity_count, 2);
        assert!(groups[1].avg_score > 1.0);
    }

    #[test]
    fn top_issues_returns_sorted_categories() {
        let mut results = AnalysisResults::empty();
        results.refactoring_candidates = vec![
            sample_candidate("src/lib.rs", "One", Priority::High, "complexity", 2.0),
            sample_candidate("src/lib.rs", "Two", Priority::High, "complexity", 2.5),
            sample_candidate("src/utils.rs", "Three", Priority::Low, "structure", 1.0),
        ];

        let issues = results.top_issues(2);
        assert_eq!(issues[0].0, "complexity");
        assert_eq!(issues[0].1, 2);
        assert_eq!(issues[1].0, "structure");
        assert_eq!(issues[1].1, 1);
    }

    #[test]
    fn from_pipeline_results_populates_dictionary_and_warnings() {
        let pipeline_results = pipeline_results_fixture();
        let analysis = AnalysisResults::from_pipeline_results(pipeline_results, std::path::PathBuf::new());

        assert_eq!(analysis.summary.total_files, 2);
        assert_eq!(analysis.refactoring_candidates.len(), 1);
        assert!(analysis.code_dictionary.issues.contains_key("CMPLX"));
        assert!(analysis
            .code_dictionary
            .suggestions
            .contains_key("RDCYCLEX"));
        assert_eq!(analysis.warnings, vec!["engine warning".to_string()]);
    }

    #[test]
    fn convert_coverage_to_packs_filters_invalid_entries() {
        let mut coverage = CoverageAnalysisResults {
            enabled: true,
            coverage_files_used: Vec::new(),
            coverage_gaps: vec![sample_coverage_pack_json(), json!({"invalid": true})],
            gaps_count: 1,
            overall_coverage_percentage: Some(42.0),
            analysis_method: "coverage-py".to_string(),
        };

        let packs = coverage_mapping::convert_coverage_to_packs(&coverage);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].pack_id, "pack-1");
        assert_eq!(packs[0].gaps.len(), 1);

        coverage.enabled = false;
        assert!(coverage_mapping::convert_coverage_to_packs(&coverage).is_empty());
    }

    #[test]
    fn convert_lsh_to_clone_analysis_returns_details() {
        let mut pipeline_results = pipeline_results_fixture();
        {
            let lsh = &mut pipeline_results.results.lsh;
            lsh.enabled = true;
            lsh.denoising_enabled = true;
            lsh.clone_pairs = vec![json!({"pair": 1})];
            lsh.duplicate_count = 3;
            lsh.avg_similarity = 0.83;
            lsh.max_similarity = 0.93;
            lsh.verification = Some(CloneVerificationResults {
                method: "apted".to_string(),
                pairs_considered: 3,
                pairs_evaluated: 2,
                pairs_scored: 2,
                avg_similarity: Some(0.9),
            });
            lsh.tfidf_stats = Some(TfIdfStats {
                total_grams: 120,
                unique_grams: 40,
                top1pct_contribution: 0.35,
            });
        }

        let clone_analysis = AnalysisResults::convert_lsh_to_clone_analysis(&pipeline_results)
            .expect("should convert");
        assert!(clone_analysis.denoising_enabled);
        assert_eq!(clone_analysis.candidates_after_denoising, 3);
        assert!(clone_analysis
            .notes
            .iter()
            .any(|note| note.to_lowercase().contains("denoising")));
    }
