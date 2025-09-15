//! Performance Benchmarks for Clone Denoising System
//! 
//! Benchmarks all four phases of the clone denoising system:
//! - Phase 1: Weighted Shingling (TF-IDF + MinHash)
//! - Phase 2: Structural Gate Validation  
//! - Phase 3: Stop-Motifs Cache Operations
//! - Phase 4: Auto-Calibration + Payoff Ranking
//! - End-to-End Pipeline Performance
//! - Memory Usage and Scalability Testing

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::time::Duration;
use tokio::runtime::Runtime;

use valknut_rs::core::featureset::CodeEntity;
use valknut_rs::detectors::lsh::WeightedShingleAnalyzer;
use valknut_rs::detectors::clone_detection::{
    StructuralGateAnalyzer, AutoCalibrationEngine, PayoffRankingSystem,
    CalibrationSettings, PayoffFormula, ComprehensiveCloneDetector
};

// Import our test fixtures
mod fixtures {
    pub use valknut_rs::tests::fixtures::clone_denoising_test_data::*;
}

/// Benchmark Phase 1: Weighted Shingling Performance
fn bench_phase1_weighted_shingling(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase1_weighted_shingling");
    
    // Test different dataset sizes
    let sizes = vec![10, 50, 100, 500];
    
    for size in sizes {
        let entities = fixtures::create_performance_test_dataset(size);
        let entity_refs: Vec<&CodeEntity> = entities.iter().collect();
        
        group.throughput(Throughput::Elements(size as u64));
        
        // Benchmark IDF table construction
        group.bench_with_input(
            BenchmarkId::new("idf_table_construction", size),
            &entity_refs,
            |b, entities| {
                b.iter(|| {
                    let mut analyzer = WeightedShingleAnalyzer::new(9);
                    black_box(analyzer.build_idf_table(entities).unwrap());
                });
            },
        );
        
        // Benchmark weighted signature computation
        group.bench_with_input(
            BenchmarkId::new("weighted_signature_computation", size),
            &entity_refs,
            |b, entities| {
                b.iter(|| {
                    let mut analyzer = WeightedShingleAnalyzer::new(9);
                    analyzer.build_idf_table(entities).unwrap();
                    black_box(analyzer.compute_weighted_signatures(entities).unwrap());
                });
            },
        );
        
        // Benchmark k-gram generation (k=9)
        group.bench_with_input(
            BenchmarkId::new("kgram_generation_k9", size),
            &entities[0..size.min(entities.len())],
            |b, entities| {
                b.iter(|| {
                    let analyzer = WeightedShingleAnalyzer::new(9);
                    for entity in entities {
                        black_box(analyzer.generate_kgrams(&entity.source_code));
                    }
                });
            },
        );
        
        // Benchmark weighted similarity calculation
        group.bench_with_input(
            BenchmarkId::new("weighted_similarity_calculation", size),
            &entity_refs,
            |b, entities| {
                b.iter(|| {
                    let mut analyzer = WeightedShingleAnalyzer::new(9);
                    analyzer.build_idf_table(entities).unwrap();
                    let signatures = analyzer.compute_weighted_signatures(entities).unwrap();
                    
                    // Calculate similarities between all pairs (limited to avoid O(n²) explosion)
                    let comparison_limit = 20.min(entities.len());
                    for i in 0..comparison_limit {
                        for j in (i+1)..comparison_limit {
                            if let (Some(sig1), Some(sig2)) = (
                                signatures.get(&entities[i].id),
                                signatures.get(&entities[j].id)
                            ) {
                                black_box(analyzer.weighted_jaccard_similarity(sig1, sig2));
                            }
                        }
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark Phase 2: Structural Gate Validation Performance
fn bench_phase2_structural_gates(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase2_structural_gates");
    
    let entities = fixtures::create_genuine_clones_dataset();
    let gate_analyzer = StructuralGateAnalyzer::new(2, 2);
    
    // Benchmark basic block counting
    group.bench_function("basic_block_counting", |b| {
        b.iter(|| {
            for entity in &entities {
                black_box(gate_analyzer.count_basic_blocks(entity).unwrap());
            }
        });
    });
    
    // Benchmark PDG motif extraction
    group.bench_function("pdg_motif_extraction", |b| {
        b.iter(|| {
            for entity in &entities {
                black_box(gate_analyzer.extract_pdg_motifs(entity).unwrap());
            }
        });
    });
    
    // Benchmark structural gate validation
    group.bench_function("structural_gate_validation", |b| {
        b.iter(|| {
            for entity in &entities {
                black_box(gate_analyzer.passes_structural_gates(entity).unwrap());
            }
        });
    });
    
    // Benchmark block overlap calculation
    group.bench_function("block_overlap_calculation", |b| {
        b.iter(|| {
            for i in 0..entities.len().min(10) {
                for j in (i+1)..entities.len().min(10) {
                    black_box(gate_analyzer.calculate_block_overlap(&entities[i], &entities[j]).unwrap());
                }
            }
        });
    });
    
    // Benchmark Weisfeiler-Lehman hashing
    group.bench_function("weisfeiler_lehman_hashing", |b| {
        b.iter(|| {
            for entity in &entities {
                let motifs = gate_analyzer.extract_pdg_motifs(entity).unwrap();
                black_box(gate_analyzer.compute_wl_hash(&motifs));
            }
        });
    });
    
    group.finish();
}

/// Benchmark Phase 3: Stop-Motifs Cache Performance
fn bench_phase3_stop_motifs_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase3_stop_motifs_cache");
    
    use valknut_rs::io::cache::{StopMotifCacheManager, CacheRefreshPolicy, StopMotifCache, MiningStats};
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();
    
    let refresh_policy = CacheRefreshPolicy {
        auto_refresh_enabled: true,
        max_age_hours: 24,
        min_codebase_change_threshold: 0.1,
        force_refresh_on_new_languages: true,
    };
    
    let mut cache_manager = StopMotifCacheManager::new(cache_dir, refresh_policy);
    
    // Create test cache for benchmarking
    let test_cache = StopMotifCache {
        version: 1,
        k_gram_size: 9,
        token_grams: vec![], // Would be populated in real usage
        pdg_motifs: vec![],
        ast_patterns: vec![],
        last_updated: chrono::Utc::now().timestamp() as u64,
        codebase_signature: "benchmark_test".to_string(),
        mining_stats: MiningStats {
            total_functions_analyzed: 1000,
            total_patterns_found: 5000,
            patterns_above_threshold: 500,
            top_1_percent_contribution: 20.0,
            processing_time_ms: 10000,
        },
    };
    
    // Benchmark cache serialization/deserialization
    group.bench_function("cache_save", |b| {
        b.iter(|| {
            black_box(cache_manager.save_cache(&test_cache).unwrap());
        });
    });
    
    // Save cache first for load benchmark
    cache_manager.save_cache(&test_cache).unwrap();
    
    group.bench_function("cache_load", |b| {
        b.iter(|| {
            black_box(cache_manager.load_cache().unwrap());
        });
    });
    
    // Benchmark cache invalidation logic
    group.bench_function("cache_invalidation_check", |b| {
        b.iter(|| {
            let should_refresh = cache_manager.should_refresh_cache(&test_cache, "new_signature");
            black_box(should_refresh);
        });
    });
    
    // Benchmark codebase signature generation
    let entities = fixtures::create_multi_language_ast_examples();
    let codebase_info = create_mock_codebase_info(&entities);
    
    group.bench_function("codebase_signature_generation", |b| {
        b.iter(|| {
            black_box(cache_manager.generate_codebase_signature(&codebase_info));
        });
    });
    
    group.finish();
}

/// Benchmark Phase 4: Auto-Calibration and Payoff Ranking Performance
fn bench_phase4_auto_calibration_payoff(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase4_auto_calibration_payoff");
    
    // Create test candidates for calibration
    let test_candidates = create_benchmark_clone_candidates(100);
    
    // Benchmark auto-calibration
    let calibration_settings = CalibrationSettings {
        target_quality_threshold: 0.8,
        binary_search_iterations: 10,
        initial_threshold_range: (0.1, 0.9),
        convergence_tolerance: 0.05,
        min_sample_size: 20,
    };
    
    group.bench_function("auto_calibration", |b| {
        b.iter(|| {
            let mut engine = AutoCalibrationEngine::new(calibration_settings.clone());
            black_box(engine.calibrate_thresholds(&test_candidates).unwrap());
        });
    });
    
    // Benchmark payoff formula calculation
    let ranking_system = PayoffRankingSystem::new(PayoffFormula::Standard);
    
    group.bench_function("payoff_formula_standard", |b| {
        b.iter(|| {
            for candidate in &test_candidates {
                black_box(ranking_system.calculate_payoff(candidate));
            }
        });
    });
    
    // Benchmark different payoff formulas
    let weighted_system = PayoffRankingSystem::new(PayoffFormula::QualityWeighted);
    let conservative_system = PayoffRankingSystem::new(PayoffFormula::Conservative);
    
    group.bench_function("payoff_formula_weighted", |b| {
        b.iter(|| {
            for candidate in &test_candidates {
                black_box(weighted_system.calculate_payoff(candidate));
            }
        });
    });
    
    group.bench_function("payoff_formula_conservative", |b| {
        b.iter(|| {
            for candidate in &test_candidates {
                black_box(conservative_system.calculate_payoff(candidate));
            }
        });
    });
    
    // Benchmark candidate ranking
    group.bench_function("candidate_ranking", |b| {
        b.iter(|| {
            black_box(ranking_system.rank_candidates(&test_candidates));
        });
    });
    
    // Benchmark quality metrics calculation
    group.bench_function("quality_metrics_calculation", |b| {
        b.iter(|| {
            for candidate in &test_candidates {
                black_box(ranking_system.calculate_quality_metrics(candidate.clone()));
            }
        });
    });
    
    group.finish();
}

/// Benchmark End-to-End Pipeline Performance
fn bench_end_to_end_pipeline(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("end_to_end_pipeline");
    group.measurement_time(Duration::from_secs(30)); // Longer measurement time for E2E
    
    let sizes = vec![25, 50, 100];
    
    for size in sizes {
        let entities = fixtures::create_realistic_codebase_sample();
        let test_entities = &entities[0..size.min(entities.len())];
        let entity_refs: Vec<&CodeEntity> = test_entities.iter().collect();
        
        group.throughput(Throughput::Elements(size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("complete_pipeline", size),
            &entity_refs,
            |b, entities| {
                b.to_async(&rt).iter(|| async {
                    let detector = ComprehensiveCloneDetector::new();
                    let results = detector.detect_clones_with_denoising(entities).await.unwrap();
                    black_box(results);
                });
            },
        );
        
        // Benchmark pipeline phases separately
        group.bench_with_input(
            BenchmarkId::new("phase1_only", size),
            &entity_refs,
            |b, entities| {
                b.iter(|| {
                    let mut analyzer = WeightedShingleAnalyzer::new(9);
                    analyzer.build_idf_table(entities).unwrap();
                    let signatures = analyzer.compute_weighted_signatures(entities).unwrap();
                    black_box(signatures);
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("phase2_only", size),
            &entity_refs,
            |b, entities| {
                b.iter(|| {
                    let gate_analyzer = StructuralGateAnalyzer::new(2, 2);
                    let mut passed_entities = Vec::new();
                    for entity in entities {
                        let result = gate_analyzer.passes_structural_gates(entity).unwrap();
                        if result.passes_all_gates {
                            passed_entities.push(entity);
                        }
                    }
                    black_box(passed_entities);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark Memory Usage and Scalability
fn bench_memory_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_scalability");
    
    // Test scaling behavior with different entity counts
    let sizes = vec![100, 500, 1000, 2000];
    
    for size in sizes {
        let entities = fixtures::create_performance_test_dataset(size);
        
        group.throughput(Throughput::Elements(size as u64));
        
        // Benchmark memory usage for signature storage
        group.bench_with_input(
            BenchmarkId::new("signature_memory_usage", size),
            &entities,
            |b, entities| {
                b.iter(|| {
                    let entity_refs: Vec<&CodeEntity> = entities.iter().collect();
                    let mut analyzer = WeightedShingleAnalyzer::new(9);
                    analyzer.build_idf_table(&entity_refs).unwrap();
                    let signatures = analyzer.compute_weighted_signatures(&entity_refs).unwrap();
                    
                    // Force memory allocation and prevent optimization
                    let signature_count = signatures.len();
                    black_box(signature_count);
                    black_box(signatures);
                });
            },
        );
        
        // Benchmark scalability of similarity comparisons
        group.bench_with_input(
            BenchmarkId::new("similarity_scaling", size),
            &entities,
            |b, entities| {
                b.iter(|| {
                    let entity_refs: Vec<&CodeEntity> = entities.iter().collect();
                    let mut analyzer = WeightedShingleAnalyzer::new(9);
                    analyzer.build_idf_table(&entity_refs).unwrap();
                    let signatures = analyzer.compute_weighted_signatures(&entity_refs).unwrap();
                    
                    // Compare first 20 entities with each other to avoid O(n²) explosion
                    let comparison_limit = 20.min(entities.len());
                    let mut similarity_sum = 0.0;
                    
                    for i in 0..comparison_limit {
                        for j in (i+1)..comparison_limit {
                            if let (Some(sig1), Some(sig2)) = (
                                signatures.get(&entities[i].id),
                                signatures.get(&entities[j].id)
                            ) {
                                similarity_sum += analyzer.weighted_jaccard_similarity(sig1, sig2);
                            }
                        }
                    }
                    
                    black_box(similarity_sum);
                });
            },
        );
    }
    
    group.finish();
}

// Helper functions for benchmarks

fn create_benchmark_clone_candidates(count: usize) -> Vec<valknut_rs::detectors::clone_detection::CloneCandidate> {
    use valknut_rs::detectors::clone_detection::{CloneCandidate, QualityMetrics};
    
    (0..count).map(|i| {
        let base_quality = 0.3 + (i as f64 / count as f64) * 0.6; // Range from 0.3 to 0.9
        
        CloneCandidate {
            id: format!("candidate_{}", i),
            entity1_id: format!("entity_{}a", i),
            entity2_id: format!("entity_{}b", i),
            similarity_score: base_quality + 0.05,
            saved_tokens: 100 + (i % 500),
            rarity_gain: 1.2 + (i as f64 / count as f64) * 2.0,
            live_reach_boost: 1.0 + (i as f64 / count as f64) * 1.5,
            quality_metrics: QualityMetrics {
                fragmentarity: 1.0 - base_quality,
                structure_ratio: base_quality * 0.9,
                uniqueness: base_quality,
                overall_quality: base_quality,
            },
            payoff_score: 0.0,
        }
    }).collect()
}

fn create_mock_codebase_info(entities: &[CodeEntity]) -> valknut_rs::io::cache::CodebaseInfo {
    use valknut_rs::io::cache::{CodebaseInfo, FileInfo, FunctionInfo};
    
    CodebaseInfo {
        total_files: entities.len(),
        total_functions: entities.len(),
        languages: vec!["python".to_string(), "javascript".to_string(), "rust".to_string()],
        file_info: entities.iter().enumerate().map(|(i, entity)| {
            FileInfo {
                path: entity.file_path.clone(),
                language: match i % 3 {
                    0 => "python".to_string(),
                    1 => "javascript".to_string(),
                    _ => "rust".to_string(),
                },
                size_bytes: entity.source_code.len(),
                last_modified: chrono::Utc::now().timestamp() as u64,
                functions: vec![
                    FunctionInfo {
                        name: entity.name.clone(),
                        start_line: entity.start_line,
                        end_line: entity.end_line,
                        complexity: entity.complexity,
                    }
                ],
            }
        }).collect(),
    }
}

// Criterion benchmark groups
criterion_group!(
    benches,
    bench_phase1_weighted_shingling,
    bench_phase2_structural_gates,
    bench_phase3_stop_motifs_cache,
    bench_phase4_auto_calibration_payoff,
    bench_end_to_end_pipeline,
    bench_memory_scalability
);

criterion_main!(benches);