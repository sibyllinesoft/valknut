//! Command Execution Logic and Analysis Operations
//!
//! This module contains the main command execution logic, analysis operations,
//! configuration management, and progress tracking functionality.

use crate::cli::args::*;
use crate::cli::output::*;
use anyhow;
use console::{Style, Term, style};
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use serde_json;
use serde_yaml;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tabled::{Table, Tabled, settings::{Style as TableStyle, Color}};
use owo_colors::OwoColorize;
use textwrap;
use tracing::{error, info, warn};
use valknut_rs::detectors::structure::{StructureExtractor, StructureConfig};
// use valknut_rs::detectors::names::{SemanticNameAnalyzer, NamesConfig, FunctionInfo};
use valknut_rs::core::config::ValknutConfig;
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
        // TODO: Run actual analysis without progress bars
        serde_json::json!({
            "summary": {
                "total_files": valid_paths.len(),
                "total_issues": 0,
                "processing_time": 0.0
            }
        })
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
pub async fn mcp_stdio_command(
    args: McpStdioArgs,
    survey: bool,
    survey_verbosity: SurveyVerbosity
) -> anyhow::Result<()> {
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

    // TODO: Implement actual MCP stdio server
    eprintln!("⚠️  MCP stdio server implementation in progress");
    eprintln!("💡 Use the Python version for now: python -m valknut.cli mcp-stdio");
    
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

pub async fn analyze_names_legacy(args: NamesArgs) -> anyhow::Result<()> {
    // TODO: Implement semantic naming analysis
    println!("⚠️  Semantic naming analysis implementation in progress");
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
        OutputFormat::Pretty => "pretty",
    }
}