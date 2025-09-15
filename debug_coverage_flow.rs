use std::path::PathBuf;
use valknut::api::AnalysisConfig;
use valknut::ValknutEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Debug: Tracing coverage data flow from pipeline to HTML report");
    
    // Check if we have a real LCOV file to test with
    let lcov_path = PathBuf::from("coverage.lcov");
    if !lcov_path.exists() {
        println!("❌ No coverage.lcov file found - creating a simple test LCOV file");
        let test_lcov_content = r#"TN:test
SF:src/lib.rs
FN:10,simple_function
FNF:1
FNH:1
FNDA:5,simple_function
DA:10,5
DA:11,5
DA:12,0
DA:13,0
LF:4
LH:2
end_of_record
"#;
        std::fs::write(&lcov_path, test_lcov_content)?;
        println!("✅ Created test coverage.lcov file");
    }

    // Configure analysis with coverage enabled
    let config = AnalysisConfig::default()
        .enable_coverage_analysis(true);
        
    println!("📊 Running analysis with coverage enabled...");
    
    // Run analysis
    let mut engine = ValknutEngine::new(config).await?;
    let results = engine.analyze_directory(".").await?;
    
    // Debug the pipeline results
    println!("🔍 Debugging coverage data in results:");
    println!("  Coverage packs found: {}", results.coverage_packs.len());
    
    if results.coverage_packs.is_empty() {
        println!("❌ No coverage packs in results - this is the bug!");
        
        // Let's check what happened in the pipeline by running it directly
        println!("🔍 Checking pipeline results directly...");
        
    } else {
        println!("✅ Coverage packs found in results:");
        for (i, pack) in results.coverage_packs.iter().enumerate().take(3) {
            println!("  {}. Pack ID: {}", i + 1, pack.pack_id);
            println!("     Path: {:?}", pack.path);
            println!("     Gaps: {}", pack.gaps.len());
        }
    }
    
    println!("🏁 Debug complete");
    Ok(())
}