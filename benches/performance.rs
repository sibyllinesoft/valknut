//! Comprehensive performance benchmarking suite for valknut-rs.
//!
//! This module provides benchmarks for all core performance-critical operations
//! including SIMD-accelerated computations, parallel processing, and memory optimization.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box as std_black_box;
use valknut_rs::core::{
    bayesian::BayesianNormalizer, config::ValknutConfig, featureset::FeatureVector,
    pipeline::AnalysisPipeline, scoring::FeatureScorer,
};
use valknut_rs::detectors::lsh::LshExtractor;

/// Generate synthetic feature vectors for benchmarking
fn generate_test_vectors(count: usize, features_per_vector: usize) -> Vec<FeatureVector> {
    (0..count)
        .map(|i| {
            let mut vector = FeatureVector::new(format!("entity_{}", i));

            // Add complexity features
            vector.add_feature("cyclomatic", (i % 20) as f64 + 1.0);
            vector.add_feature("cognitive", (i % 50) as f64);
            vector.add_feature("max_nesting", (i % 10) as f64);
            vector.add_feature("param_count", (i % 15) as f64);
            vector.add_feature("lines_of_code", (i % 500) as f64 + 10.0);

            // Add additional features to reach target count
            for j in 5..features_per_vector {
                vector.add_feature(&format!("feature_{}", j), (i * j) as f64 * 0.1);
            }

            vector
        })
        .collect()
}

/// Generate source code strings for LSH benchmarking
fn generate_test_code(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| {
            format!(
                r#"
def function_{}(param1, param2, param3):
    if param1 > 10:
        for j in range(param2):
            if j % 2 == 0:
                result = param3 * j
            else:
                result = param3 + j
    else:
        result = param1 + param2 + param3
    return result

class Class_{}:
    def __init__(self, value):
        self.value = value
        self.processed = False
    
    def process(self):
        if not self.processed:
            self.value *= 2
            self.processed = True
        return self.value
"#,
                i, i
            )
        })
        .collect()
}

/// Benchmark Bayesian normalization performance
fn benchmark_bayesian_normalization(c: &mut Criterion) {
    let mut group = c.benchmark_group("bayesian_normalization");

    for size in [100, 500, 1000, 5000].iter() {
        let vectors = generate_test_vectors(*size, 10);
        let mut normalizer = BayesianNormalizer::new("z_score");
        normalizer.fit(&vectors).unwrap();

        group.bench_with_input(BenchmarkId::new("sequential", size), size, |b, _| {
            b.iter(|| {
                let mut test_vectors = black_box(vectors.clone());
                normalizer.normalize(&mut test_vectors).unwrap();
                std_black_box(test_vectors);
            });
        });

        #[cfg(feature = "parallel")]
        group.bench_with_input(BenchmarkId::new("parallel", size), size, |b, _| {
            b.iter(|| {
                let mut test_vectors = black_box(vectors.clone());
                normalizer.normalize_parallel(&mut test_vectors).unwrap();
                std_black_box(test_vectors);
            });
        });
    }

    group.finish();
}

/// Benchmark SIMD vs scalar normalization
#[cfg(feature = "simd")]
fn benchmark_simd_normalization(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_normalization");

    let normalizer = BayesianNormalizer::new("z_score");
    let vectors = generate_test_vectors(1000, 10);
    normalizer.fit(&vectors).unwrap();

    // Create large arrays for batch processing
    for size in [1000, 5000, 10000].iter() {
        let mut test_data: Vec<f64> = (0..*size).map(|i| i as f64 * 0.1).collect();

        group.bench_with_input(BenchmarkId::new("simd_batch", size), size, |b, _| {
            b.iter(|| {
                let mut data = black_box(test_data.clone());
                normalizer
                    .normalize_batch_simd(&mut data, "cyclomatic")
                    .unwrap();
                std_black_box(data);
            });
        });

        group.bench_with_input(BenchmarkId::new("scalar_batch", size), size, |b, _| {
            b.iter(|| {
                let mut data = black_box(test_data.clone());
                // Simulate scalar normalization
                let mean = 50.0;
                let std_dev = 10.0;
                for val in &mut data {
                    *val = (*val - mean) / std_dev;
                }
                std_black_box(data);
            });
        });
    }

    group.finish();
}

/// Benchmark LSH/MinHash performance
fn benchmark_lsh_minhash(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsh_minhash");

    let extractor = LshExtractor::new(128, 3); // 128 hashes, 3-grams

    for size in [50, 100, 500].iter() {
        let code_samples = generate_test_code(*size);

        group.bench_with_input(
            BenchmarkId::new("minhash_sequential", size),
            size,
            |b, _| {
                b.iter(|| {
                    let samples = black_box(&code_samples);
                    for code in samples {
                        let signature = extractor.generate_minhash_signature(code);
                        std_black_box(signature);
                    }
                });
            },
        );

        #[cfg(feature = "simd")]
        group.bench_with_input(BenchmarkId::new("minhash_simd", size), size, |b, _| {
            b.iter(|| {
                let samples = black_box(&code_samples);
                for code in samples {
                    let signature = extractor.generate_minhash_signature_simd(code);
                    std_black_box(signature);
                }
            });
        });
    }

    group.finish();
}

