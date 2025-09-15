//! Comprehensive tests for Phase 2: Structural Gate Validation
//! 
//! Tests the structural analysis components including:
//! - BasicBlockAnalyzer - block counting and overlap analysis
//! - PdgMotifAnalyzer - Weisfeiler-Lehman hashing and motif extraction  
//! - Gate filtering logic (≥2 blocks, ≥2 shared motifs)
//! - IO/side-effects penalty system
//! - External call pattern analysis

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use approx::assert_relative_eq;
use proptest::prelude::*;

use valknut_rs::core::featureset::{CodeEntity, ExtractionContext};
use valknut_rs::core::config::{ValknutConfig, DedupeConfig};
use valknut_rs::detectors::clone_detection::{
    StructuralGateAnalyzer, BasicBlockAnalyzer, PdgMotifAnalyzer, 
    NormalizationConfig, MotifCategory, PdgMotif
};

#[cfg(test)]
mod basic_block_analyzer_tests {
    use super::*;

    /// Test basic block counting for different code structures
    #[test]
    fn test_basic_block_counting() {
        let analyzer = BasicBlockAnalyzer::new();
        
        // Test simple linear code (1 block)
        let linear_code = "def simple_func():\n    x = 1\n    y = 2\n    return x + y";
        let linear_entity = CodeEntity::new("linear", "function", "simple_func", "/test/linear.py")
            .with_source_code(linear_code);
        let linear_blocks = analyzer.count_basic_blocks(&linear_entity).unwrap();
        assert_eq!(linear_blocks, 1, "Linear code should have 1 basic block");
        
        // Test conditional code (3 blocks: before if, then, else/after)
        let conditional_code = r#"
def conditional_func(x):
    y = x + 1
    if x > 0:
        result = x * 2
    else:
        result = x * -1
    return result
"#;
        let conditional_entity = CodeEntity::new("cond", "function", "conditional_func", "/test/cond.py")
            .with_source_code(conditional_code);
        let conditional_blocks = analyzer.count_basic_blocks(&conditional_entity).unwrap();
        assert!(conditional_blocks >= 2, "Conditional code should have at least 2 basic blocks, got {}", conditional_blocks);
        
        // Test loop code (multiple blocks for loop entry, body, continuation)
        let loop_code = r#"
def loop_func(n):
    result = 0
    for i in range(n):
        if i % 2 == 0:
            result += i
        else:
            result -= i
    return result
"#;
        let loop_entity = CodeEntity::new("loop", "function", "loop_func", "/test/loop.py")
            .with_source_code(loop_code);
        let loop_blocks = analyzer.count_basic_blocks(&loop_entity).unwrap();
        assert!(loop_blocks >= 3, "Loop with conditional should have at least 3 basic blocks, got {}", loop_blocks);
    }

    /// Test block overlap analysis between similar functions
    #[test]
    fn test_block_overlap_analysis() {
        let analyzer = BasicBlockAnalyzer::new();
        
        // Similar functions with overlapping block structure
        let func1_code = r#"
def func1(x):
    if x > 0:
        return x * 2
    else:
        return x * -1
"#;
        
        let func2_code = r#"
def func2(y):
    if y > 0:
        return y * 3  # Different operation but same structure
    else:
        return y * -2 # Different operation but same structure
"#;
        
        let func1 = CodeEntity::new("f1", "function", "func1", "/test/f1.py").with_source_code(func1_code);
        let func2 = CodeEntity::new("f2", "function", "func2", "/test/f2.py").with_source_code(func2_code);
        
        let overlap = analyzer.calculate_block_overlap(&func1, &func2).unwrap();
        
        // Should detect structural similarity despite different operations
        assert!(overlap > 0.5, "Structurally similar functions should have >50% block overlap, got {}", overlap);
        assert!(overlap <= 1.0, "Block overlap should not exceed 100%");
    }

