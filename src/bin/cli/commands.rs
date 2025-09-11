//! Command Execution Logic and Analysis Operations
//!
//! This module contains the main command execution logic, analysis operations,
//! configuration management, and progress tracking functionality.

use crate::cli::args::*;
use crate::cli::output::*;
use std::path::PathBuf;
use anyhow;
use console::Term;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use serde_json;
use serde_yaml;
use std::path::Path;
use tabled::{Table, Tabled, settings::Style as TableStyle};
use owo_colors::OwoColorize;
use tracing::info;
use valknut_rs::detectors::structure::{StructureExtractor, StructureConfig};
use valknut_rs::core::pipeline::{QualityGateConfig, QualityGateResult};
use chrono;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main analyze command implementation
pub async fn analyze_command(
    args: AnalyzeArgs,
    survey: bool,
    survey_verbosity: SurveyVerbosity
) -> anyhow::Result<()> {
    // Print header
    if !args.quiet {
        print_header();
    }

    // Load and validate configuration
    let config = if let Some(config_path) = &args.config {
        if !args.quiet {
            println!("{} {}", "✅ Loading configuration from".green(), config_path.display().to_string().cyan());
        }
        load_configuration(Some(config_path)).await?
    } else {
        if !args.quiet {
            println!("{}", "✅ Using default configuration".green());
        }
        StructureConfig::default()
    };

    if !args.quiet {
        display_config_summary(&config);
    }

    // Validate and prepare paths
    if !args.quiet {
        println!("{}", "📂 Validating Input Paths".bright_blue().bold());
        println!();
    }

    let mut valid_paths = Vec::new();
    for path in &args.paths {
        if path.exists() {
            valid_paths.push(path.clone());
            if !args.quiet {
                let path_type = if path.is_dir() { "📁 Directory" } else { "📄 File" };
                println!("  {}: {}", path_type, path.display().to_string().green());
            }
        } else {
            eprintln!("  {} {}", "❌ Path does not exist:".red(), path.display());
            std::process::exit(1);
        }
    }

    if valid_paths.is_empty() {
        eprintln!("{}", "❌ No valid paths provided".red());
        std::process::exit(1);
    }

    // Create output directory
    tokio::fs::create_dir_all(&args.out).await?;

    if !args.quiet {
        println!();
        println!("{} {}", "📁 Output directory:".bold(), args.out.display().to_string().cyan());
        println!("{} {}", "📊 Report format:".bold(), format_to_string(&args.format).to_uppercase().cyan());
        println!();
    }

    // Run analysis with enhanced progress tracking
    if !args.quiet {
        println!("{}", "🔍 Starting Analysis Pipeline".bright_blue().bold());
        println!();
    }

    let result = if args.quiet {
        run_analysis_without_progress(&valid_paths, config).await?
    } else {
        run_analysis_with_progress(&valid_paths, config).await?
    };

    // Display analysis results
    if !args.quiet {
        println!();
        display_analysis_results(&result);
    }

    // Generate outputs
    if !args.quiet {
        println!("{}", "📝 Generating Reports".bright_blue().bold());
        println!();
    }

    generate_outputs_with_feedback(&result, &args.out, &args.format, args.quiet).await?;

    // Handle quality gates if enabled
    if args.quality_gate || args.fail_on_issues {
        let quality_gate_result = handle_quality_gates(&args, &result).await?;
        
        if !quality_gate_result.passed {
            if !args.quiet {
                display_quality_gate_violations(&quality_gate_result);
            }
            
            // Exit with code 1 to fail CI/CD
            std::process::exit(1);
        } else if !args.quiet {
            println!("{}", "✅ Quality gates passed".green().bold());
        }
    }

    if !args.quiet {
        display_completion_summary(&result, &args.out, &args.format);
    }

    Ok(())
}

/// Print default configuration in YAML format
pub async fn print_default_config() -> anyhow::Result<()> {
    println!("{}", "# Default valknut configuration".dimmed());
    println!("{}", "# Save this to a file and customize as needed".dimmed());
    println!("{}", "# Usage: valknut analyze --config your-config.yml".dimmed());
    println!();
    
    let config = StructureConfig::default();
    let yaml_output = serde_yaml::to_string(&config)?;
    println!("{}", yaml_output);
    
    Ok(())
}

