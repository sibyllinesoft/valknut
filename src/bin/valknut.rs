#!/usr/bin/env rust
//! Valknut CLI - AI-Powered Code Analysis & Refactoring Assistant
//!
//! This binary provides complete feature parity with the Python CLI,
//! including rich console output, progress tracking, and comprehensive
//! analysis capabilities with team-friendly reports.

use clap::Parser;
use tracing;

mod cli;

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
            cli::analyze_command(args, cli.survey, cli.survey_verbosity).await?;
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
        // Legacy commands for backward compatibility
        Commands::Structure(args) => {
            let config = cli::load_configuration(None).await?;
            cli::analyze_structure_legacy(args, config).await?;
        }
        Commands::Names(args) => {
            cli::analyze_names_legacy(args).await?;
        }
        Commands::Impact(args) => {
            cli::analyze_impact_legacy(args).await?;
        }
    }

    Ok(())
}