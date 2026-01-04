//! Configuration management commands.
//!
//! This module contains commands for managing valknut configuration files,
//! including initialization, validation, and printing defaults.

use std::path::Path;

use owo_colors::OwoColorize;
use serde_json;
use serde_yaml;
use tabled::{settings::Style as TableStyle, Table, Tabled};

use crate::cli::analysis_display::display_config_summary;
use crate::cli::args::{InitConfigArgs, ValidateConfigArgs};
use crate::cli::config_builder::load_configuration;
use valknut_rs::detectors::structure::StructureConfig;

/// Print default configuration in YAML format
pub async fn print_default_config() -> anyhow::Result<()> {
    println!("{}", "# Default valknut configuration".dimmed());
    println!(
        "{}",
        "# Save this to a file and customize as needed".dimmed()
    );
    println!(
        "{}",
        "# Usage: valknut analyze --config your-config.yml".dimmed()
    );
    println!();

    let config = valknut_rs::core::config::ValknutConfig::default();
    let yaml_output = serde_yaml::to_string(&config)?;
    println!("{}", yaml_output);

    Ok(())
}

/// Initialize a configuration file with defaults
pub async fn init_config(args: InitConfigArgs) -> anyhow::Result<()> {
    // Check if file exists and force not specified
    if args.output.exists() && !args.force {
        return Err(anyhow::anyhow!(
            "Configuration file already exists: {}. Use --force to overwrite or choose a different name with --output",
            args.output.display()
        ));
    }

    let config = valknut_rs::core::config::ValknutConfig::default();
    let yaml_content = serde_yaml::to_string(&config)?;
    tokio::fs::write(&args.output, yaml_content).await?;

    println!(
        "{} {}",
        "✅ Configuration saved to:".bright_green().bold(),
        args.output.display().to_string().cyan()
    );
    println!();
    println!("{}", "📝 Next steps:".bright_blue().bold());
    println!("   1. Edit the configuration file to customize analysis settings");
    println!(
        "   2. Run analysis with: {}",
        format!("valknut analyze --config {} <paths>", args.output.display()).cyan()
    );

    println!();
    println!(
        "{}",
        "🔧 Key settings you can customize:".bright_blue().bold()
    );

    /// Row type for the configuration tips table.
    #[derive(Tabled)]
    struct CustomizationRow {
        setting: String,
        description: String,
    }

    let customization_rows = vec![
        CustomizationRow {
            setting: "denoise.enabled".to_string(),
            description: "Enable intelligent clone detection (default: true)".to_string(),
        },
        CustomizationRow {
            setting: "denoise.auto".to_string(),
            description: "Enable auto-calibration (default: true)".to_string(),
        },
        CustomizationRow {
            setting: "denoise.min_function_tokens".to_string(),
            description: "Minimum function size for analysis (default: 40)".to_string(),
        },
        CustomizationRow {
            setting: "denoise.similarity".to_string(),
            description: "Similarity threshold for clone detection (default: 0.82)".to_string(),
        },
        CustomizationRow {
            setting: "structure.enable_branch_packs".to_string(),
            description: "Enable directory reorganization analysis".to_string(),
        },
        CustomizationRow {
            setting: "structure.enable_file_split_packs".to_string(),
            description: "Enable file splitting recommendations".to_string(),
        },
    ];

    let mut table = Table::new(customization_rows);
    table.with(TableStyle::rounded());
    println!("{}", table);

    Ok(())
}

/// Validate a Valknut configuration file
pub async fn validate_config(args: ValidateConfigArgs) -> anyhow::Result<()> {
    println!(
        "{} {}",
        "🔍 Validating configuration:".bright_blue().bold(),
        args.config.display().to_string().cyan()
    );
    println!();

    let config = match load_configuration(Some(&args.config)).await {
        Ok(config) => {
            println!(
                "{}",
                "✅ Configuration file is valid!".bright_green().bold()
            );
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
            println!(
                "{}",
                "💡 Tip: Use 'valknut print-default-config' to see valid format".dimmed()
            );
            return Err(anyhow::anyhow!("Configuration validation failed: {}", e));
        }
    };

    // Display configuration summary
    display_config_summary(&config);

    if args.verbose {
        println!("{}", "🔧 Detailed Settings".bright_blue().bold());
        println!();

        /// Row used when printing verbose configuration details.
        #[derive(Tabled)]
        struct DetailRow {
            setting: String,
            value: String,
        }

        let detail_rows = vec![
            DetailRow {
                setting: "Branch Packs Enabled".to_string(),
                value: config.enable_branch_packs.to_string(),
            },
            DetailRow {
                setting: "File Split Packs Enabled".to_string(),
                value: config.enable_file_split_packs.to_string(),
            },
            DetailRow {
                setting: "Top Packs Limit".to_string(),
                value: config.top_packs.to_string(),
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