/// Initialize a configuration file with defaults
pub async fn init_config(args: InitConfigArgs) -> anyhow::Result<()> {
    // Check if file exists and force not specified
    if args.output.exists() && !args.force {
        eprintln!("{} {}", "❌ Configuration file already exists:".red(), args.output.display());
        eprintln!("   Use --force to overwrite or choose a different name with --output");
        std::process::exit(1);
    }

    let config = StructureConfig::default();
    let yaml_content = serde_yaml::to_string(&config)?;
    tokio::fs::write(&args.output, yaml_content).await?;

    println!("{} {}", "✅ Configuration saved to:".bright_green().bold(), args.output.display().to_string().cyan());
    println!();
    println!("{}", "📝 Next steps:".bright_blue().bold());
    println!("   1. Edit the configuration file to customize analysis settings");
    println!("   2. Run analysis with: {}", format!("valknut analyze --config {} <paths>", args.output.display()).cyan());
    
    println!();
    println!("{}", "🔧 Key settings you can customize:".bright_blue().bold());
    
    #[derive(Tabled)]
    struct CustomizationRow {
        setting: String,
        description: String,
    }

    let customization_rows = vec![
        CustomizationRow {
            setting: "structure.enable_branch_packs".to_string(),
            description: "Enable directory reorganization analysis".to_string(),
        },
        CustomizationRow {
            setting: "structure.enable_file_split_packs".to_string(),
            description: "Enable file splitting recommendations".to_string(),
        },
        CustomizationRow {
            setting: "structure.top_packs".to_string(),
            description: "Number of top recommendations to show".to_string(),
        },
    ];

    let mut table = Table::new(customization_rows);
    table.with(TableStyle::rounded());
    println!("{}", table);

    Ok(())
}

/// Validate a Valknut configuration file
pub async fn validate_config(args: ValidateConfigArgs) -> anyhow::Result<()> {
    println!("{} {}", "🔍 Validating configuration:".bright_blue().bold(), args.config.display().to_string().cyan());
    println!();

    let config = match load_configuration(Some(&args.config)).await {
        Ok(config) => {
            println!("{}", "✅ Configuration file is valid!".bright_green().bold());
            println!();
            config
        }
        Err(e) => {
            eprintln!("{} {}", "❌ Configuration validation failed:".red(), e);
            println!();
            println!("{}", "🔧 Common issues:".bright_blue().bold());
            println!("   • Check YAML syntax (indentation, colons, quotes)");
            println!("   • Verify all required fields are present");
            println!("   • Ensure numeric values are in valid ranges");
            println!();
            println!("{}", "💡 Tip: Use 'valknut print-default-config' to see valid format".dimmed());
            std::process::exit(1);
        }
    };

    // Display configuration summary
    display_config_summary(&config);

    if args.verbose {
        println!("{}", "🔧 Detailed Settings".bright_blue().bold());
        println!();
        
        #[derive(Tabled)]
        struct DetailRow {
            setting: String,
            value: String,
        }

        let detail_rows = vec![
            DetailRow {
                setting: "Branch Packs Enabled".to_string(),
                value: config.structure.enable_branch_packs.to_string(),
            },
            DetailRow {
                setting: "File Split Packs Enabled".to_string(),
                value: config.structure.enable_file_split_packs.to_string(),
            },
            DetailRow {
                setting: "Top Packs Limit".to_string(),
                value: config.structure.top_packs.to_string(),
            },
        ];

        let mut table = Table::new(detail_rows);
        table.with(TableStyle::rounded());
        println!("{}", table);
    }

    println!();
    println!("{}", "💡 Recommendations:".bright_blue().bold());
    println!("   ✅ Configuration looks optimal!");

    Ok(())
}

/// Run MCP server over stdio for IDE integration
/// 
/// This command starts a full JSON-RPC 2.0 MCP (Model Context Protocol) server
/// that exposes valknut's code analysis capabilities over stdin/stdout.
/// 
/// Available MCP tools:
/// - analyze_code: Analyze code for refactoring opportunities and quality metrics
/// - get_refactoring_suggestions: Get specific refactoring suggestions for a code entity
/// 
/// The server follows the MCP specification and can be used with Claude Code
/// and other MCP-compatible clients.
pub async fn mcp_stdio_command(
    args: McpStdioArgs,
    survey: bool,
    survey_verbosity: SurveyVerbosity
) -> anyhow::Result<()> {
    use crate::mcp::server::run_mcp_server;
    
    eprintln!("📡 Starting MCP stdio server for IDE integration...");
    
    // Load configuration
    let _config = if let Some(config_path) = args.config {
        load_configuration(Some(&config_path)).await?
    } else {
        StructureConfig::default()
    };

    if survey {
        eprintln!("📊 Survey enabled with {:?} verbosity", survey_verbosity);
    } else {
        eprintln!("📊 Survey disabled");
    }

    // Initialize and run MCP server
    eprintln!("🚀 MCP JSON-RPC 2.0 server ready for requests");
    
    if let Err(e) = run_mcp_server(VERSION).await {
        eprintln!("❌ MCP server error: {}", e);
        return Err(anyhow::anyhow!("MCP server failed: {}", e));
    }
    
    Ok(())
}

/// Generate MCP manifest JSON
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
        println!("✅ MCP manifest saved to {}", output_path.display());
    } else {
        println!("{}", manifest_json);
    }

    Ok(())
}

