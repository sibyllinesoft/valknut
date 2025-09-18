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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing/logging
    let log_level = if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    // Execute command
    match cli.command {
        Commands::Analyze(args) => {
            cli::analyze_command(*args, cli.survey, cli.survey_verbosity).await?;
        }
        Commands::PrintDefaultConfig => {
            cli::print_default_config().await?;
        }
        Commands::InitConfig(args) => {
            cli::init_config(args).await?;
        }
        Commands::ValidateConfig(args) => {
            cli::validate_config(args).await?;
        }
        Commands::McpStdio(args) => {
            cli::mcp_stdio_command(args, cli.survey, cli.survey_verbosity).await?;
        }
        Commands::McpManifest(args) => {
            cli::mcp_manifest_command(args).await?;
        }
        Commands::ListLanguages => {
            cli::list_languages().await?;
        }
        Commands::LiveReach(args) => {
            cli::live_reach_command(args).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use cli::args::{OutputFormat, SurveyVerbosity};
    use std::path::PathBuf;

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
                assert!(matches!(args.format, OutputFormat::Jsonl));
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
                assert!(matches!(args.format, OutputFormat::Html));
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
                    assert!(
                        std::mem::discriminant(&args.format)
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
}
