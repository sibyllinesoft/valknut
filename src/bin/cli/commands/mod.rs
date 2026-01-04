//! CLI Command Implementations
//!
//! This module contains all command implementations for the Valknut CLI:
//! - analyze: Main code analysis command
//! - config: Configuration management commands
//! - doc_audit: Documentation audit command
//! - mcp: MCP server commands
//! - oracle: AI refactoring oracle commands

pub mod analyze;
pub mod config;
pub mod doc_audit;
pub mod mcp;
pub mod oracle;

// Re-export analyze command items (previously at cli::commands level)
pub use analyze::*;

// Re-export config command items
pub use config::{init_config, print_default_config, validate_config};
pub use super::config_builder::load_configuration;

// Re-export doc_audit command
pub use doc_audit::doc_audit_command;

// Re-export mcp commands
pub use mcp::{mcp_manifest_command, mcp_stdio_command};

// Re-export oracle commands
pub use oracle::{run_oracle_analysis, run_oracle_dry_run};