    /// Test minimum block threshold filtering
    #[test]
    fn test_minimum_block_threshold() {
        let analyzer = BasicBlockAnalyzer::with_min_blocks(2);
        
        // Single block function should be filtered
        let simple_func = CodeEntity::new("simple", "function", "simple", "/test/simple.py")
            .with_source_code("def simple(): return 42");
            
        let passes_threshold = analyzer.passes_minimum_threshold(&simple_func).unwrap();
        assert!(!passes_threshold, "Simple function should not pass minimum threshold");
        
        // Multi-block function should pass
        let complex_func = CodeEntity::new("complex", "function", "complex", "/test/complex.py")
            .with_source_code(r#"
def complex(x):
    if x > 0:
        return x
    return -x
"#);
        
        let passes_threshold = analyzer.passes_minimum_threshold(&complex_func).unwrap();
        assert!(passes_threshold, "Complex function should pass minimum threshold");
    }

    /// Test edge cases for block counting
    #[test]
    fn test_block_counting_edge_cases() {
        let analyzer = BasicBlockAnalyzer::new();
        
        // Empty function
        let empty_func = CodeEntity::new("empty", "function", "empty", "/test/empty.py")
            .with_source_code("def empty(): pass");
        let blocks = analyzer.count_basic_blocks(&empty_func).unwrap();
        assert_eq!(blocks, 1, "Empty function should have 1 basic block");
        
        // Function with only comments and whitespace
        let comment_func = CodeEntity::new("comment", "function", "comment", "/test/comment.py")
            .with_source_code(r#"
def comment_func():
    # This is a comment
    # Another comment
    pass
"#);
        let blocks = analyzer.count_basic_blocks(&comment_func).unwrap();
        assert_eq!(blocks, 1, "Function with only comments should have 1 basic block");
        
        // Complex nested control flow
        let nested_func = CodeEntity::new("nested", "function", "nested", "/test/nested.py")
            .with_source_code(r#"
def nested_func(x, y):
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
"#);
        let blocks = analyzer.count_basic_blocks(&nested_func).unwrap();
        assert!(blocks >= 5, "Deeply nested function should have many basic blocks, got {}", blocks);
    }
}

#[cfg(test)]
mod pdg_motif_analyzer_tests {
    use super::*;

    /// Test PDG motif extraction from control flow
    #[test]
    fn test_pdg_motif_extraction() {
        let analyzer = PdgMotifAnalyzer::new();
        
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
            
        let motifs = analyzer.extract_pdg_motifs(&entity).unwrap();
        
        // Should extract different types of motifs
        assert!(!motifs.is_empty(), "Should extract PDG motifs from control flow");
        
        // Check for different motif categories
        let categories: HashSet<MotifCategory> = motifs.iter().map(|m| m.category.clone()).collect();
        assert!(categories.contains(&MotifCategory::ControlFlow), 
                "Should extract control flow motifs");
    }

    /// Test Weisfeiler-Lehman hashing for motif signatures  
    #[test]
    fn test_weisfeiler_lehman_hashing() {
        let analyzer = PdgMotifAnalyzer::new();
        
        // Two functions with similar control structure
        let func1 = r#"
def similar1(x):
    if x > 0:
        return x * 2
    return 0
"#;
        
        let func2 = r#"
def similar2(y):
    if y > 0:
        return y * 3
    return 0
"#;
        
        let entity1 = CodeEntity::new("s1", "function", "similar1", "/test/s1.py").with_source_code(func1);
        let entity2 = CodeEntity::new("s2", "function", "similar2", "/test/s2.py").with_source_code(func2);
        
        let motifs1 = analyzer.extract_pdg_motifs(&entity1).unwrap();
        let motifs2 = analyzer.extract_pdg_motifs(&entity2).unwrap();
        
        // Compute WL hashes
        let hash1 = analyzer.compute_wl_hash(&motifs1);
        let hash2 = analyzer.compute_wl_hash(&motifs2);
        
        // Similar structures should have some hash overlap
        let common_hashes = hash1.intersection(&hash2).count();
        assert!(common_hashes > 0, "Similar structures should share some WL hashes");
    }

    /// Test motif shared requirement (≥2 shared motifs)
    #[test]
    fn test_shared_motif_requirement() {
        let analyzer = PdgMotifAnalyzer::new();
        
        // Two functions with multiple shared structural patterns
        let func1_code = r#"
def shared_structure1(items):
    result = []
    for item in items:
        if item.is_valid():
            processed = item.process()
            result.append(processed)
    return result
"#;
        
        let func2_code = r#"
def shared_structure2(data):
    output = []
    for entry in data:
        if entry.is_valid():
            transformed = entry.transform()
            output.append(transformed)
    return output
"#;
        
        let entity1 = CodeEntity::new("ss1", "function", "shared_structure1", "/test/ss1.py")
            .with_source_code(func1_code);
        let entity2 = CodeEntity::new("ss2", "function", "shared_structure2", "/test/ss2.py") 
            .with_source_code(func2_code);
            
        let shared_count = analyzer.count_shared_motifs(&entity1, &entity2).unwrap();
        assert!(shared_count >= 2, "Functions with similar structure should share ≥2 motifs, got {}", shared_count);
        
        // Test functions with different structures
        let diff_func_code = r#"
def different_structure(n):
    total = 0
    i = 0
    while i < n:
        total += i * i
        i += 1
    return total
"#;
        
        let diff_entity = CodeEntity::new("diff", "function", "different_structure", "/test/diff.py")
            .with_source_code(diff_func_code);
            
        let shared_count_diff = analyzer.count_shared_motifs(&entity1, &diff_entity).unwrap();
        assert!(shared_count_diff < shared_count, 
                "Different structures should share fewer motifs: {} vs {}", shared_count_diff, shared_count);
    }

    /// Test motif categorization accuracy
    #[test]
    fn test_motif_categorization() {
        let analyzer = PdgMotifAnalyzer::new();
        
        // Function with various constructs
        let mixed_code = r#"
def mixed_constructs(data, config):
    # Assignment motif
    result = initialize_result()
    
    # Control flow motif
    if config.enabled:
        # Function call motif  
        processed_data = process_data(data)
        
        # Data structure motif
        cache = {}
        for item in processed_data:
            cache[item.key] = item.value
            
        result.update(cache)
    
    return result
"#;
        
        let entity = CodeEntity::new("mixed", "function", "mixed_constructs", "/test/mixed.py")
            .with_source_code(mixed_code);
            
        let motifs = analyzer.extract_pdg_motifs(&entity).unwrap();
        
        // Check for different categories of motifs
        let categories: HashSet<MotifCategory> = motifs.iter().map(|m| m.category.clone()).collect();
        
        assert!(categories.contains(&MotifCategory::Assignment), "Should detect assignment motifs");
        assert!(categories.contains(&MotifCategory::ControlFlow), "Should detect control flow motifs");
        assert!(categories.contains(&MotifCategory::FunctionCall), "Should detect function call motifs");
        assert!(categories.contains(&MotifCategory::DataStructure), "Should detect data structure motifs");
    }
}

#[cfg(test)]
mod structural_gate_analyzer_tests {
    use super::*;

    /// Test complete structural gate validation pipeline
    #[test]
    fn test_structural_gate_validation_pipeline() {
        let gate_analyzer = StructuralGateAnalyzer::new(2, 2); // ≥2 blocks, ≥2 motifs
        
        // Function that should pass all gates
        let valid_clone_func = r#"
def valid_clone_processing(data_list):
    results = []
    for data_item in data_list:
        if data_item.is_valid():
            processed = data_item.transform()
            results.append(processed)
        else:
            results.append(None)
    return results
"#;
        
        let valid_entity = CodeEntity::new("valid", "function", "valid_clone_processing", "/test/valid.py")
            .with_source_code(valid_clone_func);
            
        let gate_result = gate_analyzer.passes_structural_gates(&valid_entity).unwrap();
        assert!(gate_result.passes_all_gates, "Valid clone should pass all structural gates");
        assert!(gate_result.block_count >= 2, "Should meet minimum block requirement");
        assert!(gate_result.motif_count >= 2, "Should meet minimum motif requirement");
        
        // Function that should fail gates (too simple)
        let invalid_func = "def simple(): return 42";
        let invalid_entity = CodeEntity::new("invalid", "function", "simple", "/test/invalid.py")
            .with_source_code(invalid_func);
            
        let gate_result = gate_analyzer.passes_structural_gates(&invalid_entity).unwrap();
        assert!(!gate_result.passes_all_gates, "Simple function should fail structural gates");
    }

    /// Test IO/side-effects penalty system
    #[test]
    fn test_io_side_effects_penalty() {
        let gate_analyzer = StructuralGateAnalyzer::new(1, 1); // Lower thresholds for testing
        
        // Function with significant IO/side effects
        let io_heavy_func = r#"
def io_heavy_function(filename):
    with open(filename, 'r') as f:
        data = f.read()
    
    print("Processing:", filename)
    
    results = []
    for line in data.split('\n'):
        if line.strip():
            processed = process_line(line)
            print("Processed:", processed)
            results.append(processed)
    
    with open(filename + ".out", 'w') as f:
        for result in results:
            f.write(str(result) + '\n')
    
    return results
"#;
        
        let io_entity = CodeEntity::new("io_heavy", "function", "io_heavy_function", "/test/io.py")
            .with_source_code(io_heavy_func);
            
        let gate_result = gate_analyzer.passes_structural_gates(&io_entity).unwrap();
        
        // Should detect IO penalty
        assert!(gate_result.io_penalty_factor > 1.0, "IO-heavy function should have penalty factor > 1.0");
        assert!(!gate_result.io_patterns.is_empty(), "Should detect IO patterns");
        
        // Compare with pure computation function
        let pure_func = r#"
def pure_computation(numbers):
    results = []
    for num in numbers:
        if num > 0:
            result = num * num + num
            results.append(result)
        else:
            results.append(0)
    return results
"#;
        
        let pure_entity = CodeEntity::new("pure", "function", "pure_computation", "/test/pure.py")
            .with_source_code(pure_func);
            
        let pure_gate_result = gate_analyzer.passes_structural_gates(&pure_entity).unwrap();
        
        assert!(pure_gate_result.io_penalty_factor <= gate_result.io_penalty_factor,
                "Pure function should have lower IO penalty than IO-heavy function");
    }

    /// Test external call pattern analysis
    #[test]
    fn test_external_call_pattern_analysis() {
        let gate_analyzer = StructuralGateAnalyzer::new(1, 1);
        
        // Function with many external calls
        let external_calls_func = r#"
def external_heavy_function(api_client, data):
    # Multiple external API calls
    user_info = api_client.get_user(data.user_id)
    permissions = api_client.get_permissions(user_info.id)
    
    results = []
    for item in data.items:
        # More external calls in loop
        validation_result = api_client.validate_item(item)
        if validation_result.is_valid:
            processed = api_client.process_item(item)
            results.append(processed)
    
    # Final external call
    api_client.log_completion(len(results))
    return results
"#;
        
        let external_entity = CodeEntity::new("external", "function", "external_heavy_function", "/test/ext.py")
            .with_source_code(external_calls_func);
            
        let gate_result = gate_analyzer.passes_structural_gates(&external_entity).unwrap();
        
        // Should detect external call patterns
        assert!(!gate_result.external_call_patterns.is_empty(), "Should detect external call patterns");
        assert!(gate_result.external_call_penalty > 1.0, "Should apply penalty for external calls");
        
        // Should still be structural enough to pass gates despite penalties
        let total_penalty = gate_result.io_penalty_factor * gate_result.external_call_penalty;
        assert!(total_penalty > 1.0, "Combined penalties should be > 1.0");
    }

    /// Test gate combination logic
    #[test]
    fn test_gate_combination_logic() {
        let gate_analyzer = StructuralGateAnalyzer::new(3, 2); // Higher thresholds
        
        // Create functions that pass/fail different combinations of gates
        
        // High blocks, low motifs
        let high_blocks_low_motifs = r#"
def high_blocks_simple(x):
    if x > 10:
        x = x * 2
    elif x > 5:
        x = x * 1.5
    elif x > 0:
        x = x * 1.2
    else:
        x = x * -1
    
    y = x + 1
    z = y - 2
    return z
"#;
        
        let entity1 = CodeEntity::new("hb_lm", "function", "high_blocks_simple", "/test/hb_lm.py")
            .with_source_code(high_blocks_low_motifs);
            
        let result1 = gate_analyzer.passes_structural_gates(&entity1).unwrap();
        
        // Low blocks, high motifs (complex single block)
        let low_blocks_high_motifs = r#"
def complex_single_block(data):
    return [item.process().validate().transform() for item in data if item.is_valid() and item.has_data()]
"#;
        
        let entity2 = CodeEntity::new("lb_hm", "function", "complex_single_block", "/test/lb_hm.py")
            .with_source_code(low_blocks_high_motifs);
            
        let result2 = gate_analyzer.passes_structural_gates(&entity2).unwrap();
        
        // Both gates should have meaningful criteria
        assert!(result1.block_count > result2.block_count, 
                "First function should have more blocks");
        // Note: Motif comparison depends on implementation details
    }
}

#[cfg(test)]
mod property_based_tests {
    use super::*;

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
                
            if let Ok(block_count) = analyzer.count_basic_blocks(&entity) {
                // More complex code should generally have more blocks
                let expected_min_blocks = 1 + num_conditions / 2 + num_loops / 2;
                assert!(block_count >= expected_min_blocks, 
                       "Block count {} should be at least {} for complexity", 
                       block_count, expected_min_blocks);
            }
        }

        /// Property: Shared motif count should be symmetric
        #[test]
        fn prop_shared_motifs_symmetry(
            func1_lines in 3usize..20,
            func2_lines in 3usize..20
        ) {
            let analyzer = PdgMotifAnalyzer::new();
            
            // Generate two functions with controlled structure
            let code1 = generate_structured_function("func1", func1_lines);
            let code2 = generate_structured_function("func2", func2_lines);
            
            let entity1 = CodeEntity::new("f1", "function", "func1", "/test/f1.py")
                .with_source_code(&code1);
            let entity2 = CodeEntity::new("f2", "function", "func2", "/test/f2.py")
                .with_source_code(&code2);
                
            if let (Ok(shared12), Ok(shared21)) = (
                analyzer.count_shared_motifs(&entity1, &entity2),
                analyzer.count_shared_motifs(&entity2, &entity1)
            ) {
                assert_eq!(shared12, shared21, 
                          "Shared motif count should be symmetric: {} vs {}", shared12, shared21);
            }
        }

        /// Property: Block overlap should be in [0, 1] range
        #[test]
        fn prop_block_overlap_range(
            lines1 in 2usize..50,
            lines2 in 2usize..50
        ) {
            let analyzer = BasicBlockAnalyzer::new();
            
            let code1 = generate_basic_function("func1", lines1);
            let code2 = generate_basic_function("func2", lines2);
            
            let entity1 = CodeEntity::new("e1", "function", "func1", "/test/e1.py")
                .with_source_code(&code1);
            let entity2 = CodeEntity::new("e2", "function", "func2", "/test/e2.py") 
                .with_source_code(&code2);
                
            if let Ok(overlap) = analyzer.calculate_block_overlap(&entity1, &entity2) {
                assert!(overlap >= 0.0 && overlap <= 1.0, 
                       "Block overlap should be in [0,1] range: {}", overlap);
            }
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
    use tokio;

    /// Test integration between block analysis and motif analysis
    #[tokio::test]
    async fn test_block_motif_integration() {
        let block_analyzer = BasicBlockAnalyzer::new();
        let motif_analyzer = PdgMotifAnalyzer::new();
        let gate_analyzer = StructuralGateAnalyzer::new(2, 2);
        
        // Complex function that should pass both analyses
        let complex_func = r#"
def complex_integration_test(data_items, config):
    results = {}
    errors = []
    
    # Preprocessing block
    if config.validate_input:
        validated_items = []
        for item in data_items:
            if item.is_valid():
                validated_items.append(item)
            else:
                errors.append(f"Invalid item: {item.id}")
        data_items = validated_items
    
    # Main processing block
    for item in data_items:
        try:
            # Transform block
            if item.needs_transform():
                transformed = item.transform()
            else:
                transformed = item
            
            # Process block
            if config.enable_processing:
                processed = process_item(transformed, config.params)
                results[item.id] = processed
            else:
                results[item.id] = transformed
                
        except Exception as e:
            errors.append(f"Error processing {item.id}: {e}")
    
    # Post-processing block
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
        }
"#;
        
        let entity = CodeEntity::new("complex", "function", "complex_integration_test", "/test/complex.py")
            .with_source_code(complex_func);
        
        // Test block analysis
        let block_count = block_analyzer.count_basic_blocks(&entity).unwrap();
        assert!(block_count >= 5, "Complex function should have many blocks: {}", block_count);
        
        // Test motif analysis  
        let motifs = motif_analyzer.extract_pdg_motifs(&entity).unwrap();
        assert!(motifs.len() >= 3, "Complex function should have many motifs: {}", motifs.len());
        
        // Test gate analysis combines both
        let gate_result = gate_analyzer.passes_structural_gates(&entity).unwrap();
        assert!(gate_result.passes_all_gates, "Complex function should pass all gates");
        assert_eq!(gate_result.block_count, block_count);
        assert!(gate_result.motif_count > 0);
    }

    /// Test structural gate filtering effectiveness
    #[tokio::test]
    async fn test_structural_gate_filtering_effectiveness() {
        let gate_analyzer = StructuralGateAnalyzer::new(3, 2);
        
        // Create a mix of functions that should/shouldn't pass gates
        let functions = vec![
            // Should pass: complex with multiple blocks and motifs
            ("pass1", r#"
def should_pass_complex(items):
    results = []
    for item in items:
        if item.is_valid():
            if item.needs_processing():
                processed = item.process()
                results.append(processed)
            else:
                results.append(item.raw_value())
        else:
            continue
    return results
"#),
            // Should pass: different structure but sufficient complexity
            ("pass2", r#"
def should_pass_different(data, options):
    if not options.enabled:
        return None
        
    total = 0
    for value in data:
        if value > options.threshold:
            total += value * options.multiplier
        elif value > 0:
            total += value
        else:
            total -= abs(value)
            
    return total if total > 0 else 0
"#),
            // Should fail: too simple
            ("fail1", "def too_simple(x): return x * 2"),
            
            // Should fail: single complex expression
            ("fail2", "def single_expression(data): return sum(x.value for x in data if x.is_valid())"),
        ];
        
        let mut pass_count = 0;
        let mut fail_count = 0;
        
        for (name, code) in functions {
            let entity = CodeEntity::new(name, "function", name, &format!("/test/{}.py", name))
                .with_source_code(code);
                
            let gate_result = gate_analyzer.passes_structural_gates(&entity).unwrap();
            
            if name.starts_with("pass") {
                assert!(gate_result.passes_all_gates, 
                       "Function {} should pass gates but didn't: blocks={}, motifs={}", 
                       name, gate_result.block_count, gate_result.motif_count);
                pass_count += 1;
            } else if name.starts_with("fail") {
                assert!(!gate_result.passes_all_gates,
                       "Function {} should fail gates but didn't: blocks={}, motifs={}", 
                       name, gate_result.block_count, gate_result.motif_count);
                fail_count += 1;
            }
        }
        
        assert!(pass_count >= 2, "Should have functions that pass gates");
        assert!(fail_count >= 2, "Should have functions that fail gates");
    }
}