//! Tests for Phase 2: Structural Analysis
//!
//! Tests the current structural analysis components including:
//! - BasicBlockAnalyzer - basic block analysis
//! - PdgMotifAnalyzer - structural motif extraction
//! - Current API functionality

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use valknut_rs::core::config::{DedupeConfig, ValknutConfig};
use valknut_rs::core::featureset::{CodeEntity, ExtractionContext};
use valknut_rs::detectors::clone_detection::{
    BasicBlockAnalyzer, MotifCategory, PdgMotif, PdgMotifAnalyzer,
};

#[cfg(test)]
mod basic_block_analyzer_tests {
    use super::*;

    /// Test basic block analysis with current API
    #[test]
    fn test_basic_block_analysis() {
        let analyzer = BasicBlockAnalyzer::new();

        // Test simple linear code
        let linear_code = "x = 1\ny = 2\nreturn x + y";
        let linear_blocks = analyzer.analyze_basic_blocks(linear_code);
        assert!(
            !linear_blocks.is_empty(),
            "Linear code should produce basic blocks"
        );

        // Test conditional code
        let conditional_code = r#"
if x > 0:
    result = x * 2
else:
    result = x * -1
return result
"#;
        let conditional_blocks = analyzer.analyze_basic_blocks(conditional_code);
        assert!(
            conditional_blocks.len() >= 1,
            "Conditional code should produce basic blocks, got {}",
            conditional_blocks.len()
        );

        // Test loop code
        let loop_code = r#"
result = 0
for i in range(n):
    if i % 2 == 0:
        result += i
    else:
        result -= i
return result
"#;
        let loop_blocks = analyzer.analyze_basic_blocks(loop_code);
        assert!(
            loop_blocks.len() >= 1,
            "Loop code should produce basic blocks, got {}",
            loop_blocks.len()
        );
        
        // Check that at least one block contains a return
        assert!(
            loop_blocks.iter().any(|b| b.contains_return),
            "Should detect return statements in blocks"
        );
    }

    /// Test basic block properties
    #[test]
    fn test_basic_block_properties() {
        let analyzer = BasicBlockAnalyzer::new();

        // Function with calls
        let func_with_calls = r#"
result = process_data(input)
if validate(result):
    return transform(result)
else:
    return default_value()
"#;

        let blocks = analyzer.analyze_basic_blocks(func_with_calls);
        
        // Should detect function calls
        assert!(
            blocks.iter().any(|b| b.contains_call),
            "Should detect function calls in blocks"
        );
        
        // Should detect return statements
        assert!(
            blocks.iter().any(|b| b.contains_return),
            "Should detect return statements in blocks"
        );
    }

    /// Test analyzer configuration
    #[test]
    fn test_analyzer_configuration() {
        use valknut_rs::detectors::clone_detection::pdg_analyzer::BasicBlockConfig;
        
        // Test with default configuration
        let default_analyzer = BasicBlockAnalyzer::new();
        let code = "x = 1\ny = 2\nreturn x + y";
        let default_blocks = default_analyzer.analyze_basic_blocks(code);
        
        // Test with custom configuration
        let custom_config = BasicBlockConfig {
            include_empty_blocks: true,
            merge_sequential_blocks: false,
            compute_dominance: false,
            analyze_dependencies: false,
        };
        let custom_analyzer = BasicBlockAnalyzer::with_config(custom_config);
        let custom_blocks = custom_analyzer.analyze_basic_blocks(code);
        
        // Both should produce some blocks
        assert!(!default_blocks.is_empty(), "Default analyzer should produce blocks");
        assert!(!custom_blocks.is_empty(), "Custom analyzer should produce blocks");
    }

    /// Test edge cases for block analysis
    #[test]
    fn test_block_analysis_edge_cases() {
        let analyzer = BasicBlockAnalyzer::new();

        // Empty code
        let empty_blocks = analyzer.analyze_basic_blocks("");
        // Should handle empty code gracefully
        
        // Code with only comments
        let comment_code = "# This is a comment\n# Another comment";
        let comment_blocks = analyzer.analyze_basic_blocks(comment_code);
        // Should handle comment-only code
        
        // Complex nested control flow
        let nested_code = r#"
result = 0
if x > 0:
    for i in range(x):
        if i % 2 == 0:
            if y > i:
                result += i * y
            else:
                result -= i
        else:
            result += i
else:
    while y > 0:
        result += y
        y -= 1
return result
"#;
        let nested_blocks = analyzer.analyze_basic_blocks(nested_code);
        assert!(
            nested_blocks.len() >= 1,
            "Deeply nested function should produce basic blocks, got {}",
            nested_blocks.len()
        );
        
        // Should detect loop patterns
        assert!(
            nested_blocks.iter().any(|b| b.is_loop_header),
            "Should detect loop headers in nested code"
        );
    }
}

