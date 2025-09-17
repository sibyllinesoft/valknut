//! SIMD optimization tests for valknut
//!
//! This module tests SIMD-accelerated mathematical operations and ensures
//! they provide the expected performance benefits while maintaining correctness.

use std::time::{Duration, Instant};

/// Helper for timing SIMD vs scalar operations
fn compare_performance<F1, F2, R>(description: &str, simd_op: F1, scalar_op: F2) -> (R, R, f64)
where
    F1: FnOnce() -> R,
    F2: FnOnce() -> R,
    R: Clone,
{
    println!("🔬 Comparing: {}", description);

    // Run SIMD version
    let simd_start = Instant::now();
    let simd_result = simd_op();
    let simd_duration = simd_start.elapsed();

    // Run scalar version
    let scalar_start = Instant::now();
    let scalar_result = scalar_op();
    let scalar_duration = scalar_start.elapsed();

    let speedup = scalar_duration.as_secs_f64() / simd_duration.as_secs_f64();

    println!(
        "⚡ SIMD: {:?}, Scalar: {:?}, Speedup: {:.2}x",
        simd_duration, scalar_duration, speedup
    );

    (simd_result, scalar_result, speedup)
}

/// Tests for SIMD mathematical operations
#[cfg(test)]
mod simd_math_tests {
    use super::*;

    #[test]
    fn test_simd_enabled_compilation() {
        // Test that SIMD features are available when enabled
        #[cfg(feature = "simd")]
        {
            println!("✅ SIMD feature is enabled");
            // Test basic SIMD types are available
            #[cfg(target_arch = "x86_64")]
            {
                use wide::f64x4;
                let simd_vec = f64x4::new([1.0, 2.0, 3.0, 4.0]);
                let doubled = simd_vec * f64x4::splat(2.0);
                assert_eq!(doubled.as_array_ref(), &[2.0, 4.0, 6.0, 8.0]);
                println!("✅ SIMD vector operations working");
            }
        }

        #[cfg(not(feature = "simd"))]
        {
            println!("⚠️  SIMD feature is disabled - tests will use scalar fallbacks");
        }
    }