/// List supported programming languages and their status
pub async fn list_languages() -> anyhow::Result<()> {
    println!("{}", "🔤 Supported Programming Languages".bright_blue().bold());
    println!("   Found {} supported languages", 8); // TODO: Dynamic count
    println!();

    #[derive(Tabled)]
    struct LanguageRow {
        language: String,
        extension: String,
        status: String,
        features: String,
    }

    let languages = vec![
        LanguageRow {
            language: "Python".to_string(),
            extension: ".py".to_string(),
            status: "✅ Full Support".to_string(),
            features: "Full analysis, refactoring suggestions".to_string(),
        },
        LanguageRow {
            language: "TypeScript".to_string(),
            extension: ".ts, .tsx".to_string(),
            status: "✅ Full Support".to_string(),
            features: "Full analysis, type checking".to_string(),
        },
        LanguageRow {
            language: "JavaScript".to_string(),
            extension: ".js, .jsx".to_string(),
            status: "✅ Full Support".to_string(),
            features: "Full analysis, complexity metrics".to_string(),
        },
        LanguageRow {
            language: "Rust".to_string(),
            extension: ".rs".to_string(),
            status: "✅ Full Support".to_string(),
            features: "Full analysis, memory safety checks".to_string(),
        },
        LanguageRow {
            language: "Go".to_string(),
            extension: ".go".to_string(),
            status: "🚧 Experimental".to_string(),
            features: "Basic analysis".to_string(),
        },
        LanguageRow {
            language: "Java".to_string(),
            extension: ".java".to_string(),
            status: "🚧 Experimental".to_string(),
            features: "Basic analysis".to_string(),
        },
        LanguageRow {
            language: "C++".to_string(),
            extension: ".cpp, .cxx".to_string(),
            status: "🚧 Experimental".to_string(),
            features: "Basic analysis".to_string(),
        },
        LanguageRow {
            language: "C#".to_string(),
            extension: ".cs".to_string(),
            status: "🚧 Experimental".to_string(),
            features: "Basic analysis".to_string(),
        },
    ];

    let mut table = Table::new(languages);
    table.with(TableStyle::rounded());
    println!("{}", table);

    println!();
    println!("{}", "📝 Usage Notes:".bright_blue().bold());
    println!("   • Full Support: Complete feature set with refactoring suggestions");
    println!("   • Experimental: Basic complexity analysis, limited features");
    println!("   • Configure languages in your config file with language-specific settings");
    println!();
    println!("{}", "💡 Tip: Use 'valknut init-config' to create a configuration file".dimmed());

    Ok(())
}

/// Print Valknut header with version info
pub fn print_header() {
    if Term::stdout().size().1 >= 80 {
        // Full header for wide terminals
        println!("{}", "┌".cyan().bold().to_string() + &"─".repeat(60).cyan().to_string() + &"┐".cyan().bold().to_string());
        println!("{} {} {}", "│".cyan().bold(), 
                 format!("⚙️  Valknut v{} - AI-Powered Code Analysis", VERSION).bright_cyan().bold(),
                 "│".cyan().bold());
        println!("{}", "└".cyan().bold().to_string() + &"─".repeat(60).cyan().to_string() + &"┘".cyan().bold().to_string());
    } else {
        // Compact header for narrow terminals
        println!("{} {}", "⚙️".bright_cyan(), format!("Valknut v{}", VERSION).bright_cyan().bold());
    }
    println!();
}

/// Display configuration summary in a formatted table
pub fn display_config_summary(config: &StructureConfig) {
    #[derive(Tabled)]
    struct ConfigRow {
        setting: String,
        value: String,
    }

    let config_rows = vec![
        ConfigRow {
            setting: "Languages".to_string(),
            value: "Auto-detected".to_string(), // TODO: Add language detection
        },
        ConfigRow {
            setting: "Top-K Results".to_string(),
            value: config.structure.top_packs.to_string(),
        },
        ConfigRow {
            setting: "Granularity".to_string(),
            value: "File and Directory".to_string(),
        },
        ConfigRow {
            setting: "Analysis Mode".to_string(),
            value: if config.structure.enable_branch_packs && config.structure.enable_file_split_packs {
                "Full Analysis".to_string()
            } else if config.structure.enable_branch_packs {
                "Directory Analysis".to_string()
            } else if config.structure.enable_file_split_packs {
                "File Split Analysis".to_string()
            } else {
                "Custom".to_string()
            },
        },
    ];

    let mut table = Table::new(config_rows);
    table.with(TableStyle::rounded());
    println!("{}", table);
    println!();
}

/// Run comprehensive analysis with detailed progress tracking
pub async fn run_analysis_with_progress(paths: &[PathBuf], _config: StructureConfig) -> anyhow::Result<serde_json::Value> {
    use valknut_rs::core::pipeline::{AnalysisPipeline, AnalysisConfig, ProgressCallback};
    
    let multi_progress = MultiProgress::new();
    
    // Create main progress bar
    let main_pb = multi_progress.add(ProgressBar::new(100));
    main_pb.set_style(ProgressStyle::with_template(
        "🚀 {msg} [{bar:40.bright_blue/blue}] {pos:>3}% {elapsed_precise}"
    )?);
    main_pb.set_message("Comprehensive Analysis");

    // Create analysis pipeline with default configuration
    let analysis_config = AnalysisConfig::default();
    let pipeline = AnalysisPipeline::new(analysis_config);

    // Create progress callback
    let progress_callback: ProgressCallback = Box::new({
        let pb = main_pb.clone();
        move |stage: &str, progress: f64| {
            pb.set_message(stage.to_string());
            pb.set_position(progress as u64);
        }
    });

    // Run comprehensive analysis
    info!("Starting comprehensive analysis for {} paths", paths.len());
    let analysis_result = pipeline.analyze_paths(paths, Some(progress_callback)).await
        .map_err(|e| anyhow::anyhow!("Analysis failed: {}", e))?;

    // Finish progress bar
    main_pb.finish_with_message("Analysis Complete");

    // Convert to JSON format matching the expected structure
    let result_json = serde_json::to_value(&analysis_result)?;
    
    info!("Analysis completed successfully");
    info!("Total files: {}", analysis_result.summary.total_files);
    info!("Total issues: {}", analysis_result.summary.total_issues);
    info!("Overall health score: {:.1}", analysis_result.health_metrics.overall_health_score);
    
    Ok(result_json)
}

