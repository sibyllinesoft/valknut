//! Clone Denoising Test Module
//! 
//! Comprehensive test suite for the complete clone denoising system.
//! Organized by testing phases and integration levels.

pub mod phase1_weighted_shingling_tests;
pub mod phase2_structural_gate_tests; 
pub mod phase3_stop_motifs_cache_tests;
pub mod phase4_auto_calibration_payoff_tests;
pub mod end_to_end_integration_tests;

/// Re-export test utilities and fixtures for other test modules
pub use crate::fixtures::clone_denoising_test_data::*;

#[cfg(test)]
mod test_suite_validation {
    use super::*;

    /// Validate that all major test categories are represented
    #[test]
    fn test_comprehensive_coverage() {
        // This test ensures all phases are covered by checking imports
        let _phase1 = phase1_weighted_shingling_tests::weighted_shingle_analyzer_tests;
        let _phase2 = phase2_structural_gate_tests::basic_block_analyzer_tests;  
        let _phase3 = phase3_stop_motifs_cache_tests::stop_motif_cache_tests;
        let _phase4 = phase4_auto_calibration_payoff_tests::auto_calibration_engine_tests;
        let _e2e = end_to_end_integration_tests::end_to_end_pipeline_tests;
        
        println!("✅ All test phases are properly imported and accessible");
    }

    /// Validate test data fixtures are accessible
    #[test]
    fn test_fixtures_available() {
        let boilerplate_data = create_boilerplate_heavy_dataset();
        let genuine_clones = create_genuine_clones_dataset();
        let multi_language = create_multi_language_ast_examples();
        let edge_cases = create_edge_case_dataset();
        
        assert!(!boilerplate_data.is_empty(), "Boilerplate dataset should not be empty");
        assert!(!genuine_clones.is_empty(), "Genuine clones dataset should not be empty");
        assert!(!multi_language.is_empty(), "Multi-language dataset should not be empty");
        assert!(!edge_cases.is_empty(), "Edge cases dataset should not be empty");
        
        println!("✅ All test fixtures are accessible and non-empty");
    }

    /// Validate performance test data scaling
    #[test]
    fn test_performance_data_scaling() {
        let sizes = vec![10, 50, 100];
        
        for size in sizes {
            let perf_data = create_performance_test_dataset(size);
            assert_eq!(perf_data.len(), size, "Performance dataset should have exact requested size");
            
            // Verify entities have varying complexity
            let complexities: Vec<_> = perf_data.iter()
                .map(|e| e.source_code.lines().count())
                .collect();
            
            let min_complexity = complexities.iter().min().unwrap();
            let max_complexity = complexities.iter().max().unwrap();
            
            assert!(max_complexity > min_complexity, 
                   "Should have varying complexity: min={}, max={}", min_complexity, max_complexity);
        }
        
        println!("✅ Performance data scaling works correctly");
    }

    /// Validate realistic codebase sample composition
    #[test]
    fn test_realistic_codebase_composition() {
        let realistic_sample = create_realistic_codebase_sample();
        let categories = categorize_test_entities(&realistic_sample);
        
        // Should have multiple categories represented
        assert!(categories.len() >= 4, "Should have at least 4 different categories: {:?}", categories.keys().collect::<Vec<_>>());
        
        // Check for expected categories
        assert!(categories.contains_key("boilerplate_decorators") || categories.contains_key("boilerplate_builders"),
               "Should contain some boilerplate patterns");
        assert!(categories.contains_key("genuine_clones"), "Should contain genuine clones");
        assert!(categories.contains_key("ast_patterns"), "Should contain AST patterns");
        
        println!("✅ Realistic codebase sample has proper composition: {:?}", categories);
    }
}