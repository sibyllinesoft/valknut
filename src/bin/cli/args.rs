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

  # Comprehensive analysis (all analyses enabled by default)
  valknut analyze
  
  # Generate team-friendly HTML report with coverage discovery
  valknut analyze --format html ./src
  
  # Disable specific analyses if not needed
  valknut analyze --no-coverage --no-impact ./src
  
  # Use specific coverage file instead of auto-discovery
  valknut analyze --coverage-file ./coverage.xml ./src
  
  # Custom output directory
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
    Analyze(Box<AnalyzeArgs>),

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

    /// Audit documentation coverage and README freshness
    #[command(name = "doc-audit")]
    DocAudit(DocAuditArgs),
}

/// Quality gate configuration for CI/CD integration
#[derive(Args)]
pub struct QualityGateArgs {
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

/// Clone detection and denoising configuration
#[derive(Args)]
pub struct CloneDetectionArgs {
    /// Enable semantic clone detection with LSH analysis
    #[arg(long)]
    pub semantic_clones: bool,

    /// Enable strict dedupe analysis with enhanced noise filtering
    #[arg(long)]
    pub strict_dedupe: bool,

    /// Enable the advanced (but slower) clone denoising system for higher accuracy  
    #[arg(long)]
    pub denoise: bool,

    /// Minimum function tokens for clone detection (default: 40)
    #[arg(long)]
    pub min_function_tokens: Option<usize>,

    /// Minimum match tokens for clone detection (default: 24)
    #[arg(long)]
    pub min_match_tokens: Option<usize>,

    /// Minimum distinct blocks required for meaningful matches (default: 2)
    #[arg(long)]
    pub require_blocks: Option<usize>,

    /// Similarity threshold for clone detection (0.0-1.0, default: 0.82)
    #[arg(long)]
    pub similarity: Option<f64>,

    /// Dry-run mode - analyze but don't change behavior (for testing)
    #[arg(long)]
    pub denoise_dry_run: bool,
}

/// Advanced clone detection tuning (rarely needed - use config file instead)
#[derive(Args)]
pub struct AdvancedCloneArgs {
    /// Disable automatic threshold calibration (denoising is enabled by default)
    #[arg(long)]
    pub no_auto: bool,

    /// Perform loose sweep analysis on top N candidates for threshold tuning
    #[arg(long)]
    pub loose_sweep: bool,

    /// Enable TF-IDF rarity weighting for structural analysis
    #[arg(long)]
    pub rarity_weighting: bool,

    /// Enable structural validation with PDG motifs and basic blocks
    #[arg(long)]
    pub structural_validation: bool,

    /// Enable optional APTED verification for structural clone confirmation
    #[arg(long)]
    pub apted_verify: bool,

    /// Maximum AST nodes to include when building APTED trees
    #[arg(long)]
    pub apted_max_nodes: Option<usize>,

    /// Maximum clone candidates per entity to verify with APTED
    #[arg(long)]
    pub apted_max_pairs: Option<usize>,

    /// Disable APTED verification (enabled by default)
    #[arg(long)]
    pub no_apted_verify: bool,

    /// Enable live reachability boost for clone prioritization
    #[arg(long)]
    pub live_reach_boost: bool,

    /// AST similarity weight (0.0-1.0, default: 0.35)
    #[arg(long)]
    pub ast_weight: Option<f64>,

    /// PDG similarity weight (0.0-1.0, default: 0.45)
    #[arg(long)]
    pub pdg_weight: Option<f64>,

    /// Embedding similarity weight (0.0-1.0, default: 0.20)
    #[arg(long)]
    pub emb_weight: Option<f64>,

    /// I/O mismatch penalty (0.0-1.0, default: 0.25)
    #[arg(long)]
    pub io_mismatch_penalty: Option<f64>,

    /// Auto-calibration quality target (0.0-1.0, default: 0.8)
    #[arg(long)]
    pub quality_target: Option<f64>,

    /// Auto-calibration sample size (default: 200)
    #[arg(long)]
    pub sample_size: Option<usize>,

    /// Minimum saved tokens for ranking (default: 100)
    #[arg(long)]
    pub min_saved_tokens: Option<usize>,