#[cfg(test)]
mod pdg_motif_analyzer_tests {
    use super::*;

    /// Test PDG motif extraction from control flow
    #[test]
    fn test_pdg_motif_extraction() {
        let mut analyzer = PdgMotifAnalyzer::new();

        let control_flow_code = r#"
def control_flow(x, y):
    if x > y:
        result = x - y
        if result > 10:
            return result * 2
        else:
            return result
    else:
        for i in range(y):
            x += i
        return x
"#;

        let entity = CodeEntity::new("cf", "function", "control_flow", "/test/cf.py")
            .with_source_code(control_flow_code);

        let motifs = analyzer.extract_motifs(control_flow_code, "control_flow_test");

        // Should extract different types of motifs
        assert!(
            !motifs.is_empty(),
            "Should extract PDG motifs from control flow"
        );

        // Check for different motif categories
        let categories: HashSet<MotifCategory> =
            motifs.iter().map(|m| m.category.clone()).collect();
        assert!(
            categories.contains(&MotifCategory::Conditional),
            "Should extract conditional motifs"
        );
    }

    /// Test PDG motif pattern matching for similar structures
    #[test]
    fn test_pdg_motif_pattern_matching() {
        let mut analyzer = PdgMotifAnalyzer::new();

        // Two functions with similar control structure
        let func1 = r#"if x > 0:
    return x * 2
return 0"#;

        let func2 = r#"if y > 0:
    return y * 3
return 0"#;

        let motifs1 = analyzer.extract_motifs(func1, "similar1");
        let motifs2 = analyzer.extract_motifs(func2, "similar2");

        // Similar structures should extract similar motifs
        assert!(!motifs1.is_empty(), "Should extract motifs from first function");
        assert!(!motifs2.is_empty(), "Should extract motifs from second function");
        
        // Both should contain conditional motifs
        assert!(motifs1.iter().any(|m| m.category == MotifCategory::Conditional));
        assert!(motifs2.iter().any(|m| m.category == MotifCategory::Conditional));
    }

    /// Test motif detection for shared structural patterns
    #[test]
    fn test_shared_structural_patterns() {
        let mut analyzer = PdgMotifAnalyzer::new();

        // Two functions with multiple shared structural patterns
        let func1_code = r#"result = []
for item in items:
    if item.is_valid():
        processed = item.process()
        result.append(processed)
return result"#;

        let func2_code = r#"output = []
for entry in data:
    if entry.is_valid():
        transformed = entry.transform()
        output.append(transformed)
return output"#;

        let motifs1 = analyzer.extract_motifs(func1_code, "shared_structure1");
        let motifs2 = analyzer.extract_motifs(func2_code, "shared_structure2");

        // Both should extract similar types of motifs
        assert!(!motifs1.is_empty(), "Should extract motifs from first function");
        assert!(!motifs2.is_empty(), "Should extract motifs from second function");
        
        // Both should contain loop and conditional motifs
        assert!(motifs1.iter().any(|m| m.category == MotifCategory::Loop));
        assert!(motifs1.iter().any(|m| m.category == MotifCategory::Conditional));
        assert!(motifs2.iter().any(|m| m.category == MotifCategory::Loop));
        assert!(motifs2.iter().any(|m| m.category == MotifCategory::Conditional));

        // Test functions with different structures
        let diff_func_code = r#"total = 0
i = 0
while i < n:
    total += i * i
    i += 1
return total"#;

        let motifs_diff = analyzer.extract_motifs(diff_func_code, "different_structure");
        
        // Should extract different pattern (while loop instead of for loop)
        assert!(!motifs_diff.is_empty(), "Should extract motifs from different function");
        assert!(motifs_diff.iter().any(|m| m.category == MotifCategory::Loop));
    }