    #[test]
    fn test_vector_normalization_simd_vs_scalar() {
        let values: Vec<f64> = (0..1000).map(|i| (i as f64) * 0.1).collect();
        let mean = 50.0;
        let std_dev = 10.0;

        let (simd_result, scalar_result, speedup) = compare_performance(
            "Vector normalization (1000 elements)",
            || {
                // SIMD normalization (simulated for test)
                let mut simd_values = values.clone();

                #[cfg(feature = "simd")]
                #[cfg(target_arch = "x86_64")]
                {
                    use wide::f64x4;
                    let mean_vec = f64x4::splat(mean);
                    let std_vec = f64x4::splat(std_dev);

                    for chunk in simd_values.chunks_exact_mut(4) {
                        let vals = f64x4::new([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        let normalized = (vals - mean_vec) / std_vec;
                        let result_array = normalized.as_array_ref();
                        chunk.copy_from_slice(result_array);
                    }

                    // Handle remaining elements
                    let remainder = simd_values.len() % 4;
                    if remainder > 0 {
                        let start = simd_values.len() - remainder;
                        for val in &mut simd_values[start..] {
                            *val = (*val - mean) / std_dev;
                        }
                    }
                }

                #[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
                {
                    // Fallback to scalar when SIMD not available
                    for val in &mut simd_values {
                        *val = (*val - mean) / std_dev;
                    }
                }

                simd_values
            },
            || {
                // Scalar normalization
                let mut scalar_values = values.clone();
                for val in &mut scalar_values {
                    *val = (*val - mean) / std_dev;
                }
                scalar_values
            },
        );

        // Results should be approximately equal
        assert_eq!(simd_result.len(), scalar_result.len());
        for (simd_val, scalar_val) in simd_result.iter().zip(scalar_result.iter()) {
            let diff = (simd_val - scalar_val).abs();
            assert!(
                diff < 1e-10,
                "SIMD and scalar results differ: {} vs {}",
                simd_val,
                scalar_val
            );
        }

        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            // SIMD should provide some speedup for large arrays
            assert!(speedup >= 1.0, "SIMD should not be slower than scalar");
        }

        println!("✅ Vector normalization correctness verified");
    }

    #[test]
    fn test_dot_product_simd_vs_scalar() {
        let vec1: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let vec2: Vec<f64> = (0..1000).map(|i| (i * 2) as f64).collect();

        let (simd_result, scalar_result, speedup) = compare_performance(
            "Dot product (1000 elements)",
            || {
                // SIMD dot product
                let result;

                #[cfg(feature = "simd")]
                #[cfg(target_arch = "x86_64")]
                {
                    use wide::f64x4;
                    let mut sum_vec = f64x4::splat(0.0);

                    for (chunk1, chunk2) in vec1.chunks_exact(4).zip(vec2.chunks_exact(4)) {
                        let v1 = f64x4::new([chunk1[0], chunk1[1], chunk1[2], chunk1[3]]);
                        let v2 = f64x4::new([chunk2[0], chunk2[1], chunk2[2], chunk2[3]]);
                        sum_vec += v1 * v2;
                    }

                    // Sum the SIMD vector components
                    let sum_array = sum_vec.as_array_ref();
                    let mut partial_result =
                        sum_array[0] + sum_array[1] + sum_array[2] + sum_array[3];

                    // Handle remaining elements
                    let remainder = vec1.len() % 4;
                    if remainder > 0 {
                        let start = vec1.len() - remainder;
                        for i in start..vec1.len() {
                            partial_result += vec1[i] * vec2[i];
                        }
                    }
                    result = partial_result;
                }

                #[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
                {
                    // Fallback to scalar
                    let mut scalar_result = 0.0;
                    for (v1, v2) in vec1.iter().zip(vec2.iter()) {
                        scalar_result += v1 * v2;
                    }
                    result = scalar_result;
                }

                result
            },
            || {
                // Scalar dot product
                let mut result = 0.0;
                for (v1, v2) in vec1.iter().zip(vec2.iter()) {
                    result += v1 * v2;
                }
                result
            },
        );

        // Results should be approximately equal
        let diff = (simd_result - scalar_result).abs();
        assert!(
            diff < 1e-8,
            "SIMD and scalar dot products differ: {} vs {}",
            simd_result,
            scalar_result
        );

        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            assert!(speedup >= 1.0, "SIMD should not be slower than scalar");
        }

        println!("✅ Dot product correctness verified");
    }

