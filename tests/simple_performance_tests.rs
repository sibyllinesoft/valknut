//! Simple performance regression tests for valknut
//!
//! This module provides basic performance tests to detect regressions
//! in critical paths without complex API dependencies.

use std::fs;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use valknut_rs::{
    api::{config_types::AnalysisConfig, engine::ValknutEngine},
    core::featureset::FeatureVector,
};

/// Helper for timing operations
fn time_operation<F, R>(description: &str, operation: F) -> (R, Duration)
where
    F: FnOnce() -> R,
{
    println!("🏃 Starting: {}", description);
    let start = Instant::now();
    let result = operation();
    let duration = start.elapsed();
    println!("✅ Completed '{}' in {:?}", description, duration);
    (result, duration)
}

/// Performance tests for core engine operations
#[cfg(test)]
mod engine_performance_tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_creation_performance() {
        let (_, duration) = time_operation("Engine creation", || {
            futures::executor::block_on(async {
                ValknutEngine::new(AnalysisConfig::default())
                    .await
                    .expect("Failed to create engine")
            })
        });

        // Engine creation should be fast
        assert!(
            duration < Duration::from_secs(5),
            "Engine creation took too long: {:?}",
            duration
        );
    }

    #[tokio::test]
    async fn test_feature_vector_analysis_performance() {
        let mut engine = ValknutEngine::new(AnalysisConfig::default())
            .await
            .expect("Failed to create engine");

        let (_, duration) = time_operation("Feature vector analysis (100 vectors)", || {
            futures::executor::block_on(async {
                let mut vectors = Vec::new();
                for i in 0..100 {
                    let mut vector = FeatureVector::new(format!("entity_{}", i));
                    vector.add_feature("complexity", (i % 50) as f64);
                    vector.add_feature("lines_of_code", ((i % 30) + 10) as f64);
                    vector.add_feature("nesting_depth", (i % 8) as f64);
                    vectors.push(vector);
                }

                engine
                    .analyze_vectors(vectors)
                    .await
                    .expect("Vector analysis failed")
            })
        });

        // Should process 100 vectors quickly
        assert!(
            duration < Duration::from_secs(10),
            "Vector analysis took too long: {:?}",
            duration
        );

        println!(
            "📊 Processed 100 vectors in {:?} ({:.1} vectors/sec)",
            duration,
            100.0 / duration.as_secs_f64()
        );
    }

    #[tokio::test]
    async fn test_small_directory_analysis_performance() {
        let temp_dir = tempdir().expect("Failed to create temp directory");

        // Create test files
        let num_files = 20;
        for i in 0..num_files {
            let file_path = temp_dir.path().join(format!("test_{}.py", i));
            let code = format!(
                "def function_{}():\n    value = {}\n    return value * 2\n",
                i,
                i * 10
            );
            fs::write(file_path, code).expect("Failed to write test file");
        }

        let mut engine = ValknutEngine::new(AnalysisConfig::default())
            .await
            .expect("Failed to create engine");

        let (_, duration) = time_operation("Small directory analysis", || {
            futures::executor::block_on(async {
                engine
                    .analyze_directory(temp_dir.path())
                    .await
                    .expect("Directory analysis failed")
            })
        });

        // Should analyze small directory quickly
        assert!(
            duration < Duration::from_secs(30),
            "Small directory analysis took too long: {:?}",
            duration
        );

        println!(
            "📊 Analyzed {} files in {:?} ({:.1} files/sec)",
            num_files,
            duration,
            num_files as f64 / duration.as_secs_f64()
        );
    }
}

/// Performance tests for feature vector operations
#[cfg(test)]
mod feature_vector_performance_tests {
    use super::*;

    #[test]
    fn test_feature_vector_creation_performance() {
        let (vectors, duration) = time_operation("Feature vector creation (1000 vectors)", || {
            let mut vectors = Vec::new();
            for i in 0..1000 {
                let mut vector = FeatureVector::new(format!("entity_{}", i));
                vector.add_feature("feature_1", i as f64);
                vector.add_feature("feature_2", (i * 2) as f64);
                vector.add_feature("feature_3", (i * 3) as f64);
                vector.add_feature("feature_4", (i * 4) as f64);
                vector.add_feature("feature_5", (i * 5) as f64);
                vectors.push(vector);
            }
            vectors
        });

        // Should create vectors quickly
        assert!(
            duration < Duration::from_secs(1),
            "Vector creation took too long: {:?}",
            duration
        );

        assert_eq!(vectors.len(), 1000);
        println!(
            "📊 Created 1000 vectors with 5 features each in {:?}",
            duration
        );
    }

