//! Simple performance test for LSH optimization validation

use std::time::Instant;
use valknut_rs::core::config::LshConfig;
use valknut_rs::core::featureset::CodeEntity;
use valknut_rs::detectors::lsh::LshExtractor;

/// Generate test entities for performance validation
fn generate_test_entities(count: usize) -> Vec<CodeEntity> {
    let mut entities = Vec::new();

    for i in 0..count {
        let source_code = format!(
            r#"
            def function_{}():
                x = {}
                y = x * 2
                z = y + {}
                if z > 10:
                    return z
                else:
                    return x + y
                for j in range({}):
                    print(f"Value: {{j}}")
                return z * {}
            "#,
            i,
            i % 10,
            i % 5,
            i % 3 + 1,
            i % 7 + 1
        );

        let entity = CodeEntity::new(
            format!("func_{}", i),
            "function",
            format!("function_{}", i),
            format!("/test/file_{}.py", i),
        )
        .with_source_code(&source_code);

        entities.push(entity);
    }

    entities
}

#[test]
fn test_lsh_optimization_performance() {
    // Test with different entity counts to validate O(n) vs O(n²) improvement
    let test_sizes = [10, 25, 50];

    for &count in &test_sizes {
        println!("\n=== Testing with {} entities ===", count);

        let entities = generate_test_entities(count);
        let entities_refs: Vec<&CodeEntity> = entities.iter().collect();

        // Configure LSH extractor with optimizations
        let lsh_extractor = LshExtractor::new().with_lsh_config(LshConfig {
            num_hashes: 64,
            num_bands: 8,
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 50,
            use_semantic_similarity: false,
        });

        // Measure O(n) similarity context building time
        let start_time = Instant::now();
        let context = lsh_extractor.create_similarity_search_context(&entities_refs);
        let build_time = start_time.elapsed();

        println!("LSH index build time: {:?}", build_time);

        // Measure similarity search performance
        let search_start = Instant::now();
        let mut total_candidates = 0;

        // Test similarity search for first few entities
        for i in 0..count.min(10) {
            let entity_id = format!("func_{}", i);
            let candidates = context.find_similar_entities(&entity_id, Some(10));
            total_candidates += candidates.len();
        }

        let search_time = search_start.elapsed();
        let stats = context.get_statistics();

        println!("Similarity search time: {:?}", search_time);
        println!("Total candidates found: {}", total_candidates);
        println!("Context stats: {:?}", stats);

        // Validate performance expectations
        assert!(
            build_time.as_millis() < 1000,
            "Build time too slow: {:?}",
            build_time
        );
        assert!(
            search_time.as_millis() < 500,
            "Search time too slow: {:?}",
            search_time
        );

        // Verify we found some similarity candidates
        if count > 5 {
            assert!(
                total_candidates > 0,
                "Should find some similarity candidates"
            );
        }
    }
}

#[test]
fn test_signature_generation_performance() {
    let entities = generate_test_entities(20);
    let lsh_extractor = LshExtractor::new();

    let start_time = Instant::now();
    let mut signatures = Vec::new();

    for entity in &entities {
        let signature = lsh_extractor.generate_minhash_signature(&entity.source_code);
        signatures.push(signature);
    }

    let elapsed = start_time.elapsed();
    println!("Generated {} signatures in {:?}", signatures.len(), elapsed);

    // Each signature should have the expected length
    for signature in &signatures {
        assert_eq!(signature.len(), 128, "Signature should have 128 hashes");
        assert!(
            signature.iter().any(|&x| x != u64::MAX),
            "Signature should not be all MAX values"
        );
    }

    // Performance expectation: should generate signatures quickly
    let avg_time_per_signature = elapsed.as_millis() / entities.len() as u128;
    println!(
        "Average signature generation time: {}ms",
        avg_time_per_signature
    );
    assert!(
        avg_time_per_signature < 100,
        "Signature generation too slow: {}ms",
        avg_time_per_signature
    );
}

#[test]
fn test_lsh_band_configuration() {
    let entities = generate_test_entities(30);
    let entities_refs: Vec<&CodeEntity> = entities.iter().collect();

    // Test different band configurations
    let configs = [
        (64, 8),   // 8 hashes per band
        (128, 16), // 8 hashes per band
        (128, 32), // 4 hashes per band
    ];

    for (num_hashes, num_bands) in configs {
        println!(
            "\nTesting LSH config: {} hashes, {} bands",
            num_hashes, num_bands
        );

        let lsh_config = LshConfig {
            num_hashes,
            num_bands,
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 20,
            use_semantic_similarity: false,
        };

        let extractor = LshExtractor::new().with_lsh_config(lsh_config);

        let start_time = Instant::now();
        let context = extractor.create_similarity_search_context(&entities_refs);
        let build_time = start_time.elapsed();

        // Quick search test
        let search_start = Instant::now();
        let candidates = context.find_similar_entities("func_0", Some(5));
        let search_time = search_start.elapsed();

        let stats = context.get_statistics();
        println!(
            "  Build time: {:?}, Search time: {:?}, Candidates: {}",
            build_time,
            search_time,
            candidates.len()
        );
        println!("  Stats: {:?}", stats);

        // Validate configuration is working
        assert_eq!(stats.num_hashes, num_hashes);
        assert_eq!(stats.num_bands, num_bands);
        assert!(
            build_time.as_millis() < 2000,
            "Build time too slow for config"
        );
        assert!(
            search_time.as_millis() < 100,
            "Search time too slow for config"
        );
    }
}

#[test]
fn test_memory_allocation_patterns() {
    let entities = generate_test_entities(50);
    let lsh_extractor = LshExtractor::new();

    // Test batch processing to validate memory efficiency
    const BATCH_SIZE: usize = 10;
    let start_time = Instant::now();

    let mut all_signatures = Vec::new();
    for chunk in entities.chunks(BATCH_SIZE) {
        let mut batch_signatures = Vec::with_capacity(BATCH_SIZE);

        for entity in chunk {
            let signature = lsh_extractor.generate_minhash_signature(&entity.source_code);
            batch_signatures.push(signature);
        }

        all_signatures.extend(batch_signatures);
    }

    let elapsed = start_time.elapsed();
    println!(
        "Batch processed {} signatures in {:?}",
        all_signatures.len(),
        elapsed
    );

    // Validate all signatures were generated correctly
    assert_eq!(all_signatures.len(), entities.len());

    // Performance check
    let avg_time = elapsed.as_millis() / entities.len() as u128;
    assert!(
        avg_time < 50,
        "Batch processing too slow: {}ms average",
        avg_time
    );
}
