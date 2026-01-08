    use super::*;
    use once_cell::sync::Lazy;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::Duration;
    use tempfile::tempdir;

    static ENV_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
    use crate::core::pipeline::*;
    // Use the 3-field MemoryStats from result_types (for AnalysisStatistics)
    use crate::core::pipeline::results::result_types::MemoryStats;
    use crate::core::scoring::Priority;

    fn oracle_config_fixture(max_tokens: usize) -> OracleConfig {
        OracleConfig {
            api_key: "test-key".to_string(),
            max_tokens,
            api_endpoint: "https://api.example.com".to_string(),
            model: "test-model".to_string(),
            enable_slicing: false,
            slice_token_budget: 200_000,
            slice_model: "gemini-2.0-flash".to_string(),
            slicing_threshold: 300_000,
        }
    }

    fn sample_candidate(
        file_path: &Path,
        entity_name: &str,
        issue_code: &str,
        suggestion_code: &str,
        suggestion_type: &str,
        priority: Priority,
        severity: f64,
        suggestion_priority: f64,
    ) -> RefactoringCandidate {
        RefactoringCandidate {
            entity_id: format!("{}::{entity_name}", file_path.display()),
            name: entity_name.to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            line_range: Some((12, 48)),
            priority,
            score: 70.0 + severity * 20.0,
            confidence: 0.8 + (severity / 5.0).min(0.15),
            issues: vec![RefactoringIssue {
                code: issue_code.to_string(),
                category: "Complexity Hotspot".to_string(),
                severity,
                contributing_features: vec![FeatureContribution {
                    feature_name: "cyclomatic_complexity".to_string(),
                    value: 18.0,
                    normalized_value: 0.9,
                    contribution: 0.45,
                }],
            }],
            suggestions: vec![RefactoringSuggestion {
                refactoring_type: suggestion_type.to_string(),
                code: suggestion_code.to_string(),
                priority: suggestion_priority,
                effort: 0.3,
                impact: 0.7,
            }],
            issue_count: 1,
            suggestion_count: 1,
            coverage_percentage: None,
        }
    }

    fn analysis_results_fixture(project_root: &Path) -> AnalysisResults {
        let lib_path = project_root.join("src/lib.rs");
        let utils_path = project_root.join("src/utils.rs");

        let summary = AnalysisSummary {
            files_processed: 3,
            entities_analyzed: 6,
            refactoring_needed: 2,
            high_priority: 1,
            critical: 1,
            avg_refactoring_score: 72.5,
            code_health_score: 0.42,
            total_files: 3,
            total_entities: 6,
            total_lines_of_code: 420,
            languages: vec!["Rust".to_string()],
            total_issues: 4,
            high_priority_issues: 2,
            critical_issues: 1,
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        let mut code_dictionary = CodeDictionary::default();
        code_dictionary.issues.insert(
            "VX001".to_string(),
            CodeDefinition {
                code: "VX001".to_string(),
                title: "Cyclomatic spike".to_string(),
                summary: "Cyclomatic complexity exceeded preferred range".to_string(),
                category: Some("complexity".to_string()),
            },
        );
        code_dictionary.issues.insert(
            "VX002".to_string(),
            CodeDefinition {
                code: "VX002".to_string(),
                title: "Excessive branching".to_string(),
                summary: "Branching factor suggests decomposition".to_string(),
                category: Some("structure".to_string()),
            },
        );
        code_dictionary.suggestions.insert(
            "RX001".to_string(),
            CodeDefinition {
                code: "RX001".to_string(),
                title: "Extract helper".to_string(),
                summary: "Split logic into dedicated helper functions".to_string(),
                category: Some("refactoring".to_string()),
            },
        );
        code_dictionary.suggestions.insert(
            "RX002".to_string(),
            CodeDefinition {
                code: "RX002".to_string(),
                title: "Simplify branches".to_string(),
                summary: "Reduce branching to clarify business rules".to_string(),
                category: Some("refactoring".to_string()),
            },
        );

        AnalysisResults {
            project_root: std::path::PathBuf::new(),
            summary,
            normalized: None,
            passes: StageResultsBundle::disabled(),
            refactoring_candidates: vec![
                sample_candidate(
                    &lib_path,
                    "crate::lib::hotspot",
                    "VX001",
                    "RX001",
                    "Extract Method",
                    Priority::Critical,
                    0.92,
                    0.9,
                ),
                sample_candidate(
                    &utils_path,
                    "crate::utils::helper",
                    "VX002",
                    "RX002",
                    "Simplify Branches",
                    Priority::High,
                    0.78,
                    0.7,
                ),
            ],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(2),
                avg_file_processing_time: Duration::from_millis(120),
                avg_entity_processing_time: Duration::from_millis(45),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 512_000,
                    final_memory_bytes: 256_000,
                    efficiency_score: 0.82,
                },
            },
            clone_analysis: None,
            coverage_packs: Vec::new(),
            warnings: Vec::new(),
            health_metrics: Some(HealthMetrics {
                overall_health_score: 58.0,
                maintainability_score: 52.0,
                technical_debt_ratio: 71.0,
                complexity_score: 83.0,
                structure_quality_score: 45.0,
                doc_health_score: 100.0,
            }),
            code_dictionary,
            documentation: None,
            directory_health: HashMap::new(),
            file_health: HashMap::new(),
            entity_health: HashMap::new(),
        }
    }

    #[test]
    fn test_oracle_config_creation() {
        let config = OracleConfig {
            api_key: "test-key".to_string(),
            max_tokens: 100_000,
            api_endpoint: "https://api.example.com".to_string(),
            model: "test-model".to_string(),
            enable_slicing: true,
            slice_token_budget: 200_000,
            slice_model: "gemini-2.0-flash".to_string(),
            slicing_threshold: 300_000,
        };

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.max_tokens, 100_000);
        assert_eq!(config.api_endpoint, "https://api.example.com");
        assert_eq!(config.model, "test-model");
        assert!(config.enable_slicing);
        assert_eq!(config.slice_token_budget, 200_000);
    }

    #[test]
    fn test_oracle_config_from_env_missing_key() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("GEMINI_API_KEY");

        let result = OracleConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("GEMINI_API_KEY"));
    }

    #[test]
    fn test_oracle_config_from_env_with_key() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("GEMINI_API_KEY", "test-api-key");

        let result = OracleConfig::from_env();
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.api_key, "test-api-key");
        assert_eq!(config.max_tokens, 400_000);
        assert_eq!(config.model, "gemini-3-flash-preview");
        assert!(config
            .api_endpoint
            .contains("generativelanguage.googleapis.com"));

        // Clean up
        std::env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn test_oracle_config_with_max_tokens() {
        let config = oracle_config_fixture(100).with_max_tokens(50_000);
        assert_eq!(config.max_tokens, 50_000);
    }

    #[test]
    fn test_refactoring_oracle_creation() {
        let config = oracle_config_fixture(100_000);
        let oracle = RefactoringOracle::new(config);
        assert_eq!(oracle.config.api_key, "test-key");
    }

    #[test]
    fn test_is_test_file_patterns() {
        // Test directory patterns
        assert!(is_test_file("src/test/mod.rs"));
        assert!(is_test_file("tests/integration.rs"));
        assert!(is_test_file("src/tests/unit.py"));

        // Test file name patterns
        assert!(is_test_file("src/module_test.rs"));
        assert!(is_test_file("src/component.test.js"));
        assert!(is_test_file("src/service.spec.ts"));
        assert!(is_test_file("test_module.py"));
        assert!(is_test_file("src/TestClass.java"));
        assert!(is_test_file("conftest.py"));

        // Non-test files
        assert!(!is_test_file("src/main.rs"));
        assert!(!is_test_file("src/lib.rs"));
        assert!(!is_test_file("src/config.py"));
        assert!(!is_test_file("src/api/mod.rs"));
    }

    #[test]
    fn test_calculate_file_priority() {
        // High priority files
        assert!(calculate_file_priority("src/main.rs", "rs", 1000) > 3.0);
        assert!(calculate_file_priority("src/lib.rs", "rs", 1000) > 3.0);
        assert!(calculate_file_priority("src/core/mod.rs", "rs", 1000) > 3.0);

        // Config and API files get boost
        assert!(calculate_file_priority("src/config.rs", "rs", 1000) > 2.0);
        assert!(calculate_file_priority("src/api/mod.rs", "rs", 1000) > 2.0);

        // Language priorities
        assert!(
            calculate_file_priority("src/module.rs", "rs", 1000)
                > calculate_file_priority("src/module.py", "py", 1000)
        );
        assert!(
            calculate_file_priority("src/module.py", "py", 1000)
                > calculate_file_priority("src/module.c", "c", 1000)
        );

        // Size penalties
        assert!(
            calculate_file_priority("src/large.rs", "rs", 100_000)
                < calculate_file_priority("src/small.rs", "rs", 1000)
        );

        // Test file penalty
        assert!(
            calculate_file_priority("src/module.rs", "rs", 1000)
                > calculate_file_priority("src/module_test.rs", "rs", 1000)
        );
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape(""), "");
        assert_eq!(html_escape("hello world"), "hello world");
        assert_eq!(html_escape("hello & world"), "hello &amp; world");
        assert_eq!(html_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(html_escape("'single'"), "&#x27;single&#x27;");
        assert_eq!(
            html_escape("<script>alert('hello');</script>"),
            "&lt;script&gt;alert(&#x27;hello&#x27;);&lt;/script&gt;"
        );
    }

    #[test]
    fn test_file_candidate_creation() {
        let candidate = FileCandidate {
            path: "src/test.rs".to_string(),
            content: "fn main() {}".to_string(),
            tokens: 100,
            priority: 2.5,
            file_type: "rs".to_string(),
        };

        assert_eq!(candidate.path, "src/test.rs");
        assert_eq!(candidate.content, "fn main() {}");
        assert_eq!(candidate.tokens, 100);
        assert_eq!(candidate.priority, 2.5);
        assert_eq!(candidate.file_type, "rs");
    }

    #[test]
    fn test_codebase_assessment_structure() {
        let assessment = CodebaseAssessment {
            summary: Some("The codebase follows a pipeline architecture with clear separation.".to_string()),
            architectural_narrative: None,
            architectural_style: Some("Pipeline Architecture with Modular Detectors".to_string()),
            strengths: vec!["Good modularity".to_string()],
            issues: vec![
                "Configuration complexity".to_string(),
                "Module boundaries".to_string(),
            ],
        };

        assert!(assessment.get_summary().contains("pipeline"));
        assert!(assessment.architectural_style.as_ref().unwrap().contains("Pipeline"));
        assert_eq!(assessment.issues.len(), 2);
    }

    #[test]
    fn test_refactoring_task_structure() {
        let task = RefactoringTask {
            id: "T1".to_string(),
            title: "Split large file".to_string(),
            description: "Break down monolithic module".to_string(),
            category: "C2".to_string(), // maintainability
            files: vec!["src/large.rs".to_string()],
            risk: Some("R2".to_string()),
            risk_level: None,
            impact: Some("I3".to_string()),
            effort: Some("E2".to_string()),
            mitigation: Some("Use feature flags".to_string()),
            required: Some(true),
            depends_on: vec![],
            benefits: vec!["Improved maintainability".to_string()],
        };

        assert_eq!(task.id, "T1");
        assert_eq!(task.category, "C2");
        assert_eq!(task.get_risk(), Some("R2"));
        assert_eq!(task.impact, Some("I3".to_string()));
        assert_eq!(task.effort, Some("E2".to_string()));
        assert!(task.required.unwrap_or(false));
        assert!(task.depends_on.is_empty());
        assert_eq!(task.files.len(), 1);
        assert_eq!(task.benefits.len(), 1);
    }

    #[test]
    fn test_refactoring_roadmap_structure() {
        let roadmap = RefactoringRoadmap { tasks: vec![] };
        assert!(roadmap.tasks.is_empty());
    }

    #[test]
    fn test_oracle_response_structure() {
        let response = RefactoringOracleResponse {
            assessment: CodebaseAssessment {
                summary: Some("The codebase is well-structured.".to_string()),
                architectural_narrative: None,
                architectural_style: None,
                strengths: vec!["Good modularity".to_string()],
                issues: vec!["Testing".to_string()],
            },
            tasks: vec![],
            refactoring_roadmap: None,
        };

        assert!(response
            .assessment
            .get_summary()
            .contains("well-structured"));
        assert!(response.all_tasks().is_empty());
    }

    #[test]
    fn test_condense_analysis_results() {
        use std::collections::HashMap;
        use std::time::Duration;

        let oracle = RefactoringOracle::new(oracle_config_fixture(100_000));

        let results = AnalysisResults {
            project_root: std::path::PathBuf::new(),
            summary: AnalysisSummary {
                code_health_score: 75.5,
                files_processed: 10,
                entities_analyzed: 50,
                refactoring_needed: 5,
                high_priority: 2,
                critical: 1,
                avg_refactoring_score: 3.2,
                total_files: 10,
                total_entities: 50,
                total_lines_of_code: 1_500,
                languages: vec!["Rust".to_string()],
                total_issues: 3,
                high_priority_issues: 2,
                critical_issues: 1,
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
            normalized: None,
            passes: StageResultsBundle::disabled(),
            refactoring_candidates: vec![],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(30),
                avg_file_processing_time: Duration::from_millis(500),
                avg_entity_processing_time: Duration::from_millis(100),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 1000000,
                    final_memory_bytes: 800000,
                    efficiency_score: 0.8,
                },
            },
            clone_analysis: None,
            coverage_packs: vec![],
            warnings: vec![],
            health_metrics: None,
            code_dictionary: CodeDictionary::default(),
            documentation: None,
            directory_health: HashMap::new(),
            file_health: HashMap::new(),
            entity_health: HashMap::new(),
        };

        let condensed = oracle.condense_analysis_results(&results);
        assert!(condensed.contains("75.5"));
        assert!(condensed.contains("files_analyzed"));
        assert!(condensed.contains("health_score"));
    }

    #[test]
    fn test_token_budget_constants() {
        assert_eq!(VALKNUT_OUTPUT_TOKEN_BUDGET, 70_000);
    }

    #[test]
    fn test_gemini_request_structure() {
        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: "test content".to_string(),
                }],
            }],
            generation_config: GeminiGenerationConfig {
                temperature: 0.2,
                top_k: 40,
                top_p: 0.95,
                max_output_tokens: 8192,
                response_mime_type: "application/json".to_string(),
            },
        };

        assert_eq!(request.contents.len(), 1);
        assert_eq!(request.generation_config.temperature, 0.2);
        assert_eq!(
            request.generation_config.response_mime_type,
            "application/json"
        );
    }

    #[test]
    fn test_gemini_response_structure() {
        let response = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiResponseContent {
                    parts: vec![GeminiResponsePart {
                        text: "response text".to_string(),
                    }],
                },
            }],
        };

        assert_eq!(response.candidates.len(), 1);
        assert_eq!(
            response.candidates[0].content.parts[0].text,
            "response text"
        );
    }

    #[test]
    fn truncate_hint_adds_ellipsis_for_long_labels() {
        let short = truncate_hint("High risk", 20);
        assert_eq!(short, "High risk");

        let long = truncate_hint("VeryLongRefactorHintIdentifierThatShouldBeTrimmed", 16);
        assert!(long.ends_with('…'));
        assert!(long.chars().count() <= 16);
    }

    #[test]
    fn normalize_path_for_key_flattens_backslashes() {
        assert_eq!(
            normalize_path_for_key(r"src\module\lib.rs"),
            "src/module/lib.rs"
        );
        assert_eq!(normalize_path_for_key(""), "");
    }

    #[test]
    fn build_refactor_hints_normalizes_paths_and_limits_size() {
        let project = tempdir().unwrap();
        let root = project.path().join("workspace");
        fs::create_dir_all(root.join("src")).unwrap();
        let results = analysis_results_fixture(&root);
        let hints = build_refactor_hints(&results, &root);

        let entry = hints
            .get("src/lib.rs")
            .expect("expected lib.rs hints entry");
        assert!(
            entry.iter().all(|hint| hint.len() <= 60),
            "hint should be truncated to configured length"
        );
        assert!(
            entry.iter().any(|hint| hint.contains("CH")),
            "category abbreviation should be included"
        );
    }

    #[tokio::test]
    async fn create_codebase_bundle_includes_readme_and_skips_large_files() {
        let project = tempdir().unwrap();
        let root = project.path().join("workspace");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("README.md"),
            "# Sample Project\n\nImportant overview.",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub fn compute(value: i32) -> i32 { value * 2 }\n",
        )
        .unwrap();
        fs::write(
            root.join("src/utils.rs"),
            "pub fn helper(flag: bool) -> bool { if flag { !flag } else { flag } }\n",
        )
        .unwrap();
        let large_body = "fn enormous_task() {}\n".repeat(400);
        fs::write(root.join("src/huge.rs"), large_body).unwrap();

        let results = analysis_results_fixture(&root);
        let config = oracle_config_fixture(180);
        let builder = BundleBuilder::new(&config);

        let bundle = builder
            .create_codebase_bundle(&root, &results)
            .await
            .expect("bundle creation");

        assert!(bundle.contains("README.md"));
        assert!(bundle.contains("src/lib.rs"));
        assert!(
            !bundle.contains("src/huge.rs"),
            "large file should be skipped when exceeding budget"
        );
        assert!(
            bundle.contains("CH 92%") && bundle.contains("EM"),
            "refactor hints should be embedded in tuple labels"
        );
    }

    #[test]
    fn condense_analysis_results_with_budget_handles_limits_and_health_section() {
        let project = tempdir().unwrap();
        let root = project.path().join("workspace");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "fn demo() {}\n").unwrap();
        fs::write(root.join("src/utils.rs"), "fn helper() {}\n").unwrap();

        let results = analysis_results_fixture(&root);

        let limited = condense_analysis_results_with_budget(&results, 90)
            .expect("condense with tight budget");
        assert!(
            !limited.contains("crate::lib::hotspot") && !limited.contains("crate::utils::helper"),
            "candidates should be omitted when budget is exhausted before listing them"
        );

        let mut expanded_results = analysis_results_fixture(&root);
        expanded_results
            .refactoring_candidates
            .push(sample_candidate(
                &root.join("src/core.rs"),
                "crate::core::planner",
                "VX002",
                "RX002",
                "Simplify Branches",
                Priority::High,
                0.68,
                0.6,
            ));

        let expanded = condense_analysis_results_with_budget(&expanded_results, 420)
            .expect("condense with ample budget");
        // Health section is optional after normalization removal
        // ensure condensed text still produced
        assert!(!expanded.is_empty());
        assert!(
            expanded.contains("helper"),
            "refactoring candidate names should appear when budget allows"
        );
    }