    #[test]
    fn test_feature_vector_l2_norm_performance() {
        // Create vectors with many features
        let mut vectors = Vec::new();
        for i in 0..100 {
            let mut vector = FeatureVector::new(format!("entity_{}", i));
            for j in 0..50 {
                vector.add_feature(&format!("feature_{}", j), (i + j) as f64);
            }
            vectors.push(vector);
        }

        let (_, duration) = time_operation(
            "L2 norm calculation (100 vectors, 50 features each)",
            || {
                for vector in &vectors {
                    let _norm = vector.l2_norm();
                }
            },
        );

        // Should calculate norms quickly
        assert!(
            duration < Duration::from_millis(100),
            "L2 norm calculation took too long: {:?}",
            duration
        );

        println!(
            "📊 Calculated L2 norm for 100 vectors (5000 features total) in {:?}",
            duration
        );
    }

    #[test]
    fn test_feature_access_performance() {
        let mut vector = FeatureVector::new("test_entity");

        // Add many features
        for i in 0..1000 {
            vector.add_feature(&format!("feature_{}", i), i as f64);
        }

        let (_, duration) = time_operation("Feature access (1000 lookups)", || {
            for i in 0..1000 {
                let _value = vector.get_feature(&format!("feature_{}", i));
            }
        });

        // Feature access should be very fast
        assert!(
            duration < Duration::from_millis(10),
            "Feature access took too long: {:?}",
            duration
        );

        println!("📊 Performed 1000 feature lookups in {:?}", duration);
    }
}

/// Memory efficiency tests
#[cfg(test)]
mod memory_efficiency_tests {
    use super::*;

    #[test]
    fn test_large_vector_set_memory_efficiency() {
        let (vectors, duration) = time_operation("Large vector set creation", || {
            let mut vectors = Vec::new();

            // Create many vectors to test memory usage
            for i in 0..5000 {
                let mut vector = FeatureVector::new(format!("entity_{}", i));

                // Add reasonable number of features per vector
                for j in 0..10 {
                    vector.add_feature(&format!("f_{}", j), (i + j) as f64);
                }

                vectors.push(vector);
            }

            vectors
        });

        // Should handle large vector sets efficiently
        assert!(
            duration < Duration::from_secs(5),
            "Large vector set creation took too long: {:?}",
            duration
        );

        assert_eq!(vectors.len(), 5000);
        println!(
            "📊 Created 5000 vectors (50k features total) in {:?}",
            duration
        );

        // Test accessing the vectors
        let (_, access_duration) = time_operation("Large vector set access", || {
            let mut total_features = 0;
            for vector in &vectors {
                total_features += vector.feature_count();
            }
            total_features
        });

        assert!(
            access_duration < Duration::from_millis(100),
            "Vector set access took too long: {:?}",
            access_duration
        );
    }

    #[test]
    fn test_metadata_efficiency() {
        let (vector, duration) = time_operation("Metadata operations", || {
            let mut vector = FeatureVector::new("metadata_test");

            // Add various metadata
            for i in 0..100 {
                vector.add_metadata(
                    format!("key_{}", i),
                    serde_json::json!(format!("value_{}", i)),
                );
            }

            vector
        });

        // Metadata operations should be efficient
        assert!(
            duration < Duration::from_millis(50),
            "Metadata operations took too long: {:?}",
            duration
        );

        assert_eq!(vector.metadata.len(), 100);
        println!("📊 Added 100 metadata entries in {:?}", duration);
    }
}

/// Concurrency and parallel processing tests
#[cfg(test)]
mod concurrency_tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_concurrent_vector_processing() {
        let num_threads = 4;
        let vectors_per_thread = 250;

