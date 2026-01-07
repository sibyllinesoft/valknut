#!/usr/bin/env rust
//! Valknut CLI - AI-Powered Code Analysis & Refactoring Assistant
//!
//! This binary provides complete feature parity with the Python CLI,
//! including rich console output, progress tracking, and comprehensive
//! analysis capabilities with team-friendly reports.

use clap::Parser;

mod cli;
mod mcp;

use cli::{Cli, Commands};

/// Entry point for the valknut CLI.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    run_cli(cli).await
}

/// Initialize tracing/logging based on verbosity setting.
fn init_logging(verbose: bool) {
    let log_level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    let _ = tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .try_init();
}

/// Runs the CLI with the parsed command and options.
async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    init_logging(cli.verbose);
    let Cli { command, survey, survey_verbosity, verbose } = cli;

    match command {
        // Analysis commands
        Commands::Analyze(args) => {
            cli::analyze_command(*args, survey, survey_verbosity, verbose).await
        }
        Commands::DocAudit(args) => cli::doc_audit_command(args),

        // Configuration commands
        Commands::PrintDefaultConfig => cli::print_default_config().await,
        Commands::InitConfig(args) => cli::init_config(args).await,
        Commands::ValidateConfig(args) => cli::validate_config(args).await,

        // MCP commands
        Commands::McpStdio(args) => {
            cli::mcp_stdio_command(args, survey, survey_verbosity).await
        }
        Commands::McpManifest(args) => cli::mcp_manifest_command(args).await,

        // Info commands
        Commands::ListLanguages => cli::list_languages().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use cli::args::{
        DocAuditFormat, InitConfigArgs, McpManifestArgs, OutputFormat, SurveyVerbosity,
        ValidateConfigArgs,
    };
    use std::path::PathBuf;
    use tempfile::tempdir;
    use valknut_rs::core::pipeline::{
        issue_code_for_category, issue_definition_for_category, suggestion_code_for_kind,
        suggestion_definition_for_kind,
    };
    use valknut_rs::doc_audit;

    #[tokio::test]
    async fn test_cli_parsing_analyze_default() {
        let cli = Cli::parse_from(["valknut", "analyze"]);
        assert!(!cli.verbose);
        assert!(!cli.survey);
        assert!(matches!(cli.survey_verbosity, SurveyVerbosity::Maximum));

        match cli.command {
            Commands::Analyze(args) => {
                assert_eq!(args.paths, vec![PathBuf::from(".")]);
                assert_eq!(args.out, PathBuf::from(".valknut"));
                // No format specified, so vec is empty (effective_formats() will default to jsonl)
                assert!(args.format.is_empty());
                assert!(!args.quiet);
                assert!(!args.quality_gate.quality_gate);
                assert!(!args.quality_gate.fail_on_issues);
            }
            _ => panic!("Expected Analyze command"),
        }
    }

    #[tokio::test]
    async fn test_cli_parsing_analyze_with_options() {
        let cli = Cli::parse_from([
            "valknut",
            "analyze",
            "--verbose",
            "--survey",
            "--survey-verbosity",
            "low",
            "--config",
            "test.yml",
            "--out",
            "reports",
            "--format",
            "html",
            "--quiet",
            "--quality-gate",
            "--max-complexity",
            "80",
            "src/",
        ]);

        assert!(cli.verbose);
        assert!(cli.survey);
        assert!(matches!(cli.survey_verbosity, SurveyVerbosity::Low));

        match cli.command {
            Commands::Analyze(args) => {
                assert_eq!(args.paths, vec![PathBuf::from("src/")]);
                assert_eq!(args.config, Some(PathBuf::from("test.yml")));
                assert_eq!(args.out, PathBuf::from("reports"));
                assert_eq!(args.format, vec![OutputFormat::Html]);
                assert!(args.quiet);
                assert!(args.quality_gate.quality_gate);
                assert_eq!(args.quality_gate.max_complexity, Some(80.0));
            }
            _ => panic!("Expected Analyze command"),
        }
    }

    #[tokio::test]
    async fn test_cli_parsing_print_default_config() {
        let cli = Cli::parse_from(["valknut", "print-default-config"]);
        match cli.command {
            Commands::PrintDefaultConfig => {}
            _ => panic!("Expected PrintDefaultConfig command"),
        }
    }

    #[tokio::test]
    async fn test_cli_parsing_init_config() {
        let cli = Cli::parse_from([
            "valknut",
            "init-config",
            "--output",
            "custom.yml",
            "--force",
        ]);
        match cli.command {
            Commands::InitConfig(args) => {
                assert_eq!(args.output, PathBuf::from("custom.yml"));
                assert!(args.force);
            }
            _ => panic!("Expected InitConfig command"),
        }
    }

    #[tokio::test]
    async fn test_cli_parsing_validate_config() {
        let cli = Cli::parse_from([
            "valknut",
            "validate-config",
            "--config",
            "test.yml",
            "--verbose",
        ]);
        match cli.command {
            Commands::ValidateConfig(args) => {
                assert_eq!(args.config, PathBuf::from("test.yml"));
                assert!(args.verbose);
            }
            _ => panic!("Expected ValidateConfig command"),
        }
    }

    #[tokio::test]
    async fn test_run_cli_print_default_config_executes() {
        let cli = Cli {
            command: Commands::PrintDefaultConfig,
            verbose: false,
            survey: false,
            survey_verbosity: SurveyVerbosity::Maximum,
        };

        run_cli(cli).await.expect("print default config succeeds");
    }

    #[tokio::test]
    async fn test_run_cli_init_and_validate_config() {
        let temp = tempdir().expect("temp dir");
        let config_path = temp.path().join("valknut.yml");

        let init_cli = Cli {
            command: Commands::InitConfig(InitConfigArgs {
                output: config_path.clone(),
                force: true,
            }),
            verbose: false,
            survey: false,
            survey_verbosity: SurveyVerbosity::Maximum,
        };
        run_cli(init_cli)
            .await
            .expect("init-config command should succeed");
        assert!(config_path.exists(), "config file should be created");

        let validate_cli = Cli {
            command: Commands::ValidateConfig(ValidateConfigArgs {
                config: config_path.clone(),
                verbose: true,
            }),
            verbose: false,
            survey: false,
            survey_verbosity: SurveyVerbosity::Maximum,
        };
        let validation_result = run_cli(validate_cli).await;
        assert!(
            validation_result.is_err(),
            "expected validation to surface configuration issues for generated defaults"
        );
    }

    #[tokio::test]
    async fn test_run_cli_mcp_manifest_writes_file() {
        let temp = tempdir().expect("temp dir");
        let manifest_path = temp.path().join("manifest.json");

        let cli = Cli {
            command: Commands::McpManifest(McpManifestArgs {
                output: Some(manifest_path.clone()),
            }),
            verbose: false,
            survey: false,
            survey_verbosity: SurveyVerbosity::Maximum,
        };

        run_cli(cli)
            .await
            .expect("mcp-manifest command should succeed");
        assert!(manifest_path.exists(), "manifest file should be created");
    }

    #[tokio::test]
    async fn test_run_cli_list_languages_executes() {
        let cli = Cli {
            command: Commands::ListLanguages,
            verbose: false,
            survey: false,
            survey_verbosity: SurveyVerbosity::Maximum,
        };

        run_cli(cli)
            .await
            .expect("list-languages command should succeed");
    }

    #[test]
    fn test_cli_parsing_doc_audit_defaults() {
        let cli = Cli::parse_from(["valknut", "doc-audit"]);
        assert!(!cli.verbose);
        match cli.command {
            Commands::DocAudit(args) => {
                assert_eq!(args.root, PathBuf::from("."));
                assert_eq!(
                    args.complexity_threshold,
                    doc_audit::DEFAULT_COMPLEXITY_THRESHOLD
                );
                assert_eq!(
                    args.max_readme_commits,
                    doc_audit::DEFAULT_MAX_README_COMMITS
                );
                assert!(!args.strict);
                assert!(matches!(args.format, DocAuditFormat::Text));
            }
            _ => panic!("Expected DocAudit command"),
        }
    }

    #[tokio::test]
    async fn test_run_cli_list_languages() {
        let cli = Cli::parse_from(["valknut", "list-languages"]);
        run_cli(cli).await.expect("list-languages should succeed");
    }

    #[tokio::test]
    async fn test_run_cli_print_default_config() {
        let cli = Cli::parse_from(["valknut", "print-default-config"]);
        run_cli(cli)
            .await
            .expect("print-default-config should succeed");
    }

    #[tokio::test]
    async fn test_run_cli_doc_audit_with_temp_project() {
        let project = tempdir().unwrap();
        let root = project.path();
        std::fs::write(root.join("README.md"), "# Test Project\n\nDocs.").unwrap();
        let src_dir = root.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("lib.rs"),
            "/// Sample function\npub fn sample() {}\n",
        )
        .unwrap();

        let root_str = root.to_string_lossy().to_string();
        let cli = Cli::parse_from([
            "valknut",
            "doc-audit",
            "--root",
            &root_str,
            "--format",
            "text",
        ]);

        run_cli(cli).await.expect("doc-audit should succeed");
    }

    #[tokio::test]
    async fn test_cli_parsing_mcp_stdio() {
        let cli = Cli::parse_from(["valknut", "mcp-stdio", "--config", "test.yml"]);
        match cli.command {
            Commands::McpStdio(args) => {
                assert_eq!(args.config, Some(PathBuf::from("test.yml")));
            }
            _ => panic!("Expected McpStdio command"),
        }
    }

    #[tokio::test]
    async fn test_cli_parsing_mcp_manifest() {
        let cli = Cli::parse_from(["valknut", "mcp-manifest", "--output", "manifest.json"]);
        match cli.command {
            Commands::McpManifest(args) => {
                assert_eq!(args.output, Some(PathBuf::from("manifest.json")));
            }
            _ => panic!("Expected McpManifest command"),
        }
    }

    #[tokio::test]
    async fn test_cli_parsing_list_languages() {
        let cli = Cli::parse_from(["valknut", "list-languages"]);
        match cli.command {
            Commands::ListLanguages => {}
            _ => panic!("Expected ListLanguages command"),
        }
    }

    #[tokio::test]
    async fn test_cli_parsing_survey_verbosity_variants() {
        let cli_low = Cli::parse_from(["valknut", "analyze", "--survey-verbosity", "low"]);
        assert!(matches!(cli_low.survey_verbosity, SurveyVerbosity::Low));

        let cli_medium = Cli::parse_from(["valknut", "analyze", "--survey-verbosity", "medium"]);
        assert!(matches!(
            cli_medium.survey_verbosity,
            SurveyVerbosity::Medium
        ));

        let cli_high = Cli::parse_from(["valknut", "analyze", "--survey-verbosity", "high"]);
        assert!(matches!(cli_high.survey_verbosity, SurveyVerbosity::High));

        let cli_maximum = Cli::parse_from(["valknut", "analyze", "--survey-verbosity", "maximum"]);
        assert!(matches!(
            cli_maximum.survey_verbosity,
            SurveyVerbosity::Maximum
        ));
    }

    #[tokio::test]
    async fn test_cli_parsing_output_format_variants() {
        let formats = [
            ("jsonl", OutputFormat::Jsonl),
            ("json", OutputFormat::Json),
            ("yaml", OutputFormat::Yaml),
            ("markdown", OutputFormat::Markdown),
            ("html", OutputFormat::Html),
            ("sonar", OutputFormat::Sonar),
            ("csv", OutputFormat::Csv),
            ("ci-summary", OutputFormat::CiSummary),
            ("pretty", OutputFormat::Pretty),
        ];

        for (format_str, expected_format) in formats {
            let cli = Cli::parse_from(["valknut", "analyze", "--format", format_str]);
            match cli.command {
                Commands::Analyze(args) => {
                    assert_eq!(args.format.len(), 1, "Expected single format");
                    assert!(
                        std::mem::discriminant(&args.format[0])
                            == std::mem::discriminant(&expected_format)
                    );
                }
                _ => panic!("Expected Analyze command"),
            }
        }
    }

    #[tokio::test]
    async fn test_cli_parsing_quality_gate_options() {
        let cli = Cli::parse_from([
            "valknut",
            "analyze",
            "--fail-on-issues",
            "--max-complexity",
            "75.5",
            "--min-health",
            "60.0",
            "--max-debt",
            "30.0",
            "--min-maintainability",
            "20.0",
            "--max-issues",
            "50",
            "--max-critical",
            "0",
            "--max-high-priority",
            "5",
        ]);

        match cli.command {
            Commands::Analyze(args) => {
                assert!(args.quality_gate.fail_on_issues);
                assert_eq!(args.quality_gate.max_complexity, Some(75.5));
                assert_eq!(args.quality_gate.min_health, Some(60.0));
                assert_eq!(args.quality_gate.max_debt, Some(30.0));
                assert_eq!(args.quality_gate.min_maintainability, Some(20.0));
                assert_eq!(args.quality_gate.max_issues, Some(50));
                assert_eq!(args.quality_gate.max_critical, Some(0));
                assert_eq!(args.quality_gate.max_high_priority, Some(5));
            }
            _ => panic!("Expected Analyze command"),
        }
    }

    #[tokio::test]
    async fn test_cli_global_flags() {
        let cli = Cli::parse_from([
            "valknut",
            "--verbose",
            "--survey",
            "--survey-verbosity",
            "medium",
            "analyze",
        ]);

        assert!(cli.verbose);
        assert!(cli.survey);
        assert!(matches!(cli.survey_verbosity, SurveyVerbosity::Medium));
    }

    #[test]
    fn code_dictionary_known_categories_have_expected_codes() {
        let expectations = [
            ("complexity", "CMPLX"),
            ("cognitive", "COGNIT"),
            ("structure", "STRUCTR"),
            ("graph", "COUPLNG"),
            ("style", "STYLE"),
            ("coverage", "COVGAP"),
            ("debt", "TECHDEBT"),
            ("maintainability", "MAINTAIN"),
            ("readability", "READABL"),
            ("refactoring", "REFACTR"),
        ];

        for (category, expected_code) in expectations {
            let definition = issue_definition_for_category(category);
            assert_eq!(
                definition.code, expected_code,
                "unexpected code for category {category}"
            );
            assert_eq!(
                definition.category.as_deref(),
                Some(category),
                "category field should echo the input"
            );
            assert!(
                !definition.summary.is_empty(),
                "summary should not be empty for {category}"
            );
            assert_eq!(
                issue_code_for_category(category),
                expected_code,
                "helper shortcut should mirror definition"
            );
        }
    }

    #[test]
    fn code_dictionary_helpers_cover_unknown_inputs() {
        let issue = issue_definition_for_category("custom-signal");
        assert!(
            issue.summary.contains("custom-signal"),
            "fallback summary should mention the original category"
        );
        assert_eq!(
            issue_code_for_category("custom-signal"),
            issue.code,
            "issue helper should reuse fallback code"
        );

        let suggestion = suggestion_definition_for_kind("rename@something");
        assert!(
            suggestion.title.to_lowercase().contains("rename@something"),
            "fallback title should include the original kind"
        );
        assert_eq!(
            suggestion_code_for_kind("rename@something"),
            suggestion.code,
            "suggestion helper should reuse fallback code"
        );
        assert_eq!(
            suggestion.category.as_deref(),
            Some("refactoring"),
            "fallback suggestions stay in refactoring category"
        );
    }

    #[test]
    fn code_dictionary_suggestion_matrix_matches_codes() {
        let mapping = [
            ("eliminate_duplication_block", "DEDUP"),
            ("extract_method_for_cleanup", "XTRMTH"),
            ("extract_class_controller", "XTRCLS"),
            ("simplify_nested_conditional_paths", "SIMPCND"),
            ("reduce_cyclomatic_complexity_in_loop", "RDCYCLEX"),
            ("reduce_cognitive_complexity_hotspot", "RDCOGN"),
            ("reduce_fan_in_hotspot", "RDFANIN"),
            ("reduce_fan_out_calls", "RDFANOUT"),
            ("reduce_centrality_module", "RDCNTRL"),
            ("reduce_chokepoint_service", "RDCHOKE"),
            ("address_nested_branching_issue", "RDNEST"),
            ("simplify_logic", "SMPLOGIC"),
            ("split_responsibilities_module", "SPLRESP"),
            ("move_method_to_helper", "MOVEMTH"),
            ("organize_imports_cleanup", "ORGIMPT"),
            ("introduce_facade_layer", "FACAD"),
            ("extract_interface_adapter", "XTRIFCE"),
            ("inline_temp_variable", "INLTEMP"),
            ("rename_class_handler", "RENCLSS"),
            ("rename_method_handler", "RENMTHD"),
            ("extract_variable_threshold", "XTRVAR"),
            ("add_comments_for_complex_flow", "ADDCMNT"),
            ("rename_variable_precisely", "RENVAR"),
            ("replace_magic_number_pi", "REPMAG"),
            ("format_code_style_update", "FMTSTYLE"),
            ("refactor_code_quality_sweep", "REFQLTY"),
        ];

        for (kind, expected_code) in mapping {
            let definition = suggestion_definition_for_kind(kind);
            assert_eq!(
                definition.code, expected_code,
                "unexpected code for suggestion kind {kind}"
            );
            assert_eq!(
                definition.category.as_deref(),
                Some("refactoring"),
                "suggestions should remain in the refactoring category"
            );
            assert_eq!(
                suggestion_code_for_kind(kind),
                expected_code,
                "helper shortcut should track definition results"
            );
        }
    }
}
