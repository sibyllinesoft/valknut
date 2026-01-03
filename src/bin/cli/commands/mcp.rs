//! MCP (Model Context Protocol) server commands.
//!
//! This module provides commands for starting the MCP stdio server
//! and generating MCP manifest files for IDE integration.

use crate::cli::args::{McpManifestArgs, McpStdioArgs, SurveyVerbosity};
use crate::cli::commands::load_configuration;
use valknut_rs::detectors::structure::StructureConfig;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Start the MCP stdio server for IDE integration.
///
/// This command starts a JSON-RPC 2.0 server that communicates via stdio,
/// enabling IDE integration with tools like Claude Code.
///
/// Available tools exposed by the server:
/// - analyze_code: Analyze code for refactoring opportunities and quality metrics
/// - get_refactoring_suggestions: Get specific refactoring suggestions for a code entity
///
/// The server follows the MCP specification and can be used with Claude Code
/// and other MCP-compatible clients.
pub async fn mcp_stdio_command(
    args: McpStdioArgs,
    survey: bool,
    survey_verbosity: SurveyVerbosity,
) -> anyhow::Result<()> {
    use crate::mcp::server::run_mcp_server;

    eprintln!("Starting MCP stdio server for IDE integration...");

    // Load configuration
    let _config = if let Some(config_path) = args.config {
        load_configuration(Some(&config_path)).await?
    } else {
        StructureConfig::default()
    };

    if survey {
        eprintln!("Survey enabled with {:?} verbosity", survey_verbosity);
    } else {
        eprintln!("Survey disabled");
    }

    // Initialize and run MCP server
    eprintln!("MCP JSON-RPC 2.0 server ready for requests");

    if let Err(e) = run_mcp_server(VERSION).await {
        eprintln!("MCP server error: {}", e);
        return Err(anyhow::anyhow!("MCP server failed: {}", e));
    }

    Ok(())
}

/// Generate MCP manifest JSON.
///
/// The manifest describes the server capabilities and available tools
/// for MCP-compatible clients.
pub async fn mcp_manifest_command(args: McpManifestArgs) -> anyhow::Result<()> {
    let manifest = serde_json::json!({
        "name": "valknut",
        "version": VERSION,
        "description": "AI-Powered Code Analysis & Refactoring Assistant",
        "author": "Nathan Rice",
        "license": "MIT",
        "homepage": "https://github.com/nathanricedev/valknut",
        "capabilities": {
            "tools": [
                {
                    "name": "analyze_code",
                    "description": "Analyze code for complexity, technical debt, and refactoring opportunities",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string", "description": "Path to code directory or file"},
                            "format": {"type": "string", "enum": ["json", "markdown", "html"], "description": "Output format"}
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "get_refactoring_suggestions",
                    "description": "Get specific refactoring suggestions for code entities",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "entity_id": {"type": "string", "description": "Code entity identifier"},
                            "max_suggestions": {"type": "integer", "description": "Maximum number of suggestions"}
                        },
                        "required": ["entity_id"]
                    }
                },
                {
                    "name": "validate_quality_gates",
                    "description": "Validate code against quality gate thresholds for CI/CD integration",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string", "description": "Path to code directory or file"},
                            "max_complexity": {"type": "number", "description": "Maximum allowed complexity score"},
                            "min_health": {"type": "number", "description": "Minimum required health score"},
                            "max_debt": {"type": "number", "description": "Maximum allowed technical debt ratio"},
                            "max_issues": {"type": "integer", "description": "Maximum allowed number of issues"}
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "analyze_file_quality",
                    "description": "Analyze quality metrics and issues for a specific file",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "file_path": {"type": "string", "description": "Path to the specific file to analyze"},
                            "include_suggestions": {"type": "boolean", "description": "Whether to include refactoring suggestions"}
                        },
                        "required": ["file_path"]
                    }
                }
            ]
        },
        "server": {
            "command": "valknut",
            "args": ["mcp-stdio"]
        }
    });

    let manifest_json = serde_json::to_string_pretty(&manifest)?;

    if let Some(output_path) = args.output {
        tokio::fs::write(&output_path, &manifest_json).await?;
        println!("MCP manifest saved to {}", output_path.display());
    } else {
        println!("{}", manifest_json);
    }

    Ok(())
}
