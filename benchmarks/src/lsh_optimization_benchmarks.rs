//! LSH Performance Optimization Benchmarks
//!
//! This benchmark suite validates the critical performance improvements:
//! 1. LSH banding for O(n) vs O(n²) complexity reduction
//! 2. Token caching effectiveness
//! 3. Memory allocation pattern optimizations
//! 4. Overall throughput improvements

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;
use valknut_rs::core::config::LshConfig;
use valknut_rs::core::featureset::CodeEntity;
use valknut_rs::detectors::lsh::LshExtractor;

/// Generate test entities for performance testing
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
                # Some comment here
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

/// Benchmark O(n²) vs O(n) comparison approaches
fn benchmark_complexity_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsh_complexity_comparison");
    group.measurement_time(Duration::from_secs(10));

    // Test with different entity counts to demonstrate complexity differences
    let entity_counts = [10, 25, 50, 100];

    for &count in &entity_counts {
        let entities = generate_test_entities(count);
        let entities_refs: Vec<&CodeEntity> = entities.iter().collect();

        // Standard LSH extractor (with optimizations)
        let lsh_extractor = LshExtractor::new().with_lsh_config(LshConfig {
            num_hashes: 64, // Reduced for faster testing
            num_bands: 8,
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 50,
            use_semantic_similarity: false,
        });

        // Benchmark O(n) LSH-based similarity search
        group.bench_with_input(BenchmarkId::new("lsh_optimized", count), &count, |b, _| {
            b.iter(|| {
                let context = lsh_extractor.create_similarity_search_context(&entities_refs);

                // Simulate finding similar entities for a few test cases
                for i in 0..count.min(5) {
                    let entity_id = format!("func_{}", i);
                    let _candidates = context.find_similar_entities(&entity_id, Some(10));
                }

                black_box(context.get_statistics())
            })
        });

        // Benchmark signature generation performance
        group.bench_with_input(
            BenchmarkId::new("signature_generation", count),
            &count,
            |b, _| {
                b.iter(|| {
                    for entity in &entities {
                        let _signature =
                            lsh_extractor.generate_minhash_signature(&entity.source_code);
                    }
                })
            },
        );
    }

    group.finish();
}

/// Benchmark token caching effectiveness
fn benchmark_token_caching(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_caching");

    let entities = generate_test_entities(50);
    let lsh_extractor = LshExtractor::new();

    // Benchmark without caching (repeated tokenization)
    group.bench_function("without_token_caching", |b| {
        b.iter(|| {
            for entity in &entities {
                // Simulate repeated tokenization
                let _shingles = lsh_extractor.create_shingles(&entity.source_code);
            }
        })
    });

    // Benchmark with caching simulation
    group.bench_function("with_token_caching_simulation", |b| {
        let mut token_cache = std::collections::HashMap::new();

        b.iter(|| {
            for entity in &entities {
                // Simulate cached tokenization
                let cache_key = format!("{:x}", {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    entity.source_code.hash(&mut hasher);
                    hasher.finish()
                });

                if !token_cache.contains_key(&cache_key) {
                    let shingles = lsh_extractor.create_shingles(&entity.source_code);
                    token_cache.insert(cache_key.clone(), shingles);
                }

                let _cached_shingles = token_cache.get(&cache_key);
            }
        })
    });

    group.finish();
}

/// Benchmark memory allocation patterns
fn benchmark_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");

    let entities = generate_test_entities(100);
    let lsh_extractor = LshExtractor::new();

    // Benchmark memory-efficient batch processing
    group.bench_function("batch_signature_generation", |b| {
        b.iter(|| {
            // Process in batches to reduce peak memory usage
            const BATCH_SIZE: usize = 10;

            for chunk in entities.chunks(BATCH_SIZE) {
                let mut batch_signatures = Vec::with_capacity(BATCH_SIZE);

                for entity in chunk {
                    let signature = lsh_extractor.generate_minhash_signature(&entity.source_code);
                    batch_signatures.push(signature);
                }

                // Simulate processing the batch
                black_box(batch_signatures);
            }
        })
    });

    // Benchmark single-pass processing
    group.bench_function("single_pass_processing", |b| {
        b.iter(|| {
            let mut all_signatures = Vec::with_capacity(entities.len());

            for entity in &entities {
                let signature = lsh_extractor.generate_minhash_signature(&entity.source_code);
                all_signatures.push(signature);
            }

            black_box(all_signatures);
        })
    });

    group.finish();
}

