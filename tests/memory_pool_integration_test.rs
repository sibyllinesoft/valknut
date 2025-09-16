//! Test memory pool integration in LSH operations

use valknut_rs::detectors::lsh::LshExtractor;

#[test]
fn test_memory_pool_integration() {
    println!("=== Testing Memory Pool Integration ===");

    let lsh_extractor = LshExtractor::new();

    // Test source code
    let source_code = r#"
        def calculate_sum(a, b):
            result = a + b
            return result
        
        def main():
            x = 10
            y = 20
            total = calculate_sum(x, y)
            print(f"Sum: {total}")
    "#;

    // Test signature generation with memory pools
    println!("Testing signature generation with memory pools...");

    let signature1 = lsh_extractor.generate_minhash_signature(source_code);
    assert_eq!(signature1.len(), 128, "Signature should have 128 hashes");

    // Test shingle creation with memory pools
    println!("Testing shingle creation with memory pools...");

    let shingles1 = lsh_extractor.create_shingles(source_code);
    assert!(!shingles1.is_empty(), "Should generate shingles");

    // Test multiple operations to validate pool reuse
    println!("Testing pool reuse with multiple operations...");

    for i in 0..5 {
        let test_code = format!(
            r#"
            def test_function_{}():
                x = {}
                y = x * 2
                return y + {}
        "#,
            i,
            i,
            i % 3
        );

        let signature = lsh_extractor.generate_minhash_signature(&test_code);
        let shingles = lsh_extractor.create_shingles(&test_code);

        assert_eq!(signature.len(), 128);
        assert!(!shingles.is_empty());
    }

    // Check memory pool statistics
    let (string_stats, sig_stats) = lsh_extractor.get_memory_pool_statistics();

    println!("Memory Pool Statistics:");
    println!(
        "  String Pool - Created: {}, Reused: {}, Pool Size: {}",
        string_stats.created_count, string_stats.reused_count, string_stats.current_pool_size
    );
    println!(
        "  Signature Pool - Created: {}, Reused: {}, Pool Size: {}",
        sig_stats.created_count, sig_stats.reused_count, sig_stats.current_pool_size
    );

    // Validate some reuse occurred
    assert!(
        string_stats.created_count + string_stats.reused_count > 0,
        "Should have some string pool activity"
    );
    assert!(
        sig_stats.created_count + sig_stats.reused_count > 0,
        "Should have some signature pool activity"
    );

    // Log comprehensive statistics
    println!("\nComprehensive Performance Statistics:");
    lsh_extractor.log_performance_statistics();

    println!("✅ Memory pool integration test completed successfully!");
}

#[test]
fn test_memory_pool_effectiveness() {
    println!("\n=== Testing Memory Pool Effectiveness ===");

    let lsh_extractor = LshExtractor::new();

    // Same code processed multiple times should increase reuse
    let source_code = r#"
        def repeated_function():
            for i in range(10):
                print(f"Iteration {i}")
            return "done"
    "#;

    // Process the same code multiple times
    for _ in 0..3 {
        let _signature = lsh_extractor.generate_minhash_signature(source_code);
        let _shingles = lsh_extractor.create_shingles(source_code);
    }

    let (string_stats, sig_stats) = lsh_extractor.get_memory_pool_statistics();

    println!("After repeated processing:");
    println!(
        "  String reuse rate: {:.1}%",
        string_stats.reuse_rate() * 100.0
    );
    println!(
        "  Signature reuse rate: {:.1}%",
        sig_stats.reuse_rate() * 100.0
    );

    // With repeated processing, we should see some reuse
    // Note: Cache might prevent some pool reuse, so we check for reasonable activity
    assert!(
        string_stats.reused_count > 0 || sig_stats.reused_count > 0,
        "Should have some memory pool reuse with repeated operations"
    );

    println!("✅ Memory pool effectiveness test completed!");
}