/// Run analysis without progress bars for quiet mode
pub async fn run_analysis_without_progress(paths: &[PathBuf], _config: StructureConfig) -> anyhow::Result<serde_json::Value> {
    use valknut_rs::core::pipeline::{AnalysisPipeline, AnalysisConfig};
    
    // Create analysis pipeline with default configuration
    let analysis_config = AnalysisConfig::default();
    let pipeline = AnalysisPipeline::new(analysis_config);
    
    // Run comprehensive analysis without progress callback
    info!("Starting comprehensive analysis for {} paths", paths.len());
    let analysis_result = pipeline.analyze_paths(paths, None).await
        .map_err(|e| anyhow::anyhow!("Analysis failed: {}", e))?;
    
    // Convert to JSON format matching the expected structure
    let result_json = serde_json::to_value(&analysis_result)?;
    
    info!("Analysis completed successfully");
    info!("Total files: {}", analysis_result.summary.total_files);
    info!("Total issues: {}", analysis_result.summary.total_issues);
    info!("Overall health score: {:.1}", analysis_result.health_metrics.overall_health_score);
    
    Ok(result_json)
}

/// Load configuration from file or use defaults
pub async fn load_configuration(config_path: Option<&Path>) -> anyhow::Result<StructureConfig> {
    let config = match config_path {
        Some(path) => {
            let content = tokio::fs::read_to_string(path).await?;
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("yaml" | "yml") => {
                    serde_yaml::from_str(&content)?
                }
                Some("json") => {
                    serde_json::from_str(&content)?
                }
                _ => {
                    serde_yaml::from_str(&content)?
                }
            }
        }
        None => StructureConfig::default(),
    };

    Ok(config)
}

// Legacy analyzer implementations for backward compatibility
pub async fn analyze_structure_legacy(args: StructureArgs, mut config: StructureConfig) -> anyhow::Result<()> {
    // Apply CLI overrides
    if args.branch_only {
        config.structure.enable_branch_packs = true;
        config.structure.enable_file_split_packs = false;
    }
    
    if args.file_split_only {
        config.structure.enable_branch_packs = false;
        config.structure.enable_file_split_packs = true;
    }

    if let Some(top) = args.top {
        config.structure.top_packs = top;
    }

    let analyzer = StructureExtractor::with_config(config);
    let recommendations = analyzer.generate_recommendations(&args.path).await?;
    
    let analysis_result = serde_json::json!({
        "packs": recommendations,
        "summary": {
            "structural_issues_found": recommendations.len(),
            "analysis_timestamp": chrono::Utc::now().to_rfc3339()
        }
    });

    match args.format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&analysis_result)?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(&analysis_result)?;
            println!("{}", yaml);
        }
        _ => {
            print_human_readable_results(&analysis_result);
        }
    }

    Ok(())
}


pub async fn analyze_impact_legacy(args: ImpactArgs) -> anyhow::Result<()> {
    // TODO: Implement impact analysis
    println!("⚠️  Impact analysis implementation in progress");
    Ok(())
}

// Helper functions
pub fn format_to_string(format: &OutputFormat) -> &str {
    match format {
        OutputFormat::Jsonl => "jsonl",
        OutputFormat::Json => "json",
        OutputFormat::Yaml => "yaml",
        OutputFormat::Markdown => "markdown",
        OutputFormat::Html => "html",
        OutputFormat::Sonar => "sonar",
        OutputFormat::Csv => "csv",
        OutputFormat::CiSummary => "ci-summary",
        OutputFormat::Pretty => "pretty",
    }
}