    /// Minimum rarity gain threshold (default: 1.2)
    #[arg(long)]
    pub min_rarity_gain: Option<f64>,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum DocAuditFormat {
    Text,
    Json,
}

/// Documentation audit configuration options
#[derive(Args, Clone, Debug)]
pub struct DocAuditArgs {
    /// Project root to scan (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub root: PathBuf,

    /// Require READMEs for directories above this descendant threshold
    #[arg(long, default_value_t = doc_audit::DEFAULT_COMPLEXITY_THRESHOLD)]
    pub complexity_threshold: usize,

    /// Mark README as stale after this many commits touch the directory
    #[arg(long, default_value_t = doc_audit::DEFAULT_MAX_README_COMMITS)]
    pub max_readme_commits: usize,

    /// Exit with non-zero status when any issues are detected
    #[arg(long)]
    pub strict: bool,

    /// Output format for audit results
    #[arg(long, value_enum, default_value = "text")]
    pub format: DocAuditFormat,

    /// Additional directory names to ignore (repeatable)
    #[arg(long)]
    pub ignore_dir: Vec<String>,

    /// Additional file suffixes to ignore (repeatable)
    #[arg(long)]
    pub ignore_suffix: Vec<String>,
}

/// Coverage analysis configuration
#[derive(Args)]
pub struct CoverageArgs {
    /// Disable coverage analysis (enabled by default for comprehensive analysis)
    #[arg(long)]
    pub no_coverage: bool,

    /// Specific coverage file to use (overrides auto-discovery)
    #[arg(long)]
    pub coverage_file: Option<PathBuf>,

    /// Disable automatic coverage file discovery
    #[arg(long)]
    pub no_coverage_auto_discover: bool,

    /// Maximum age of coverage files in days (default: 7, 0 = no limit)
    #[arg(long)]
    pub coverage_max_age_days: Option<u32>,
}

/// Analysis module enable/disable flags
#[derive(Args)]
pub struct AnalysisControlArgs {
    /// Disable complexity analysis
    #[arg(long)]
    pub no_complexity: bool,

    /// Disable structure analysis
    #[arg(long)]
    pub no_structure: bool,

    /// Disable refactoring analysis
    #[arg(long)]
    pub no_refactoring: bool,

    /// Disable impact analysis (dependency cycles, centrality)
    #[arg(long)]
    pub no_impact: bool,

    /// Disable LSH clone detection analysis
    #[arg(long)]
    pub no_lsh: bool,
}

/// AI-powered analysis features
#[derive(Args)]
pub struct AIFeaturesArgs {
    /// Enable AI refactoring oracle using Gemini 2.5 Pro (requires GEMINI_API_KEY env var)
    #[arg(long)]
    pub oracle: bool,

    /// Maximum tokens to send to refactoring oracle (default: 500000)
    #[arg(long)]
    pub oracle_max_tokens: Option<usize>,
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

    /// Performance optimization profile to balance speed vs thoroughness
    #[arg(long, value_enum, default_value = "fast")]
    pub profile: PerformanceProfile,

    #[command(flatten)]
    pub quality_gate: QualityGateArgs,

    #[command(flatten)]
    pub clone_detection: CloneDetectionArgs,

    #[command(flatten)]
    pub advanced_clone: AdvancedCloneArgs,

    #[command(flatten)]
    pub coverage: CoverageArgs,

    #[command(flatten)]
    pub analysis_control: AnalysisControlArgs,

    #[command(flatten)]
    pub ai_features: AIFeaturesArgs,
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

impl OutputFormat {
    #[must_use]
    pub fn is_machine_readable(&self) -> bool {
        matches!(
            self,
            OutputFormat::Json
                | OutputFormat::Jsonl
                | OutputFormat::Yaml
                | OutputFormat::Csv
                | OutputFormat::Sonar
                | OutputFormat::CiSummary
        )
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum SurveyVerbosity {
    Low,
    Medium,
    High,
    Maximum,
}

/// Performance optimization profiles
#[derive(Debug, Clone, ValueEnum)]
pub enum PerformanceProfile {
    /// Fast mode - minimal analysis, optimized for speed
    Fast,
    /// Balanced mode - good balance of speed and thoroughness (default)
    Balanced,
    /// Thorough mode - comprehensive analysis, optimized for accuracy
    Thorough,
    /// Extreme mode - maximum analysis depth, all optimizations enabled
    Extreme,
}
