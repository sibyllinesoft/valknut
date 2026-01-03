    use super::*;
    use crate::cli::args::{DocAuditArgs, DocAuditFormat, McpManifestArgs, McpStdioArgs};
    use crate::cli::config_builder::apply_performance_profile;
    use anyhow::Result;
    use gag::BufferRedirect;
    use serial_test::serial;
    use std::collections::HashMap;
    use std::time::Duration;
    use std::{
        env, fs,
        io::{Read, Write},
    };
    use tempfile::{NamedTempFile, TempDir};
    use tokio::runtime::Runtime;
    use valknut_rs::api::results::{
        AnalysisResults, AnalysisStatistics, AnalysisSummary, CloneAnalysisResults,
        FeatureContribution, MemoryStats, RefactoringIssue, RefactoringSuggestion,
    };
    use valknut_rs::core::config::{CoverageConfig, ValknutConfig};
    use valknut_rs::core::pipeline::{
        CodeDictionary, HealthMetrics, QualityGateConfig, QualityGateResult, QualityGateViolation,
    };
    use valknut_rs::oracle::{
        CodebaseAssessment, RefactoringOracleResponse, RefactoringRoadmap, RefactoringTask,
    };

    /// Handle quality gate evaluation for JSON results (test helper).
    async fn handle_quality_gates(
        args: &AnalyzeArgs,
        result: &serde_json::Value,
    ) -> anyhow::Result<QualityGateResult> {
        let quality_gate_config = build_quality_gate_config(args);
        let mut violations = Vec::new();

        let summary = result
            .get("summary")
            .ok_or_else(|| anyhow::anyhow!("Summary not found in analysis result"))?;

        let total_issues = summary
            .get("total_issues")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        if quality_gate_config.max_critical_issues > 0
            && total_issues > quality_gate_config.max_critical_issues
        {
            violations.push(QualityGateViolation {
                rule_name: "Total Issues Count".to_string(),
                current_value: total_issues as f64,
                threshold: quality_gate_config.max_critical_issues as f64,
                description: format!(
                    "Total issues ({}) exceeds maximum allowed ({})",
                    total_issues, quality_gate_config.max_critical_issues
                ),
                severity: if total_issues > quality_gate_config.max_critical_issues * 2 {
                    "Critical".to_string()
                } else {
                    "High".to_string()
                },
                affected_files: Vec::new(),
                recommended_actions: vec!["Review and address high-priority issues".to_string()],
            });
        }

        if let Some(health_metrics) = result.get("health_metrics") {
            if let Some(overall_health) = health_metrics
                .get("overall_health_score")
                .and_then(|v| v.as_f64())
            {
                if overall_health < quality_gate_config.min_maintainability_score {
                    violations.push(QualityGateViolation {
                        rule_name: "Overall Health Score".to_string(),
                        current_value: overall_health,
                        threshold: quality_gate_config.min_maintainability_score,
                        description: format!(
                            "Health score ({:.1}) is below minimum required ({:.1})",
                            overall_health, quality_gate_config.min_maintainability_score
                        ),
                        severity: if overall_health
                            < quality_gate_config.min_maintainability_score - 20.0
                        {
                            "Blocker".to_string()
                        } else {
                            "Critical".to_string()
                        },
                        affected_files: Vec::new(),
                        recommended_actions: vec![
                            "Improve code structure and reduce technical debt".to_string()
                        ],
                    });
                }
            }

            if let Some(complexity_score) = health_metrics
                .get("complexity_score")
                .and_then(|v| v.as_f64())
            {
                if complexity_score > quality_gate_config.max_complexity_score {
                    violations.push(QualityGateViolation {
                        rule_name: "Complexity Score".to_string(),
                        current_value: complexity_score,
                        threshold: quality_gate_config.max_complexity_score,
                        description: format!(
                            "Complexity score ({:.1}) exceeds maximum allowed ({:.1})",
                            complexity_score, quality_gate_config.max_complexity_score
                        ),
                        severity: if complexity_score > quality_gate_config.max_complexity_score + 10.0
                        {
                            "Critical".to_string()
                        } else {
                            "High".to_string()
                        },
                        affected_files: Vec::new(),
                        recommended_actions: vec![
                            "Simplify complex functions and reduce nesting".to_string()
                        ],
                    });
                }
            }

            if let Some(debt_ratio) = health_metrics
                .get("technical_debt_ratio")
                .and_then(|v| v.as_f64())
            {
                if debt_ratio > quality_gate_config.max_technical_debt_ratio {
                    violations.push(QualityGateViolation {
                        rule_name: "Technical Debt Ratio".to_string(),
                        current_value: debt_ratio,
                        threshold: quality_gate_config.max_technical_debt_ratio,
                        description: format!(
                            "Technical debt ratio ({:.1}%) exceeds maximum allowed ({:.1}%)",
                            debt_ratio, quality_gate_config.max_technical_debt_ratio
                        ),
                        severity: if debt_ratio > quality_gate_config.max_technical_debt_ratio + 20.0 {
                            "Critical".to_string()
                        } else {
                            "High".to_string()
                        },
                        affected_files: Vec::new(),
                        recommended_actions: vec!["Refactor code to reduce technical debt".to_string()],
                    });
                }
            }
        }

        let passed = violations.is_empty();
        let overall_score = result
            .get("health_metrics")
            .and_then(|hm| hm.get("overall_health_score"))
            .and_then(|v| v.as_f64())
            .unwrap_or(50.0);

        Ok(QualityGateResult {
            passed,
            violations,
            overall_score,
        })
    }

    struct ColorOverrideGuard {
        previous: Option<String>,
    }

    impl ColorOverrideGuard {
        fn new() -> Self {
            let previous = env::var("NO_COLOR").ok();
            env::set_var("NO_COLOR", "1");
            Self { previous }
        }
    }

    impl Drop for ColorOverrideGuard {
        fn drop(&mut self) {
            if let Some(ref value) = self.previous {
                env::set_var("NO_COLOR", value);
            } else {
                env::remove_var("NO_COLOR");
            }
        }
    }

    fn capture_stdout<F: FnOnce()>(action: F) -> String {
        let mut buffer = Vec::new();
        {
            let _color_guard = ColorOverrideGuard::new();
            if let Ok(mut redirect) = BufferRedirect::stdout() {
                action();
                std::io::stdout().flush().expect("flush stdout");
                redirect
                    .read_to_end(&mut buffer)
                    .expect("read captured stdout");
            } else {
                // If stdout is already redirected (rare in concurrent tests), just run the action.
                action();
            }
        }
        String::from_utf8(buffer).expect("stdout should be valid utf8")
    }

    fn sample_candidate(path: &str, priority: Priority, score: f64) -> RefactoringCandidate {
        RefactoringCandidate {
            entity_id: format!("{path}::entity"),
            name: "entity".to_string(),
            file_path: path.to_string(),
            line_range: Some((1, 20)),
            priority,
            score,
            confidence: 0.85,
            issues: vec![RefactoringIssue {
                code: "CMPLX".to_string(),
                category: "complexity".to_string(),
                severity: 1.2,
                contributing_features: vec![FeatureContribution {
                    feature_name: "cyclomatic_complexity".to_string(),
                    value: 18.0,
                    normalized_value: 0.7,
                    contribution: 1.3,
                }],
            }],
            suggestions: vec![RefactoringSuggestion {
                refactoring_type: "extract_method".to_string(),
                code: "XTRMTH".to_string(),
                priority: 0.9,
                effort: 0.4,
                impact: 0.85,
            }],
            issue_count: 1,
            suggestion_count: 1,
            coverage_percentage: None,
        }
    }

    // Helper function to create default AnalyzeArgs for tests
    fn create_default_analyze_args() -> AnalyzeArgs {
        AnalyzeArgs {
            paths: vec![PathBuf::from("test")],
            out: PathBuf::from("output"),
            format: OutputFormat::Json,
            config: None,
            quiet: false,
            profile: PerformanceProfile::Balanced,
            quality_gate: QualityGateArgs {
                quality_gate: false,
                fail_on_issues: false,
                max_complexity: None,
                min_health: None,
                min_doc_health: None,
                max_debt: None,
                min_maintainability: None,
                max_issues: None,
                max_critical: None,
                max_high_priority: None,
            },
            clone_detection: CloneDetectionArgs {
                semantic_clones: false,
                strict_dedupe: false,
                denoise: false,
                min_function_tokens: None,
                min_match_tokens: None,
                require_blocks: None,
                similarity: None,
                denoise_dry_run: false,
            },
            advanced_clone: AdvancedCloneArgs {
                no_auto: false,
                loose_sweep: false,
                rarity_weighting: false,
                structural_validation: false,
                apted_verify: false,
                apted_max_nodes: None,
                apted_max_pairs: None,
                no_apted_verify: false,
                live_reach_boost: false,
                ast_weight: None,
                pdg_weight: None,
                emb_weight: None,
                io_mismatch_penalty: None,
                quality_target: None,
                sample_size: None,
                min_saved_tokens: None,
                min_rarity_gain: None,
            },
            coverage: CoverageArgs {
                no_coverage: false,
                coverage_file: None,
                no_coverage_auto_discover: false,
                coverage_max_age_days: None,
            },
            analysis_control: AnalysisControlArgs {
                no_complexity: false,
                no_structure: false,
                no_refactoring: false,
                no_impact: false,
                no_lsh: false,
                cohesion: false,
            },
            cohesion: CohesionArgs {
                cohesion_min_score: None,
                cohesion_min_doc_alignment: None,
                cohesion_outlier_percentile: None,
            },
            ai_features: AIFeaturesArgs {
                oracle: false,
                oracle_max_tokens: None,
                oracle_slice_budget: None,
                no_oracle_slicing: false,
                oracle_slicing_threshold: None,
                oracle_dry_run: false,
            },
        }
    }

    fn create_doc_args(root: PathBuf) -> DocAuditArgs {
        DocAuditArgs {
            root,
            complexity_threshold: usize::MAX,
            max_readme_commits: usize::MAX,
            strict: false,
            format: DocAuditFormat::Text,
            ignore_dir: vec![],
            ignore_suffix: vec![],
            ignore: vec![],
            config: None,
        }
    }

    struct DirGuard {
        original: PathBuf,
    }

    impl DirGuard {
        fn change_to(path: &Path) -> Self {
            let original = env::current_dir().expect("read current dir");
            env::set_current_dir(path).expect("set current dir");
            Self { original }
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.original);
        }
    }

    fn create_sample_analysis_project() -> TempDir {
        let project = TempDir::new().expect("temp project");
        let root = project.path();

        fs::write(
            root.join("analytics.py"),
            r#"
def compute(values):
    total = sum(values)
    return total / max(len(values), 1)

def duplicate(values):
    return [value for value in values if value > 0]
"#,
        )
        .expect("write python file");

        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn helper(value: i32) -> i32 {
    if value > 0 {
        value + 1
    } else {
        value - 1
    }
}
"#,
        )
        .expect("write rust file");

        fs::write(
            root.join("metrics.ts"),
            r#"
export function accumulate(values: number[]): number {
    return values.reduce((sum, value) => sum + value, 0);
}
"#,
        )
        .expect("write ts file");

        project
    }

    fn write_lcov_fixture(root: &Path) -> PathBuf {
        let coverage_dir = root.join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        let file = coverage_dir.join("coverage.lcov");
        fs::write(
            &file,
            "TN:valknut\nSF:src/lib.rs\nFN:2,helper\nFNF:1\nFNH:1\nFNDA:4,helper\nDA:2,4\nDA:3,4\nDA:4,4\nDA:5,4\nLF:4\nLH:4\nend_of_record\n",
        )
        .expect("write coverage");
        file
    }

    fn sample_analysis_results() -> AnalysisResults {
        let candidate = sample_candidate("src/lib.rs", Priority::High, 2.5);

        AnalysisResults {
            summary: AnalysisSummary {
                files_processed: 1,
                entities_analyzed: 1,
                refactoring_needed: 1,
                high_priority: 1,
                critical: 0,
                avg_refactoring_score: 0.75,
                code_health_score: 0.65,
                total_files: 1,
                total_entities: 1,
                total_lines_of_code: 120,
                languages: vec!["Rust".to_string()],
                total_issues: 1,
                high_priority_issues: 1,
                critical_issues: 0,
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
            normalized: None,
            passes: valknut_rs::api::results::StageResultsBundle::disabled(),
            refactoring_candidates: vec![candidate],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_millis(25),
                avg_file_processing_time: Duration::from_millis(25),
                avg_entity_processing_time: Duration::from_millis(25),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 2048,
                    final_memory_bytes: 1024,
                    efficiency_score: 0.9,
                },
            },
            health_metrics: Some(HealthMetrics {
                overall_health_score: 72.0,
                maintainability_score: 70.0,
                technical_debt_ratio: 25.0,
                complexity_score: 45.0,
                structure_quality_score: 78.0,
                doc_health_score: 100.0,
            }),
            clone_analysis: None,
            coverage_packs: Vec::new(),
            warnings: Vec::new(),
            code_dictionary: CodeDictionary::default(),
            documentation: None,
        }
    }

    fn sample_oracle_response() -> RefactoringOracleResponse {
        RefactoringOracleResponse {
            assessment: CodebaseAssessment {
                summary: Some("The codebase follows a modular design with room for improvement in clone density.".to_string()),
                architectural_narrative: None,
                architectural_style: Some("Modular Architecture".to_string()),
                strengths: vec!["Good separation of concerns".to_string()],
                issues: vec!["Clone density".to_string()],
            },
            tasks: vec![RefactoringTask {
                id: "T1".to_string(),
                title: "Extract helper utilities".to_string(),
                description: "Split monolithic helper into focused modules.".to_string(),
                category: "C2".to_string(),
                files: vec!["src/lib.rs".to_string()],
                risk: Some("R1".to_string()),
                risk_level: None,
                impact: Some("I3".to_string()),
                effort: Some("E2".to_string()),
                mitigation: None,
                required: Some(true),
                depends_on: vec![],
                benefits: vec!["Improved readability".to_string()],
            }],
            refactoring_roadmap: None,
        }
    }

    #[test]
    fn output_format_machine_readable_detection() {
        assert!(OutputFormat::Json.is_machine_readable());
        assert!(OutputFormat::Jsonl.is_machine_readable());
        assert!(OutputFormat::Yaml.is_machine_readable());
        assert!(OutputFormat::Csv.is_machine_readable());
        assert!(OutputFormat::Sonar.is_machine_readable());
        assert!(OutputFormat::CiSummary.is_machine_readable());
        assert!(!OutputFormat::Markdown.is_machine_readable());
        assert!(!OutputFormat::Html.is_machine_readable());
        assert!(!OutputFormat::Pretty.is_machine_readable());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_respects_quiet_mode() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = true;
        args.format = OutputFormat::Json;

        let result = sample_analysis_results();
        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("json report generation should succeed");

        assert!(temp.path().join("analysis-results.json").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_html_with_ai_data() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Html;

        let result = sample_analysis_results();
        let oracle = sample_oracle_response();

        generate_reports_with_oracle(&result, &Some(oracle), &args)
            .await
            .expect("html report generation should succeed");

        let html_count = fs::read_dir(temp.path())
            .expect("read output dir")
            .filter(|entry| {
                entry
                    .as_ref()
                    .ok()
                    .and_then(|e| e.path().extension().map(|ext| ext == "html"))
                    .unwrap_or(false)
            })
            .count();

        assert!(
            html_count > 0,
            "expected at least one html report in {:?}",
            temp.path()
        );
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_markdown() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Markdown;

        let result = sample_analysis_results();

        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("markdown report generation should succeed");

        assert!(temp.path().join("team-report.md").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_csv() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Csv;

        let result = sample_analysis_results();

        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("csv report generation should succeed");

        assert!(temp.path().join("analysis-data.csv").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_yaml() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Yaml;

        let result = sample_analysis_results();

        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("yaml report generation should succeed");

        assert!(temp.path().join("analysis-results.yaml").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_jsonl() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Jsonl;

        let result = sample_analysis_results();

        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("jsonl report generation should succeed");

        assert!(temp.path().join("analysis-results.jsonl").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_writes_sonar() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::Sonar;

        let result = sample_analysis_results();

        generate_reports_with_oracle(&result, &None, &args)
            .await
            .expect("sonar report generation should succeed");

        assert!(temp.path().join("sonarqube-issues.json").exists());
    }

    #[tokio::test]
    async fn generate_reports_with_oracle_combines_for_ci_summary() {
        let temp = TempDir::new().expect("temp dir");
        let mut args = create_default_analyze_args();
        args.out = temp.path().to_path_buf();
        args.quiet = false;
        args.format = OutputFormat::CiSummary;
        args.ai_features.oracle = true;

        let result = sample_analysis_results();
        let oracle = sample_oracle_response();

        generate_reports_with_oracle(&result, &Some(oracle), &args)
            .await
            .expect("ci summary should fall back to combined json");

        let combined_path = temp.path().join("analysis-results.json");
        assert!(combined_path.exists());
        let contents = fs::read_to_string(combined_path).expect("read combined output");
        assert!(
            contents.contains("oracle_refactoring_plan"),
            "combined report should include oracle data"
        );
    }

    #[test]
    fn evaluate_quality_gates_disabled_returns_health_score() {
        let result = sample_analysis_results();
        let expected = result
            .health_metrics
            .as_ref()
            .map(|m| m.overall_health_score)
            .unwrap();

        let config = QualityGateConfig {
            enabled: false,
            ..Default::default()
        };

        let gate = evaluate_quality_gates(&result, &config, false)
            .expect("quality gate evaluation succeeds");

        assert!(gate.passed);
        assert!(gate.violations.is_empty());
        assert_eq!(gate.overall_score, expected);
    }

    #[test]
    fn evaluate_quality_gates_reports_violations() {
        let mut result = sample_analysis_results();
        result.summary.critical = 3;
        result.summary.high_priority = 4;
        result.summary.total_issues = 7;
        if let Some(metrics) = result.health_metrics.as_mut() {
            metrics.complexity_score = 88.0;
            metrics.technical_debt_ratio = 65.0;
            metrics.maintainability_score = 52.0;
            metrics.doc_health_score = 10.0;
        }

        let config = QualityGateConfig {
            enabled: true,
            min_health_score: QualityGateConfig::default().min_health_score,
            min_doc_health_score: 50.0,
            max_complexity_score: 55.0,
            max_technical_debt_ratio: 25.0,
            min_maintainability_score: 85.0,
            max_critical_issues: 1,
            max_high_priority_issues: 2,
        };

        let gate = evaluate_quality_gates(&result, &config, false)
            .expect("quality gate evaluation succeeds");

        assert!(!gate.passed);
        assert!(!gate.violations.is_empty());

        let rule_names: Vec<_> = gate
            .violations
            .iter()
            .map(|v| v.rule_name.as_str())
            .collect();
        assert!(rule_names.contains(&"Complexity Threshold"));
        assert!(rule_names.contains(&"Technical Debt Ratio"));
        assert!(rule_names.contains(&"Maintainability Score"));
        assert!(rule_names.contains(&"Critical Issues"));
        assert!(rule_names.contains(&"High Priority Issues"));

        assert!(
            gate.violations
                .iter()
                .all(|v| !v.recommended_actions.is_empty()),
            "violations should include actionable guidance"
        );
    }

    #[test]
    fn evaluate_quality_gates_handles_missing_metrics_when_verbose() {
        let mut result = sample_analysis_results();
        result.health_metrics = None;

        let config = QualityGateConfig {
            enabled: true,
            min_health_score: QualityGateConfig::default().min_health_score,
            min_doc_health_score: 0.0,
            max_complexity_score: 90.0,
            max_technical_debt_ratio: 90.0,
            min_maintainability_score: 10.0,
            max_critical_issues: 10,
            max_high_priority_issues: 10,
        };

        let gate = evaluate_quality_gates(&result, &config, true)
            .expect("quality gate evaluation succeeds");

        assert!(gate.passed);
        assert!(gate.violations.is_empty());
        assert!(
            (gate.overall_score - (result.summary.code_health_score * 100.0)).abs() < f64::EPSILON
        );
    }

    #[tokio::test]
    #[serial]
    async fn run_oracle_analysis_returns_none_without_api_key() {
        // Ensure GEMINI_API_KEY is unset for this test
        std::env::remove_var("GEMINI_API_KEY");

        let project = create_sample_analysis_project();
        let mut args = create_default_analyze_args();
        args.paths = vec![project.path().to_path_buf()];
        args.ai_features.oracle = true;

        let result = run_oracle_analysis(
            &[project.path().to_path_buf()],
            &sample_analysis_results(),
            &args,
        )
        .await
        .expect("oracle analysis should not error when key missing");

        assert!(
            result.is_none(),
            "Oracle should be skipped when GEMINI_API_KEY is absent"
        );
    }

    #[tokio::test]
    #[serial]
    async fn run_oracle_analysis_handles_generation_error() {
        // Provide a dummy API key to exercise request failure path
        std::env::set_var("GEMINI_API_KEY", "test-api-key");

        let project = create_sample_analysis_project();
        let mut args = create_default_analyze_args();
        args.paths = vec![project.path().to_path_buf()];
        args.ai_features.oracle = true;
        args.ai_features.oracle_max_tokens = Some(256);

        let oracle_result = run_oracle_analysis(
            &[project.path().to_path_buf()],
            &sample_analysis_results(),
            &args,
        )
        .await
        .expect("oracle analysis should gracefully handle request failures");

        assert!(
            oracle_result.is_none(),
            "Oracle failures should not propagate fatal errors"
        );

        std::env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn doc_audit_command_rejects_missing_root() {
        let args = create_doc_args(PathBuf::from("./does-not-exist"));
        assert!(doc_audit_command(args).is_err());
    }

    #[test]
    fn doc_audit_command_generates_report() {
        let temp = TempDir::new().expect("temp dir");
        fs::write(
            temp.path().join("lib.rs"),
            "/// docs\npub fn documented() {}\n",
        )
        .expect("write file");

        let mut args = create_doc_args(temp.path().to_path_buf());
        args.format = DocAuditFormat::Json;
        doc_audit_command(args).expect("doc audit should succeed");
    }

    #[test]
    fn doc_audit_command_strict_flags_issues() {
        let temp = TempDir::new().expect("temp dir");
        fs::write(temp.path().join("main.rs"), "pub fn missing_docs() {}\n").expect("write file");

        let mut args = create_doc_args(temp.path().to_path_buf());
        args.strict = true;
        let err = doc_audit_command(args).expect_err("strict mode should fail");
        assert!(err.to_string().contains("Documentation audit found issues"));
    }

    #[test]
    fn is_quiet_respects_format_overrides() {
        let mut args = create_default_analyze_args();
        args.quiet = false;
        args.format = OutputFormat::Json;
        assert!(super::is_quiet(&args));

        args.format = OutputFormat::Pretty;
        assert!(!super::is_quiet(&args));

        args.quiet = true;
        assert!(super::is_quiet(&args));
    }

    #[test]
    fn test_print_header() {
        // Test that print_header doesn't panic
        print_header();
    }

    #[test]
    fn test_header_lines_for_wide_terminal() {
        let lines = header_lines_for_width(120);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Valknut"));
    }

    #[test]
    fn test_header_lines_for_narrow_terminal() {
        let lines = header_lines_for_width(40);
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].contains("Valknut"),
            "expected compact header to mention Valknut"
        );
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
    fn test_display_config_summary() {
        let config = StructureConfig::default();
        // Test that display_config_summary doesn't panic
        display_config_summary(&config);
    }

    #[tokio::test]
    async fn test_load_configuration_default() {
        let result = load_configuration(None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_configuration_yaml_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let result = load_configuration(Some(temp_file.path())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_configuration_json_file() {
        let temp_dir = TempDir::new().unwrap();
        let json_path = temp_dir.path().join("config.json");
        let config = StructureConfig::default();
        let json_content = serde_json::to_string(&config).unwrap();
        fs::write(&json_path, json_content).unwrap();

        let result = load_configuration(Some(&json_path)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_configuration_invalid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), "invalid: yaml: content:").unwrap();

        let result = load_configuration(Some(temp_file.path())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_print_default_config() {
        let result = print_default_config().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_init_config_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yml");

        let args = InitConfigArgs {
            output: config_path.clone(),
            force: false,
        };

        let result = init_config(args).await;
        assert!(result.is_ok());
        assert!(config_path.exists());

        // Verify file contains valid YAML
        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: serde_yaml::Result<valknut_rs::core::config::ValknutConfig> =
            serde_yaml::from_str(&content);
        assert!(parsed.is_ok());
    }

    #[tokio::test]
    async fn test_init_config_force_overwrite() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("existing_config.yml");

        // Create existing file
        fs::write(&config_path, "existing content").unwrap();

        let args = InitConfigArgs {
            output: config_path.clone(),
            force: true,
        };

        let result = init_config(args).await;
        assert!(result.is_ok());

        // Verify file was overwritten with valid YAML
        let content = fs::read_to_string(&config_path).unwrap();
        assert_ne!(content, "existing content");
        let parsed: serde_yaml::Result<valknut_rs::core::config::ValknutConfig> =
            serde_yaml::from_str(&content);
        assert!(parsed.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_valid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let args = ValidateConfigArgs {
            config: temp_file.path().to_path_buf(),
            verbose: false,
        };

        let result = validate_config(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_verbose() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let args = ValidateConfigArgs {
            config: temp_file.path().to_path_buf(),
            verbose: true,
        };

        let result = validate_config(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_stdio_command() {
        let args = McpStdioArgs { config: None };

        let result = mcp_stdio_command(args, false, SurveyVerbosity::Low).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_stdio_command_with_config() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let args = McpStdioArgs {
            config: Some(temp_file.path().to_path_buf()),
        };

        let result = mcp_stdio_command(args, true, SurveyVerbosity::High).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_manifest_command_stdout() {
        let args = McpManifestArgs { output: None };

        let result = mcp_manifest_command(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_manifest_command_file_output() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("manifest.json");

        let args = McpManifestArgs {
            output: Some(manifest_path.clone()),
        };

        let result = mcp_manifest_command(args).await;
        assert!(result.is_ok());
        assert!(manifest_path.exists());

        // Verify file contains valid JSON
        let content = fs::read_to_string(&manifest_path).unwrap();
        let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(&content);
        assert!(parsed.is_ok());

        let manifest = parsed.unwrap();
        assert_eq!(manifest["name"], "valknut");
        assert!(manifest["capabilities"]["tools"].is_array());
    }

    #[tokio::test]
    async fn test_list_languages() {
        let result = list_languages().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_quality_gate_config_defaults() {
        let args = create_default_analyze_args();

        let config = build_quality_gate_config(&args);
        assert!(!config.enabled);
    }

    #[test]
    fn test_build_quality_gate_config_quality_gate_enabled() {
        let mut args = create_default_analyze_args();
        args.quality_gate.quality_gate = true;
        args.quality_gate.max_complexity = Some(75.0);
        args.quality_gate.min_health = Some(60.0);
        args.quality_gate.max_debt = Some(30.0);
        args.quality_gate.min_maintainability = Some(65.0);
        args.quality_gate.max_issues = Some(10);
        args.quality_gate.max_critical = Some(5);
        args.quality_gate.max_high_priority = Some(15);

        let config = build_quality_gate_config(&args);
        assert!(config.enabled);
        assert_eq!(config.max_complexity_score, 75.0);
        assert_eq!(config.min_maintainability_score, 65.0);
        assert_eq!(config.max_technical_debt_ratio, 30.0);
        assert_eq!(config.max_critical_issues, 5);
        assert_eq!(config.max_high_priority_issues, 15);
    }

    #[test]
    fn test_build_quality_gate_config_fail_on_issues() {
        let mut args = create_default_analyze_args();
        args.quality_gate.fail_on_issues = true;

        let config = build_quality_gate_config(&args);
        assert!(config.enabled);
        assert_eq!(config.max_critical_issues, 0);
        assert_eq!(config.max_high_priority_issues, 0);
    }

    #[test]
    fn test_severity_for_excess_handles_zero_threshold() {
        assert_eq!(severity_for_excess(10.0, 0.0), "Critical");
        assert_eq!(severity_for_excess(2.0, 0.0), "High");
        assert_eq!(severity_for_excess(0.5, 0.0), "Medium");
    }

    #[test]
    fn test_severity_for_excess_relative_thresholds() {
        assert_eq!(severity_for_excess(150.0, 200.0), "Medium");
        assert_eq!(severity_for_excess(108.0, 100.0), "Medium");
        assert_eq!(severity_for_excess(75.0, 60.0), "High");
        assert_eq!(severity_for_excess(95.0, 60.0), "Critical");
    }

    #[test]
    fn test_severity_for_shortfall_levels() {
        assert_eq!(severity_for_shortfall(95.0, 100.0), "Medium");
        assert_eq!(severity_for_shortfall(85.0, 100.0), "High");
        assert_eq!(severity_for_shortfall(70.0, 100.0), "Critical");
    }

    #[test]
    fn test_top_issue_files_ranks_and_limits() {
        let mut results = AnalysisResults::empty();
        results.refactoring_candidates = vec![
            sample_candidate("src/a.rs", Priority::High, 0.82),
            sample_candidate("src/a.rs", Priority::Medium, 0.65),
            sample_candidate("src/b.rs", Priority::Critical, 0.91),
            sample_candidate("src/c.rs", Priority::Low, 0.15),
        ];

        let top = top_issue_files(
            &results,
            |candidate| matches!(candidate.priority, Priority::High | Priority::Critical),
            2,
        );

        assert_eq!(top.len(), 2);
        assert_eq!(top[0], PathBuf::from("src/b.rs"));
        assert_eq!(top[1], PathBuf::from("src/a.rs"));
    }

    #[test]
    fn test_priority_label_variants() {
        assert_eq!(priority_label(Priority::None), "none");
        assert_eq!(priority_label(Priority::Low), "low");
        assert_eq!(priority_label(Priority::Medium), "medium");
        assert_eq!(priority_label(Priority::High), "high");
        assert_eq!(priority_label(Priority::Critical), "critical");
    }

    #[test]
    fn test_is_quiet_considers_flag_and_format() {
        let mut args = create_default_analyze_args();
        assert!(is_quiet(&args)); // machine-readable default

        args.quiet = true;
        args.format = OutputFormat::Markdown;
        assert!(is_quiet(&args)); // explicit quiet flag

        args.quiet = false;
        args.format = OutputFormat::Markdown;
        assert!(!is_quiet(&args)); // human-readable without quiet flag
    }

    #[test]
    fn test_display_quality_gate_violations_with_violations() {
        let violations = vec![
            QualityGateViolation {
                rule_name: "Test Rule".to_string(),
                current_value: 85.0,
                threshold: 70.0,
                description: "Test violation".to_string(),
                severity: "Critical".to_string(),
                affected_files: vec![],
                recommended_actions: vec!["Fix the issue".to_string()],
            },
            QualityGateViolation {
                rule_name: "Warning Rule".to_string(),
                current_value: 25.0,
                threshold: 20.0,
                description: "Warning violation".to_string(),
                severity: "Warning".to_string(),
                affected_files: vec![],
                recommended_actions: vec!["Consider fixing".to_string()],
            },
        ];

        let result = QualityGateResult {
            passed: false,
            violations,
            overall_score: 65.0,
        };

        let _ = capture_stdout(|| display_quality_gate_violations(&result));
    }

    #[test]
    fn test_display_quality_gate_violations_no_violations() {
        let result = QualityGateResult {
            passed: true,
            violations: vec![],
            overall_score: 85.0,
        };

        let _ = capture_stdout(|| display_quality_gate_violations(&result));
    }

    #[test]
    fn test_preview_coverage_discovery_reports_absence_stdout() {
        let runtime = Runtime::new().expect("runtime");
        let workspace = TempDir::new().expect("temp workspace");

        let mut coverage_config = CoverageConfig::default();
        coverage_config.search_paths = vec![".".into()];
        coverage_config.file_patterns = vec!["coverage.lcov".into()];
        coverage_config.auto_discover = true;

        let paths = vec![workspace.path().to_path_buf()];
        let _ = capture_stdout(|| {
            runtime.block_on(async {
                preview_coverage_discovery(&paths, &coverage_config, false)
                    .await
                    .expect("preview discovery");
            });
        });
    }

    #[test]
    fn test_preview_coverage_discovery_lists_files_stdout() {
        let runtime = Runtime::new().expect("runtime");
        let workspace = TempDir::new().expect("temp workspace");
        let coverage_dir = workspace.path().join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        let coverage_file = coverage_dir.join("coverage.lcov");
        fs::write(
            &coverage_file,
            "TN:valknut\nSF:src/lib.rs\nFN:1,foo\nFNF:1\nFNH:1\nDA:1,1\nLF:1\nLH:1\n",
        )
        .expect("write coverage file");

        let mut coverage_config = CoverageConfig::default();
        coverage_config.search_paths = vec!["coverage".into()];
        coverage_config.file_patterns = vec!["coverage.lcov".into()];
        coverage_config.auto_discover = true;

        let paths = vec![workspace.path().to_path_buf()];
        let _ = capture_stdout(|| {
            runtime.block_on(async {
                preview_coverage_discovery(&paths, &coverage_config, false)
                    .await
                    .expect("preview discovery");
            });
        });
    }

    #[test]
    fn test_display_quality_gate_violations_blocker_severity() {
        let violations = vec![QualityGateViolation {
            rule_name: "Blocker Rule".to_string(),
            current_value: 95.0,
            threshold: 70.0,
            description: "Blocker violation".to_string(),
            severity: "Blocker".to_string(),
            affected_files: vec!["test.rs".to_string().into()],
            recommended_actions: vec!["Immediate fix required".to_string()],
        }];

        let result = QualityGateResult {
            passed: false,
            violations,
            overall_score: 30.0,
        };

        let _ = capture_stdout(|| display_quality_gate_violations(&result));
    }

    #[test]
    fn test_display_quality_failures_with_recommendations() {
        let result = QualityGateResult {
            passed: false,
            violations: vec![
                QualityGateViolation {
                    rule_name: "Maintainability Score".to_string(),
                    description: "Maintainability below threshold".to_string(),
                    current_value: 55.0,
                    threshold: 75.0,
                    severity: "Critical".to_string(),
                    affected_files: vec![],
                    recommended_actions: vec![
                        "Refactor large modules".to_string(),
                        "Improve documentation".to_string(),
                    ],
                },
                QualityGateViolation {
                    rule_name: "High Priority Issues".to_string(),
                    description: "High-priority issues exceed limit".to_string(),
                    current_value: 8.0,
                    threshold: 3.0,
                    severity: "High".to_string(),
                    affected_files: vec![],
                    recommended_actions: Vec::new(),
                },
            ],
            overall_score: 62.5,
        };

        let _ = capture_stdout(|| display_quality_failures(&result, true));
    }

    #[test]
    fn test_display_quality_failures_without_violations() {
        let result = QualityGateResult {
            passed: true,
            violations: Vec::new(),
            overall_score: 91.0,
        };

        let _ = capture_stdout(|| display_quality_failures(&result, true));
    }

    // Mock test for handle_quality_gates since it requires complex analysis result structure
    #[tokio::test]
    async fn test_handle_quality_gates_basic() {
        let mut args = create_default_analyze_args();
        args.quality_gate.quality_gate = true;

        // Create a minimal analysis result
        let analysis_result = serde_json::json!({
            "summary": {
                "total_issues": 5,
                "total_files": 10
            },
            "health_metrics": {
                "overall_health_score": 75.0,
                "complexity_score": 65.0,
                "technical_debt_ratio": 15.0
            }
        });

        let result = handle_quality_gates(&args, &analysis_result).await;
        assert!(result.is_ok());

        let quality_result = result.unwrap();
        assert!(quality_result.passed); // Should pass with default thresholds
    }

    #[tokio::test]
    async fn test_handle_quality_gates_violations() {
        let mut args = create_default_analyze_args();
        args.quality_gate.quality_gate = true;
        args.quality_gate.max_complexity = Some(50.0); // Set low threshold to trigger violation
        args.quality_gate.min_health = Some(80.0); // Set high threshold to trigger violation
        args.quality_gate.max_issues = Some(3); // Set low threshold to trigger violation

        // Create analysis result that will violate quality gates
        let analysis_result = serde_json::json!({
            "summary": {
                "total_issues": 5, // Exceeds max_issues of 3
                "total_files": 10
            },
            "health_metrics": {
                "overall_health_score": 75.0, // Below min_health of 80
                "complexity_score": 65.0, // Exceeds max_complexity of 50
                "technical_debt_ratio": 15.0
            }
        });

        let result = handle_quality_gates(&args, &analysis_result).await;
        assert!(result.is_ok());

        let quality_result = result.unwrap();
        assert!(!quality_result.passed); // Should fail due to violations
        assert!(!quality_result.violations.is_empty());
    }

    #[tokio::test]
    async fn test_handle_quality_gates_missing_summary() {
        let mut args = create_default_analyze_args();
        args.quality_gate.quality_gate = true;

        // Create analysis result without summary
        let analysis_result = serde_json::json!({
            "health_metrics": {
                "overall_health_score": 75.0
            }
        });

        let result = handle_quality_gates(&args, &analysis_result).await;
        assert!(result.is_err()); // Should fail due to missing summary
    }

    #[tokio::test]
    #[serial]
    async fn analyze_command_errors_on_missing_path() {
        let temp_out = TempDir::new().expect("temp out dir");
        let mut args = create_default_analyze_args();
        args.paths = vec![PathBuf::from("definitely_missing_path")];
        args.out = temp_out.path().join("reports");
        args.quiet = false;
        args.format = OutputFormat::Json;

        let result = analyze_command(args, false, SurveyVerbosity::Low, false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn analyze_command_errors_when_no_paths() {
        let temp_out = TempDir::new().expect("temp out dir");
        let mut args = create_default_analyze_args();
        args.paths.clear();
        args.out = temp_out.path().join("reports");
        args.quiet = false;
        args.format = OutputFormat::Json;

        let result = analyze_command(args, false, SurveyVerbosity::Low, false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_analyze_command_quiet_mode_on_minimal_project() {
        let project = TempDir::new().expect("temp project");
        let project_root = project.path().to_path_buf();
        fs::write(
            project_root.join("lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        )
        .expect("write sample file");

        let output = TempDir::new().expect("output dir");
        let out_path = output.path().join("reports");

        let mut args = create_default_analyze_args();
        args.paths = vec![project_root];
        args.out = out_path;
        args.quiet = true;
        args.format = OutputFormat::Json;
        args.profile = PerformanceProfile::Fast;
        args.coverage.no_coverage = true;
        args.coverage.no_coverage_auto_discover = true;
        args.analysis_control.no_complexity = true;
        args.analysis_control.no_structure = true;
        args.analysis_control.no_refactoring = true;
        args.analysis_control.no_impact = true;
        args.analysis_control.no_lsh = true;

        let result = analyze_command(args, false, SurveyVerbosity::Low, false).await;
        assert!(
            result.is_ok(),
            "analyze_command should succeed for minimal quiet invocation: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[serial]
    async fn run_analysis_with_progress_handles_denoise_configuration() -> Result<()> {
        let project = create_sample_analysis_project();
        let project_path = project.path().to_path_buf();
        let coverage_file = write_lcov_fixture(project.path());
        let output_dir = TempDir::new().expect("output dir");

        let mut args = create_default_analyze_args();
        args.paths = vec![project_path.clone()];
        args.out = output_dir.path().to_path_buf();
        args.format = OutputFormat::Pretty;
        args.clone_detection.denoise = true;
        args.clone_detection.denoise_dry_run = true;
        args.clone_detection.min_function_tokens = Some(12);
        args.clone_detection.min_match_tokens = Some(4);
        args.clone_detection.require_blocks = Some(1);
        args.clone_detection.similarity = Some(0.88);
        args.advanced_clone.ast_weight = Some(0.6);
        args.advanced_clone.pdg_weight = Some(0.25);
        args.advanced_clone.emb_weight = Some(0.15);
        args.advanced_clone.apted_verify = true;
        args.advanced_clone.apted_max_nodes = Some(256);
        args.advanced_clone.apted_max_pairs = Some(24);
        args.advanced_clone.quality_target = Some(0.92);
        args.advanced_clone.sample_size = Some(42);
        args.advanced_clone.min_saved_tokens = Some(3);
        args.advanced_clone.min_rarity_gain = Some(0.05);
        args.advanced_clone.io_mismatch_penalty = Some(0.33);
        args.coverage.coverage_file = Some(coverage_file.clone());
        args.coverage.coverage_max_age_days = Some(30);
        args.analysis_control.no_complexity = true;
        args.analysis_control.no_refactoring = true;

        let _guard = DirGuard::change_to(&project_path);
        let result =
            run_analysis_with_progress(&args.paths, StructureConfig::default(), &args).await?;

        assert!(
            result["summary"]["total_files"]
                .as_u64()
                .unwrap_or_default()
                >= 1
        );
        let cache_dir = project_path.join(".valknut/cache/denoise");
        assert!(cache_dir.join("stop_motifs.v1.json").exists());
        assert!(cache_dir.join("auto_calibration.v1.json").exists());

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn run_analysis_without_progress_toggles_modules() -> Result<()> {
        let project = create_sample_analysis_project();
        let project_path = project.path().to_path_buf();
        let output_dir = TempDir::new().expect("output dir");

        let mut args = create_default_analyze_args();
        args.paths = vec![project_path.clone()];
        args.out = output_dir.path().to_path_buf();
        args.format = OutputFormat::Json;
        args.clone_detection.denoise = true;
        args.clone_detection.min_function_tokens = Some(8);
        args.clone_detection.min_match_tokens = Some(4);
        args.clone_detection.require_blocks = Some(1);
        args.clone_detection.similarity = Some(0.9);
        args.advanced_clone.no_auto = true;
        args.advanced_clone.no_apted_verify = true;
        args.coverage.no_coverage = true;
        args.coverage.no_coverage_auto_discover = true;
        args.coverage.coverage_max_age_days = Some(14);
        args.analysis_control.no_complexity = true;
        args.analysis_control.no_structure = true;
        args.analysis_control.no_refactoring = true;
        args.analysis_control.no_impact = true;
        args.analysis_control.no_lsh = true;

        {
            let _guard = DirGuard::change_to(&project_path);
            let summary =
                run_analysis_without_progress(&args.paths, StructureConfig::default(), &args)
                    .await?;
            assert!(
                summary["summary"]["total_files"]
                    .as_u64()
                    .unwrap_or_default()
                    >= 1
            );
        }

        let mut args_no_denoise = create_default_analyze_args();
        args_no_denoise.paths = vec![project_path.clone()];
        args_no_denoise.out = output_dir.path().to_path_buf();
        args_no_denoise.format = OutputFormat::Json;
        args_no_denoise.clone_detection.denoise = false;
        args_no_denoise.clone_detection.denoise_dry_run = false;
        args_no_denoise.coverage.no_coverage = true;
        args_no_denoise.coverage.no_coverage_auto_discover = true;
        args_no_denoise.coverage.coverage_max_age_days = Some(14);
        args_no_denoise.analysis_control.no_complexity = true;
        args_no_denoise.analysis_control.no_structure = true;
        args_no_denoise.analysis_control.no_refactoring = true;
        args_no_denoise.analysis_control.no_impact = true;
        args_no_denoise.analysis_control.no_lsh = true;

        {
            let _guard = DirGuard::change_to(&project_path);
            let summary = run_analysis_without_progress(
                &args_no_denoise.paths,
                StructureConfig::default(),
                &args_no_denoise,
            )
            .await?;
            assert!(
                summary["summary"]["total_files"]
                    .as_u64()
                    .unwrap_or_default()
                    >= 1
            );
        }

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn create_denoise_cache_directories_is_idempotent() -> Result<()> {
        let temp = TempDir::new().expect("temp dir");
        let _guard = DirGuard::change_to(temp.path());
        create_denoise_cache_directories().await?;
        let stop_file = temp
            .path()
            .join(".valknut/cache/denoise/stop_motifs.v1.json");
        let auto_file = temp
            .path()
            .join(".valknut/cache/denoise/auto_calibration.v1.json");
        assert!(stop_file.exists());
        assert!(auto_file.exists());

        create_denoise_cache_directories().await?;
        assert!(stop_file.exists());
        assert!(auto_file.exists());

        Ok(())
    }

    #[test]
    fn apply_performance_profile_adjusts_configuration() {
        let mut config = ValknutConfig::default();
        apply_performance_profile(&mut config, &PerformanceProfile::Fast);
        assert_eq!(config.analysis.max_files, 500);
        apply_performance_profile(&mut config, &PerformanceProfile::Balanced);
        apply_performance_profile(&mut config, &PerformanceProfile::Thorough);
        assert!(config.denoise.enabled);
        apply_performance_profile(&mut config, &PerformanceProfile::Extreme);
        assert_eq!(config.lsh.num_hashes, 200);
    }

    #[test]
    fn test_display_enabled_analyses_all_features() {
        let mut config = ValknutConfig::default();
        config.analysis.enable_scoring = true;
        config.analysis.enable_structure_analysis = true;
        config.analysis.enable_refactoring_analysis = true;
        config.analysis.enable_graph_analysis = true;
        config.analysis.enable_lsh_analysis = true;
        config.analysis.enable_coverage_analysis = true;
        config.coverage.auto_discover = true;
        config.denoise.enabled = true;
        config.lsh.verify_with_apted = true;

        display_enabled_analyses(&config, true);
    }

    #[test]
    fn test_display_analysis_config_summary_with_flags() {
        let mut config = ValknutConfig::default();
        config.analysis.enable_coverage_analysis = true;
        config.coverage.max_age_days = 7;
        config.coverage.file_patterns = vec!["coverage.lcov".into()];
        config.analysis.max_files = 42;
        config.denoise.enabled = true;
        config.denoise.similarity = 0.87;
        config.analysis.enable_lsh_analysis = true;

        display_analysis_config_summary(&config);
    }

    #[tokio::test]
    async fn test_preview_coverage_discovery_handles_absence() {
        let temp_dir = TempDir::new().unwrap();
        let config = CoverageConfig::default();

        let result =
            preview_coverage_discovery(&[temp_dir.path().to_path_buf()], &config, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_preview_coverage_discovery_lists_files() {
        let coverage_dir = TempDir::new().unwrap();
        let root = coverage_dir.path();
        let nested = root.join("coverage");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("coverage.lcov"), "TN:demo\nend_of_record\n").unwrap();

        let mut config = CoverageConfig::default();
        config.auto_discover = true;
        config.file_patterns = vec!["coverage.lcov".into()];

        let result = preview_coverage_discovery(&[root.to_path_buf()], &config, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_preview_coverage_discovery_truncates_listing() {
        let coverage_dir = TempDir::new().unwrap();
        let root = coverage_dir.path();
        let nested = root.join("coverage");
        fs::create_dir_all(&nested).unwrap();

        for idx in 0..4 {
            let file_path = nested.join(format!("report_{idx}.lcov"));
            fs::write(&file_path, "TN:demo\nend_of_record\n").unwrap();
        }

        let mut config = CoverageConfig::default();
        config.auto_discover = true;
        config.search_paths = vec!["coverage".to_string()];
        config.file_patterns = vec!["*.lcov".to_string()];

        let result = preview_coverage_discovery(&[root.to_path_buf()], &config, false).await;
        assert!(result.is_ok());
    }

    #[test]
    fn severity_for_excess_covers_threshold_cases() {
        assert_eq!(severity_for_excess(10.0, 0.0), "Critical");
        assert_eq!(severity_for_excess(20.0, 10.0), "Critical");
        assert_eq!(severity_for_excess(26.0, 20.0), "High");
        assert_eq!(severity_for_excess(22.0, 20.0), "Medium");
    }

    #[test]
    fn severity_for_shortfall_respects_delta() {
        assert_eq!(severity_for_shortfall(50.0, 80.0), "Critical");
        assert_eq!(severity_for_shortfall(65.0, 80.0), "High");
        assert_eq!(severity_for_shortfall(75.0, 80.0), "Medium");
    }

    #[test]
    fn display_analysis_summary_prints_hotspots_and_metrics() {
        let mut result = sample_analysis_results();
        result.summary.refactoring_needed = 2;
        result.summary.high_priority = 2;
        result.summary.critical = 1;

        result.refactoring_candidates.push(sample_candidate(
            "src/utils.rs",
            Priority::Critical,
            3.8,
        ));

        result.refactoring_candidates.push(sample_candidate(
            "src/helpers/mod.rs",
            Priority::High,
            2.9,
        ));

        result.clone_analysis = Some(CloneAnalysisResults {
            denoising_enabled: true,
            auto_calibration_applied: Some(true),
            candidates_before_denoising: Some(10),
            candidates_after_denoising: 4,
            calibrated_threshold: Some(0.75),
            quality_score: Some(0.82),
            avg_similarity: Some(0.68),
            max_similarity: Some(0.91),
            verification: None,
            phase_filtering_stats: None,
            performance_metrics: None,
            notes: vec!["Filtered duplicates".to_string()],
            clone_pairs: Vec::new(),
        });

        result.warnings = vec!["Sample warning".to_string()];

        display_comprehensive_results(&result, true);
    }

    #[test]
    fn combine_analysis_results_merges_runs() {
        let mut first = sample_analysis_results();
        first.summary.files_processed = 2;
        first.summary.entities_analyzed = 4;
        first.summary.avg_refactoring_score = 0.6;
        first.summary.code_health_score = 0.7;
        first.statistics.total_duration = Duration::from_millis(30);
        first
            .statistics
            .features_per_entity
            .insert("cyclomatic".into(), 3.0);
        first.summary.refactoring_needed = 1;
        first.summary.high_priority = 1;
        first.statistics.memory_stats = MemoryStats {
            peak_memory_bytes: 2048,
            final_memory_bytes: 1024,
            efficiency_score: 0.8,
        };

        let mut second = sample_analysis_results();
        second.summary.files_processed = 3;
        second.summary.entities_analyzed = 6;
        second.summary.avg_refactoring_score = 0.9;
        second.summary.code_health_score = 0.5;
        second.statistics.total_duration = Duration::from_millis(60);
        second
            .statistics
            .features_per_entity
            .insert("cyclomatic".into(), 5.0);
        second
            .statistics
            .features_per_entity
            .insert("maintainability".into(), 2.0);
        second.summary.refactoring_needed = 2;
        second.summary.high_priority = 1;
        second.statistics.memory_stats = MemoryStats {
            peak_memory_bytes: 4096,
            final_memory_bytes: 2048,
            efficiency_score: 0.6,
        };
        second.warnings.push("Second warning".into());

        let expected_files = first.summary.files_processed + second.summary.files_processed;
        let expected_entities = first.summary.entities_analyzed + second.summary.entities_analyzed;
        let expected_refactoring =
            first.summary.refactoring_needed + second.summary.refactoring_needed;
        let expected_high_priority = first.summary.high_priority + second.summary.high_priority;
        let expected_duration = first.statistics.total_duration + second.statistics.total_duration;

        let combined = combine_analysis_results(vec![first, second]).expect("merge succeeds");

        assert_eq!(combined.summary.files_processed, expected_files);
        assert_eq!(combined.summary.entities_analyzed, expected_entities);
        assert_eq!(combined.summary.refactoring_needed, expected_refactoring);
        assert_eq!(combined.summary.high_priority, expected_high_priority);
        assert!(
            combined.summary.avg_refactoring_score >= 0.6
                && combined.summary.avg_refactoring_score <= 0.9
        );
        assert!(
            combined.summary.code_health_score >= 0.5 && combined.summary.code_health_score <= 0.7
        );
        assert_eq!(combined.statistics.total_duration, expected_duration);
        assert!(combined
            .statistics
            .features_per_entity
            .contains_key("maintainability"));
        assert_eq!(combined.warnings.len(), 1);
        assert_eq!(combined.refactoring_candidates.len(), 2);
    }

    #[test]
    fn combine_analysis_results_errors_on_empty() {
        let err = combine_analysis_results(vec![]);
        assert!(err.is_err());
    }
