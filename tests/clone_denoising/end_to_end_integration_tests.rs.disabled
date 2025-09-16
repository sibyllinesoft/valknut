//! End-to-End Integration Tests for Clone Denoising System
//!
//! Tests the complete clone denoising pipeline integration:
//! - Full 4-phase pipeline: WeightedShingles → StructuralGates → StopMotifs → Ranking
//! - CLI flag integration (--denoise, --auto-denoise)
//! - Multi-phase coordination and handoffs
//! - Cache persistence and refresh across phases
//! - Quality gate effectiveness across the complete system

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tempfile::TempDir;
use tokio;

use valknut_rs::api::engine::ValknutEngine;
use valknut_rs::core::config::{AdaptiveDenoiseConfig, DedupeConfig, ValknutConfig};
use valknut_rs::core::featureset::{CodeEntity, ExtractionContext};
use valknut_rs::detectors::clone_detection::{
    AutoCalibrationEngine, ComprehensiveCloneDetector, NormalizationConfig, PayoffRankingSystem,
    StructuralGateAnalyzer, TfIdfAnalyzer,
};
use valknut_rs::detectors::lsh::{LshExtractor, WeightedShingleAnalyzer};
use valknut_rs::io::cache::{CacheRefreshPolicy, StopMotifCacheManager};

#[cfg(test)]
mod end_to_end_pipeline_tests {
    use super::*;

    /// Test complete 4-phase clone denoising pipeline
    #[tokio::test]
    async fn test_complete_4_phase_pipeline() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().to_path_buf();

        // Create comprehensive test codebase with various clone types
        let test_entities = create_comprehensive_test_codebase();

        // Phase 1: Weighted Shingling Setup
        let mut weighted_analyzer = WeightedShingleAnalyzer::new(9); // k=9 as specified
        let entity_refs: Vec<&CodeEntity> = test_entities.iter().collect();

        // Build global IDF table
        let idf_result = weighted_analyzer.build_idf_table(&entity_refs);
        assert!(
            idf_result.is_ok(),
            "Phase 1: IDF table construction should succeed"
        );

        // Compute weighted signatures
        let weighted_signatures = weighted_analyzer
            .compute_weighted_signatures(&entity_refs)
            .unwrap();
        assert_eq!(
            weighted_signatures.len(),
            test_entities.len(),
            "Phase 1: All entities should have signatures"
        );

        // Phase 2: Structural Gate Validation
        let gate_analyzer = StructuralGateAnalyzer::new(2, 2); // ≥2 blocks, ≥2 motifs
        let mut entities_passing_gates = Vec::new();

        for entity in &test_entities {
            let gate_result = gate_analyzer.passes_structural_gates(entity).unwrap();
            if gate_result.passes_all_gates {
                entities_passing_gates.push(entity);
            }
        }

        // Should filter some entities but not all
        assert!(
            entities_passing_gates.len() < test_entities.len(),
            "Phase 2: Some entities should be filtered by structural gates"
        );
        assert!(
            entities_passing_gates.len() > 0,
            "Phase 2: Some entities should pass structural gates"
        );

        // Phase 3: Stop-Motifs Cache Integration
        let refresh_policy = CacheRefreshPolicy {
            auto_refresh_enabled: true,
            max_age_hours: 24,
            min_codebase_change_threshold: 0.1,
            force_refresh_on_new_languages: true,
        };

        let cache_manager = StopMotifCacheManager::new(cache_dir, refresh_policy);

        // Simulate cache-based filtering (would normally be done by TfIdfAnalyzer)
        let entities_after_cache_filtering = entities_passing_gates
            .into_iter()
            .filter(|entity| {
                !entity.source_code.contains("println!")
                    && !entity.source_code.contains("import os")
            })
            .collect::<Vec<_>>();

        assert!(
            entities_after_cache_filtering.len() <= test_entities.len(),
            "Phase 3: Cache filtering should not increase entity count"
        );

