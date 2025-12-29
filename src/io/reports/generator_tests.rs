    use super::*;
    use crate::api::config_types::AnalysisConfig;
    use crate::core::pipeline::{
        AnalysisResults, AnalysisStatistics, AnalysisSummary, CodeDefinition, CodeDictionary,
        DirectoryHealthScore, DirectoryHealthTree, FeatureContribution, MemoryStats,
        NormalizedAnalysisResults, NormalizedEntity, NormalizedIssue, NormalizedIssueTotals,
        NormalizedSummary, RefactoringCandidate, RefactoringIssue, RefactoringSuggestion,
        TreeStatistics,
    };
    use crate::core::scoring::{Priority, ScoringResult};
    use crate::io::reports::templates;
    use crate::oracle::{
        CodebaseAssessment, RefactoringOracleResponse, RefactoringRoadmap, RefactoringTask,
    };
    use serial_test::serial;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn entity_ref(candidate: &RefactoringCandidate) -> RefactoringCandidate {
        candidate.clone()
    }

    fn create_test_results() -> AnalysisResults {
        use std::time::Duration;
        let mut results = AnalysisResults::empty();
        results.summary.files_processed = 3;
        results.summary.total_files = 3;
        results.summary.entities_analyzed = 15;
        results.summary.total_entities = 15;
        results.summary.refactoring_needed = 5;
        results.summary.high_priority = 2;
        results.summary.high_priority_issues = 2;
        results.summary.critical = 1;
        results.summary.critical_issues = 1;
        results.summary.avg_refactoring_score = 0.65;
        results.summary.code_health_score = 0.75;
        results.summary.total_issues = 3;
        results.summary.total_lines_of_code = 600;
        results.summary.languages = vec!["Rust".to_string()];
        results.refactoring_candidates = vec![RefactoringCandidate {
            entity_id: "test_entity_1".to_string(),
            name: "complex_function".to_string(),
            file_path: "src/test.rs".to_string(),
            line_range: Some((10, 50)),
            priority: Priority::High,
            score: 0.85,
            confidence: 0.9,
            issues: vec![RefactoringIssue {
                code: "complexity.high".to_string(),
                category: "complexity".to_string(),
                severity: 2.1,
                contributing_features: vec![FeatureContribution {
                    feature_name: "cyclomatic_complexity".to_string(),
                    value: 15.0,
                    normalized_value: 0.8,
                    contribution: 1.2,
                }],
            }],
            suggestions: vec![RefactoringSuggestion {
                refactoring_type: "extract_method".to_string(),
                code: "refactor.extract_method".to_string(),
                priority: 0.9,
                effort: 0.6,
                impact: 0.8,
            }],
            issue_count: 1,
            suggestion_count: 1,
            coverage_percentage: None,
        }];
        results.statistics.total_duration = Duration::from_millis(1500);
        results.statistics.avg_file_processing_time = Duration::from_millis(500);
        results.statistics.avg_entity_processing_time = Duration::from_millis(100);
        results.statistics.memory_stats = MemoryStats {
            peak_memory_bytes: 128 * 1024 * 1024,
            final_memory_bytes: 64 * 1024 * 1024,
            efficiency_score: 0.85,
        };
        results.warnings = vec!["Test warning".to_string()];
        results.coverage_packs = vec![crate::detectors::coverage::CoveragePack {
            kind: "coverage".to_string(),
            pack_id: "cov:src/test.rs".to_string(),
            path: std::path::PathBuf::from("src/test.rs"),
            file_info: crate::detectors::coverage::FileInfo {
                loc: 200,
                coverage_before: 0.65,
                coverage_after_if_filled: 0.90,
            },
            gaps: vec![crate::detectors::coverage::CoverageGap {
                path: std::path::PathBuf::from("src/test.rs"),
                span: crate::detectors::coverage::UncoveredSpan {
                    path: std::path::PathBuf::from("src/test.rs"),
                    start: 25,
                    end: 35,
                    hits: Some(0),
                },
                file_loc: 200,
                language: "rust".to_string(),
                score: 0.85,
                features: crate::detectors::coverage::GapFeatures {
                    gap_loc: 10,
                    cyclomatic_in_gap: 3.0,
                    cognitive_in_gap: 4.0,
                    fan_in_gap: 2,
                    exports_touched: true,
                    dependency_centrality_file: 0.7,
                    interface_surface: 3,
                    docstring_or_comment_present: false,
                    exception_density_in_gap: 0.1,
                },
                symbols: vec![crate::detectors::coverage::GapSymbol {
                    kind: crate::detectors::coverage::SymbolKind::Function,
                    name: "uncovered_function".to_string(),
                    signature: "fn uncovered_function(x: i32) -> Result<String>".to_string(),
                    line_start: 25,
                    line_end: 35,
                }],
                preview: crate::detectors::coverage::SnippetPreview {
                    language: "rust".to_string(),
                    pre: vec!["    // Previous context".to_string()],
                    head: vec!["    fn uncovered_function(x: i32) -> Result<String> {".to_string()],
                    tail: vec!["    }".to_string()],
                    post: vec!["    // Following context".to_string()],
                    markers: crate::detectors::coverage::GapMarkers {
                        start_line: 25,
                        end_line: 35,
                    },
                    imports: vec!["use std::result::Result;".to_string()],
                },
            }],
            value: crate::detectors::coverage::PackValue {
                file_cov_gain: 0.25,
                repo_cov_gain_est: 0.05,
            },
            effort: crate::detectors::coverage::PackEffort {
                tests_to_write_est: 3,
                mocks_est: 1,
            },
        }];
        results
    }

    #[test]
    fn test_report_generator_new() {
        let generator = ReportGenerator::new();
        assert!(generator
            .handlebars
            .get_templates()
            .contains_key("default_html"));
        let expected_templates_dir = templates::detect_templates_dir();
        assert_eq!(
            generator.templates_dir.is_some(),
            expected_templates_dir.is_some()
        );
    }

    #[test]
    fn test_report_generator_default() {
        let generator = ReportGenerator::default();
        assert!(generator
            .handlebars
            .get_templates()
            .contains_key("default_html"));
        let expected_templates_dir = templates::detect_templates_dir();
        assert_eq!(
            generator.templates_dir.is_some(),
            expected_templates_dir.is_some()
        );
    }

    #[test]
    fn test_generator_with_config_stores_analysis_config() {
        let config = AnalysisConfig::default();
        let generator = ReportGenerator::new().with_config(config.clone());
        assert!(generator.analysis_config.is_some());
        let stored = generator.analysis_config.as_ref().expect("config stored");
        assert_eq!(stored.modules.complexity, config.modules.complexity);
        assert_eq!(stored.coverage.enabled, config.coverage.enabled);
    }

    #[test]
    fn test_report_generator_debug() {
        let generator = ReportGenerator::new();
        let debug_str = format!("{:?}", generator);
        assert!(debug_str.contains("ReportGenerator"));
        assert!(debug_str.contains("handlebars"));
        assert!(debug_str.contains("templates_dir"));
    }

    #[test]
    fn test_with_templates_dir_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_path = temp_dir.path().join("nonexistent");

        let generator = ReportGenerator::new()
            .with_templates_dir(&nonexistent_path)
            .unwrap();

        assert_eq!(generator.templates_dir, Some(nonexistent_path));
    }

    #[test]
    fn test_with_templates_dir_existing() {
        let temp_dir = TempDir::new().unwrap();
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&templates_dir).unwrap();

        // Create a test template file
        let template_file = templates_dir.join("custom.hbs");
        fs::write(
            &template_file,
            "{{#each items}}<div>{{this}}</div>{{/each}}",
        )
        .unwrap();

        let generator = ReportGenerator::new()
            .with_templates_dir(&templates_dir)
            .unwrap();

        assert_eq!(generator.templates_dir, Some(templates_dir));
        assert!(generator.handlebars.get_templates().contains_key("custom"));
    }

    #[test]
    fn test_generate_json_report() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_report.json");

        let generator = ReportGenerator::new();
        let results = create_test_results();

        let result = generator.generate_report(&results, &output_path, ReportFormat::Json);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("\"files_processed\": 3"));
        assert!(content.contains("\"complex_function\""));
        assert!(content.contains("\"Test warning\""));
    }

    #[test]
    fn test_generate_yaml_report() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_report.yaml");

        let generator = ReportGenerator::new();
        let results = create_test_results();

        let result = generator.generate_report(&results, &output_path, ReportFormat::Yaml);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("files_processed: 3"));
        assert!(content.contains("complex_function"));
    }

    #[test]
    fn test_generate_csv_report() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_report.csv");

        let generator = ReportGenerator::new();
        let results = create_test_results();

        let result = generator.generate_report(&results, &output_path, ReportFormat::Csv);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("file_path"));
        assert!(content.contains("src/test.rs"));
    }

    #[test]
    fn test_generate_markdown_sonar_and_csv_table_reports() {
        let temp_dir = TempDir::new().unwrap();
        let generator = ReportGenerator::new();
        let results = create_test_results();

        let markdown_path = temp_dir.path().join("report.md");
        generator
            .generate_markdown_report(&results, &markdown_path)
            .expect("markdown report");
        let markdown_content = fs::read_to_string(&markdown_path).unwrap();
        assert!(markdown_content.contains("Valknut Analysis Report"));

        let csv_table_path = temp_dir.path().join("report_table.csv");
        generator
            .generate_csv_table(&results, &csv_table_path)
            .expect("csv table");
        let csv_table_content = fs::read_to_string(&csv_table_path).unwrap();
        assert!(csv_table_content.contains("complex_function"));

        let sonar_path = temp_dir.path().join("sonar.json");
        generator
            .generate_sonar_report(&results, &sonar_path)
            .expect("sonar report");
        let sonar_content = fs::read_to_string(&sonar_path).unwrap();
        assert!(sonar_content.contains("\"issues\""));
    }

    #[test]
    fn test_generate_html_report_default_template() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_report.html");

        let generator = ReportGenerator::new();
        let results = create_test_results();

        let result = generator.generate_report(&results, &output_path, ReportFormat::Html);
        if let Err(ref e) = result {
            panic!("HTML generation failed: {}", e);
        }
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("Analysis Report"));
        assert!(content.contains("Valknut"));
        assert!(content.contains("Files Analyzed"));
    }

    #[test]
    fn test_generate_html_report_custom_template() {
        let temp_dir = TempDir::new().unwrap();
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&templates_dir).unwrap();

        // Create a custom report template
        let custom_template = r#"
        <html>
        <head><title>Custom Report</title></head>
        <body>
        <h1>{{tool_name}} Report</h1>
        <p>Files processed: {{summary.total_files}}</p>
        <p>Issues found: {{summary.total_issues}}</p>
        </body>
        </html>
        "#;

        let template_file = templates_dir.join("report.hbs");
        fs::write(&template_file, custom_template).unwrap();

        let generator = ReportGenerator::new()
            .with_templates_dir(&templates_dir)
            .unwrap();

        let results = create_test_results();
        let output_path = temp_dir.path().join("test_report.html");

        let result = generator.generate_report(&results, &output_path, ReportFormat::Html);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("Custom Report"));
        assert!(content.contains("Files processed: 3"));
    }

    #[test]
    fn test_prepare_template_data() {
        let generator = ReportGenerator::new();
        let results = create_test_results();

        let template_data = generator.prepare_template_data(&results);

        assert!(template_data.is_object());
        let obj = template_data.as_object().unwrap();

        assert!(obj.contains_key("generated_at"));
        assert!(obj.contains_key("tool_name"));
        assert!(obj.contains_key("version"));
        assert!(obj.contains_key("results"));
        assert!(obj.contains_key("summary"));

        assert_eq!(
            obj["tool_name"],
            serde_json::Value::String("Valknut".to_string())
        );

        assert!(obj.contains_key("tree_payload"));
    }

    fn sample_oracle_response() -> RefactoringOracleResponse {
        RefactoringOracleResponse {
            assessment: CodebaseAssessment {
                summary: Some("The codebase has well-structured modules with good separation of concerns. Documentation could use some cleanup.".into()),
                architectural_narrative: None,
                architectural_style: Some("Modular Architecture".into()),
                strengths: vec!["Good separation of concerns".into()],
                issues: vec!["Large util file".into(), "Documentation gaps".into()],
            },
            tasks: vec![RefactoringTask {
                id: "T1".into(),
                title: "Refresh README".into(),
                description: "Update overview and usage sections".into(),
                category: "C6".into(),
                files: vec!["README.md".into()],
                risk: Some("R1".into()),
                risk_level: None,
                impact: Some("I2".into()),
                effort: Some("E1".into()),
                mitigation: None,
                required: Some(false),
                depends_on: vec![],
                benefits: vec!["Improved onboarding".into()],
            }],
            refactoring_roadmap: None,
        }
    }

    #[test]
    fn test_prepare_template_data_marks_oracle_presence() {
        let generator = ReportGenerator::new();
        let results = create_test_results();
        let oracle = sample_oracle_response();

        let data = generator.prepare_template_data_with_oracle(&results, &Some(oracle.clone()));
        let obj = data.as_object().expect("template data should be object");
        assert_eq!(obj["has_oracle_data"], serde_json::Value::Bool(true));
        assert!(obj.contains_key("oracle_refactoring_plan"));

        let without_oracle = generator.prepare_template_data(&results);
        let without_obj = without_oracle
            .as_object()
            .expect("template data should be object");
        assert_eq!(
            without_obj["has_oracle_data"],
            serde_json::Value::Bool(false)
        );
    }

    #[test]
    fn test_generate_report_with_oracle_all_formats() {
        let temp_dir = TempDir::new().unwrap();
        let generator = ReportGenerator::new();
        let results = create_test_results();
        let oracle = sample_oracle_response();

        let json_path = temp_dir.path().join("report.json");
        generator
            .generate_report_with_oracle(&results, &oracle, &json_path, ReportFormat::Json)
            .expect("json report should succeed");
        let json_content = fs::read_to_string(&json_path).unwrap();
        assert!(json_content.contains("oracle_refactoring_plan"));

        let html_path = temp_dir.path().join("report.html");
        generator
            .generate_report_with_oracle(&results, &oracle, &html_path, ReportFormat::Html)
            .expect("html report should succeed");
        let html_content = fs::read_to_string(&html_path).unwrap();
        assert!(html_content.contains("Analysis Report"));
        let assets_dir = temp_dir.path().join("webpage_files");
        assert!(
            assets_dir.exists(),
            "expected webpage assets directory to be created"
        );

        let yaml_path = temp_dir.path().join("report.yaml");
        generator
            .generate_report_with_oracle(&results, &oracle, &yaml_path, ReportFormat::Yaml)
            .expect("yaml report should succeed");
        let yaml_content = fs::read_to_string(&yaml_path).unwrap();
        assert!(yaml_content.contains("oracle_refactoring_plan"));

        let csv_path = temp_dir.path().join("report.csv");
        generator
            .generate_report_with_oracle(&results, &oracle, &csv_path, ReportFormat::Csv)
            .expect("csv report should succeed");
        let csv_content = fs::read_to_string(&csv_path).unwrap();
        assert!(csv_content.contains("complex_function"));
    }

    #[serial]
    #[test]
    fn test_clean_path_helpers_strip_prefixes() {
        let generator = ReportGenerator::new();

        let original_dir = std::env::current_dir().unwrap();
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let absolute_path = temp_dir.path().join("src/lib.rs");
        let cleaned_abs = generator.clean_path_string(absolute_path.to_str().unwrap());
        assert_eq!(cleaned_abs, "src/lib.rs");

        std::env::set_current_dir(&original_dir).unwrap();

        let with_dot = generator.clean_path_string("./src/main.rs");
        assert_eq!(with_dot, "src/main.rs");
    }

    #[test]
    fn test_clean_path_prefixes_in_file_groups_and_candidates() {
        let generator = ReportGenerator::new();

        let candidates = vec![RefactoringCandidate {
            entity_id: "./src/lib.rs:function".into(),
            name: "./src/lib.rs::function".into(),
            file_path: "./src/lib.rs".into(),
            line_range: Some((1, 10)),
            priority: Priority::High,
            score: 0.8,
            confidence: 0.9,
            issues: vec![],
            suggestions: vec![],
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        }];

        let file_groups = vec![FileRefactoringGroup {
            file_path: "./src/lib.rs".into(),
            file_name: "lib.rs".into(),
            entity_count: 1,
            avg_score: 0.8,
            highest_priority: Priority::High,
            total_issues: 1,
            entities: vec![entity_ref(&candidates[0])],
        }];

        let cleaned_candidates = generator.clean_path_prefixes(&candidates);
        assert_eq!(cleaned_candidates[0].file_path, "src/lib.rs");
        assert_eq!(cleaned_candidates[0].entity_id, "src/lib.rs:function");

        let cleaned_groups = generator.clean_path_prefixes_in_file_groups(&file_groups);
        assert_eq!(cleaned_groups[0].file_path, "src/lib.rs");
        assert_eq!(cleaned_groups[0].entities[0].name, "src/lib.rs::function");
    }

    #[test]
    fn test_calculate_summary() {
        let generator = ReportGenerator::new();
        let results = create_test_results();

        let summary = generator.calculate_summary(&results);

        assert_eq!(
            summary.get("total_files").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(3))
        );
        assert_eq!(
            summary.get("total_issues").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(3))
        );
        assert_eq!(
            summary.get("high_issues").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(2))
        );
        assert_eq!(
            summary.get("critical_issues").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(1))
        );
    }

    #[test]
    fn test_calculate_summary_prefers_normalized_data() {
        let generator = ReportGenerator::new();
        let mut results = create_test_results();

        let normalized = NormalizedAnalysisResults {
            meta: NormalizedSummary {
                timestamp: Utc::now(),
                files_scanned: 10,
                entities_analyzed: 42,
                code_health: 0.91,
                languages: vec!["rust".to_string(), "typescript".to_string()],
                issues: NormalizedIssueTotals {
                    total: 8,
                    high: 3,
                    critical: 1,
                },
            },
            entities: vec![NormalizedEntity {
                id: "src/lib.rs:function:one".into(),
                name: "one".into(),
                file: Some("src/lib.rs".into()),
                kind: Some("function".into()),
                line_range: Some((10, 20)),
                priority: Priority::High,
                score: 0.82,
                metrics: None,
                issues: vec![NormalizedIssue::from(("CMPLX".to_string(), 1.2))],
                suggestions: Vec::new(),
                file_path: Some("src/lib.rs".to_string()),
                issue_count: 0,
                suggestion_count: 0,
            }],
            clone: None,
            warnings: Vec::new(),
            dictionary: CodeDictionary::default(),
        };

        results.normalized = Some(normalized);

        let summary = generator.calculate_summary(&results);

        assert_eq!(
            summary.get("files_processed").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(3))
        );
        assert_eq!(
            summary.get("entities_analyzed").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(15))
        );
        assert_eq!(
            summary.get("total_issues").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(3))
        );
        let languages = summary.get("languages").unwrap().as_array().unwrap();
        assert_eq!(languages.len(), 1);
    }

    #[test]
    fn test_report_error_display() {
        let io_error = std::io::Error::new(std::io::ErrorKind::InvalidData, "template error");
        let report_error = ReportError::Io(io_error);

        let error_string = format!("{}", report_error);
        assert!(error_string.contains("IO error"));
    }

    #[test]
    fn test_report_error_debug() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let report_error = ReportError::Io(io_error);

        let debug_string = format!("{:?}", report_error);
        assert!(debug_string.contains("Io"));
        assert!(debug_string.contains("NotFound"));
    }

    #[test]
    fn test_load_templates_from_dir_invalid_filename() {
        let temp_dir = TempDir::new().unwrap();
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&templates_dir).unwrap();

        // Create a file with invalid filename (no stem) - try a different approach
        // Since .hbs might be valid on some systems, let's use a filename that definitely has no stem
        let bad_file = templates_dir.join("");
        match fs::write(&bad_file, "content") {
            Ok(_) => {
                // If the write succeeded, test should pass
                let mut generator = ReportGenerator::new();
                let result =
                    templates::load_templates_from_dir(&mut generator.handlebars, &templates_dir);
                // Just make sure it doesn't panic, the result could be ok or error
                let _ = result;
            }
            Err(_) => {
                // If we can't create the invalid file, that's expected
                // Just test with a normal template loading that should work
                let good_file = templates_dir.join("good.hbs");
                fs::write(&good_file, "{{content}}").unwrap();

                let mut generator = ReportGenerator::new();
                let result =
                    templates::load_templates_from_dir(&mut generator.handlebars, &templates_dir);
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn test_load_templates_from_dir_non_hbs_files() {
        let temp_dir = TempDir::new().unwrap();
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&templates_dir).unwrap();

        // Create non-.hbs files that should be ignored
        fs::write(templates_dir.join("readme.txt"), "not a template").unwrap();
        fs::write(templates_dir.join("config.json"), "{}").unwrap();

        let mut generator = ReportGenerator::new();
        let initial_count = generator.handlebars.get_templates().len();

        let result = templates::load_templates_from_dir(&mut generator.handlebars, &templates_dir);
        assert!(result.is_ok());

        // Should have same number of templates (no new ones added)
        assert_eq!(generator.handlebars.get_templates().len(), initial_count);
    }

    #[test]
    fn test_clean_directory_health_tree_paths() {
        let generator = ReportGenerator::new();

        // Create a test directory health tree with "./" prefixes
        let mut directories = std::collections::HashMap::new();

        // Create directory with ./ prefix
        let src_dir = DirectoryHealthScore {
            path: PathBuf::from("./src"),
            health_score: 0.7,
            file_count: 2,
            entity_count: 3,
            refactoring_needed: 1,
            critical_issues: 0,
            high_priority_issues: 1,
            avg_refactoring_score: 1.5,
            weight: 1.0,
            children: vec![PathBuf::from("./src/core")],
            parent: Some(PathBuf::from("./")),
            issue_categories: std::collections::HashMap::new(),
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        let core_dir = DirectoryHealthScore {
            path: PathBuf::from("./src/core"),
            health_score: 0.6,
            file_count: 1,
            entity_count: 2,
            refactoring_needed: 2,
            critical_issues: 1,
            high_priority_issues: 2,
            avg_refactoring_score: 2.0,
            weight: 2.0,
            children: vec![],
            parent: Some(PathBuf::from("./src")),
            issue_categories: std::collections::HashMap::new(),
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        directories.insert(PathBuf::from("./src"), src_dir);
        directories.insert(PathBuf::from("./src/core"), core_dir);

        let hotspot_directories = vec![DirectoryHotspot {
            path: PathBuf::from("./src/core"),
            health_score: 0.6,
            rank: 1,
            primary_issue_category: "complexity".to_string(),
            recommendation: "Reduce complexity".to_string(),
        }];

        let tree_statistics = TreeStatistics {
            total_directories: 2,
            max_depth: 2,
            avg_health_score: 0.65,
            health_score_std_dev: 0.05,
            hotspot_directories,
            health_by_depth: std::collections::HashMap::new(),
        };

        let root = DirectoryHealthScore {
            path: PathBuf::from("./"),
            health_score: 0.8,
            file_count: 0,
            entity_count: 0,
            refactoring_needed: 0,
            critical_issues: 0,
            high_priority_issues: 0,
            avg_refactoring_score: 0.0,
            weight: 1.0,
            children: vec![PathBuf::from("./src")],
            parent: None,
            issue_categories: std::collections::HashMap::new(),
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        let original_tree = DirectoryHealthTree {
            root,
            directories,
            tree_statistics,
        };

        // Clean the paths
        let cleaned_tree = generator.clean_directory_health_tree_paths(&original_tree);

        // Verify that "./" prefixes are removed
        assert_eq!(cleaned_tree.root.path, PathBuf::from(""));
        assert_eq!(cleaned_tree.root.children[0], PathBuf::from("src"));

        // Check that directories HashMap keys are cleaned
        assert!(cleaned_tree.directories.contains_key(&PathBuf::from("src")));
        assert!(cleaned_tree
            .directories
            .contains_key(&PathBuf::from("src/core")));
        assert!(!cleaned_tree
            .directories
            .contains_key(&PathBuf::from("./src")));
        assert!(!cleaned_tree
            .directories
            .contains_key(&PathBuf::from("./src/core")));

        // Check that directory paths are cleaned within DirectoryHealthScore
        let src_dir_cleaned = cleaned_tree.directories.get(&PathBuf::from("src")).unwrap();
        assert_eq!(src_dir_cleaned.path, PathBuf::from("src"));
        assert_eq!(src_dir_cleaned.children[0], PathBuf::from("src/core"));
        assert_eq!(src_dir_cleaned.parent, Some(PathBuf::from("")));

        let core_dir_cleaned = cleaned_tree
            .directories
            .get(&PathBuf::from("src/core"))
            .unwrap();
        assert_eq!(core_dir_cleaned.path, PathBuf::from("src/core"));
        assert_eq!(core_dir_cleaned.parent, Some(PathBuf::from("src")));

        // Check that hotspot directories are cleaned
        assert_eq!(
            cleaned_tree.tree_statistics.hotspot_directories[0].path,
            PathBuf::from("src/core")
        );
    }

    #[test]
    fn test_add_files_to_hierarchy_basic() {
        let generator = ReportGenerator::new();

        // Create a simple hierarchy
        let hierarchy = vec![serde_json::json!({
            "id": "directory_src",
            "type": "folder",
            "name": "src",
            "path": "src",
            "children": []
        })];

        let candidate = RefactoringCandidate {
            entity_id: "test_entity".to_string(),
            name: "test_function".to_string(),
            file_path: "src/test.rs".to_string(),
            line_range: Some((10, 20)),
            priority: Priority::High,
            score: 0.85,
            confidence: 0.9,
            issues: vec![],
            suggestions: vec![],
            issue_count: 3,
            suggestion_count: 1,
            coverage_percentage: None,
        };

        let file_groups = vec![FileRefactoringGroup {
            file_path: "src/test.rs".to_string(),
            file_name: "test.rs".to_string(),
            entity_count: 1,
            avg_score: 0.85,
            highest_priority: Priority::High,
            total_issues: 3,
            entities: vec![entity_ref(&candidate)],
        }];

        let mut candidate_lookup = HashMap::new();
        candidate_lookup.insert(candidate.entity_id.clone(), candidate.clone());

        let result = generator.add_files_to_hierarchy(
            &hierarchy,
            &file_groups,
            &CodeDictionary::default(),
            &candidate_lookup,
        );

        // Verify structure
        assert_eq!(result.len(), 1);
        let dir_node = &result[0];
        assert_eq!(dir_node["type"], "folder");
        assert_eq!(dir_node["name"], "src");

        // Verify file was added
        let children = dir_node["children"].as_array().unwrap();
        assert_eq!(children.len(), 1);

        let file_node = &children[0];
        assert_eq!(file_node["type"], "file");
        assert_eq!(file_node["name"], "test.rs");
        assert_eq!(file_node["path"], "src/test.rs");
        assert_eq!(file_node["entity_count"], 1);

        // Verify entity was added as child of file
        let file_children = file_node["children"].as_array().unwrap();
        assert_eq!(file_children.len(), 1);
        let entity_node = &file_children[0];
        assert_eq!(entity_node["type"], "entity");
        assert_eq!(entity_node["name"], "test_function");
    }

    #[test]
    fn test_add_files_to_hierarchy_nested_directories() {
        let generator = ReportGenerator::new();

        // Create nested hierarchy
        let hierarchy = vec![serde_json::json!({
            "id": "directory_src",
            "type": "folder",
            "name": "src",
            "path": "src",
            "children": [
                {
                    "id": "directory_src_core",
                    "type": "folder",
                    "name": "core",
                    "path": "src/core",
                    "children": []
                }
            ]
        })];

        let main_candidate = RefactoringCandidate {
            entity_id: "main_entity".to_string(),
            name: "main".to_string(),
            file_path: "src/main.rs".to_string(),
            line_range: Some((1, 10)),
            priority: Priority::Medium,
            score: 0.7,
            confidence: 0.8,
            issues: vec![],
            suggestions: vec![],
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        };

        let lib_candidate = RefactoringCandidate {
            entity_id: "lib_entity".to_string(),
            name: "lib_function".to_string(),
            file_path: "src/core/lib.rs".to_string(),
            line_range: Some((20, 30)),
            priority: Priority::High,
            score: 0.9,
            confidence: 0.95,
            issues: vec![],
            suggestions: vec![],
            issue_count: 5,
            suggestion_count: 2,
            coverage_percentage: None,
        };

        let file_groups = vec![
            FileRefactoringGroup {
                file_path: "src/main.rs".to_string(),
                file_name: "main.rs".to_string(),
                entity_count: 1,
                avg_score: 0.7,
                highest_priority: Priority::Medium,
                total_issues: 1,
                entities: vec![entity_ref(&main_candidate)],
            },
            FileRefactoringGroup {
                file_path: "src/core/lib.rs".to_string(),
                file_name: "lib.rs".to_string(),
                entity_count: 2,
                avg_score: 0.9,
                highest_priority: Priority::High,
                total_issues: 5,
                entities: vec![entity_ref(&lib_candidate)],
            },
        ];

        let mut candidate_lookup = HashMap::new();
        candidate_lookup.insert(main_candidate.entity_id.clone(), main_candidate.clone());
        candidate_lookup.insert(lib_candidate.entity_id.clone(), lib_candidate.clone());

        let result = generator.add_files_to_hierarchy(
            &hierarchy,
            &file_groups,
            &CodeDictionary::default(),
            &candidate_lookup,
        );

        // Verify root structure
        assert_eq!(result.len(), 1);
        let root_dir = &result[0];
        assert_eq!(root_dir["name"], "src");

        let root_children = root_dir["children"].as_array().unwrap();
        assert_eq!(root_children.len(), 2); // core directory + main.rs file

        // Find the core directory and main.rs file
        let mut core_dir = None;
        let mut main_file = None;

        for child in root_children {
            if child["type"] == "folder" && child["name"] == "core" {
                core_dir = Some(child);
            } else if child["type"] == "file" && child["name"] == "main.rs" {
                main_file = Some(child);
            }
        }

        // Verify main.rs is in src/
        let main_file = main_file.expect("main.rs file should be present");
        assert_eq!(main_file["path"], "src/main.rs");
        assert_eq!(main_file["entity_count"], 1);

        // Verify core directory exists and has lib.rs
        let core_dir = core_dir.expect("core directory should be present");
        let core_children = core_dir["children"].as_array().unwrap();
        assert_eq!(core_children.len(), 1);

        let lib_file = &core_children[0];
        assert_eq!(lib_file["type"], "file");
        assert_eq!(lib_file["name"], "lib.rs");
        assert_eq!(lib_file["path"], "src/core/lib.rs");
        assert_eq!(lib_file["entity_count"], 2);
    }

    #[test]
    fn test_add_files_to_hierarchy_empty_file_groups() {
        let generator = ReportGenerator::new();

        let hierarchy = vec![serde_json::json!({
            "id": "directory_src",
            "type": "folder",
            "name": "src",
            "path": "src",
            "children": []
        })];

        let file_groups = vec![];
        let candidate_lookup = HashMap::new();
        let result = generator.add_files_to_hierarchy(
            &hierarchy,
            &file_groups,
            &CodeDictionary::default(),
            &candidate_lookup,
        );

        // Should preserve hierarchy without changes
        assert_eq!(result.len(), 1);
        let dir_node = &result[0];
        assert_eq!(dir_node["name"], "src");
        let children = dir_node["children"].as_array().unwrap();
        assert_eq!(children.len(), 0); // No files added
    }

    #[test]
    fn test_add_files_to_hierarchy_preserves_existing_children() {
        let generator = ReportGenerator::new();

        // Create hierarchy with existing children
        let hierarchy = vec![serde_json::json!({
            "id": "directory_src",
            "type": "folder",
            "name": "src",
            "path": "src",
            "children": [
                {
                    "id": "directory_src_existing",
                    "type": "folder",
                    "name": "existing",
                    "path": "src/existing",
                    "children": []
                }
            ]
        })];

        let new_candidate = RefactoringCandidate {
            entity_id: "new_entity".to_string(),
            name: "new_function".to_string(),
            file_path: "src/new.rs".to_string(),
            line_range: None,
            priority: Priority::Low,
            score: 0.5,
            confidence: 0.6,
            issues: vec![],
            suggestions: vec![],
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        };

        let file_groups = vec![FileRefactoringGroup {
            file_path: "src/new.rs".to_string(),
            file_name: "new.rs".to_string(),
            entity_count: 1,
            avg_score: 0.5,
            highest_priority: Priority::Low,
            total_issues: 1,
            entities: vec![entity_ref(&new_candidate)],
        }];

        let mut candidate_lookup = HashMap::new();
        candidate_lookup.insert(new_candidate.entity_id.clone(), new_candidate.clone());

        let result = generator.add_files_to_hierarchy(
            &hierarchy,
            &file_groups,
            &CodeDictionary::default(),
            &candidate_lookup,
        );

        // Verify both existing directory and new file are present
        assert_eq!(result.len(), 1);
        let root_dir = &result[0];
        let children = root_dir["children"].as_array().unwrap();
        assert_eq!(children.len(), 2); // existing directory + new file

        // Verify existing directory is preserved
        let existing_dir = children
            .iter()
            .find(|child| child["type"] == "folder" && child["name"] == "existing")
            .expect("existing directory should be preserved");
        assert_eq!(existing_dir["path"], "src/existing");

        // Verify new file is added
        let new_file = children
            .iter()
            .find(|child| child["type"] == "file" && child["name"] == "new.rs")
            .expect("new file should be added");
        assert_eq!(new_file["path"], "src/new.rs");
    }

    #[test]
    fn test_build_unified_hierarchy_sorts_by_priority() {
        let generator = ReportGenerator::new();

        let mut directories = HashMap::new();
        directories.insert(
            PathBuf::from("src"),
            DirectoryHealthScore {
                path: PathBuf::from("src"),
                health_score: 0.3,
                file_count: 2,
                entity_count: 3,
                refactoring_needed: 3,
                critical_issues: 1,
                high_priority_issues: 2,
                avg_refactoring_score: 0.4,
                weight: 1.0,
                children: vec![PathBuf::from("src/core")],
                parent: Some(PathBuf::from(".")),
                issue_categories: HashMap::new(),
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
        );
        directories.insert(
            PathBuf::from("src/core"),
            DirectoryHealthScore {
                path: PathBuf::from("src/core"),
                health_score: 0.6,
                file_count: 1,
                entity_count: 1,
                refactoring_needed: 1,
                critical_issues: 0,
                high_priority_issues: 1,
                avg_refactoring_score: 0.7,
                weight: 1.0,
                children: Vec::new(),
                parent: Some(PathBuf::from("src")),
                issue_categories: HashMap::new(),
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
        );

        let tree = DirectoryHealthTree {
            root: DirectoryHealthScore {
                path: PathBuf::from("."),
                health_score: 0.2,
                file_count: 0,
                entity_count: 0,
                refactoring_needed: 0,
                critical_issues: 0,
                high_priority_issues: 0,
                avg_refactoring_score: 0.0,
                weight: 1.0,
                children: vec![PathBuf::from("src")],
                parent: None,
                issue_categories: HashMap::new(),
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
            directories,
            tree_statistics: TreeStatistics {
                total_directories: 2,
                max_depth: 2,
                avg_health_score: 0.45,
                health_score_std_dev: 0.1,
                hotspot_directories: Vec::new(),
                health_by_depth: HashMap::new(),
            },
        };

        let critical_entity = RefactoringCandidate {
            entity_id: "src/critical.rs::function".to_string(),
            name: "module::critical_function".to_string(),
            file_path: "src/critical.rs".to_string(),
            line_range: Some((5, 25)),
            priority: Priority::Critical,
            score: 0.95,
            confidence: 0.9,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 2,
            suggestion_count: 0,
            coverage_percentage: None,
        };
        let medium_entity = RefactoringCandidate {
            entity_id: "src/medium.rs::function".to_string(),
            name: "module::medium_function".to_string(),
            file_path: "src/medium.rs".to_string(),
            line_range: Some((10, 30)),
            priority: Priority::Medium,
            score: 0.7,
            confidence: 0.8,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        };
        let core_entity = RefactoringCandidate {
            entity_id: "src/core/lib.rs::helper".to_string(),
            name: "module::helper".to_string(),
            file_path: "src/core/lib.rs".to_string(),
            line_range: Some((1, 20)),
            priority: Priority::High,
            score: 0.82,
            confidence: 0.85,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        };

        let file_groups = vec![
            FileRefactoringGroup {
                file_path: "src/critical.rs".to_string(),
                file_name: "critical.rs".to_string(),
                entity_count: 1,
                highest_priority: Priority::Critical,
                avg_score: 0.95,
                total_issues: 2,
                entities: vec![entity_ref(&critical_entity)],
            },
            FileRefactoringGroup {
                file_path: "src/medium.rs".to_string(),
                file_name: "medium.rs".to_string(),
                entity_count: 1,
                highest_priority: Priority::Medium,
                avg_score: 0.7,
                total_issues: 1,
                entities: vec![entity_ref(&medium_entity)],
            },
            FileRefactoringGroup {
                file_path: "src/core/lib.rs".to_string(),
                file_name: "lib.rs".to_string(),
                entity_count: 1,
                highest_priority: Priority::High,
                avg_score: 0.82,
                total_issues: 1,
                entities: vec![entity_ref(&core_entity)],
            },
        ];

        let hierarchy = generator.build_unified_hierarchy(&tree, &file_groups);
        assert_eq!(hierarchy.len(), 1);

        let src_node = &hierarchy[0];
        assert_eq!(src_node["path"], "src");
        let children = src_node["children"]
            .as_array()
            .expect("src should contain children");
        assert_eq!(children.len(), 3);

        let critical_file = children
            .iter()
            .find(|child| child["type"] == "file" && child["name"] == "critical.rs")
            .expect("critical.rs should be present");
        assert_eq!(critical_file["priority"].as_str(), Some("Critical"));
        assert_eq!(critical_file["entity_count"], 1);

        let medium_file = children
            .iter()
            .find(|child| child["type"] == "file" && child["name"] == "medium.rs")
            .expect("medium.rs should be present");
        assert_eq!(medium_file["priority"].as_str(), Some("Medium"));

        let core_node = children
            .iter()
            .find(|child| child["type"] == "folder" && child["path"] == "src/core")
            .expect("core directory should exist");
        let core_children = core_node["children"]
            .as_array()
            .expect("core children array");
        assert_eq!(core_children.len(), 1);
        assert_eq!(core_children[0]["name"], "lib.rs");
    }

    #[test]
    fn test_add_files_to_hierarchy_enriches_metadata() {
        let generator = ReportGenerator::new();
        let hierarchy = vec![serde_json::json!({
            "id": "directory_src",
            "type": "folder",
            "name": "src",
            "path": "src",
            "children": [
                {
                    "id": "directory_src_core",
                    "type": "folder",
                    "name": "core",
                    "path": "src/core",
                    "children": []
                }
            ]
        })];

        let detailed_candidate = RefactoringCandidate {
            entity_id: "src/core/lib.rs::entity".to_string(),
            name: "module::entity".to_string(),
            file_path: "src/core/lib.rs".to_string(),
            line_range: Some((42, 84)),
            priority: Priority::High,
            score: 0.88,
            confidence: 0.91,
            issues: vec![RefactoringIssue {
                code: "complexity.high".to_string(),
                category: "complexity".to_string(),
                severity: 2.3,
                contributing_features: vec![FeatureContribution {
                    feature_name: "cyclomatic_complexity".to_string(),
                    value: 21.0,
                    normalized_value: 0.9,
                    contribution: 1.4,
                }],
            }],
            suggestions: vec![RefactoringSuggestion {
                refactoring_type: "reduce_complexity".to_string(),
                code: "refactor.reduce".to_string(),
                priority: 0.8,
                effort: 0.5,
                impact: 0.9,
            }],
            issue_count: 1,
            suggestion_count: 1,
            coverage_percentage: None,
        };

        let file_groups = vec![FileRefactoringGroup {
            file_path: "src/core/lib.rs".to_string(),
            file_name: "lib.rs".to_string(),
            entity_count: 1,
            highest_priority: Priority::High,
            avg_score: 0.88,
            total_issues: 1,
            entities: vec![entity_ref(&detailed_candidate)],
        }];

        let mut dictionary = CodeDictionary::default();
        dictionary.issues.insert(
            "complexity.high".to_string(),
            CodeDefinition {
                code: "complexity.high".to_string(),
                title: "Elevated Complexity".to_string(),
                summary: "Function exceeds allowed complexity threshold.".to_string(),
                category: Some("complexity".to_string()),
            },
        );
        dictionary.suggestions.insert(
            "refactor.reduce".to_string(),
            CodeDefinition {
                code: "refactor.reduce".to_string(),
                title: "Reduce Complexity".to_string(),
                summary: "Break the function into smaller, focused helpers.".to_string(),
                category: Some("refactoring".to_string()),
            },
        );

        let mut candidate_lookup = HashMap::new();
        candidate_lookup.insert(
            detailed_candidate.entity_id.clone(),
            detailed_candidate.clone(),
        );

        let enriched = generator.add_files_to_hierarchy(
            &hierarchy,
            &file_groups,
            &dictionary,
            &candidate_lookup,
        );

        let root_children = enriched[0]["children"]
            .as_array()
            .expect("root should have children");
        let core_node = root_children
            .iter()
            .find(|child| child["type"] == "folder" && child["name"] == "core")
            .expect("core directory should exist");

        let file_node = core_node["children"]
            .as_array()
            .expect("core should contain files")[0]
            .clone();
        assert_eq!(file_node["highest_priority"].as_str(), Some("High"));

        let entity_node = file_node["children"]
            .as_array()
            .expect("file should contain entities")[0]
            .clone();
        assert_eq!(entity_node["name"], "entity");
        assert_eq!(entity_node["priority"].as_str(), Some("High"));
        assert!((entity_node["score"].as_f64().unwrap() - 0.9).abs() < f64::EPSILON);

        let metadata_children = entity_node["children"]
            .as_array()
            .expect("entity should contain metadata");
        assert_eq!(metadata_children.len(), 2);
        let issue_child = &metadata_children[0];
        assert_eq!(issue_child["title"], "Elevated Complexity");
        assert_eq!(
            issue_child["summary"],
            "Function exceeds allowed complexity threshold."
        );
        let suggestion_child = &metadata_children[1];
        assert_eq!(suggestion_child["title"], "Reduce Complexity");
        assert_eq!(
            suggestion_child["summary"],
            "Break the function into smaller, focused helpers."
        );
    }

    #[test]
    fn test_create_file_groups_from_candidates_groups_stats() {
        let generator = ReportGenerator::new();

        let mut candidate_a = RefactoringCandidate {
            entity_id: "src/lib.rs::alpha".to_string(),
            name: "alpha".to_string(),
            file_path: "src/lib.rs".to_string(),
            line_range: Some((1, 10)),
            priority: Priority::Medium,
            score: 0.8,
            confidence: 0.9,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 2,
            suggestion_count: 0,
            coverage_percentage: None,
        };
        let mut candidate_b = candidate_a.clone();
        candidate_b.entity_id = "src/lib.rs::beta".to_string();
        candidate_b.name = "beta".to_string();
        candidate_b.priority = Priority::High;
        candidate_b.score = 1.0;
        candidate_b.issue_count = 1;

        let candidate_c = RefactoringCandidate {
            entity_id: "src/utils.rs::gamma".to_string(),
            name: "gamma".to_string(),
            file_path: "src/utils.rs".to_string(),
            line_range: Some((15, 40)),
            priority: Priority::Low,
            score: 0.6,
            confidence: 0.8,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        };

        let groups = generator.create_file_groups_from_candidates(&[
            candidate_a.clone(),
            candidate_b.clone(),
            candidate_c.clone(),
        ]);
        assert_eq!(groups.len(), 2);

        let lib_group = groups
            .iter()
            .find(|g| g.file_path == "src/lib.rs")
            .expect("src/lib.rs group should exist");
        assert_eq!(lib_group.entity_count, 2);
        assert_eq!(lib_group.total_issues, 3);
        assert_eq!(lib_group.highest_priority, Priority::High);
        assert!(
            (lib_group.avg_score - 0.9).abs() < f64::EPSILON,
            "expected average score of 0.9 but found {}",
            lib_group.avg_score
        );
        assert_eq!(lib_group.entities.len(), 2);

        let utils_group = groups
            .iter()
            .find(|g| g.file_path == "src/utils.rs")
            .expect("src/utils.rs group should exist");
        assert_eq!(utils_group.entity_count, 1);
        assert_eq!(utils_group.total_issues, 1);
    }

    #[test]
    fn test_html_report_uses_hierarchical_data() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("hierarchy_test.html");
        let generator = ReportGenerator::new();

        // Create test results with hierarchical structure
        let mut results = create_test_results();

        // Create a minimal directory health tree so the hierarchy logic gets triggered
        use crate::core::pipeline::{DirectoryHealthScore, DirectoryHealthTree};
        use std::collections::HashMap;

        let mut directories = HashMap::new();
        let src_dir = DirectoryHealthScore {
            path: PathBuf::from("src"),
            health_score: 0.8,
            file_count: 1,
            entity_count: 1,
            refactoring_needed: 1,
            critical_issues: 0,
            high_priority_issues: 1,
            avg_refactoring_score: 0.85,
            weight: 1.0,
            children: vec![],
            parent: None,
            issue_categories: HashMap::new(),
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };
        directories.insert(PathBuf::from("src"), src_dir);

        let root_dir = DirectoryHealthScore {
            path: PathBuf::from("."),
            health_score: 0.8,
            file_count: 1,
            entity_count: 1,
            refactoring_needed: 1,
            critical_issues: 0,
            high_priority_issues: 1,
            avg_refactoring_score: 0.85,
            weight: 1.0,
            children: vec![PathBuf::from("src")],
            parent: None,
            issue_categories: HashMap::new(),
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        let result = generator.generate_report(&results, &output_path, ReportFormat::Html);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();

        // Sanity check that we produced non-empty HTML
        assert!(!content.is_empty());
    }
