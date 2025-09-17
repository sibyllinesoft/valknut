//! Demonstration of the simplified configuration API
//!
//! This example shows how the new unified configuration system makes
//! it easier to configure Valknut for different use cases.

use valknut_rs::api::config_types::{AnalysisConfig, AnalysisModules};

type DynError = Box<dyn std::error::Error>;

fn main() -> Result<(), DynError> {
    println!("🔧 Valknut Configuration Simplification Demo");
    println!("============================================\n");

    // Example 1: Simple configuration for basic code analysis
    println!("📊 Example 1: Basic Code Quality Analysis");
    let basic_config = AnalysisConfig::new()
        .with_languages(vec!["rust".to_string(), "python".to_string()])
        .with_confidence_threshold(0.8)
        .with_max_files(1000);

    println!("Languages: {:?}", basic_config.languages.enabled);
    println!(
        "Confidence: {:.1}%",
        basic_config.quality.confidence_threshold * 100.0
    );
    println!("Max files: {:?}\n", basic_config.files.max_files);

    // Example 2: Using the fluent interface for complex configuration
    println!("🎯 Example 2: Advanced Configuration with Fluent Interface");
    let advanced_config = AnalysisConfig::new()
        .modules(|_| AnalysisModules::code_quality())
        .languages(|l| {
            l.add_language("rust")
                .add_language("typescript")
                .with_complexity_threshold("rust", 15.0)
                .with_max_file_size_mb(5.0)
        })
        .files(|f| {
            f.with_max_files(500).exclude_patterns(vec![
                "*/target/*".to_string(),
                "*/node_modules/*".to_string(),
            ])
        })
        .quality(|q| q.strict().with_timeout(120))
        .coverage(|c| c.with_search_paths(vec!["./coverage/".to_string()]));

    println!("Modules enabled:");
    println!("  • Complexity: {}", advanced_config.modules.complexity);
    println!("  • Dependencies: {}", advanced_config.modules.dependencies);
    println!("  • Duplicates: {}", advanced_config.modules.duplicates);
    println!("  • Refactoring: {}", advanced_config.modules.refactoring);

    println!("Languages: {:?}", advanced_config.languages.enabled);
    println!(
        "Rust complexity threshold: {:?}",
        advanced_config.languages.complexity_thresholds.get("rust")
    );
    println!("Strict mode: {}", advanced_config.quality.strict_mode);
    println!(
        "Coverage search paths: {:?}\n",
        advanced_config.coverage.search_paths
    );

    // Example 3: Quick presets for common use cases
    println!("⚡ Example 3: Quick Presets");

    let fast_analysis = AnalysisConfig::new()
        .essential_modules_only()
        .with_max_files(100);
    println!(
        "Fast analysis - only complexity module: {}",
        fast_analysis.modules.complexity
    );

    let comprehensive = AnalysisConfig::new().enable_all_modules();
    println!(
        "Comprehensive analysis - all modules: {}",
        comprehensive.modules.complexity
            && comprehensive.modules.dependencies
            && comprehensive.modules.duplicates
    );

    // Example 4: Validation in action
    println!("\n🔍 Example 4: Configuration Validation");

    // This should validate successfully
    match basic_config.validate() {
        Ok(()) => println!("✅ Basic config validation passed"),
        Err(e) => println!("❌ Basic config validation failed: {}", e),
    }

    // This should fail validation (invalid confidence threshold)
    let invalid_config = AnalysisConfig::new().with_confidence_threshold(1.5); // Invalid: > 1.0

    match invalid_config.validate() {
        Ok(()) => println!("❌ Invalid config should have failed validation"),
        Err(e) => println!("✅ Invalid config correctly rejected: {}", e),
    }

    // Example 5: Serialization and deserialization
    println!("\n💾 Example 5: Configuration Serialization");
    let json_config = serde_json::to_string_pretty(&basic_config)?;
    println!("Configuration serialized to JSON:");
    println!("{}", json_config);

    let deserialized: AnalysisConfig = serde_json::from_str(&json_config)?;
    println!("✅ Successfully deserialized configuration");

    println!("\n🎉 Configuration simplification complete!");
    println!("The new API reduces cognitive load while maintaining full functionality.");

    Ok(())
}
