//! Legacy Analysis Commands
//!
//! This module contains deprecated CLI commands for backward compatibility.
//! These commands are hidden from help output and will be removed in v2.0.

use crate::cli::args::{ImpactArgs, StructureArgs};
use crate::cli::output::*;
use anyhow;
use std::path::Path;
use tracing::{debug, info, warn};

use valknut_rs::core::config::{AnalysisConfig, ValknutConfig};
use valknut_rs::detectors::structure::{StructureConfig, StructureExtractor};

/// Load legacy structure analysis configuration  
pub async fn load_configuration(_config_path: Option<&Path>) -> anyhow::Result<StructureConfig> {
    debug!("Using default configuration for legacy structure analysis");
    Ok(StructureConfig::default())
}

/// Legacy structure analysis command (hidden, for backward compatibility)
pub async fn analyze_structure_legacy(args: StructureArgs) -> anyhow::Result<()> {
    warn!("The 'structure' command is deprecated. Use 'analyze --no-coverage --no-refactoring --no-impact' instead.");

    let _config = load_configuration(None).await?;
    let extractor = StructureExtractor::new();

    let start_time = std::time::Instant::now();
    let results = extractor
        .analyze_directory_for_reorg(&args.path)
        .map_err(|e| anyhow::anyhow!("Structure analysis failed: {}", e))?;
    let duration = start_time.elapsed();

    // Format and display results
    match results {
        Some(pack) => println!(
            "Structure analysis found reorganization opportunity in {}",
            pack.dir.display()
        ),
        None => println!("Structure analysis completed - no reorganization needed"),
    }

    info!(
        "Structure analysis completed in {:.2}s",
        duration.as_secs_f64()
    );

    Ok(())
}

/// Legacy impact analysis command (hidden, for backward compatibility)
pub async fn analyze_impact_legacy(args: ImpactArgs) -> anyhow::Result<()> {
    warn!("The 'impact' command is deprecated. Use 'analyze --no-coverage --no-refactoring --no-structure' instead.");

    // Create a basic configuration for impact analysis
    let valknut_config = ValknutConfig {
        analysis: AnalysisConfig {
            enable_scoring: false,
            enable_graph_analysis: true, // Enable impact analysis
            enable_lsh_analysis: false,
            enable_refactoring_analysis: false,
            enable_coverage_analysis: false,
            enable_structure_analysis: false,
            enable_names_analysis: false,
            ..Default::default()
        },
        ..Default::default()
    };

    println!("🔍 Legacy Impact Analysis");
    println!("Path: {}", args.path.display());
    println!();

    // Note: Full impact analysis implementation would go here
    // For now, we just show the deprecation warning

    println!("⚠️  This command is deprecated and provides limited functionality.");
    println!("💡 Use 'valknut analyze --no-coverage --no-refactoring --no-structure {}' for full analysis.", args.path.display());

    Ok(())
}
