//! CLI Module Organization
//!
//! This module organizes the CLI functionality into cohesive sub-modules:
//! - args: CLI argument structures and configuration types
//! - commands: Main command execution logic and analysis operations
//! - config_builder: Configuration building from CLI arguments
//! - config_layer: Configuration layer management and merging
//! - output: Output formatting, report generation, and display functions
//! - quality_gates: Quality gate evaluation and violation handling
//! - reports: Report generation for various output formats

pub mod args;
pub mod commands;
pub mod config_builder;
pub mod config_layer;
pub mod output;
pub mod quality_gates;
pub mod reports;

// Re-export commonly used items for convenience
pub use args::*;
pub use commands::*;
