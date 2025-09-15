#[cfg(test)]
mod coverage_fix_tests {
    use super::*;
    use crate::detectors::coverage::{CoverageExtractor, CoverageConfig};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_coverage_analysis_uses_real_data_not_fake() {
        // Set up coverage extractor
        let config = CoverageConfig::default();
        let extractor = CoverageExtractor::new(config);
        
        // Test with our real LCOV file
        let lcov_path = PathBuf::from("coverage.lcov");
        
        if !lcov_path.exists() {
            println!("Skipping test - coverage.lcov file not found");
            return;
        }
        
        println!("Testing coverage analysis with real LCOV file...");
        
        // Build coverage packs using the real analysis
        let coverage_packs = extractor.build_coverage_packs(vec![lcov_path]).await.unwrap();
        
        println!("Coverage packs found: {}", coverage_packs.len());
        
        // Verify we have some coverage packs
        assert!(!coverage_packs.is_empty(), "Should have found coverage packs");
        
        // Verify we're getting real source files, not the LCOV file itself
        let lcov_file_packs: Vec<_> = coverage_packs.iter()
            .filter(|pack| pack.path.to_str().map(|s| s.contains("coverage.lcov")).unwrap_or(false))
            .collect();
            
        assert!(lcov_file_packs.is_empty(), "Should not have coverage packs with LCOV file path");
        
        // Verify we have real Rust source files  
        let rust_source_packs: Vec<_> = coverage_packs.iter()
            .filter(|pack| pack.path.extension().map(|ext| ext == "rs").unwrap_or(false))
            .collect();
            
        assert!(!rust_source_packs.is_empty(), "Should have Rust source file coverage packs");
        
        // Check for fake functions
        let fake_functions: Vec<_> = coverage_packs.iter()
            .flat_map(|pack| &pack.gaps)
            .flat_map(|gap| &gap.symbols)
            .filter(|symbol| symbol.name.starts_with("uncovered_function_"))
            .collect();
            
        assert!(fake_functions.is_empty(), "Should not have any fake 'uncovered_function_X' symbols");
        
        // Print some results for manual verification
        if let Some(pack) = coverage_packs.first() {
            println!("First pack path: {:?}", pack.path);
            if let Some(gap) = pack.gaps.first() {
                println!("First gap span: {}:{}", gap.span.start, gap.span.end);
                if let Some(symbol) = gap.symbols.first() {
                    println!("First symbol: {} (type: {:?})", symbol.name, symbol.kind);
                }
            }
        }
        
        println!("✅ Coverage analysis is working correctly - no fake data found!");
    }
}