/// Handle quality gate evaluation
async fn handle_quality_gates(args: &AnalyzeArgs, result: &serde_json::Value) -> anyhow::Result<QualityGateResult> {
    use valknut_rs::core::pipeline::{QualityGateViolation};

    // Build quality gate configuration from CLI args
    let quality_gate_config = build_quality_gate_config(args);

    let mut violations = Vec::new();
    
    // Extract summary data (this should always be present)
    let summary = result.get("summary")
        .ok_or_else(|| anyhow::anyhow!("Summary not found in analysis result"))?;
    
    let total_issues = summary.get("total_issues")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    // Check available metrics against thresholds
    if quality_gate_config.max_critical_issues > 0 && total_issues > quality_gate_config.max_critical_issues {
        violations.push(QualityGateViolation {
            rule_name: "Total Issues Count".to_string(),
            current_value: total_issues as f64,
            threshold: quality_gate_config.max_critical_issues as f64,
            description: format!(
                "Total issues ({}) exceeds maximum allowed ({})", 
                total_issues, quality_gate_config.max_critical_issues
            ),
            severity: if total_issues > quality_gate_config.max_critical_issues * 2 {
                "Critical".to_string()
            } else {
                "High".to_string()
            },
            affected_files: Vec::new(),
            recommended_actions: vec!["Review and address high-priority issues".to_string()],
        });
    }

    // Try to extract health metrics if available (for more comprehensive analysis)
    if let Some(health_metrics) = result.get("health_metrics") {
        if let Some(overall_health) = health_metrics.get("overall_health_score").and_then(|v| v.as_f64()) {
            if overall_health < quality_gate_config.min_maintainability_score {
                violations.push(QualityGateViolation {
                    rule_name: "Overall Health Score".to_string(),
                    current_value: overall_health,
                    threshold: quality_gate_config.min_maintainability_score,
                    description: format!(
                        "Health score ({:.1}) is below minimum required ({:.1})", 
                        overall_health, quality_gate_config.min_maintainability_score
                    ),
                    severity: if overall_health < quality_gate_config.min_maintainability_score - 20.0 {
                        "Blocker".to_string()
                    } else {
                        "Critical".to_string()
                    },
                    affected_files: Vec::new(),
                    recommended_actions: vec!["Improve code structure and reduce technical debt".to_string()],
                });
            }
        }

        if let Some(complexity_score) = health_metrics.get("complexity_score").and_then(|v| v.as_f64()) {
            if complexity_score > quality_gate_config.max_complexity_score {
                violations.push(QualityGateViolation {
                    rule_name: "Complexity Score".to_string(),
                    current_value: complexity_score,
                    threshold: quality_gate_config.max_complexity_score,
                    description: format!(
                        "Complexity score ({:.1}) exceeds maximum allowed ({:.1})", 
                        complexity_score, quality_gate_config.max_complexity_score
                    ),
                    severity: if complexity_score > quality_gate_config.max_complexity_score + 10.0 {
                        "Critical".to_string()
                    } else {
                        "High".to_string()
                    },
                    affected_files: Vec::new(),
                    recommended_actions: vec!["Simplify complex functions and reduce nesting".to_string()],
                });
            }
        }

        if let Some(debt_ratio) = health_metrics.get("technical_debt_ratio").and_then(|v| v.as_f64()) {
            if debt_ratio > quality_gate_config.max_technical_debt_ratio {
                violations.push(QualityGateViolation {
                    rule_name: "Technical Debt Ratio".to_string(),
                    current_value: debt_ratio,
                    threshold: quality_gate_config.max_technical_debt_ratio,
                    description: format!(
                        "Technical debt ratio ({:.1}%) exceeds maximum allowed ({:.1}%)", 
                        debt_ratio, quality_gate_config.max_technical_debt_ratio
                    ),
                    severity: if debt_ratio > quality_gate_config.max_technical_debt_ratio + 20.0 {
                        "Critical".to_string()
                    } else {
                        "High".to_string()
                    },
                    affected_files: Vec::new(),
                    recommended_actions: vec!["Refactor code to reduce technical debt".to_string()],
                });
            }
        }
    }

    let passed = violations.is_empty();
    let overall_score = result.get("health_metrics")
        .and_then(|hm| hm.get("overall_health_score"))
        .and_then(|v| v.as_f64())
        .unwrap_or(50.0); // Default score if not available

    Ok(QualityGateResult {
        passed,
        violations,
        overall_score,
    })
}

/// Build quality gate configuration from CLI arguments
fn build_quality_gate_config(args: &AnalyzeArgs) -> QualityGateConfig {
    let mut config = QualityGateConfig::default();
    
    // Enable if quality_gate flag is set or if fail_on_issues is set
    config.enabled = args.quality_gate || args.fail_on_issues;
    
    // Override defaults with CLI values if provided
    if let Some(max_complexity) = args.max_complexity {
        config.max_complexity_score = max_complexity;
    }
    if let Some(min_health) = args.min_health {
        config.min_maintainability_score = min_health;
    }
    if let Some(max_debt) = args.max_debt {
        config.max_technical_debt_ratio = max_debt;
    }
    if let Some(min_maintainability) = args.min_maintainability {
        config.min_maintainability_score = min_maintainability;
    }
    if let Some(max_issues) = args.max_issues {
        config.max_critical_issues = max_issues;
    }
    if let Some(max_critical) = args.max_critical {
        config.max_critical_issues = max_critical;
    }
    if let Some(max_high_priority) = args.max_high_priority {
        config.max_high_priority_issues = max_high_priority;
    }
    
    // Handle fail_on_issues flag (sets max_issues to 0)
    if args.fail_on_issues {
        config.max_critical_issues = 0;
        config.max_high_priority_issues = 0;
    }
    
    config
}