/// Benchmark pipeline performance
fn benchmark_pipeline_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_performance");

    let config = ValknutConfig::default();
    let mut pipeline = AnalysisPipeline::new(config);

    // Prepare training data
    let training_vectors = generate_test_vectors(100, 8);
    pipeline.fit(&training_vectors).await.unwrap();

    for size in [100, 500, 1000].iter() {
        let test_vectors = generate_test_vectors(*size, 8);

        group.bench_with_input(
            BenchmarkId::new("sequential_analysis", size),
            size,
            |b, _| {
                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let vectors = black_box(test_vectors.clone());
                        let results = pipeline.analyze_vectors(vectors).await.unwrap();
                        std_black_box(results);
                    });
            },
        );

        #[cfg(feature = "parallel")]
        group.bench_with_input(BenchmarkId::new("parallel_analysis", size), size, |b, _| {
            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let vectors = black_box(test_vectors.clone());
                    let results = pipeline.analyze_vectors_parallel(vectors).await.unwrap();
                    std_black_box(results);
                });
        });
    }

    group.finish();
}

/// Benchmark memory allocation patterns
fn benchmark_memory_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_optimization");

    // Test vector creation performance
    for size in [1000, 5000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("vector_creation", size), size, |b, _| {
            b.iter(|| {
                let vectors = generate_test_vectors(black_box(*size), 10);
                std_black_box(vectors);
            });
        });

        group.bench_with_input(BenchmarkId::new("vector_cloning", size), size, |b, _| {
            let original_vectors = generate_test_vectors(*size, 10);
            b.iter(|| {
                let cloned = black_box(original_vectors.clone());
                std_black_box(cloned);
            });
        });

        // Test memory-optimized operations
        group.bench_with_input(
            BenchmarkId::new("memory_optimized_processing", size),
            size,
            |b, _| {
                let mut vectors = generate_test_vectors(*size, 10);
                b.iter(|| {
                    for vector in &mut vectors {
                        // Simulate memory optimization
                        vector.features.shrink_to_fit();
                        vector.normalized_features.reserve(vector.features.len());

                        // Simulate processing
                        for (key, value) in vector.features.clone() {
                            vector.normalized_features.insert(key, value * 0.5);
                        }
                    }
                    std_black_box(&vectors);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent data structure performance
#[cfg(feature = "parallel")]
fn benchmark_concurrent_structures(c: &mut Criterion) {
    use rayon::prelude::*;
    use std::sync::Arc;
    use valknut_rs::detectors::graph::ConcurrentDependencyGraph;

    let mut group = c.benchmark_group("concurrent_structures");

    for size in [100, 500, 1000].iter() {
        let entity_ids: Vec<String> = (0..*size).map(|i| format!("entity_{}", i)).collect();

        group.bench_with_input(
            BenchmarkId::new("concurrent_graph_creation", size),
            size,
            |b, _| {
                b.iter(|| {
                    let graph = Arc::new(ConcurrentDependencyGraph::new());
                    let ids = black_box(&entity_ids);

                    graph.add_entities_parallel(ids);
                    std_black_box(graph);
                });
            },
        );

        // Benchmark parallel dependency analysis
        let test_entities = generate_test_vectors(*size, 5)
            .into_iter()
            .enumerate()
            .map(|(i, mut vector)| {
                use valknut_rs::core::featureset::CodeEntity;
                CodeEntity::new(
                    format!("entity_{}", i),
                    "Function".to_string(),
                    format!("function_{}", i),
                    "test.py".to_string(),
                )
            })
            .collect::<Vec<_>>();

        group.bench_with_input(
            BenchmarkId::new("parallel_dependency_analysis", size),
            size,
            |b, _| {
                b.iter(|| {
                    let graph = ConcurrentDependencyGraph::new();
                    let entities = black_box(&test_entities);

                    let results = graph.analyze_dependencies_parallel(entities);
                    std_black_box(results);
                });
            },
        );
    }

    group.finish();
}

// Configure criterion groups
criterion_group!(
    benches,
    benchmark_bayesian_normalization,
    benchmark_lsh_minhash,
    benchmark_pipeline_performance,
    benchmark_memory_optimization,
);

#[cfg(feature = "simd")]
criterion_group!(simd_benches, benchmark_simd_normalization);

#[cfg(feature = "parallel")]
criterion_group!(parallel_benches, benchmark_concurrent_structures);

// Main benchmark runner
#[cfg(all(feature = "simd", feature = "parallel"))]
criterion_main!(benches, simd_benches, parallel_benches);

#[cfg(all(feature = "simd", not(feature = "parallel")))]
criterion_main!(benches, simd_benches);

#[cfg(all(not(feature = "simd"), feature = "parallel"))]
criterion_main!(benches, parallel_benches);

#[cfg(all(not(feature = "simd"), not(feature = "parallel")))]
criterion_main!(benches);