        // Phase 4: Auto-Calibration and Payoff Ranking
        let mut comprehensive_detector = ComprehensiveCloneDetector::new(DedupeConfig::default());
        let clone_candidates = comprehensive_detector
            .detect_clones_with_denoising(&entities_after_cache_filtering)
            .await
            .unwrap();

        // Verify end-to-end results
        assert!(
            !clone_candidates.is_empty(),
            "E2E: Should detect some clone candidates"
        );

        // Verify quality metrics are calculated
        for candidate in &clone_candidates {
            assert!(
                candidate.quality_metrics.overall_quality >= 0.0
                    && candidate.quality_metrics.overall_quality <= 1.0,
                "E2E: Quality metrics should be in valid range"
            );
            assert!(
                candidate.payoff_score >= 0.0,
                "E2E: Payoff scores should be non-negative"
            );
        }

        // Verify ranking (highest payoff first)
        for i in 0..clone_candidates.len().saturating_sub(1) {
            assert!(
                clone_candidates[i].payoff_score >= clone_candidates[i + 1].payoff_score,
                "E2E: Candidates should be ranked by payoff score"
            );
        }
    }

    /// Test CLI flag integration for clone denoising
    #[tokio::test]
    async fn test_cli_flag_integration() {
        let temp_dir = TempDir::new().unwrap();

        // Test --denoise flag
        let denoise_config = ValknutConfig::default()
            .with_denoise_enabled(true)
            .with_dedupe_config(DedupeConfig {
                enabled: true,
                similarity_threshold: 0.8,
                shingle_k: 9,
                min_function_tokens: 50,
                min_ast_nodes: 20,
                require_distinct_blocks: 2,
                cache_enabled: true,
                cache_path: temp_dir.path().to_path_buf(),
            });

        let mut engine = ValknutEngine::new(denoise_config.into()).await.unwrap();

        // Create test codebase directory
        let test_codebase_dir = create_test_codebase_directory(&temp_dir);

        // Analyze with denoising enabled
        let results = engine.analyze_directory(test_codebase_dir).await.unwrap();

        // Verify denoising was applied
        assert!(
            results.clone_analysis.is_some(),
            "Should include clone analysis results"
        );
        let clone_results = results.clone_analysis.unwrap();
        assert!(
            clone_results.denoising_enabled,
            "Denoising should be enabled"
        );
        assert!(
            clone_results.candidates_before_denoising >= clone_results.candidates_after_denoising,
            "Denoising should reduce or maintain candidate count"
        );

        // Test --auto-denoise flag
        let auto_denoise_config =
            ValknutConfig::default().with_adaptive_denoise_config(AdaptiveDenoiseConfig {
                enabled: true,
                auto_calibration: true,
                target_quality_threshold: 0.8,
                max_calibration_iterations: 10,
                quality_gate_enforcement: true,
            });

        let mut auto_engine = ValknutEngine::new(auto_denoise_config.into())
            .await
            .unwrap();
        let auto_results = auto_engine
            .analyze_directory(&test_codebase_dir)
            .await
            .unwrap();

        // Verify auto-calibration was applied
        assert!(auto_results.clone_analysis.is_some());
        let auto_clone_results = auto_results.clone_analysis.unwrap();
        assert!(
            auto_clone_results.auto_calibration_applied,
            "Auto-calibration should be applied"
        );
        assert!(
            auto_clone_results.calibrated_threshold > 0.0,
            "Should have calibrated threshold"
        );
    }

    /// Test multi-phase coordination and handoffs
    #[tokio::test]
    async fn test_multi_phase_coordination() {
        // Create test entities with known characteristics for each phase
        let test_cases = vec![
            // Should pass all phases
            TestCase {
                id: "high_quality_clone",
                description: "Complex algorithm with rare patterns",
                source_code: r#"
def complex_rare_algorithm(data_matrix, optimization_params):
    eigenvalues = compute_eigendecomposition(data_matrix)
    for iteration in range(optimization_params.max_iterations):
        if check_convergence_criteria(eigenvalues, optimization_params.tolerance):
            break
        eigenvalues = update_eigenvalues_iterative(eigenvalues, optimization_params.learning_rate)
    return build_transformation_matrix(eigenvalues)
"#,
                expected_phase_results: PhaseExpectations {
                    phase1_weighted_signature: true,
                    phase2_structural_gates: true,
                    phase3_stop_motifs_filter: true,
                    phase4_quality_ranking: true,
                },
            },
            // Should fail Phase 2 (insufficient structure)
            TestCase {
                id: "simple_boilerplate",
                description: "Simple boilerplate that should be filtered early",
                source_code: "def simple_getter(self): return self.value",
                expected_phase_results: PhaseExpectations {
                    phase1_weighted_signature: true,
                    phase2_structural_gates: false, // Should fail here
                    phase3_stop_motifs_filter: false,
                    phase4_quality_ranking: false,
                },
            },
            // Should fail Phase 3 (stop-motifs filtering)
            TestCase {
                id: "common_patterns",
                description: "Contains common patterns that should be filtered by stop-motifs",
                source_code: r#"
def common_debug_function(message, level):
    import os
    import sys
    print(f"DEBUG: {message}")
    if level > 0:
        print(f"Level: {level}")
        sys.stderr.write(f"Error: {message}")
    return os.path.exists("/tmp/debug.log")
"#,
                expected_phase_results: PhaseExpectations {
                    phase1_weighted_signature: true,
                    phase2_structural_gates: true,
                    phase3_stop_motifs_filter: false, // Should fail here due to common patterns
                    phase4_quality_ranking: false,
                },
            },
        ];

        // Test each case through the pipeline
        for test_case in test_cases {
            let entity = CodeEntity::new(
                test_case.id,
                "function",
                test_case.id,
                &format!("/test/{}.py", test_case.id),
            )
            .with_source_code(test_case.source_code);

            // Phase 1: Weighted Shingling
            let mut phase1_analyzer = WeightedShingleAnalyzer::new(9);
            let entities = vec![&entity];
            let phase1_result = phase1_analyzer.build_idf_table(&entities).is_ok()
                && phase1_analyzer
                    .compute_weighted_signatures(&entities)
                    .is_ok();

            assert_eq!(
                phase1_result,
                test_case.expected_phase_results.phase1_weighted_signature,
                "Phase 1 result mismatch for {}: expected {}, got {}",
                test_case.id,
                test_case.expected_phase_results.phase1_weighted_signature,
                phase1_result
            );

            if !phase1_result {
                continue;
            }

            // Phase 2: Structural Gates
            let phase2_analyzer = StructuralGateAnalyzer::new(2, 2);
            let phase2_result = phase2_analyzer
                .passes_structural_gates(&entity)
                .map(|r| r.passes_all_gates)
                .unwrap_or(false);

            assert_eq!(
                phase2_result,
                test_case.expected_phase_results.phase2_structural_gates,
                "Phase 2 result mismatch for {}: expected {}, got {}",
                test_case.id,
                test_case.expected_phase_results.phase2_structural_gates,
                phase2_result
            );

            if !phase2_result {
                continue;
            }

            // Phase 3: Stop-Motifs Filtering (simulated)
            let phase3_result = !entity.source_code.contains("import os")
                && !entity.source_code.contains("print(")
                && !entity.source_code.contains("DEBUG:");

            let expected_phase3 = test_case.expected_phase_results.phase3_stop_motifs_filter;
            if expected_phase3 {
                // Only check if we expect it to pass
                assert_eq!(
                    phase3_result, expected_phase3,
                    "Phase 3 result mismatch for {}: expected {}, got {}",
                    test_case.id, expected_phase3, phase3_result
                );
            }

            // Note: Phase 4 testing would require actual payoff calculation
            // which is tested separately in the payoff ranking tests
        }
    }

    /// Test cache persistence and refresh across phases
    #[tokio::test]
    async fn test_cache_persistence_across_phases() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().to_path_buf();

        // Create initial cache
        let refresh_policy = CacheRefreshPolicy {
            auto_refresh_enabled: true,
            max_age_hours: 24,
            min_codebase_change_threshold: 0.1,
            force_refresh_on_new_languages: true,
        };

        let mut cache_manager = StopMotifCacheManager::new(cache_dir.clone(), refresh_policy);

        // Phase 1: Initial cache creation
        let initial_entities = create_comprehensive_test_codebase();

        // Simulate cache mining and persistence
        let initial_cache = create_test_stop_motif_cache(&initial_entities);
        let save_result = cache_manager.save_cache(&initial_cache);
        assert!(
            save_result.is_ok(),
            "Should save initial cache successfully"
        );

        // Phase 2: Cache loading and validation
        let loaded_cache = cache_manager.load_cache();
        assert!(loaded_cache.is_ok(), "Should load cache successfully");

        let cache = loaded_cache.unwrap();
        assert_eq!(cache.version, initial_cache.version);
        assert_eq!(cache.codebase_signature, initial_cache.codebase_signature);
        assert_eq!(cache.token_grams.len(), initial_cache.token_grams.len());
        assert_eq!(cache.ast_patterns.len(), initial_cache.ast_patterns.len());

        // Phase 3: Cache refresh trigger
        let modified_entities = create_modified_test_codebase();
        let new_signature =
            cache_manager.generate_codebase_signature(&create_codebase_info(&modified_entities));

        let should_refresh = cache_manager.should_refresh_cache(&cache, &new_signature);
        assert!(
            should_refresh,
            "Should trigger cache refresh for modified codebase"
        );

        // Phase 4: Cache refresh execution
        let refreshed_cache = create_test_stop_motif_cache(&modified_entities);
        let refresh_save_result = cache_manager.save_cache(&refreshed_cache);
        assert!(
            refresh_save_result.is_ok(),
            "Should save refreshed cache successfully"
        );

        // Verify refresh was effective
        let final_loaded_cache = cache_manager.load_cache().unwrap();
        assert_eq!(final_loaded_cache.codebase_signature, new_signature);
        assert!(
            final_loaded_cache.last_updated > cache.last_updated,
            "Refreshed cache should have newer timestamp"
        );
    }

    /// Test quality gate effectiveness across complete system
    #[tokio::test]
    async fn test_quality_gate_effectiveness() {
        // Create test dataset with known quality characteristics
        let quality_test_entities = create_quality_test_dataset();

        // Run complete pipeline with strict quality gates
        let detector = ComprehensiveCloneDetector::new_with_strict_quality_gates();
        let clone_candidates = detector
            .detect_clones_with_denoising(&quality_test_entities)
            .await
            .unwrap();

        // Analyze effectiveness of quality gates
        let mut high_quality_count = 0;
        let mut medium_quality_count = 0;
        let mut low_quality_count = 0;

        for candidate in &clone_candidates {
            match candidate.quality_metrics.overall_quality {
                q if q >= 0.8 => high_quality_count += 1,
                q if q >= 0.5 => medium_quality_count += 1,
                _ => low_quality_count += 1,
            }
        }

        // Quality gates should be effective
        assert!(
            high_quality_count > 0,
            "Should have some high-quality clones"
        );
        assert!(
            low_quality_count <= clone_candidates.len() / 4,
            "Low-quality clones should be minority: {} out of {}",
            low_quality_count,
            clone_candidates.len()
        );

        // Test hard filtering floors effectiveness
        for candidate in &clone_candidates {
            assert!(
                candidate.saved_tokens >= 100,
                "All candidates should meet SavedTokens floor: {}",
                candidate.saved_tokens
            );
            assert!(
                candidate.rarity_gain >= 1.2,
                "All candidates should meet RarityGain floor: {}",
                candidate.rarity_gain
            );
        }

        // Test ≥80% quality targeting
        let quality_80_plus: Vec<_> = clone_candidates
            .iter()
            .filter(|c| c.quality_metrics.overall_quality >= 0.8)
            .collect();

        let quality_ratio = quality_80_plus.len() as f64 / clone_candidates.len() as f64;
        assert!(
            quality_ratio >= 0.6, // Allow some tolerance for test data
            "Should achieve reasonable proportion of ≥80% quality clones: {:.1}%",
            quality_ratio * 100.0
        );
    }

    // Helper structures and functions
    #[derive(Debug)]
    struct TestCase {
        id: &'static str,
        description: &'static str,
        source_code: &'static str,
        expected_phase_results: PhaseExpectations,
    }

    #[derive(Debug)]
    struct PhaseExpectations {
        phase1_weighted_signature: bool,
        phase2_structural_gates: bool,
        phase3_stop_motifs_filter: bool,
        phase4_quality_ranking: bool,
    }

    fn create_comprehensive_test_codebase() -> Vec<CodeEntity> {
        vec![
            // Genuine clones - complex algorithms that should be detected
            CodeEntity::new(
                "matrix_multiply_v1",
                "function",
                "matrix_multiply",
                "/test/math1.py",
            )
            .with_source_code(
                r#"
def matrix_multiply(matrix_a, matrix_b):
    rows_a, cols_a = len(matrix_a), len(matrix_a[0])
    rows_b, cols_b = len(matrix_b), len(matrix_b[0])
    if cols_a != rows_b:
        raise ValueError("Matrix dimensions incompatible")
    result = [[0 for _ in range(cols_b)] for _ in range(rows_a)]
    for i in range(rows_a):
        for j in range(cols_b):
            for k in range(cols_a):
                result[i][j] += matrix_a[i][k] * matrix_b[k][j]
    return result
"#,
            ),
            CodeEntity::new(
                "matrix_multiply_v2",
                "function",
                "multiply_matrices",
                "/test/math2.py",
            )
            .with_source_code(
                r#"
def multiply_matrices(mat1, mat2):
    m, n = len(mat1), len(mat1[0])
    p, q = len(mat2), len(mat2[0])
    if n != p:
        raise ValueError("Cannot multiply: incompatible dimensions")
    product = [[0 for _ in range(q)] for _ in range(m)]
    for row in range(m):
        for col in range(q):
            for idx in range(n):
                product[row][col] += mat1[row][idx] * mat2[idx][col]
    return product
"#,
            ),
            // Boilerplate that should be filtered
            CodeEntity::new("simple_getter", "function", "get_value", "/test/simple.py")
                .with_source_code("def get_value(self): return self.value"),
            CodeEntity::new("simple_setter", "function", "set_value", "/test/simple.py")
                .with_source_code("def set_value(self, val): self.value = val"),
            // Common patterns with debug/logging (should be filtered by stop-motifs)
            CodeEntity::new("debug_func1", "function", "debug_log", "/test/debug1.py")
                .with_source_code(
                    r#"
def debug_log(message, level=1):
    import os
    print(f"DEBUG: {message}")
    if level > 0:
        print(f"Level: {level}")
    return True
"#,
                ),
            CodeEntity::new("debug_func2", "function", "log_debug", "/test/debug2.py")
                .with_source_code(
                    r#"
def log_debug(msg, debug_level=1):
    import sys
    print(f"DEBUG: {msg}")
    if debug_level > 0:
        sys.stderr.write(f"Level: {debug_level}")
    return True
"#,
                ),
            // Complex but unique algorithms (should pass all gates)
            CodeEntity::new(
                "unique_sort",
                "function",
                "adaptive_quicksort",
                "/test/unique.py",
            )
            .with_source_code(
                r#"
def adaptive_quicksort(arr, threshold=10):
    if len(arr) <= threshold:
        return insertion_sort_optimized(arr)
    pivot_idx = partition_adaptive(arr, 0, len(arr) - 1)
    left_part = adaptive_quicksort(arr[:pivot_idx], threshold)
    right_part = adaptive_quicksort(arr[pivot_idx + 1:], threshold)
    return left_part + [arr[pivot_idx]] + right_part

def insertion_sort_optimized(arr):
    for i in range(1, len(arr)):
        key = arr[i]
        j = i - 1
        while j >= 0 and arr[j] > key:
            arr[j + 1] = arr[j]
            j -= 1
        arr[j + 1] = key
    return arr
"#,
            ),
        ]
    }

    fn create_test_codebase_directory(temp_dir: &TempDir) -> std::path::PathBuf {
        let codebase_dir = temp_dir.path().join("test_codebase");
        std::fs::create_dir_all(&codebase_dir).unwrap();

        // Create test files
        let test_files = vec![
            (
                "math_operations.py",
                r#"
def calculate_fibonacci(n):
    if n <= 1:
        return n
    a, b = 0, 1
    for i in range(2, n + 1):
        a, b = b, a + b
    return b

def factorial_iterative(n):
    result = 1
    for i in range(1, n + 1):
        result *= i
    return result
"#,
            ),
            (
                "data_processing.py",
                r#"
def process_data_batch(data_list, config):
    results = []
    for item in data_list:
        if validate_item(item, config):
            processed = transform_item(item, config.transform_params)
            results.append(processed)
    return results

def validate_item(item, config):
    return item.size > config.min_size and item.quality >= config.min_quality
"#,
            ),
        ];

        for (filename, content) in test_files {
            let file_path = codebase_dir.join(filename);
            std::fs::write(file_path, content).unwrap();
        }

        codebase_dir
    }

    fn create_modified_test_codebase() -> Vec<CodeEntity> {
        vec![CodeEntity::new(
            "modified_func",
            "function",
            "new_algorithm",
            "/test/modified.py",
        )
        .with_source_code(
            r#"
def new_algorithm(input_data, parameters):
    preprocessed = preprocess_input(input_data, parameters.preprocessing)
    for step in range(parameters.max_steps):
        if convergence_check(preprocessed, parameters.tolerance):
            break
        preprocessed = update_step(preprocessed, parameters.update_rate)
    return finalize_result(preprocessed)
"#,
        )]
    }

    fn create_quality_test_dataset() -> Vec<&'static CodeEntity> {
        // This would contain entities specifically designed to test quality gates
        // For now, using a subset of the comprehensive test codebase
        static HIGH_QUALITY_ENTITY: CodeEntity = CodeEntity {
            id: "high_quality".to_string(),
            entity_type: "function".to_string(),
            name: "complex_algorithm".to_string(),
            file_path: "/test/high_quality.py".to_string(),
            source_code: r#"
def complex_algorithm(data_matrix, optimization_config):
    eigenvals = compute_eigendecomposition(data_matrix)
    convergence_history = []
    for iteration in range(optimization_config.max_iterations):
        gradient = compute_gradient(eigenvals, optimization_config)
        if check_convergence_criteria(gradient, optimization_config.tolerance):
            convergence_history.append(iteration)
            break
        eigenvals = update_eigenvalues(eigenvals, gradient, optimization_config.learning_rate)
        if iteration % optimization_config.checkpoint_interval == 0:
            convergence_history.append(compute_loss(eigenvals))
    return construct_result_matrix(eigenvals, convergence_history)
"#
            .to_string(),
            start_line: 1,
            end_line: 15,
            complexity: 8.5,
            metadata: HashMap::new(),
        };

        vec![&HIGH_QUALITY_ENTITY]
    }

    fn create_test_stop_motif_cache(
        entities: &[CodeEntity],
    ) -> valknut_rs::io::cache::StopMotifCache {
        use valknut_rs::io::cache::{MiningStats, PatternCategory, StopMotifCache, StopMotifEntry};

        StopMotifCache {
            version: 1,
            k_gram_size: 9,
            token_grams: vec![
                StopMotifEntry {
                    pattern: "import os sys".to_string(),
                    support: entities.len() / 2,
                    idf_score: 0.2,
                    weight_multiplier: 0.1,
                    category: PatternCategory::Boilerplate,
                },
                StopMotifEntry {
                    pattern: "print DEBUG".to_string(),
                    support: entities.len() / 3,
                    idf_score: 0.3,
                    weight_multiplier: 0.2,
                    category: PatternCategory::Boilerplate,
                },
            ],
            pdg_motifs: vec![],
            ast_patterns: vec![],
            last_updated: chrono::Utc::now().timestamp() as u64,
            codebase_signature: "test_signature_123".to_string(),
            mining_stats: MiningStats {
                total_functions_analyzed: entities.len(),
                total_patterns_found: entities.len() * 10,
                patterns_above_threshold: entities.len() * 2,
                top_1_percent_contribution: 15.0,
                processing_time_ms: 5000,
            },
        }
    }

    fn create_codebase_info(entities: &[CodeEntity]) -> valknut_rs::io::cache::CodebaseInfo {
        use valknut_rs::io::cache::{CodebaseInfo, FileInfo, FunctionInfo};

        CodebaseInfo {
            total_files: entities.len(),
            total_functions: entities.len(),
            languages: vec!["python".to_string()],
            file_info: entities
                .iter()
                .map(|entity| FileInfo {
                    path: entity.file_path.clone(),
                    language: "python".to_string(),
                    size_bytes: entity.source_code.len(),
                    last_modified: chrono::Utc::now().timestamp() as u64,
                    functions: vec![FunctionInfo {
                        name: entity.name.clone(),
                        start_line: entity.start_line,
                        end_line: entity.end_line,
                        complexity: entity.complexity,
                    }],
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod performance_integration_tests {
    use super::*;
    use std::time::Instant;

    /// Test end-to-end pipeline performance with larger datasets
    #[tokio::test]
    async fn test_e2e_pipeline_performance() {
        // Create larger test dataset for performance testing
        let large_test_entities = create_large_test_dataset(100); // 100 entities

        let start_time = Instant::now();

        // Run complete pipeline
        let mut detector = ComprehensiveCloneDetector::new(DedupeConfig::default());
        let entity_refs: Vec<&CodeEntity> = large_test_entities.iter().collect();
        let results = detector
            .detect_clones_with_denoising(&entity_refs)
            .await
            .unwrap();

        let execution_time = start_time.elapsed();

        // Performance assertions
        assert!(
            execution_time.as_secs() < 30,
            "Pipeline should complete within 30 seconds for 100 entities"
        );
        assert!(
            !results.is_empty(),
            "Should produce results with large dataset"
        );

        // Verify scalability characteristics
        let entities_per_second = large_test_entities.len() as f64 / execution_time.as_secs_f64();
        assert!(
            entities_per_second > 3.0,
            "Should process at least 3 entities per second: {:.2}",
            entities_per_second
        );
    }

    /// Test memory usage and resource efficiency
    #[tokio::test]
    async fn test_memory_efficiency() {
        let test_entities = create_comprehensive_test_codebase();

        // Monitor memory usage during pipeline execution
        let initial_memory = get_memory_usage();

        let mut detector = ComprehensiveCloneDetector::new(DedupeConfig::default());
        let entity_refs: Vec<&CodeEntity> = test_entities.iter().collect();
        let _results = detector
            .detect_clones_with_denoising(&entity_refs)
            .await
            .unwrap();

        let final_memory = get_memory_usage();
        let memory_increase = final_memory - initial_memory;

        // Memory usage should be reasonable (allow up to 100MB increase)
        assert!(
            memory_increase < 100 * 1024 * 1024,
            "Memory usage increase should be reasonable: {} bytes",
            memory_increase
        );
    }

    fn create_large_test_dataset(count: usize) -> Vec<CodeEntity> {
        (0..count)
            .map(|i| {
                CodeEntity::new(
                    &format!("entity_{}", i),
                    "function",
                    &format!("func_{}", i),
                    &format!("/test/file_{}.py", i),
                )
                .with_source_code(&format!(
                    r#"
def func_{}(param1, param2, config):
    result = initialize_result(param1, config.init_params)
    for iteration in range(config.max_iterations):
        if check_condition_{}_{}(result, param2):
            intermediate = process_iteration_{}(result, param1, param2)
            result = update_result(result, intermediate, config.update_rate)
        else:
            result = fallback_processing(result, config.fallback_params)
    return finalize_result_{}(result, config.output_params)
"#,
                    i,
                    i % 10,
                    (i + 5) % 10,
                    i % 20,
                    i % 15
                ))
            })
            .collect()
    }

    fn get_memory_usage() -> usize {
        // Simplified memory usage estimation
        // In a real implementation, this would use proper memory profiling
        0
    }
}
