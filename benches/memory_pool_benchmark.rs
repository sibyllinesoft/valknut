//! Benchmark to validate memory pool integration and effectiveness

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use valknut_rs::detectors::lsh::LshExtractor;

fn benchmark_memory_pool_effectiveness(c: &mut Criterion) {
    let lsh_extractor = LshExtractor::new();

    // Test code for benchmarking
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

    c.bench_function("signature_generation_with_pools", |b| {
        b.iter(|| black_box(lsh_extractor.generate_minhash_signature(black_box(source_code))));
    });

    c.bench_function("shingle_creation_with_pools", |b| {
        b.iter(|| black_box(lsh_extractor.create_shingles(black_box(source_code))));
    });

    // Benchmark memory pool reuse by running multiple times
    c.bench_function("repeated_operations_with_pools", |b| {
        b.iter(|| {
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

                black_box(lsh_extractor.generate_minhash_signature(black_box(&test_code)));
                black_box(lsh_extractor.create_shingles(black_box(&test_code)));
            }
        });
    });
}

fn benchmark_memory_pool_statistics(c: &mut Criterion) {
    let lsh_extractor = LshExtractor::new();

    // Generate some activity first
    for i in 0..10 {
        let test_code = format!("def func_{}(): return {}", i, i);
        lsh_extractor.generate_minhash_signature(&test_code);
        lsh_extractor.create_shingles(&test_code);
    }

    c.bench_function("memory_pool_statistics", |b| {
        b.iter(|| black_box(lsh_extractor.get_memory_pool_statistics()));
    });
}

criterion_group!(
    benches,
    benchmark_memory_pool_effectiveness,
    benchmark_memory_pool_statistics
);
criterion_main!(benches);
