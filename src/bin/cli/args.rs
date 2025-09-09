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

  # Quick analysis of current directory (default)
  valknut analyze
  
  # Generate team-friendly HTML report
  valknut analyze --format html ./src
  
  # Analyze with custom output directory
  valknut analyze --out .valknut/reports
  
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
    
    
    /// Analyze dependency cycles and clone detection for impact assessment
    Impact(ImpactArgs),
}

#[derive(Args)]
pub struct AnalyzeArgs {
    /// One or more directories or files to analyze (defaults to current directory)
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Output directory for reports and analysis results
    #[arg(short, long, default_value = ".valknut")]
    pub out: PathBuf,

    /// Output format: jsonl (line-delimited JSON), json (single file), markdown (team report), html (interactive report), sonar (SonarQube integration), csv (spreadsheet data)
    #[arg(short, long, value_enum, default_value = "jsonl")]
    pub format: OutputFormat,

    /// Suppress non-essential output
    #[arg(short, long)]
    pub quiet: bool,

    // === Quality Gate Options ===
    /// Enable quality gate mode - fail with exit code 1 if thresholds are exceeded
    #[arg(long)]
    pub quality_gate: bool,

    /// Fail build if any issues are found (shorthand for quality gate mode)
    #[arg(long)]
    pub fail_on_issues: bool,

    /// Maximum allowed complexity score (0-100, lower is better) [default: 75]
    #[arg(long)]
    pub max_complexity: Option<f64>,

    /// Minimum required health score (0-100, higher is better) [default: 60]
    #[arg(long)]
    pub min_health: Option<f64>,

    /// Maximum allowed technical debt ratio (0-100, lower is better) [default: 30]
    #[arg(long)]
    pub max_debt: Option<f64>,

    /// Minimum required maintainability index (0-100, higher is better) [default: 20]
    #[arg(long)]
    pub min_maintainability: Option<f64>,

    /// Maximum allowed total issues count [default: 50]
    #[arg(long)]
    pub max_issues: Option<usize>,

    /// Maximum allowed critical issues count [default: 0]
    #[arg(long)]
    pub max_critical: Option<usize>,

    /// Maximum allowed high-priority issues count [default: 5]
    #[arg(long)]
    pub max_high_priority: Option<usize>,
}

#[derive(Args)]
pub struct InitConfigArgs {
    /// Output configuration file name
    #[arg(short, long, default_value = ".valknut.yml")]
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
    /// CI/CD summary format (concise JSON for automated systems)
    CiSummary,
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