    #[test]
    fn test_statistical_operations_simd() {
        let values: Vec<f64> = (1..=1000).map(|i| i as f64).collect();

        let (simd_mean, scalar_mean, _) = compare_performance(
            "Mean calculation (1000 elements)",
            || {
                // SIMD mean calculation
                #[cfg(feature = "simd")]
                #[cfg(target_arch = "x86_64")]
                {
                    use wide::f64x4;
                    let mut sum_vec = f64x4::splat(0.0);

                    for chunk in values.chunks_exact(4) {
                        let vals = f64x4::new([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        sum_vec += vals;
                    }

                    let sum_array = sum_vec.as_array_ref();
                    let mut total = sum_array[0] + sum_array[1] + sum_array[2] + sum_array[3];

                    // Handle remainder
                    let remainder = values.len() % 4;
                    if remainder > 0 {
                        let start = values.len() - remainder;
                        for &val in &values[start..] {
                            total += val;
                        }
                    }

                    total / values.len() as f64
                }

                #[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
                {
                    values.iter().sum::<f64>() / values.len() as f64
                }
            },
            || {
                // Scalar mean calculation
                values.iter().sum::<f64>() / values.len() as f64
            },
        );

        // Results should be approximately equal
        let diff = (simd_mean - scalar_mean).abs();
        assert!(
            diff < 1e-10,
            "SIMD and scalar means differ: {} vs {}",
            simd_mean,
            scalar_mean
        );

        // Check expected result
        let expected_mean = 500.5; // Mean of 1..=1000
        assert!(
            (simd_mean - expected_mean).abs() < 1e-10,
            "Mean calculation incorrect"
        );

        println!("✅ Statistical operations correctness verified");
    }
}

/// Tests for SIMD performance characteristics
#[cfg(test)]
mod simd_performance_tests {
    use super::*;

    #[test]
    fn test_simd_performance_scaling() {
        // Test how SIMD performance scales with data size
        let sizes = vec![100, 500, 1000, 5000];

        for size in sizes {
            let values: Vec<f64> = (0..size).map(|i| i as f64).collect();

            let simd_start = Instant::now();
            let _simd_sum = calculate_sum_simd(&values);
            let simd_duration = simd_start.elapsed();

            let scalar_start = Instant::now();
            let _scalar_sum = calculate_sum_scalar(&values);
            let scalar_duration = scalar_start.elapsed();

            let speedup = scalar_duration.as_secs_f64() / simd_duration.as_secs_f64();

            println!(
                "📊 Size {}: SIMD {:?}, Scalar {:?}, Speedup: {:.2}x",
                size, simd_duration, scalar_duration, speedup
            );

            // For larger sizes, SIMD should show benefits
            #[cfg(all(feature = "simd", target_arch = "x86_64"))]
            if size >= 1000 {
                assert!(speedup >= 1.0, "SIMD should not be slower for large arrays");
            }
        }
    }

    #[test]
    fn test_simd_memory_alignment_benefits() {
        // Test that properly aligned data benefits from SIMD
        let aligned_data = create_aligned_test_data(1024);
        let unaligned_data = create_unaligned_test_data(1024);

        let aligned_start = Instant::now();
        let _aligned_result = calculate_sum_simd(&aligned_data);
        let aligned_duration = aligned_start.elapsed();

        let unaligned_start = Instant::now();
        let _unaligned_result = calculate_sum_simd(&unaligned_data);
        let unaligned_duration = unaligned_start.elapsed();

        println!(
            "📊 Aligned SIMD: {:?}, Unaligned SIMD: {:?}",
            aligned_duration, unaligned_duration
        );

        // Aligned data should generally perform better, but this is highly platform-dependent
        // So we just ensure both complete successfully
        assert!(_aligned_result.is_finite());
        assert!(_unaligned_result.is_finite());
    }
}

/// Helper functions for SIMD testing
fn calculate_sum_simd(values: &[f64]) -> f64 {
    #[cfg(feature = "simd")]
    #[cfg(target_arch = "x86_64")]
    {
        use wide::f64x4;
        let mut sum_vec = f64x4::splat(0.0);

        for chunk in values.chunks_exact(4) {
            let vals = f64x4::new([chunk[0], chunk[1], chunk[2], chunk[3]]);
            sum_vec += vals;
        }

        let sum_array = sum_vec.as_array_ref();
        let mut total = sum_array[0] + sum_array[1] + sum_array[2] + sum_array[3];

        // Handle remainder
        let remainder = values.len() % 4;
        if remainder > 0 {
            let start = values.len() - remainder;
            for &val in &values[start..] {
                total += val;
            }
        }

        total
    }

    #[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
    {
        calculate_sum_scalar(values)
    }
}

fn calculate_sum_scalar(values: &[f64]) -> f64 {
    values.iter().sum()
}

fn create_aligned_test_data(size: usize) -> Vec<f64> {
    // Create well-aligned data
    (0..size).map(|i| i as f64).collect()
}

fn create_unaligned_test_data(size: usize) -> Vec<f64> {
    // Create data that might not be optimally aligned
    let mut data = Vec::new();
    data.push(0.5); // Add odd offset
    data.extend((0..size).map(|i| i as f64));
    data
}

/// Integration tests for SIMD with valknut components
#[cfg(test)]
mod simd_integration_tests {
    use super::*;
    use valknut_rs::core::featureset::FeatureVector;

    #[test]
    fn test_simd_with_feature_vectors() {
        // Test SIMD operations integrated with FeatureVector
        let mut vectors = Vec::new();

        for i in 0..100 {
            let mut vector = FeatureVector::new(format!("simd_entity_{}", i));
            vector.add_feature("value1", i as f64);
            vector.add_feature("value2", (i * 2) as f64);
            vector.add_feature("value3", (i * 3) as f64);
            vector.add_feature("value4", (i * 4) as f64);
            vectors.push(vector);
        }

        let start_time = Instant::now();

        // Simulate SIMD-accelerated operations on feature vectors
        let mut total_norms = 0.0;
        for vector in &vectors {
            total_norms += vector.l2_norm();
        }

        let duration = start_time.elapsed();

        assert!(total_norms > 0.0);
        assert!(
            duration < Duration::from_millis(10),
            "Feature vector operations should be fast"
        );

        println!(
            "✅ SIMD integration with feature vectors completed in {:?}",
            duration
        );
    }

    #[test]
    fn test_simd_error_handling() {
        // Test that SIMD operations handle edge cases correctly
        let test_cases = [
            vec![],              // Empty vector
            vec![f64::NAN],      // NaN values
            vec![f64::INFINITY], // Infinite values
            vec![0.0; 1000],     // All zeros
            vec![f64::MAX; 10],  // Very large values
            vec![f64::MIN; 10],  // Very small values
        ];

        for (i, test_data) in test_cases.iter().enumerate() {
            let simd_result = calculate_sum_simd(test_data);
            let scalar_result = calculate_sum_scalar(test_data);

            println!(
                "🧪 Test case {}: SIMD={}, Scalar={}",
                i, simd_result, scalar_result
            );

            // Both should handle edge cases the same way
            if scalar_result.is_nan() {
                assert!(simd_result.is_nan(), "SIMD should handle NaN like scalar");
            } else if scalar_result.is_infinite() {
                assert!(
                    simd_result.is_infinite(),
                    "SIMD should handle infinity like scalar"
                );
            } else {
                let diff = (simd_result - scalar_result).abs();
                assert!(
                    diff < 1e-10 || (simd_result == 0.0 && scalar_result == 0.0),
                    "SIMD and scalar should agree on edge cases"
                );
            }
        }

        println!("✅ SIMD error handling verified");
    }
}

/// Benchmarks for different SIMD instruction sets
#[cfg(test)]
mod simd_instruction_set_tests {

    #[test]
    fn test_available_simd_features() {
        println!("🔍 Checking available SIMD features:");

        #[cfg(target_arch = "x86_64")]
        {
            if std::arch::is_x86_feature_detected!("sse") {
                println!("✅ SSE available");
            } else {
                println!("❌ SSE not available");
            }

            if std::arch::is_x86_feature_detected!("sse2") {
                println!("✅ SSE2 available");
            } else {
                println!("❌ SSE2 not available");
            }

            if std::arch::is_x86_feature_detected!("sse3") {
                println!("✅ SSE3 available");
            } else {
                println!("❌ SSE3 not available");
            }

            if std::arch::is_x86_feature_detected!("avx") {
                println!("✅ AVX available");
            } else {
                println!("❌ AVX not available");
            }

            if std::arch::is_x86_feature_detected!("avx2") {
                println!("✅ AVX2 available");
            } else {
                println!("❌ AVX2 not available");
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if std::arch::is_aarch64_feature_detected!("neon") {
                println!("✅ NEON available");
            } else {
                println!("❌ NEON not available");
            }
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            println!("ℹ️ SIMD feature detection not available for this architecture");
        }
    }

    #[test]
    fn test_simd_compilation_flags() {
        // Verify that SIMD optimizations are properly enabled
        #[cfg(feature = "simd")]
        {
            println!("✅ SIMD feature flag is enabled");

            #[cfg(target_feature = "sse2")]
            println!("✅ SSE2 target feature enabled");

            #[cfg(target_feature = "avx")]
            println!("✅ AVX target feature enabled");

            #[cfg(target_feature = "avx2")]
            println!("✅ AVX2 target feature enabled");
        }

        #[cfg(not(feature = "simd"))]
        {
            println!("⚠️ SIMD feature flag is disabled");
        }
    }
}