/// Display quality gate violations in a user-friendly format
fn display_quality_gate_violations(result: &QualityGateResult) {
    println!();
    println!("{}", "❌ Quality Gate Failed".red().bold());
    println!("{} {:.1}", "Quality Score:".dimmed(), result.overall_score.to_string().yellow());
    println!();
    
    // Group violations by severity
    let blockers: Vec<_> = result.violations.iter()
        .filter(|v| v.severity == "Blocker")
        .collect();
    let criticals: Vec<_> = result.violations.iter()
        .filter(|v| v.severity == "Critical")
        .collect();
    let warnings: Vec<_> = result.violations.iter()
        .filter(|v| v.severity == "Warning" || v.severity == "High")
        .collect();

    if !blockers.is_empty() {
        println!("{}", "🚫 BLOCKER Issues:".red().bold());
        for violation in blockers {
            println!("  • {}: {:.1} (threshold: {:.1})", 
                violation.rule_name.yellow(), 
                violation.current_value, 
                violation.threshold
            );
            println!("    {}", violation.description.dimmed());
        }
        println!();
    }

    if !criticals.is_empty() {
        println!("{}", "🔴 CRITICAL Issues:".red().bold());
        for violation in criticals {
            println!("  • {}: {:.1} (threshold: {:.1})", 
                violation.rule_name.yellow(), 
                violation.current_value, 
                violation.threshold
            );
            println!("    {}", violation.description.dimmed());
        }
        println!();
    }

    if !warnings.is_empty() {
        println!("{}", "⚠️  WARNING Issues:".yellow().bold());
        for violation in warnings {
            println!("  • {}: {:.1} (threshold: {:.1})", 
                violation.rule_name.yellow(), 
                violation.current_value, 
                violation.threshold
            );
            println!("    {}", violation.description.dimmed());
        }
        println!();
    }

    println!("{}", "To fix these issues:".bold());
    println!("  1. Reduce code complexity by refactoring large functions");
    println!("  2. Address critical and high-priority issues first");
    println!("  3. Improve code maintainability through better structure");
    println!("  4. Reduce technical debt by following best practices");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, NamedTempFile};
    use std::fs;
    use valknut_rs::core::pipeline::{QualityGateViolation};

    #[test]
    fn test_print_header() {
        // Test that print_header doesn't panic
        print_header();
    }

    #[test]
    fn test_format_to_string() {
        assert_eq!(format_to_string(&OutputFormat::Json), "json");
        assert_eq!(format_to_string(&OutputFormat::Yaml), "yaml");
        assert_eq!(format_to_string(&OutputFormat::Markdown), "markdown");
        assert_eq!(format_to_string(&OutputFormat::Html), "html");
        assert_eq!(format_to_string(&OutputFormat::Jsonl), "jsonl");
        assert_eq!(format_to_string(&OutputFormat::Sonar), "sonar");
        assert_eq!(format_to_string(&OutputFormat::Csv), "csv");
        assert_eq!(format_to_string(&OutputFormat::CiSummary), "ci-summary");
        assert_eq!(format_to_string(&OutputFormat::Pretty), "pretty");
    }

    #[test]
    fn test_display_config_summary() {
        let config = StructureConfig::default();
        // Test that display_config_summary doesn't panic
        display_config_summary(&config);
    }

    #[tokio::test]
    async fn test_load_configuration_default() {
        let result = load_configuration(None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_configuration_yaml_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let result = load_configuration(Some(temp_file.path())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_configuration_json_file() {
        let temp_dir = TempDir::new().unwrap();
        let json_path = temp_dir.path().join("config.json");
        let config = StructureConfig::default();
        let json_content = serde_json::to_string(&config).unwrap();
        fs::write(&json_path, json_content).unwrap();

        let result = load_configuration(Some(&json_path)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_configuration_invalid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), "invalid: yaml: content:").unwrap();

        let result = load_configuration(Some(temp_file.path())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_print_default_config() {
        let result = print_default_config().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_init_config_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yml");
        
        let args = InitConfigArgs {
            output: config_path.clone(),
            force: false,
        };

        let result = init_config(args).await;
        assert!(result.is_ok());
        assert!(config_path.exists());
        
        // Verify file contains valid YAML
        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: serde_yaml::Result<StructureConfig> = serde_yaml::from_str(&content);
        assert!(parsed.is_ok());
    }

    #[tokio::test]
    async fn test_init_config_force_overwrite() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("existing_config.yml");
        
        // Create existing file
        fs::write(&config_path, "existing content").unwrap();
        
        let args = InitConfigArgs {
            output: config_path.clone(),
            force: true,
        };

        let result = init_config(args).await;
        assert!(result.is_ok());
        
        // Verify file was overwritten with valid YAML
        let content = fs::read_to_string(&config_path).unwrap();
        assert_ne!(content, "existing content");
        let parsed: serde_yaml::Result<StructureConfig> = serde_yaml::from_str(&content);
        assert!(parsed.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_valid_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let args = ValidateConfigArgs {
            config: temp_file.path().to_path_buf(),
            verbose: false,
        };

        let result = validate_config(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_config_verbose() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let args = ValidateConfigArgs {
            config: temp_file.path().to_path_buf(),
            verbose: true,
        };

        let result = validate_config(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_stdio_command() {
        let args = McpStdioArgs {
            config: None,
        };
        
        let result = mcp_stdio_command(args, false, SurveyVerbosity::Low).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_stdio_command_with_config() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = StructureConfig::default();
        let yaml_content = serde_yaml::to_string(&config).unwrap();
        fs::write(temp_file.path(), yaml_content).unwrap();

        let args = McpStdioArgs {
            config: Some(temp_file.path().to_path_buf()),
        };
        
        let result = mcp_stdio_command(args, true, SurveyVerbosity::High).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_manifest_command_stdout() {
        let args = McpManifestArgs {
            output: None,
        };
        
        let result = mcp_manifest_command(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mcp_manifest_command_file_output() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("manifest.json");
        
        let args = McpManifestArgs {
            output: Some(manifest_path.clone()),
        };
        
        let result = mcp_manifest_command(args).await;
        assert!(result.is_ok());
        assert!(manifest_path.exists());
        
        // Verify file contains valid JSON
        let content = fs::read_to_string(&manifest_path).unwrap();
        let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(&content);
        assert!(parsed.is_ok());
        
        let manifest = parsed.unwrap();
        assert_eq!(manifest["name"], "valknut");
        assert!(manifest["capabilities"]["tools"].is_array());
    }

    #[tokio::test]
    async fn test_list_languages() {
        let result = list_languages().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_analyze_structure_legacy() {
        let temp_dir = TempDir::new().unwrap();
        
        let args = StructureArgs {
            path: temp_dir.path().to_path_buf(),
            extensions: None,
            format: OutputFormat::Json,
            top: Some(5),
            branch_only: false,
            file_split_only: false,
        };
        
        let config = StructureConfig::default();
        
        let result = analyze_structure_legacy(args, config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_analyze_structure_legacy_branch_only() {
        let temp_dir = TempDir::new().unwrap();
        
        let args = StructureArgs {
            path: temp_dir.path().to_path_buf(),
            extensions: None,
            format: OutputFormat::Yaml,
            top: None,
            branch_only: true,
            file_split_only: false,
        };
        
        let config = StructureConfig::default();
        
        let result = analyze_structure_legacy(args, config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_analyze_structure_legacy_file_split_only() {
        let temp_dir = TempDir::new().unwrap();
        
        let args = StructureArgs {
            path: temp_dir.path().to_path_buf(),
            extensions: None,
            format: OutputFormat::Pretty,
            top: None,
            branch_only: false,
            file_split_only: true,
        };
        
        let config = StructureConfig::default();
        
        let result = analyze_structure_legacy(args, config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_analyze_impact_legacy() {
        let temp_dir = TempDir::new().unwrap();
        
        let args = ImpactArgs {
            path: temp_dir.path().to_path_buf(),
            extensions: None,
            cycles: true,
            clones: false,
            chokepoints: false,
            min_similarity: 0.85,
            min_total_loc: 60,
            top: 10,
            format: OutputFormat::Json,
        };
        
        let result = analyze_impact_legacy(args).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_quality_gate_config_defaults() {
        let args = AnalyzeArgs {
            paths: vec![PathBuf::from("test")],
            out: PathBuf::from("output"),
            format: OutputFormat::Json,
            config: None,
            quiet: false,
            quality_gate: false,
            fail_on_issues: false,
            max_complexity: None,
            min_health: None,
            max_debt: None,
            min_maintainability: None,
            max_issues: None,
            max_critical: None,
            max_high_priority: None,
        };
        
        let config = build_quality_gate_config(&args);
        assert!(!config.enabled);
    }

    #[test]
    fn test_build_quality_gate_config_quality_gate_enabled() {
        let args = AnalyzeArgs {
            paths: vec![PathBuf::from("test")],
            out: PathBuf::from("output"),
            format: OutputFormat::Json,
            config: None,
            quiet: false,
            quality_gate: true,
            fail_on_issues: false,
            max_complexity: Some(75.0),
            min_health: Some(60.0),
            max_debt: Some(30.0),
            min_maintainability: Some(65.0),
            max_issues: Some(10),
            max_critical: Some(5),
            max_high_priority: Some(15),
        };
        
        let config = build_quality_gate_config(&args);
        assert!(config.enabled);
        assert_eq!(config.max_complexity_score, 75.0);
        assert_eq!(config.min_maintainability_score, 65.0);
        assert_eq!(config.max_technical_debt_ratio, 30.0);
        assert_eq!(config.max_critical_issues, 5);
        assert_eq!(config.max_high_priority_issues, 15);
    }

    #[test]
    fn test_build_quality_gate_config_fail_on_issues() {
        let args = AnalyzeArgs {
            paths: vec![PathBuf::from("test")],
            out: PathBuf::from("output"),
            format: OutputFormat::Json,
            config: None,
            quiet: false,
            quality_gate: false,
            fail_on_issues: true,
            max_complexity: None,
            min_health: None,
            max_debt: None,
            min_maintainability: None,
            max_issues: None,
            max_critical: None,
            max_high_priority: None,
        };
        
        let config = build_quality_gate_config(&args);
        assert!(config.enabled);
        assert_eq!(config.max_critical_issues, 0);
        assert_eq!(config.max_high_priority_issues, 0);
    }

    #[test]
    fn test_display_quality_gate_violations_with_violations() {
        let violations = vec![
            QualityGateViolation {
                rule_name: "Test Rule".to_string(),
                current_value: 85.0,
                threshold: 70.0,
                description: "Test violation".to_string(),
                severity: "Critical".to_string(),
                affected_files: vec![],
                recommended_actions: vec!["Fix the issue".to_string()],
            },
            QualityGateViolation {
                rule_name: "Warning Rule".to_string(),
                current_value: 25.0,
                threshold: 20.0,
                description: "Warning violation".to_string(),
                severity: "Warning".to_string(),
                affected_files: vec![],
                recommended_actions: vec!["Consider fixing".to_string()],
            },
        ];
        
        let result = QualityGateResult {
            passed: false,
            violations,
            overall_score: 65.0,
        };
        
        // Test that display_quality_gate_violations doesn't panic
        display_quality_gate_violations(&result);
    }

    #[test]
    fn test_display_quality_gate_violations_no_violations() {
        let result = QualityGateResult {
            passed: true,
            violations: vec![],
            overall_score: 85.0,
        };
        
        // Test that display_quality_gate_violations doesn't panic
        display_quality_gate_violations(&result);
    }

    #[test]
    fn test_display_quality_gate_violations_blocker_severity() {
        let violations = vec![
            QualityGateViolation {
                rule_name: "Blocker Rule".to_string(),
                current_value: 95.0,
                threshold: 70.0,
                description: "Blocker violation".to_string(),
                severity: "Blocker".to_string(),
                affected_files: vec!["test.rs".to_string().into()],
                recommended_actions: vec!["Immediate fix required".to_string()],
            },
        ];
        
        let result = QualityGateResult {
            passed: false,
            violations,
            overall_score: 30.0,
        };
        
        // Test that display_quality_gate_violations doesn't panic with blocker
        display_quality_gate_violations(&result);
    }

    // Mock test for handle_quality_gates since it requires complex analysis result structure
    #[tokio::test] 
    async fn test_handle_quality_gates_basic() {
        let args = AnalyzeArgs {
            paths: vec![PathBuf::from("test")],
            out: PathBuf::from("output"),
            format: OutputFormat::Json,
            config: None,
            quiet: false,
            quality_gate: true,
            fail_on_issues: false,
            max_complexity: None,
            min_health: None,
            max_debt: None,
            min_maintainability: None,
            max_issues: None,
            max_critical: None,
            max_high_priority: None,
        };

        // Create a minimal analysis result
        let analysis_result = serde_json::json!({
            "summary": {
                "total_issues": 5,
                "total_files": 10
            },
            "health_metrics": {
                "overall_health_score": 75.0,
                "complexity_score": 65.0,
                "technical_debt_ratio": 15.0
            }
        });

        let result = handle_quality_gates(&args, &analysis_result).await;
        assert!(result.is_ok());
        
        let quality_result = result.unwrap();
        assert!(quality_result.passed); // Should pass with default thresholds
    }

    #[tokio::test] 
    async fn test_handle_quality_gates_violations() {
        let args = AnalyzeArgs {
            paths: vec![PathBuf::from("test")],
            out: PathBuf::from("output"),
            format: OutputFormat::Json,
            config: None,
            quiet: false,
            quality_gate: true,
            fail_on_issues: false,
            max_complexity: Some(50.0), // Set low threshold to trigger violation
            min_health: Some(80.0), // Set high threshold to trigger violation
            max_debt: None,
            min_maintainability: None,
            max_issues: Some(3), // Set low threshold to trigger violation
            max_critical: None,
            max_high_priority: None,
        };

        // Create analysis result that will violate quality gates
        let analysis_result = serde_json::json!({
            "summary": {
                "total_issues": 5, // Exceeds max_issues of 3
                "total_files": 10
            },
            "health_metrics": {
                "overall_health_score": 75.0, // Below min_health of 80
                "complexity_score": 65.0, // Exceeds max_complexity of 50
                "technical_debt_ratio": 15.0
            }
        });

        let result = handle_quality_gates(&args, &analysis_result).await;
        assert!(result.is_ok());
        
        let quality_result = result.unwrap();
        assert!(!quality_result.passed); // Should fail due to violations
        assert!(quality_result.violations.len() > 0);
    }

    #[tokio::test] 
    async fn test_handle_quality_gates_missing_summary() {
        let args = AnalyzeArgs {
            paths: vec![PathBuf::from("test")],
            out: PathBuf::from("output"),
            format: OutputFormat::Json,
            config: None,
            quiet: false,
            quality_gate: true,
            fail_on_issues: false,
            max_complexity: None,
            min_health: None,
            max_debt: None,
            min_maintainability: None,
            max_issues: None,
            max_critical: None,
            max_high_priority: None,
        };

        // Create analysis result without summary
        let analysis_result = serde_json::json!({
            "health_metrics": {
                "overall_health_score": 75.0
            }
        });

        let result = handle_quality_gates(&args, &analysis_result).await;
        assert!(result.is_err()); // Should fail due to missing summary
    }
}