    /// Test motif categorization accuracy
    #[test]
    fn test_motif_categorization() {
        let mut analyzer = PdgMotifAnalyzer::new();

        // Function with various constructs
        let mixed_code = r#"result = initialize_result()
if config.enabled:
    processed_data = process_data(data)
    cache = {}
    for item in processed_data:
        cache[item.key] = item.value
    result.update(cache)
return result"#;

        let motifs = analyzer.extract_motifs(mixed_code, "mixed_constructs");

        // Check for different categories of motifs
        let categories: HashSet<MotifCategory> =
            motifs.iter().map(|m| m.category.clone()).collect();

        assert!(
            !categories.is_empty(),
            "Should detect various motifs"
        );
        
        // Should detect at least conditional and loop motifs
        assert!(
            categories.contains(&MotifCategory::Conditional),
            "Should detect conditional motifs"
        );
        assert!(
            categories.contains(&MotifCategory::Loop),
            "Should detect loop motifs"
        );
    }
}

// NOTE: structural_gate_analyzer_tests module was removed because StructuralGateAnalyzer
// no longer exists in the current API after refactoring. The functionality has been
// simplified and integrated into other components.

#[cfg(test)]
mod property_based_tests {
    use super::*;
    use proptest::proptest;

    proptest! {
        /// Property: Block count should be monotonic with code complexity
        #[test]
        fn prop_block_count_monotonic_with_complexity(
            num_conditions in 0usize..10,
            num_loops in 0usize..5
        ) {
            let analyzer = BasicBlockAnalyzer::new();

            // Generate code with controlled complexity
            let mut code = "def test_func(x):\n".to_string();

            // Add sequential conditions
            for i in 0..num_conditions {
                code.push_str(&format!("    if x > {}:\n        x += {}\n", i, i));
            }

            // Add loops
            for i in 0..num_loops {
                code.push_str(&format!("    for j in range({}):\n        x += j\n", i + 1));
            }

            code.push_str("    return x\n");

            let entity = CodeEntity::new("test", "function", "test_func", "/test/test.py")
                .with_source_code(&code);

            let blocks = analyzer.analyze_basic_blocks(&code);
            if !blocks.is_empty() {
                let block_count = blocks.len();
                // More complex code should generally have more blocks
                let expected_min_blocks = 1 + num_conditions / 2 + num_loops / 2;
                assert!(block_count >= expected_min_blocks,
                       "Block count {} should be at least {} for complexity",
                       block_count, expected_min_blocks);
            }
        }

        /// Property: Motif extraction should be consistent
        #[test]
        fn prop_motif_extraction_consistency(
            func1_lines in 3usize..20,
            func2_lines in 3usize..20
        ) {
            let mut analyzer = PdgMotifAnalyzer::new();

            // Generate two functions with controlled structure
            let code1 = generate_structured_function("func1", func1_lines);
            let code2 = generate_structured_function("func2", func2_lines);

            // Extract motifs multiple times to ensure consistency
            let motifs1_a = analyzer.extract_motifs(&code1, "func1_a");
            let motifs1_b = analyzer.extract_motifs(&code1, "func1_b");
            let motifs2 = analyzer.extract_motifs(&code2, "func2");

            // Same code should produce same number of motifs
            assert_eq!(motifs1_a.len(), motifs1_b.len(),
                      "Same code should produce consistent motif count");
            
            // Both should extract some motifs for non-trivial code
            if func1_lines > 5 {
                assert!(!motifs1_a.is_empty(), "Non-trivial code should have motifs");
            }
            if func2_lines > 5 {
                assert!(!motifs2.is_empty(), "Non-trivial code should have motifs");
            }
        }

        /// Property: Block analysis should be deterministic
        #[test]
        fn prop_block_analysis_deterministic(
            lines1 in 2usize..50,
            lines2 in 2usize..50
        ) {
            let analyzer = BasicBlockAnalyzer::new();

            let code1 = generate_basic_function("func1", lines1);
            let code2 = generate_basic_function("func2", lines2);

            // Analyze the same code multiple times
            let blocks1_a = analyzer.analyze_basic_blocks(&code1);
            let blocks1_b = analyzer.analyze_basic_blocks(&code1);
            let blocks2 = analyzer.analyze_basic_blocks(&code2);

            // Same code should produce same number of blocks
            assert_eq!(blocks1_a.len(), blocks1_b.len(),
                      "Same code should produce consistent block count");
            
            // Both should produce reasonable block counts
            assert!(!blocks1_a.is_empty(), "Should always produce at least one block");
            assert!(!blocks2.is_empty(), "Should always produce at least one block");
        }
    }

