    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs;
    use std::time::Duration;
    use tempfile::{tempdir, TempDir};
    use tokio;
    use valknut_rs::core::pipeline::{
        AnalysisResults, AnalysisStatistics, AnalysisSummary, CodeDictionary, FeatureContribution,
        RefactoringCandidate, RefactoringIssue, RefactoringSuggestion,
    };
    // Use the 3-field MemoryStats (for AnalysisStatistics)
    use valknut_rs::core::pipeline::results::result_types::MemoryStats;
    use valknut_rs::core::scoring::Priority;

    fn minimal_analysis_value() -> serde_json::Value {
        json!({
            "summary": {
                "total_files": 3,
                "total_issues": 0,
                "processing_time": 1.25,
                "critical_issues": 0,
                "high_priority_issues": 0,
                "languages": ["rust"]
            },
            "health_metrics": {
                "overall_health_score": 82.5,
                "complexity_score": 24.0,
                "maintainability_score": 70.0,
                "technical_debt_ratio": 12.0
            },
            "complexity": {
                "average_cyclomatic_complexity": 3.2,
                "average_cognitive_complexity": 4.6,
                "enabled": true,
                "detailed_results": []
            },
            "refactoring": {
                "enabled": true,
                "opportunities_count": 0,
                "detailed_results": []
            },
            "analysis_id": "test-analysis",
            "timestamp": "2024-01-01T00:00:00Z"
        })
    }

    fn rich_analysis_value() -> serde_json::Value {
        json!({
            "summary": {
                "total_files": 4,
                "total_issues": 3,
                "processing_time": 2.5,
                "critical_issues": 1,
                "high_priority_issues": 2,
                "languages": ["rust", "python"],
                "health_score": 45.0
            },
            "health_metrics": {
                "overall_health_score": 58.2,
                "complexity_score": 70.0,
                "maintainability_score": 40.0,
                "technical_debt_ratio": 35.0
            },
            "complexity": {
                "average_cyclomatic_complexity": 18.0,
                "average_cognitive_complexity": 22.0,
                "detailed_results": [
                    {
                        "file_path": "src/lib.rs",
                        "issues": [
                            {
                                "severity": "Critical",
                                "description": "Function `analyze` has excessive branching",
                                "category": "cyclomatic",
                                "line": 42
                            },
                            {
                                "severity": "High",
                                "description": "Function `process` exceeds recommended length",
                                "category": "size",
                                "line": 58
                            }
                        ],
                        "recommendations": [
                            { "description": "Split `analyze` into focused helpers", "effort": 6 },
                            { "description": "Simplify nested conditionals in `process`", "effort": 4 }
                        ]
                    }
                ],
                "top_entities": [
                    {
                        "name": "src/lib.rs::analyze",
                        "kind": "function",
                        "cyclomatic_complexity": 21.0,
                        "cognitive_complexity": 27.0
                    }
                ],
                "hotspots": [
                    { "path": "src/lib.rs", "commit_count": 12, "change_frequency": 0.8 }
                ]
            },
            "refactoring": {
                "opportunities_count": 2,
                "detailed_results": [
                    {
                        "file_path": "src/lib.rs",
                        "recommendations": [
                            {
                                "refactoring_type": "ExtractMethod",
                                "description": "Extract helper for parsing block",
                                "estimated_impact": 8.5,
                                "estimated_effort": 3.0,
                                "priority_score": 0.92,
                                "location": [42]
                            },
                            {
                                "refactoring_type": "ReduceComplexity",
                                "description": "Flatten nested loops in `process`",
                                "estimated_impact": 6.7,
                                "estimated_effort": 2.5,
                                "priority_score": 0.61,
                                "location": [58]
                            }
                        ]
                    }
                ]
            },
            "structure": {
                "packs": [
                    {
                        "kind": "branch",
                        "file": "src/lib.rs",
                        "reasons": ["Too many sibling modules"]
                    },
                    {
                        "kind": "file_split",
                        "directory": "src",
                        "reasons": ["File exceeds recommended size"]
                    }
                ]
            },
            "comprehensive_analysis": {
                "structure": {
                    "packs": [
                        { "kind": "branch" },
                        { "kind": "file_split" },
                        { "kind": "other" }
                    ]
                }
            },
            "coverage": {
                "summary": {
                    "overall_coverage": 72.4
                }
            },
            "issues": [
                { "severity": "Critical", "description": "Unreachable branch detected" }
            ],
            "analysis_id": "rich-analysis",
            "timestamp": "2024-01-02T03:04:05Z"
        })
    }

    fn build_sample_analysis_results() -> AnalysisResults {
        let mut features_per_entity = HashMap::new();
        features_per_entity.insert("complexity".to_string(), 3.0);

        let mut priority_distribution = HashMap::new();
        priority_distribution.insert("high".to_string(), 1);

        let mut issue_distribution = HashMap::new();
        issue_distribution.insert("complexity".to_string(), 1);

        let issue = RefactoringIssue {
            code: "complexity_high".to_string(),
            category: "complexity".to_string(),
            severity: 0.85,
            contributing_features: vec![FeatureContribution {
                feature_name: "cyclomatic_complexity".to_string(),
                value: 22.0,
                normalized_value: 0.9,
                contribution: 0.6,
            }],
        };

        let suggestion = RefactoringSuggestion {
            refactoring_type: "extract_method".to_string(),
            code: "extract_method".to_string(),
            priority: 0.9,
            effort: 0.4,
            impact: 0.8,
        };

        let candidate = RefactoringCandidate {
            entity_id: "entity-1".to_string(),
            name: "analyze_module".to_string(),
            file_path: "src/lib.rs".to_string(),
            line_range: Some((5, 25)),
            priority: Priority::High,
            score: 0.91,
            confidence: 0.88,
            issues: vec![issue],
            suggestions: vec![suggestion],
            issue_count: 1,
            suggestion_count: 1,
            coverage_percentage: None,
        };

        AnalysisResults {
            project_root: std::path::PathBuf::new(),
            summary: AnalysisSummary {
                files_processed: 1,
                entities_analyzed: 1,
                refactoring_needed: 1,
                high_priority: 1,
                critical: 0,
                avg_refactoring_score: 0.91,
                code_health_score: 0.74,
                total_files: 1,
                total_entities: 1,
                total_lines_of_code: 140,
                languages: vec!["rust".to_string()],
                total_issues: 1,
                high_priority_issues: 1,
                critical_issues: 0,
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
            normalized: None,
            passes: valknut_rs::api::results::StageResultsBundle::disabled(),
            refactoring_candidates: vec![candidate.clone()],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(1),
                avg_file_processing_time: Duration::from_millis(400),
                avg_entity_processing_time: Duration::from_millis(200),
                features_per_entity,
                priority_distribution,
                issue_distribution,
                memory_stats: MemoryStats {
                    peak_memory_bytes: 2_048,
                    final_memory_bytes: 1_024,
                    efficiency_score: 0.85,
                },
            },
            health_metrics: None,
            clone_analysis: None,
            coverage_packs: Vec::new(),
            warnings: vec!["Sample warning".to_string()],
            code_dictionary: CodeDictionary::default(),
            documentation: None,
            directory_health: HashMap::new(),
            file_health: HashMap::new(),
            entity_health: HashMap::new(),
        }
    }

    fn typed_analysis_results_json() -> serde_json::Value {
        serde_json::to_value(build_sample_analysis_results())
            .expect("analysis results should serialize")
    }

    #[test]
    fn test_format_to_string() {
        assert_eq!(format_to_string(&OutputFormat::Json), "json");
        assert_eq!(format_to_string(&OutputFormat::Yaml), "yaml");
        assert_eq!(format_to_string(&OutputFormat::Markdown), "markdown");
        assert_eq!(format_to_string(&OutputFormat::Html), "html");
        assert_eq!(format_to_string(&OutputFormat::Jsonl), "jsonl");
        assert_eq!(format_to_string(&OutputFormat::Sonar), "sonar");
        assert_eq!(format_to_string(&OutputFormat::Csv), "csv");
        assert_eq!(format_to_string(&OutputFormat::CiSummary), "ci-summary");
        assert_eq!(format_to_string(&OutputFormat::Pretty), "pretty");
    }

    #[test]
    fn test_display_analysis_results() {
        let result = json!({
            "summary": {
                "total_files": 10,
                "total_lines": 1000,
                "health_score": 75.5,
                "complexity_score": 82.3,
                "technical_debt_ratio": 15.2,
                "maintainability_score": 68.1,
                "total_issues": 25,
                "critical_issues": 3,
                "high_priority_issues": 8
            },
            "timestamp": "2024-01-15T10:30:00Z"
        });

        // Test that display_analysis_results doesn't panic
        display_analysis_results(&result);
    }

    #[test]
    fn test_display_analysis_results_minimal() {
        let result = json!({});

        // Test that display_analysis_results handles missing fields gracefully
        display_analysis_results(&result);
    }

    #[test]
    fn test_display_analysis_results_low_issue_branch() {
        let result = json!({
            "summary": {
                "total_files": 12,
                "total_issues": 3,
                "high_priority_issues": 1,
                "critical_issues": 0,
                "processing_time": 12.5
            }
        });

        display_analysis_results(&result);
    }

    #[test]
    fn test_display_completion_summary() {
        let result = json!({
            "summary": {
                "total_files": 100,
                "issues_count": 5
            }
        });
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path();

        // Test that display_completion_summary doesn't panic
        display_completion_summary(&result, out_path, &OutputFormat::Json);
    }

    #[test]
    fn test_display_completion_summary_with_structure_packs() {
        let result = rich_analysis_value();
        let temp_dir = TempDir::new().unwrap();
        display_completion_summary(&result, temp_dir.path(), &OutputFormat::Html);
    }

    #[test]
    fn test_display_completion_summary_with_hotspots_and_coverage() {
        let result = json!({
            "summary": {
                "total_files": 42,
                "total_issues": 7
            },
            "comprehensive_analysis": {
                "structure": {
                    "packs": [
                        {
                            "kind": "file_split",
                            "name": "Large module.rs",
                            "value": {
                                "score": 0.88
                            },
                            "effort": {
                                "exports": 5,
                                "external_importers": 2
                            }
                        },
                        {
                            "kind": "branch_pack",
                            "name": "services/api.py",
                            "value": {
                                "score": 0.75
                            },
                            "effort": {
                                "exports": 2,
                                "external_importers": 1
                            }
                        }
                    ]
                }
            },
            "coverage": {
                "recommendations": [
                    {
                        "file": "src/lib.rs",
                        "reason": "Low branch coverage"
                    }
                ]
            }
        });

        let temp_dir = TempDir::new().unwrap();
        display_completion_summary(&result, temp_dir.path(), &OutputFormat::Json);
    }

    #[test]
    fn test_display_completion_summary_no_issues() {
        let result = json!({
            "summary": {
                "total_files": 15,
                "total_issues": 0
            }
        });

        let temp_dir = TempDir::new().unwrap();
        display_completion_summary(&result, temp_dir.path(), &OutputFormat::Pretty);
    }

    #[test]
    fn test_display_completion_summary_handles_missing_summary() {
        let result = json!({
            "comprehensive_analysis": {
                "structure": {
                    "packs": []
                }
            }
        });

        let temp_dir = TempDir::new().unwrap();
        display_completion_summary(&result, temp_dir.path(), &OutputFormat::Markdown);
    }

    #[test]
    fn test_display_completion_summary_with_existing_html_report() {
        let result = json!({
            "summary": {
                "total_files": 8,
                "total_issues": 2
            }
        });

        let temp_dir = TempDir::new().unwrap();
        let html_path = temp_dir.path().join("team_report.html");
        fs::write(&html_path, "<!doctype html>").expect("html file should be created");

        display_completion_summary(&result, temp_dir.path(), &OutputFormat::Html);
    }

    #[test]
    fn test_display_completion_summary_sonar_branch() {
        let result = json!({
            "summary": {
                "total_issues": 5
            }
        });

        let temp_dir = TempDir::new().unwrap();
        display_completion_summary(&result, temp_dir.path(), &OutputFormat::Sonar);
    }

    #[test]
    fn test_display_completion_summary_csv_branch() {
        let result = json!({
            "summary": {
                "total_issues": 4
            }
        });

        let temp_dir = TempDir::new().unwrap();
        display_completion_summary(&result, temp_dir.path(), &OutputFormat::Csv);
    }

    #[test]
    fn test_display_completion_summary_ci_summary_branch() {
        let result = json!({
            "summary": {
                "total_issues": 1
            }
        });

        let temp_dir = TempDir::new().unwrap();
        display_completion_summary(&result, temp_dir.path(), &OutputFormat::CiSummary);
    }

    #[tokio::test]
    async fn test_generate_outputs_writes_expected_files_without_analysis_results() {
        let result = minimal_analysis_value();
        let formats = vec![
            (OutputFormat::Jsonl, "report.jsonl"),
            (OutputFormat::Json, "analysis_results.json"),
            (OutputFormat::Yaml, "analysis_results.yaml"),
            (OutputFormat::Markdown, "team_report.md"),
            (OutputFormat::Html, "team_report.html"),
            (OutputFormat::Sonar, "sonarqube_issues.json"),
            (OutputFormat::Csv, "analysis_data.csv"),
            (OutputFormat::CiSummary, "ci_summary.json"),
        ];

        for (format, expected_file) in formats {
            let temp_dir = tempdir().unwrap();
            generate_outputs(&result, temp_dir.path(), &format)
                .await
                .unwrap();

            let output_path = temp_dir.path().join(expected_file);
            assert!(
                output_path.exists(),
                "Expected {} output at {}",
                format_to_string(&format),
                output_path.display()
            );

            match format {
                OutputFormat::Jsonl => {
                    let content = tokio::fs::read_to_string(&output_path).await.unwrap();
                    let expected = serde_json::to_string_pretty(&result).unwrap();
                    assert_eq!(content, expected);
                }
                OutputFormat::Json => {
                    let content = tokio::fs::read_to_string(&output_path).await.unwrap();
                    let expected = serde_json::to_string_pretty(&result).unwrap();
                    assert_eq!(content, expected);
                }
                OutputFormat::Yaml => {
                    let content = tokio::fs::read_to_string(&output_path).await.unwrap();
                    serde_yaml::from_str::<serde_json::Value>(&content).unwrap();
                }
                OutputFormat::Markdown => {
                    let content = tokio::fs::read_to_string(&output_path).await.unwrap();
                    assert!(content.contains("# Valknut Analysis Report"));
                }
                OutputFormat::Html => {
                    let content = tokio::fs::read_to_string(&output_path).await.unwrap();
                    assert!(content.contains("<!DOCTYPE html>"));
                }
                OutputFormat::Sonar => {
                    let content = tokio::fs::read_to_string(&output_path).await.unwrap();
                    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
                    assert!(parsed.get("issues").is_some());
                }
                OutputFormat::Csv => {
                    let content = tokio::fs::read_to_string(&output_path).await.unwrap();
                    assert!(content.starts_with("File,Issue Type,Severity,Description"));
                }
                OutputFormat::CiSummary => {
                    let content = tokio::fs::read_to_string(&output_path).await.unwrap();
                    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
                    assert_eq!(parsed["status"], "success");
                }
                _ => unreachable!(),
            }
        }

        let pretty_dir = tempdir().unwrap();
        generate_outputs(&result, pretty_dir.path(), &OutputFormat::Pretty)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_generate_markdown_report() {
        let result = json!({
            "summary": {
                "total_files": 10,
                "total_lines": 1000,
                "health_score": 75.5
            },
            "issues": [],
            "refactoring_opportunities": []
        });

        let markdown = generate_markdown_report(&result).await.unwrap();
        assert!(markdown.contains("# Valknut Analysis Report"));
        assert!(markdown.contains("Files Analyzed**: 10"));
        assert!(markdown.contains("Issues Found**: 0"));
    }

    #[tokio::test]
    async fn test_generate_markdown_report_with_detailed_sections() {
        let result = rich_analysis_value();
        let markdown = generate_markdown_report(&result).await.unwrap();
        assert!(markdown.contains("## Issues Requiring Attention"));
        assert!(markdown.contains("### High Priority Files"));
        assert!(markdown.contains("Split `analyze` into focused helpers"));
        assert!(markdown.contains("Average Cyclomatic Complexity"));
    }

    #[tokio::test]
    async fn test_generate_outputs_with_feedback_runs_spinner() {
        let result = minimal_analysis_value();
        let temp_dir = tempdir().unwrap();

        generate_outputs_with_feedback(&result, temp_dir.path(), &OutputFormat::Json, false)
            .await
            .expect("spinner path should succeed");

        let output_path = temp_dir.path().join("analysis_results.json");
        assert!(
            output_path.exists(),
            "json output should exist after generation with feedback"
        );
    }

    #[tokio::test]
    async fn test_generate_outputs_with_feedback_quiet_mode() {
        let result = minimal_analysis_value();
        let temp_dir = tempdir().unwrap();

        generate_outputs_with_feedback(&result, temp_dir.path(), &OutputFormat::Jsonl, true)
            .await
            .expect("quiet path should succeed");

        let output_path = temp_dir.path().join("report.jsonl");
        assert!(
            output_path.exists(),
            "jsonl output should exist after quiet generation"
        );
    }

    #[tokio::test]
    async fn test_generate_html_report() {
        let result = json!({
            "summary": {
                "total_files": 5,
                "total_lines": 500,
                "health_score": 85.0
            },
            "issues": []
        });

        let html = generate_html_report(&result).await.unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<title>Valknut Analysis Report</title>"));
        assert!(html.contains("5"));
        assert!(html.contains("body"));
    }

    #[tokio::test]
    async fn test_generate_html_report_with_detailed_sections() {
        let result = rich_analysis_value();
        let html = generate_html_report(&result).await.unwrap();
        assert!(html.contains("🔥 High Priority Files"));
        assert!(html.contains("📊 Health Metrics"));
        assert!(html.contains("Extract helper for parsing block"));
        assert!(html.contains("metric-card"));
    }

    #[tokio::test]
    async fn test_generate_sonar_report() {
        let result = json!({
            "issues": [
                {
                    "file": "test.rs",
                    "line": 10,
                    "column": 5,
                    "severity": "major",
                    "rule": "complexity",
                    "message": "High complexity function"
                }
            ]
        });

        let sonar = generate_sonar_report(&result).await.unwrap();
        assert!(sonar.contains("\"issues\": []"));
        assert!(sonar.contains("\"version\": \"1.0\""));
        assert!(sonar.contains("\"summary\""));
    }

    #[tokio::test]
    async fn test_generate_sonar_report_with_nested_data() {
        let result = rich_analysis_value();
        let sonar = generate_sonar_report(&result).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&sonar).unwrap();
        let issues = parsed["issues"].as_array().unwrap();
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|issue| {
            issue["ruleId"]
                .as_str()
                .unwrap_or_default()
                .contains("cyclomatic")
        }));
        assert!(issues.iter().any(|issue| {
            issue["ruleId"]
                .as_str()
                .unwrap_or_default()
                .contains("extractmethod")
        }));
    }

    #[tokio::test]
    async fn test_generate_csv_report() {
        let result = json!({
            "issues": [
                {
                    "file": "main.rs",
                    "line": 20,
                    "severity": "high",
                    "category": "complexity",
                    "description": "Function too complex"
                },
                {
                    "file": "utils.rs",
                    "line": 35,
                    "severity": "medium",
                    "category": "maintainability",
                    "description": "Poor naming"
                }
            ]
        });

        let csv = generate_csv_report(&result).await.unwrap();
        assert!(csv.contains("File,Issue Type,Severity,Description"));
    }

    #[tokio::test]
    async fn test_generate_csv_report_with_nested_data() {
        let result = rich_analysis_value();
        let csv = generate_csv_report(&result).await.unwrap();
        assert!(csv.contains("ExtractMethod"));
        assert!(csv.contains("ReduceComplexity"));
        assert!(csv.contains("branch"));
    }

    #[tokio::test]
    async fn test_generate_csv_report_empty() {
        let result = json!({
            "issues": []
        });

        let csv = generate_csv_report(&result).await.unwrap();
        assert!(csv.contains("File,Issue Type,Severity,Description"));
        assert_eq!(csv.lines().count(), 2); // Header + "No issues found" line
    }

    #[tokio::test]
    async fn test_generate_ci_summary_report() {
        let result = json!({
            "summary": {
                "total_files": 15,
                "total_issues": 0,
                "critical_issues": 0,
                "high_priority_issues": 0
            },
            "health_metrics": {
                "overall_health_score": 72.5
            }
        });

        let summary = generate_ci_summary_report(&result).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&summary).unwrap();

        assert_eq!(parsed["status"], "success");
        assert_eq!(parsed["summary"]["total_files"], 15);
        assert_eq!(parsed["summary"]["total_issues"], 0);
        assert_eq!(parsed["summary"]["critical_issues"], 0);
        assert_eq!(parsed["metrics"]["overall_health_score"], 72.5);
    }

    #[tokio::test]
    async fn test_generate_ci_summary_report_fail() {
        let result = json!({
            "summary": {
                "total_files": 10,
                "total_issues": 25,
                "critical_issues": 8,
                "high_priority_issues": 12,
                "health_score": 45.0
            }
        });

        let summary = generate_ci_summary_report(&result).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&summary).unwrap();

        assert_eq!(parsed["status"], "issues_found");
        assert_eq!(parsed["summary"]["total_issues"], 25);
        assert_eq!(parsed["summary"]["critical_issues"], 8);
    }

    #[tokio::test]
    async fn test_generate_ci_summary_report_with_metrics() {
        let result = rich_analysis_value();
        let summary = generate_ci_summary_report(&result).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&summary).unwrap();
        assert_eq!(parsed["status"], "issues_found");
        assert_eq!(parsed["summary"]["total_files"], 4);
        assert!(
            (parsed["metrics"]["average_cyclomatic_complexity"]
                .as_f64()
                .unwrap()
                - 18.0)
                .abs()
                < f64::EPSILON
        );
        assert!(
            parsed["quality_gates"]["recommendations"]
                .as_array()
                .unwrap()
                .len()
                >= 1
        );
    }

    #[test]
    fn test_print_human_readable_results() {
        let results = json!({
            "summary": {
                "total_files": 20,
                "total_lines": 2000,
                "health_score": 88.5
            },
            "issues": [
                {
                    "severity": "high",
                    "description": "Test issue"
                }
            ]
        });

        // Test that print_human_readable_results doesn't panic
        print_human_readable_results(&results);
    }

    #[test]
    fn test_print_human_readable_results_with_packs() {
        let results = json!({
            "packs": [
                {
                    "kind": "branch",
                    "file": "src/lib.rs",
                    "directory": "src",
                    "reasons": [
                        "Directory has divergent responsibilities",
                        "High change frequency"
                    ]
                }
            ]
        });

        print_human_readable_results(&results);
    }

    #[test]
    fn test_print_human_readable_results_with_empty_packs() {
        let results = json!({
            "packs": []
        });

        print_human_readable_results(&results);
    }

    #[test]
    fn test_print_comprehensive_results_pretty() {
        let results = json!({
            "summary": {
                "total_files": 15,
                "health_score": 75.0,
                "complexity_score": 65.2,
                "technical_debt_ratio": 20.1
            },
            "issues": []
        });

        // Test that print_comprehensive_results_pretty doesn't panic
        print_comprehensive_results_pretty(&results);
    }

    #[test]
    fn test_print_comprehensive_results_pretty_with_issues() {
        let results = json!({
            "summary": {
                "total_files": 12,
                "total_issues": 6
            }
        });

        print_comprehensive_results_pretty(&results);
    }

    #[test]
    fn test_display_refactoring_suggestions_renders_recommendations() {
        let results = json!({
            "refactoring": {
                "enabled": true,
                "opportunities_count": 2,
                "detailed_results": [
                    {
                        "file_path": "src/lib.rs",
                        "recommendations": [
                            {
                                "refactoring_type": "ExtractMethod",
                                "description": "Extract helper function",
                                "estimated_impact": 8.0,
                                "estimated_effort": 3.0,
                                "priority_score": 0.95
                            },
                            {
                                "refactoring_type": "ReduceComplexity",
                                "description": "Flatten nested loops",
                                "estimated_impact": 6.5,
                                "estimated_effort": 4.0,
                                "priority_score": 0.75
                            }
                        ]
                    },
                    {
                        "file_path": "src/helpers.rs",
                        "recommendations": [
                            {
                                "refactoring_type": "ImproveNaming",
                                "description": "Clarify helper names",
                                "estimated_impact": 4.0,
                                "estimated_effort": 2.0,
                                "priority_score": 0.4
                            }
                        ]
                    }
                ]
            }
        });

        display_refactoring_suggestions(&results);
    }

    #[test]
    fn test_display_refactoring_suggestions_empty() {
        let results = json!({
            "refactoring": {
                "enabled": true,
                "opportunities_count": 0,
                "detailed_results": []
            }
        });

        display_refactoring_suggestions(&results);
    }

    #[test]
    fn test_display_complexity_recommendations_outputs_effort_labels() {
        let results = json!({
            "complexity": {
                "enabled": true,
                "detailed_results": [
                    {
                        "file_path": "src/service.rs",
                        "recommendations": [
                            {
                                "description": "Split handler into smaller modules",
                                "effort": 3
                            },
                            {
                                "description": "Introduce early returns",
                                "effort": 6
                            }
                        ]
                    },
                    {
                        "file_path": "src/worker.rs",
                        "recommendations": [
                            {
                                "description": "Reduce branching depth",
                                "effort": 8
                            }
                        ]
                    }
                ]
            }
        });

        display_complexity_recommendations(&results);
    }

    #[test]
    fn test_display_complexity_recommendations_empty() {
        let results = json!({
            "complexity": {
                "enabled": true,
                "detailed_results": [
                    {
                        "file_path": "src/lib.rs",
                        "recommendations": []
                    }
                ]
            }
        });

        display_complexity_recommendations(&results);
    }

    #[tokio::test]
    async fn test_generate_outputs_json() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 5
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Json).await;
        assert!(result.is_ok());

        let json_file = out_path.join("analysis_results.json");
        assert!(json_file.exists());

        let content = fs::read_to_string(&json_file).unwrap();
        assert!(content.contains("total_files"));
    }

    #[tokio::test]
    async fn test_generate_outputs_json_with_serialized_results() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output_structured");

        let mut analysis = AnalysisResults::empty();
        analysis.summary.files_processed = 2;
        analysis.summary.entities_analyzed = 4;
        analysis.summary.code_health_score = 0.82;
        let value = serde_json::to_value(&analysis).expect("serialize analysis results");

        generate_outputs(&value, &out_path, &OutputFormat::Json)
            .await
            .expect("structured output generation");

        let json_file = out_path.join("analysis_results.json");
        assert!(
            json_file.exists(),
            "expected generator to write json report"
        );
        let content = fs::read_to_string(&json_file).unwrap();
        assert!(content.contains("\"files_processed\": 2"));
    }

    #[tokio::test]
    async fn test_generate_outputs_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "health_score": 85.5
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Yaml).await;
        assert!(result.is_ok());

        let yaml_file = out_path.join("analysis_results.yaml");
        assert!(yaml_file.exists());

        let content = fs::read_to_string(&yaml_file).unwrap();
        assert!(content.contains("health_score"));
    }

    #[tokio::test]
    async fn test_generate_outputs_markdown() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 10,
                "health_score": 70.0
            },
            "issues": []
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Markdown).await;
        assert!(result.is_ok());

        let md_file = out_path.join("team_report.md");
        assert!(md_file.exists());

        let content = fs::read_to_string(&md_file).unwrap();
        assert!(content.contains("# Valknut Analysis Report"));
        assert!(content.contains("Files Analyzed**: 10"));
    }

    #[tokio::test]
    async fn test_generate_outputs_html() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 8,
                "health_score": 92.1
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Html).await;
        assert!(result.is_ok());

        let html_file = out_path.join("team_report.html");
        assert!(html_file.exists());

        let content = fs::read_to_string(&html_file).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("html"));
    }

    #[tokio::test]
    async fn test_generate_outputs_csv() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "issues": [
                {
                    "file": "test.rs",
                    "line": 15,
                    "severity": "high",
                    "category": "complexity",
                    "description": "Too complex"
                }
            ]
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Csv).await;
        assert!(result.is_ok());

        let csv_file = out_path.join("analysis_data.csv");
        assert!(csv_file.exists());

        let content = fs::read_to_string(&csv_file).unwrap();
        assert!(content.contains("File,Issue Type,Severity,Description"));
    }

    #[tokio::test]
    async fn test_generate_outputs_sonar() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "issues": [
                {
                    "file": "main.rs",
                    "line": 20,
                    "severity": "major",
                    "rule": "complexity",
                    "message": "High complexity"
                }
            ]
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Sonar).await;
        assert!(result.is_ok());

        let sonar_file = out_path.join("sonarqube_issues.json");
        assert!(sonar_file.exists());

        let content = fs::read_to_string(&sonar_file).unwrap();
        assert!(content.contains("\"issues\": []"));
        assert!(content.contains("\"version\": \"1.0\""));
    }

    #[tokio::test]
    async fn test_generate_outputs_ci_summary() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 12,
                "total_issues": 3,
                "critical_issues": 0,
                "health_score": 88.5
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::CiSummary).await;
        assert!(result.is_ok());

        let ci_file = out_path.join("ci_summary.json");
        assert!(ci_file.exists());

        let content = fs::read_to_string(&ci_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["status"], "issues_found");
        assert_eq!(parsed["summary"]["total_files"], 12);
    }

    #[tokio::test]
    async fn test_generate_outputs_with_feedback_quiet() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 3
            }
        });

        let result =
            generate_outputs_with_feedback(&result, &out_path, &OutputFormat::Json, true).await;
        assert!(result.is_ok());

        let json_file = out_path.join("analysis_results.json");
        assert!(json_file.exists());
    }

    #[tokio::test]
    async fn test_generate_outputs_with_feedback_not_quiet() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 7
            }
        });

        let result =
            generate_outputs_with_feedback(&result, &out_path, &OutputFormat::Yaml, false).await;
        assert!(result.is_ok());

        let yaml_file = out_path.join("analysis_results.yaml");
        assert!(yaml_file.exists());
    }

    #[tokio::test]
    async fn test_generate_outputs_pretty() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 25,
                "health_score": 78.3
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Pretty).await;
        assert!(result.is_ok());

        // Pretty format should not create files, just display
        assert!(!out_path.join("analysis.txt").exists());
    }

    #[tokio::test]
    async fn test_generate_outputs_jsonl() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({
            "summary": {
                "total_files": 6
            }
        });

        let result = generate_outputs(&result, &out_path, &OutputFormat::Jsonl).await;
        assert!(result.is_ok());

        let jsonl_file = out_path.join("report.jsonl");
        assert!(jsonl_file.exists());

        let content = fs::read_to_string(&jsonl_file).unwrap();
        assert!(content.contains("total_files"));
    }

    // Test edge cases and error conditions
    #[tokio::test]
    async fn test_generate_outputs_missing_fields() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("output");

        let result = json!({});

        // Should handle missing fields gracefully
        let result = generate_outputs(&result, &out_path, &OutputFormat::Json).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_generate_outputs_with_structured_analysis_results() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("typed_results");

        let structured = typed_analysis_results_json();

        generate_outputs(&structured, &out_path, &OutputFormat::Json)
            .await
            .expect("json report generation should succeed");
        generate_outputs(&structured, &out_path, &OutputFormat::Markdown)
            .await
            .expect("markdown report generation should succeed");

        let json_path = out_path.join("analysis_results.json");
        let markdown_path = out_path.join("team_report.md");

        assert!(json_path.exists());
        assert!(markdown_path.exists());

        let json_contents = fs::read_to_string(&json_path).unwrap();
        assert!(
            json_contents.contains("refactoring_candidates"),
            "structured JSON output should include candidate data"
        );

        let markdown_contents = fs::read_to_string(&markdown_path).unwrap();
        assert!(
            markdown_contents.contains("Files Analyzed"),
            "markdown output should include summary heading"
        );
    }

    #[tokio::test]
    async fn test_generate_outputs_with_analysis_results_across_formats() {
        let temp_dir = TempDir::new().unwrap();
        let out_path = temp_dir.path().join("analysis_formats");
        let structured = serde_json::to_value(build_sample_analysis_results()).unwrap();

        generate_outputs(&structured, &out_path, &OutputFormat::Html)
            .await
            .expect("html generation with analysis results should succeed");
        let html = fs::read_to_string(out_path.join("team_report.html")).unwrap();
        assert!(
            html.contains("Analysis Overview") || html.contains("Valknut"),
            "html report should include rendered content"
        );

        generate_outputs(&structured, &out_path, &OutputFormat::Csv)
            .await
            .expect("csv generation with analysis results should succeed");
        let csv = fs::read_to_string(out_path.join("analysis_data.csv")).unwrap();
        assert!(
            csv.contains("src/lib.rs"),
            "csv output should reference file paths from AnalysisResults"
        );

        generate_outputs(&structured, &out_path, &OutputFormat::Sonar)
            .await
            .expect("sonar generation with analysis results should succeed");
        let sonar = fs::read_to_string(out_path.join("sonarqube_issues.json")).unwrap();
        assert!(
            sonar.contains("\"issues\""),
            "sonar output should contain issues array"
        );
    }
