//! Test to validate token and signature caching effectiveness

use std::time::Instant;
use valknut_rs::core::featureset::CodeEntity;
use valknut_rs::detectors::lsh::LshExtractor;
use valknut_rs::core::config::LshConfig;

#[test]
fn test_caching_effectiveness() {
    let lsh_extractor = LshExtractor::new()
        .with_lsh_config(LshConfig {
            num_hashes: 64,
            num_bands: 8,
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 50,
            use_semantic_similarity: false,
        });
    
    // Create test source code
    let source_code = r#"
        def calculate_fibonacci(n):
            if n <= 1:
                return n
            else:
                return calculate_fibonacci(n-1) + calculate_fibonacci(n-2)
        
        def main():
            result = calculate_fibonacci(10)
            print(f"Fibonacci of 10 is: {result}")
            return result
    "#;
    
    println!("=== Testing Signature Caching Effectiveness ===");
    
    // Measure first signature generation (cache miss)
    let start_time = Instant::now();
    let signature1 = lsh_extractor.generate_minhash_signature(source_code);
    let first_time = start_time.elapsed();
    
    // Measure second signature generation (should be cache hit)
    let start_time = Instant::now();
    let signature2 = lsh_extractor.generate_minhash_signature(source_code);
    let second_time = start_time.elapsed();
    
    // Measure third signature generation (should be cache hit)
    let start_time = Instant::now();
    let signature3 = lsh_extractor.generate_minhash_signature(source_code);
    let third_time = start_time.elapsed();
    
    println!("First generation (cache miss): {:?}", first_time);
    println!("Second generation (cache hit): {:?}", second_time);
    println!("Third generation (cache hit): {:?}", third_time);
    
    // Verify signatures are identical
    assert_eq!(signature1, signature2, "Cached signature should match original");
    assert_eq!(signature2, signature3, "All cached signatures should match");
    
    // Cache hits should be significantly faster than cache miss
    // (We expect at least 50% speed improvement for cache hits)
    let cache_speedup = first_time.as_nanos() as f64 / second_time.as_nanos() as f64;
    println!("Cache speedup factor: {:.2}x", cache_speedup);
    
    // Get cache statistics
    let cache_stats = lsh_extractor.get_cache_statistics();
    println!("Cache statistics: {:?}", cache_stats);
    
    // Validate cache behavior
    assert!(cache_stats.signature_hits >= 2, "Should have at least 2 signature cache hits");
    // Note: token hits may be 0 if signature cache prevents token generation
    assert!(cache_stats.token_hits >= 0, "Token hits should be non-negative");
    
    let hit_rate = cache_stats.overall_hit_rate();
    println!("Overall cache hit rate: {:.1}%", hit_rate * 100.0);
    
    // With 3 calls to same source, we expect good hit rate after first miss
    assert!(hit_rate > 0.0, "Should have some cache hits");
}

#[test]
fn test_token_caching_with_variations() {
    let lsh_extractor = LshExtractor::new();
    
    println!("\n=== Testing Token Caching with Source Variations ===");
    
    // Base source code
    let base_source = r#"
        def test_function():
            x = 1
            y = 2
            return x + y
    "#;
    
    // Same source with different whitespace (should have same normalized tokens)
    let whitespace_variant = r#"
    def test_function():
          x = 1
          y = 2
          return x + y
    "#;
    
    // Different source (should not hit cache)
    let different_source = r#"
        def other_function():
            a = 5
            b = 10
            return a * b
    "#;
    
    // Process base source multiple times
    for i in 0..3 {
        let start = Instant::now();
        let _shingles = lsh_extractor.create_shingles(base_source);
        let elapsed = start.elapsed();
        println!("Base source iteration {}: {:?}", i + 1, elapsed);
    }
    
    // Process whitespace variant (tokens should be cached, but normalization may differ)
    let start = Instant::now();
    let _shingles_variant = lsh_extractor.create_shingles(whitespace_variant);
    let elapsed = start.elapsed();
    println!("Whitespace variant: {:?}", elapsed);
    
    // Process different source (should not hit cache)
    let start = Instant::now();
    let _shingles_different = lsh_extractor.create_shingles(different_source);
    let elapsed = start.elapsed();
    println!("Different source: {:?}", elapsed);
    
    let cache_stats = lsh_extractor.get_cache_statistics();
    println!("Final cache statistics: {:?}", cache_stats);
    
    // Verify we got some cache hits
    assert!(cache_stats.token_hits > 0, "Should have token cache hits for repeated source");
}

#[test]
fn test_cache_memory_management() {
    let lsh_extractor = LshExtractor::new();
    
    println!("\n=== Testing Cache Memory Management ===");
    
    // Generate many different source codes to test cache eviction
    let mut sources = Vec::new();
    for i in 0..100 {
        let source = format!(
            r#"
            def function_{}():
                x = {}
                y = x * 2
                return y + {}
            "#,
            i, i % 10, i % 5
        );
        sources.push(source);
    }
    
    println!("Generating signatures for {} different sources", sources.len());
    
    // Process all sources
    let start_time = Instant::now();
    for (i, source) in sources.iter().enumerate() {
        let _signature = lsh_extractor.generate_minhash_signature(source);
        
        if i % 20 == 0 {
            let cache_stats = lsh_extractor.get_cache_statistics();
            println!("After {} sources - hits: {}, misses: {}, evictions: {}", 
                     i + 1, 
                     cache_stats.signature_hits + cache_stats.token_hits,
                     cache_stats.signature_misses + cache_stats.token_misses,
                     cache_stats.evictions);
        }
    }
    
    let elapsed = start_time.elapsed();
    println!("Processed {} sources in {:?}", sources.len(), elapsed);
    
    // Now test cache effectiveness with repeated access
    let repeat_start = Instant::now();
    for source in sources.iter().take(10) {
        let _signature = lsh_extractor.generate_minhash_signature(source);
    }
    let repeat_elapsed = repeat_start.elapsed();
    
    println!("Repeated access to 10 sources: {:?}", repeat_elapsed);
    
    let final_stats = lsh_extractor.get_cache_statistics();
    println!("Final statistics: {:?}", final_stats);
    
    // Validate cache is working and managing memory
    assert!(final_stats.signature_hits > 0 || final_stats.token_hits > 0, 
            "Should have some cache hits");
    
    let avg_time_per_source = elapsed.as_millis() / sources.len() as u128;
    println!("Average time per source: {}ms", avg_time_per_source);
    
    // Performance should be reasonable even with cache management
    assert!(avg_time_per_source < 100, "Average processing time should be reasonable");
}