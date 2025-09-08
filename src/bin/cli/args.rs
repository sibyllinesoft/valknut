//! CLI Argument Structures and Configuration
//!
//! This module contains all CLI argument definitions, command structures,
//! and configuration enums used by the Valknut CLI binary.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// AI-Powered Code Analysis & Refactoring Assistant
#[derive(Parser)]
#[command(name = "valknut")]
#[command(version = VERSION)]
#[command(about = "🔍 Valknut - AI-Powered Code Analysis & Refactoring Assistant")]
#[command(long_about = "
Analyze your codebase for technical debt, complexity, and refactoring opportunities.
Generate professional reports for teams and integrate with development workflows.

Common Usage:

  # Quick analysis of current directory
  valknut analyze .
  
  # Generate team-friendly HTML report
  valknut analyze --format html --out reports/ ./src
  
  # Start MCP server for IDE integration
  valknut mcp-stdio
  
  # List supported programming languages
  valknut list-languages

Learn more: https://github.com/nathanricedev/valknut
")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging for debugging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Enable/disable usage analytics collection (default: enabled)
    #[arg(long, global = true)]
    pub survey: bool,

    /// Set survey invitation verbosity level
    #[arg(long, global = true, value_enum, default_value = "maximum")]
    pub survey_verbosity: SurveyVerbosity,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Analyze code repositories for refactorability
    Analyze(AnalyzeArgs),
    
    /// Print default configuration in YAML format
    #[command(name = "print-default-config")]
    PrintDefaultConfig,
    
    /// Initialize a configuration file with defaults
    #[command(name = "init-config")]
    InitConfig(InitConfigArgs),
    
    /// Validate a Valknut configuration file
    #[command(name = "validate-config")]
    ValidateConfig(ValidateConfigArgs),
    
    /// Run MCP server over stdio (for Claude Code integration)
    #[command(name = "mcp-stdio")]
    McpStdio(McpStdioArgs),
    
    /// Generate MCP manifest JSON
    #[command(name = "mcp-manifest")]
    McpManifest(McpManifestArgs),
    
    /// List supported programming languages and their status
    #[command(name = "list-languages")]
    ListLanguages,
    
    // Legacy individual analyzers for backward compatibility
    /// Analyze code structure and generate refactoring recommendations
    Structure(StructureArgs),
    
    /// Analyze semantic naming issues and generate renaming recommendations
    Names(NamesArgs),
    
    /// Analyze dependency cycles and clone detection for impact assessment
    Impact(ImpactArgs),
}

#[derive(Args)]
pub struct AnalyzeArgs {
    /// One or more directories or files to analyze
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,

    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Output directory for reports and analysis results
    #[arg(short, long, default_value = "out")]
    pub out: PathBuf,

    /// Output format: jsonl (line-delimited JSON), json (single file), markdown (team report), html (interactive report), sonar (SonarQube integration), csv (spreadsheet data)
    #[arg(short, long, value_enum, default_value = "jsonl")]
    pub format: OutputFormat,

    /// Suppress non-essential output
    #[arg(short, long)]
    pub quiet: bool,
}

#[derive(Args)]
pub struct InitConfigArgs {
    /// Output configuration file name
    #[arg(short, long, default_value = "valknut-config.yml")]
    pub output: PathBuf,

    /// Overwrite existing configuration file
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args)]
pub struct ValidateConfigArgs {
    /// Path to configuration file to validate
    #[arg(short, long, required = true)]
    pub config: PathBuf,

    /// Show detailed configuration breakdown
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct McpStdioArgs {
    /// Configuration file
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

#[derive(Args)]
pub struct McpManifestArgs {
    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

// Legacy analyzer args (backward compatibility)
#[derive(Args)]
pub struct StructureArgs {
    /// Path to the code directory to analyze
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Analyze specific file types (extensions separated by commas)
    #[arg(short = 'e', long, value_delimiter = ',')]
    pub extensions: Option<Vec<String>>,

    /// Enable only branch reorganization analysis
    #[arg(long)]
    pub branch_only: bool,

    /// Enable only file splitting analysis  
    #[arg(long)]
    pub file_split_only: bool,

    /// Maximum number of top recommendations to show
    #[arg(short = 'n', long)]
    pub top: Option<usize>,

    /// Output format for results
    #[arg(short = 'f', long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct NamesArgs {
    /// Path to the code directory to analyze
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Analyze specific file types (extensions separated by commas)
    #[arg(short = 'e', long, value_delimiter = ',')]
    pub extensions: Option<Vec<String>>,

    /// Minimum mismatch score threshold (0.0-1.0)
    #[arg(long, default_value = "0.65")]
    pub min_mismatch: f64,

    /// Minimum impact (external references) threshold
    #[arg(long, default_value = "3")]
    pub min_impact: usize,

    /// Include protected public API functions
    #[arg(long)]
    pub include_public_api: bool,

    /// Maximum number of rename suggestions to show
    #[arg(short = 'n', long, default_value = "20")]
    pub top: usize,

    /// Show only rename packs (exclude contract mismatch packs)
    #[arg(long)]
    pub renames_only: bool,

    /// Show only contract mismatch packs (exclude rename packs)
    #[arg(long)]
    pub contracts_only: bool,

    /// Output format for results
    #[arg(short = 'f', long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct ImpactArgs {
    /// Path to the code directory to analyze
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Analyze specific file types (extensions separated by commas)
    #[arg(short = 'e', long, value_delimiter = ',')]
    pub extensions: Option<Vec<String>>,

    /// Enable cycle detection and breaking recommendations
    #[arg(long)]
    pub cycles: bool,

    /// Enable clone detection and consolidation recommendations
    #[arg(long)]
    pub clones: bool,

    /// Enable chokepoint detection (high-centrality modules)
    #[arg(long)]
    pub chokepoints: bool,

    /// Minimum similarity threshold for clone detection (0.0-1.0)
    #[arg(long, default_value = "0.85")]
    pub min_similarity: f64,

    /// Minimum total lines of code for clone groups
    #[arg(long, default_value = "60")]
    pub min_total_loc: usize,

    /// Maximum number of recommendations to show
    #[arg(short = 'n', long, default_value = "10")]
    pub top: usize,

    /// Output format for results
    #[arg(short = 'f', long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    /// Line-delimited JSON format
    Jsonl,
    /// JSON format output
    Json,
    /// YAML format output  
    Yaml,
    /// Markdown team report
    Markdown,
    /// Interactive HTML report
    Html,
    /// SonarQube integration format
    Sonar,
    /// CSV spreadsheet data
    Csv,
    /// Human-readable format
    Pretty,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum SurveyVerbosity {
    Low,
    Medium,
    High,
    Maximum,
}