    // Helper function to generate structured functions for property testing
    fn generate_structured_function(name: &str, num_lines: usize) -> String {
        let mut code = format!("def {}(x):\n", name);

        for i in 0..num_lines {
            match i % 4 {
                0 => code.push_str(&format!("    if x > {}:\n        y{} = x * {}\n", i, i, i)),
                1 => code.push_str(&format!("    for j in range({}):\n        x += j\n", i + 1)),
                2 => code.push_str(&format!("    result_{} = process_{}(x)\n", i, i)),
                _ => code.push_str(&format!("    x += {}\n", i)),
            }
        }

        code.push_str("    return x\n");
        code
    }

    // Helper function to generate basic functions
    fn generate_basic_function(name: &str, num_lines: usize) -> String {
        let mut code = format!("def {}(x):\n", name);

        for i in 0..num_lines {
            code.push_str(&format!("    x += {}\n", i));
        }

        code.push_str("    return x\n");
        code
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Test integration between block analysis and motif analysis
    #[test]
    fn test_block_motif_integration() {
        let block_analyzer = BasicBlockAnalyzer::new();
        let mut motif_analyzer = PdgMotifAnalyzer::new();

        // Complex function for analysis
        let complex_func = r#"results = {}
errors = []
if config.validate_input:
    validated_items = []
    for item in data_items:
        if item.is_valid():
            validated_items.append(item)
        else:
            errors.append(f"Invalid item: {item.id}")
    data_items = validated_items
for item in data_items:
    try:
        if item.needs_transform():
            transformed = item.transform()
        else:
            transformed = item
        if config.enable_processing:
            processed = process_item(transformed, config.params)
            results[item.id] = processed
        else:
            results[item.id] = transformed
    except Exception as e:
        errors.append(f"Error processing {item.id}: {e}")
if config.aggregate_results:
    aggregated = aggregate_results(results)
    return {
        'data': aggregated,
        'errors': errors,
        'count': len(results)
    }
else:
    return {
        'data': results,
        'errors': errors,
        'count': len(results)
    }"#;

        // Test block analysis
        let blocks = block_analyzer.analyze_basic_blocks(complex_func);
        assert!(
            blocks.len() >= 5,
            "Complex function should have many blocks: {}",
            blocks.len()
        );

        // Test motif analysis
        let motifs = motif_analyzer.extract_motifs(complex_func, "complex_integration_test");
        assert!(
            motifs.len() >= 3,
            "Complex function should have many motifs: {}",
            motifs.len()
        );

        // Both analyzers should detect structural complexity
        assert!(blocks.iter().any(|b| b.contains_call), "Should detect function calls");
        assert!(motifs.iter().any(|m| m.category == MotifCategory::Conditional), "Should detect conditionals");
        assert!(motifs.iter().any(|m| m.category == MotifCategory::Loop), "Should detect loops");
    }

    /// Test basic analysis functionality with simple examples
    #[test]
    fn test_analysis_functionality() {
        let block_analyzer = BasicBlockAnalyzer::new();
        let mut motif_analyzer = PdgMotifAnalyzer::new();

        // Simple function examples
        let simple_func = "x = 1\ny = 2\nreturn x + y";
        let conditional_func = "if x > 0:\n    return x\nelse:\n    return 0";
        let loop_func = "for i in range(10):\n    print(i)\nreturn i";

        // Test block analysis
        let simple_blocks = block_analyzer.analyze_basic_blocks(simple_func);
        let conditional_blocks = block_analyzer.analyze_basic_blocks(conditional_func);
        let loop_blocks = block_analyzer.analyze_basic_blocks(loop_func);

        assert!(!simple_blocks.is_empty(), "Simple function should have blocks");
        assert!(!conditional_blocks.is_empty(), "Conditional function should have blocks");
        assert!(!loop_blocks.is_empty(), "Loop function should have blocks");

        // Test motif analysis
        let simple_motifs = motif_analyzer.extract_motifs(simple_func, "simple");
        let conditional_motifs = motif_analyzer.extract_motifs(conditional_func, "conditional");
        let loop_motifs = motif_analyzer.extract_motifs(loop_func, "loop");

        // Conditional and loop functions should have more complex motifs
        assert!(conditional_motifs.iter().any(|m| m.category == MotifCategory::Conditional), 
               "Conditional function should have conditional motifs");
        assert!(loop_motifs.iter().any(|m| m.category == MotifCategory::Loop), 
               "Loop function should have loop motifs");
    }
}
