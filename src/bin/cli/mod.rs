//! CLI Module Organization
//!
//! This module organizes the CLI functionality into cohesive sub-modules:
//! - args: CLI argument structures and configuration types
//! - commands: Main command execution logic and analysis operations
//! - output: Output formatting, report generation, and display functions

pub mod args;
pub mod commands;
pub mod output;

// Re-export commonly used items for convenience
pub use args::*;
pub use commands::*;
pub use output::*;