        let (_, duration) = time_operation("Concurrent vector processing", || {
            let handles: Vec<_> = (0..num_threads)
                .map(|thread_id| {
                    thread::spawn(move || {
                        let mut local_vectors = Vec::new();

                        for i in 0..vectors_per_thread {
                            let mut vector =
                                FeatureVector::new(format!("thread_{}_entity_{}", thread_id, i));

                            vector.add_feature("complexity", (i % 20) as f64);
                            vector.add_feature("thread_id", thread_id as f64);

                            local_vectors.push(vector);
                        }

                        // Simulate processing
                        let mut total_norm = 0.0;
                        for vector in &local_vectors {
                            total_norm += vector.l2_norm();
                        }

                        total_norm
                    })
                })
                .collect();

            let mut total = 0.0;
            for handle in handles {
                total += handle.join().expect("Thread panicked");
            }
            total
        });

        // Concurrent processing should be efficient
        assert!(
            duration < Duration::from_secs(2),
            "Concurrent processing took too long: {:?}",
            duration
        );

        println!(
            "📊 Processed {} vectors across {} threads in {:?}",
            num_threads * vectors_per_thread,
            num_threads,
            duration
        );
    }

    #[tokio::test]
    async fn test_async_engine_operations() {
        let (_, duration) = time_operation("Async engine operations", || {
            futures::executor::block_on(async {
                let config = AnalysisConfig::default();

                // Create multiple engines concurrently
                let mut handles = Vec::new();
                for i in 0..3 {
                    let config_clone = config.clone();
                    let handle = tokio::spawn(async move {
                        let mut engine = ValknutEngine::new(config_clone)
                            .await
                            .expect("Failed to create engine");

                        // Create some vectors to analyze
                        let mut vectors = Vec::new();
                        for j in 0..20 {
                            let mut vector =
                                FeatureVector::new(format!("async_entity_{}_{}", i, j));
                            vector.add_feature("value", (i * 20 + j) as f64);
                            vectors.push(vector);
                        }

                        engine
                            .analyze_vectors(vectors)
                            .await
                            .expect("Vector analysis failed")
                    });
                    handles.push(handle);
                }

                // Wait for all to complete
                for handle in handles {
                    let _result = handle.await.expect("Task panicked");
                }
            })
        });

        // Async operations should complete efficiently
        assert!(
            duration < Duration::from_secs(15),
            "Async operations took too long: {:?}",
            duration
        );

        println!("📊 Completed 3 concurrent async analyses in {:?}", duration);
    }
}

/// Stress tests for performance validation
#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    fn test_stress_feature_vector_operations() {
        let (_, duration) = time_operation("Stress test: feature vector operations", || {
            let mut vectors = Vec::new();

            // Create a large number of vectors with varying feature counts
            for i in 0..1000 {
                let mut vector = FeatureVector::new(format!("stress_entity_{}", i));

                let feature_count = (i % 20) + 1; // 1 to 20 features
                for j in 0..feature_count {
                    vector.add_feature(&format!("feature_{}", j), (i * j) as f64);
                }

                vectors.push(vector);
            }

            // Perform various operations on all vectors
            let mut total_operations = 0;
            for vector in &vectors {
                total_operations += vector.feature_count();
                let _norm = vector.l2_norm();
                let _has_complexity = vector.has_feature("feature_0");
            }

            total_operations
        });

        // Stress test should complete in reasonable time
        assert!(
            duration < Duration::from_secs(3),
            "Stress test took too long: {:?}",
            duration
        );

        println!(
            "📊 Stress test completed 1000 vectors with varying features in {:?}",
            duration
        );
    }

    #[test]
    fn test_unicode_handling_performance() {
        let (_, duration) = time_operation("Unicode handling performance", || {
            let mut vectors = Vec::new();

            // Test with various Unicode entity IDs
            let unicode_ids = vec![
                "测试实体",
                "файл.py",
                "🦀_entity",
                "café_function",
                "naïve_algorithm",
                "résumé_parser",
                "señor_módulo",
            ];

            for (i, unicode_id) in unicode_ids.iter().cycle().take(100).enumerate() {
                let mut vector = FeatureVector::new(format!("{}_{}", unicode_id, i));
                vector.add_feature("complexity", (i % 10) as f64);
                vectors.push(vector);
            }

            // Access all vectors
            for vector in &vectors {
                let _count = vector.feature_count();
            }
        });

        // Unicode handling should not significantly impact performance
        assert!(
            duration < Duration::from_millis(100),
            "Unicode handling performance degraded: {:?}",
            duration
        );

        println!("📊 Unicode handling test completed in {:?}", duration);
    }
}