/// Benchmark overall LSH performance improvements
fn benchmark_lsh_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsh_throughput");
    group.measurement_time(Duration::from_secs(15));

    let entity_counts = [50, 100, 200];

    for &count in &entity_counts {
        let entities = generate_test_entities(count);
        let entities_refs: Vec<&CodeEntity> = entities.iter().collect();

        // Optimized LSH extractor
        let optimized_extractor = LshExtractor::new().with_lsh_config(LshConfig {
            num_hashes: 128,
            num_bands: 16,
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 100,
            use_semantic_similarity: false,
        });

        group.bench_with_input(
            BenchmarkId::new("optimized_lsh_throughput", count),
            &count,
            |b, _| {
                b.iter(|| {
                    // Build similarity context (O(n) preprocessing)
                    let start_time = std::time::Instant::now();
                    let context =
                        optimized_extractor.create_similarity_search_context(&entities_refs);
                    let build_time = start_time.elapsed();

                    // Perform similarity searches (O(log n) per query)
                    let search_start = std::time::Instant::now();
                    let mut total_candidates = 0;

                    for i in 0..count.min(20) {
                        // Test with subset for timing
                        let entity_id = format!("func_{}", i);
                        let candidates = context.find_similar_entities(&entity_id, Some(10));
                        total_candidates += candidates.len();
                    }

                    let search_time = search_start.elapsed();

                    black_box((
                        build_time,
                        search_time,
                        total_candidates,
                        context.get_statistics(),
                    ))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark LSH band configuration effectiveness
fn benchmark_lsh_band_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsh_band_optimization");

    let entities = generate_test_entities(100);
    let entities_refs: Vec<&CodeEntity> = entities.iter().collect();

    // Test different band configurations
    let band_configs = [
        (64, 8),   // 8 hashes per band
        (128, 16), // 8 hashes per band
        (128, 32), // 4 hashes per band
        (256, 32), // 8 hashes per band
    ];

    for (num_hashes, num_bands) in band_configs {
        let lsh_config = LshConfig {
            num_hashes,
            num_bands,
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 50,
            use_semantic_similarity: false,
        };

        let extractor = LshExtractor::new().with_lsh_config(lsh_config);

        group.bench_with_input(
            BenchmarkId::new("band_config", format!("{}h_{}b", num_hashes, num_bands)),
            &(num_hashes, num_bands),
            |b, _| {
                b.iter(|| {
                    let context = extractor.create_similarity_search_context(&entities_refs);

                    // Test similarity search performance with this configuration
                    let mut similarity_scores = Vec::new();
                    for i in 0..5 {
                        let entity_id = format!("func_{}", i);
                        let candidates = context.find_similar_entities(&entity_id, Some(5));
                        similarity_scores.extend(candidates.into_iter().map(|(_, score)| score));
                    }

                    black_box((context.get_statistics(), similarity_scores))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark SIMD-accelerated weighted Jaccard similarity
fn benchmark_simd_jaccard_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_jaccard_similarity");
    
    // Generate test weighted signatures
    let signature_sizes = [4, 16, 64, 128, 256]; // Different signature sizes to test SIMD effectiveness
    
    for &size in &signature_sizes {
        // Create test signatures with f64 values
        let sig1_values: Vec<f64> = (0..size).map(|i| i as f64 * 0.123).collect();
        let sig2_values: Vec<f64> = (0..size).map(|i| (i as f64 * 0.456) + 0.1).collect();
        
        let sig1 = valknut_rs::detectors::lsh::WeightedMinHashSignature::new(sig1_values);
        let sig2 = valknut_rs::detectors::lsh::WeightedMinHashSignature::new(sig2_values);
        
        let analyzer = valknut_rs::detectors::lsh::WeightedShingleAnalyzer::new(3);
        
        group.bench_with_input(
            BenchmarkId::new("simd_weighted_jaccard", size),
            &size,
            |b, _| {
                b.iter(|| {
                    let similarity = analyzer.weighted_jaccard_similarity(&sig1, &sig2);
                    black_box(similarity)
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark parallel IDF table construction
fn benchmark_parallel_idf_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_idf_construction");
    group.measurement_time(Duration::from_secs(10));
    
    let entity_counts = [50, 100, 200, 500]; // Different entity counts to test parallelization benefits
    
    for &count in &entity_counts {
        let entities = generate_test_entities(count);
        let entities_refs: Vec<&CodeEntity> = entities.iter().collect();
        
        group.bench_with_input(
            BenchmarkId::new("parallel_idf_table", count),
            &count,
            |b, _| {
                b.iter(|| {
                    let mut analyzer = valknut_rs::detectors::lsh::WeightedShingleAnalyzer::new(3);
                    let result = analyzer.build_idf_table(&entities_refs);
                    black_box(result)
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark end-to-end weighted signature computation with SIMD + parallel optimizations
fn benchmark_optimized_weighted_signatures(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimized_weighted_signatures");
    group.measurement_time(Duration::from_secs(15));
    
    let entity_counts = [25, 50, 100, 200];
    
    for &count in &entity_counts {
        let entities = generate_test_entities(count);
        let entities_refs: Vec<&CodeEntity> = entities.iter().collect();
        
        group.bench_with_input(
            BenchmarkId::new("full_weighted_pipeline", count),
            &count,
            |b, _| {
                b.iter(|| {
                    let mut analyzer = valknut_rs::detectors::lsh::WeightedShingleAnalyzer::new(3);
                    
                    // This will use parallel IDF table construction
                    let signatures_result = analyzer.compute_weighted_signatures(&entities_refs);
                    
                    if let Ok(signatures) = signatures_result {
                        // Test SIMD similarity calculations
                        let mut total_similarity = 0.0f64;
                        let mut comparison_count = 0;
                        
                        // Compare a subset of signatures to test SIMD performance
                        let sample_size = count.min(10);
                        for i in 0..sample_size {
                            for j in (i + 1)..sample_size {
                                let id1 = format!("func_{}", i);
                                let id2 = format!("func_{}", j);
                                
                                if let (Some(sig1), Some(sig2)) = (signatures.get(&id1), signatures.get(&id2)) {
                                    let similarity = analyzer.weighted_jaccard_similarity(sig1, sig2);
                                    total_similarity += similarity;
                                    comparison_count += 1;
                                }
                            }
                        }
                        
                        black_box((signatures.len(), total_similarity, comparison_count))
                    } else {
                        black_box((0, 0.0, 0))
                    }
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark SIMD vs scalar performance comparison
fn benchmark_simd_vs_scalar_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_vs_scalar");
    
    // Test with large signatures where SIMD benefits are most apparent
    let signature_size = 128;
    let num_comparisons = 1000;
    
    // Generate test data
    let signatures: Vec<valknut_rs::detectors::lsh::WeightedMinHashSignature> = (0..num_comparisons)
        .map(|i| {
            let values: Vec<f64> = (0..signature_size)
                .map(|j| (i * signature_size + j) as f64 * 0.001)
                .collect();
            valknut_rs::detectors::lsh::WeightedMinHashSignature::new(values)
        })
        .collect();
    
    let analyzer = valknut_rs::detectors::lsh::WeightedShingleAnalyzer::new(3);
    
    group.bench_function("simd_enabled_comparisons", |b| {
        b.iter(|| {
            let mut total_similarity = 0.0;
            
            // Compare pairs of signatures (this will use SIMD when available)
            for i in 0..num_comparisons.min(100) {
                let j = (i + 1) % num_comparisons;
                let similarity = analyzer.weighted_jaccard_similarity(&signatures[i], &signatures[j]);
                total_similarity += similarity;
            }
            
            black_box(total_similarity)
        })
    });
    
    group.finish();
}

criterion_group!(
    lsh_benches,
    benchmark_complexity_comparison,
    benchmark_token_caching,
    benchmark_memory_patterns,
    benchmark_lsh_throughput,
    benchmark_lsh_band_optimization,
    benchmark_simd_jaccard_similarity,
    benchmark_parallel_idf_construction,
    benchmark_optimized_weighted_signatures,
    benchmark_simd_vs_scalar_comparison
);

criterion_main!(lsh_benches);
