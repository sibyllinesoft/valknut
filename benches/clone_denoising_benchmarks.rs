//! Performance Benchmarks for Clone Denoising System
//!
//! Benchmarks the available clone denoising functionality:
//! - Phase 1: Weighted Shingling (TF-IDF + MinHash)
//! - LSH-based similarity detection
//! - Memory usage and scalability testing

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

use valknut_rs::core::config::LshConfig;
use valknut_rs::core::featureset::CodeEntity;
use valknut_rs::detectors::lsh::{LshExtractor, WeightedShingleAnalyzer};

/// Generate test entities for performance testing
fn generate_test_entities(count: usize) -> Vec<CodeEntity> {
    let mut entities = Vec::new();

    for i in 0..count {
        let source_code = format!(
            r#"
            def function_{}():
                # This is function {}
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
                    if j % 2 == 0:
                        result = process_even(j)
                    else:
                        result = process_odd(j)
                return z * {}
            "#,
            i,
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

/// Generate varied entities with different patterns
fn generate_varied_entities(count: usize) -> Vec<CodeEntity> {
    let mut entities = Vec::new();

    let patterns = vec![
        // Python decorator pattern
        r#"
@app.route('/api/users/<int:user_id>', methods=['GET'])
@login_required
@permission_required('user.read')
def get_user_{id}(user_id):
    user = user_service.get_user(user_id)
    if not user:
        return jsonify({{"error": "User not found"}}), 404
    return jsonify(user.to_dict())
"#,
        // JavaScript class pattern
        r#"
class DataProcessor_{id} {{
    constructor(config) {{
        this.config = config;
        this.cache = new Map();
    }}
    
    async processData(data) {{
        const key = this.generateKey(data);
        if (this.cache.has(key)) {{
            return this.cache.get(key);
        }}
        
        const result = await this.transform(data);
        this.cache.set(key, result);
        return result;
    }}
}}
"#,
        // Rust pattern
        r#"
impl DataProcessor_{id} {{
    pub fn new(config: Config) -> Self {{
        Self {{
            config,
            cache: HashMap::new(),
        }}
    }}
    
    pub fn process(&mut self, input: &str) -> Result<String, ProcessError> {{
        if let Some(cached) = self.cache.get(input) {{
            return Ok(cached.clone());
        }}
        
        let result = self.transform(input)?;
        self.cache.insert(input.to_string(), result.clone());
        Ok(result)
    }}
}}
"#,
    ];

    for i in 0..count {
        let pattern_idx = i % patterns.len();
        let source_code = patterns[pattern_idx].replace("{id}", &i.to_string());

        let file_ext = match pattern_idx {
            0 => "py",
            1 => "js",
            _ => "rs",
        };

        let entity = CodeEntity::new(
            format!("entity_{}", i),
            "function",
            format!("entity_{}", i),
            format!("/test/file_{}.{}", i, file_ext),
        )
        .with_source_code(&source_code);

        entities.push(entity);
    }

    entities
}

/// Benchmark Phase 1: Weighted Shingling Performance
fn bench_phase1_weighted_shingling(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase1_weighted_shingling");

    // Test different dataset sizes
    let sizes = vec![10, 25, 50, 100];

    for size in sizes {
        let entities = generate_test_entities(size);
        let entity_refs: Vec<&CodeEntity> = entities.iter().collect();

        group.throughput(Throughput::Elements(size as u64));

        // Benchmark IDF table construction
        group.bench_with_input(
            BenchmarkId::new("idf_table_construction", size),
            &entity_refs,
            |b, entities| {
                b.iter(|| {
                    let mut analyzer = WeightedShingleAnalyzer::new(9);
                    analyzer.build_idf_table(entities).unwrap();
                    black_box(&analyzer);
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
                    let comparison_limit = 10.min(entities.len());
                    for i in 0..comparison_limit {
                        for j in (i + 1)..comparison_limit {
                            if let (Some(sig1), Some(sig2)) = (
                                signatures.get(&entities[i].id),
                                signatures.get(&entities[j].id),
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

/// Benchmark LSH Operations
fn bench_lsh_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsh_operations");

    let entities = generate_varied_entities(50);
    let entity_refs: Vec<&CodeEntity> = entities.iter().collect();

    let lsh_extractor = LshExtractor::new().with_lsh_config(LshConfig {
        num_hashes: 128,
        num_bands: 16,
        shingle_size: 3,
        similarity_threshold: 0.7,
        max_candidates: 50,
        use_semantic_similarity: false,
    });

    // Benchmark LSH similarity context creation
    group.bench_function("lsh_context_creation", |b| {
        b.iter(|| {
            let context = lsh_extractor.create_similarity_search_context(&entity_refs);
            black_box(context);
        });
    });

    // Benchmark similarity searches
    group.bench_function("lsh_similarity_searches", |b| {
        b.iter(|| {
            let context = lsh_extractor.create_similarity_search_context(&entity_refs);

            // Perform multiple similarity searches
            for i in 0..10.min(entities.len()) {
                let entity_id = &entities[i].id;
                let candidates = context.find_similar_entities(entity_id, Some(5));
                black_box(candidates);
            }
        });
    });

    // Benchmark signature generation
    group.bench_function("signature_generation", |b| {
        b.iter(|| {
            for entity in &entities {
                let signature = lsh_extractor.generate_minhash_signature(&entity.source_code);
                black_box(signature);
            }
        });
    });

    // Benchmark shingle creation
    group.bench_function("shingle_creation", |b| {
        b.iter(|| {
            for entity in &entities {
                let shingles = lsh_extractor.create_shingles(&entity.source_code);
                black_box(shingles);
            }
        });
    });

    group.finish();
}

/// Benchmark Memory Usage and Scalability
fn bench_memory_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_scalability");

    // Test scaling behavior with different entity counts
    let sizes = vec![50, 100, 200, 500];

    for size in sizes {
        let entities = generate_varied_entities(size);

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

        // Benchmark LSH index scaling
        group.bench_with_input(
            BenchmarkId::new("lsh_index_scaling", size),
            &entities,
            |b, entities| {
                b.iter(|| {
                    let entity_refs: Vec<&CodeEntity> = entities.iter().collect();
                    let lsh_extractor = LshExtractor::new().with_lsh_config(LshConfig {
                        num_hashes: 64,
                        num_bands: 8,
                        shingle_size: 3,
                        similarity_threshold: 0.7,
                        max_candidates: 25,
                        use_semantic_similarity: false,
                    });

                    let context = lsh_extractor.create_similarity_search_context(&entity_refs);

                    // Perform searches to stress test the index
                    let search_count = 5.min(entities.len());
                    for i in 0..search_count {
                        let candidates = context.find_similar_entities(&entities[i].id, Some(3));
                        black_box(candidates);
                    }

                    black_box(context.get_statistics());
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

                    // Compare first 15 entities with each other to avoid O(n²) explosion
                    let comparison_limit = 15.min(entities.len());
                    let mut similarity_sum = 0.0;

                    for i in 0..comparison_limit {
                        for j in (i + 1)..comparison_limit {
                            if let (Some(sig1), Some(sig2)) = (
                                signatures.get(&entities[i].id),
                                signatures.get(&entities[j].id),
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

/// Benchmark Different K-gram Sizes
fn bench_kgram_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("kgram_sizes");

    let entities = generate_varied_entities(25);
    let entity_refs: Vec<&CodeEntity> = entities.iter().collect();

    // Test different k-gram sizes
    let k_sizes = vec![3, 5, 7, 9, 11];

    for k in k_sizes {
        group.bench_with_input(
            BenchmarkId::new("weighted_shingling", k),
            &entity_refs,
            |b, entities| {
                b.iter(|| {
                    let mut analyzer = WeightedShingleAnalyzer::new(k);
                    analyzer.build_idf_table(entities).unwrap();
                    let signatures = analyzer.compute_weighted_signatures(entities).unwrap();
                    black_box(signatures);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Different LSH Configurations
fn bench_lsh_configurations(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsh_configurations");

    let entities = generate_varied_entities(50);
    let entity_refs: Vec<&CodeEntity> = entities.iter().collect();

    // Test different LSH configurations
    let configs = vec![
        ("small", 32, 4),
        ("medium", 64, 8),
        ("large", 128, 16),
        ("xlarge", 256, 32),
    ];

    for (name, num_hashes, num_bands) in configs {
        let lsh_config = LshConfig {
            num_hashes,
            num_bands,
            shingle_size: 3,
            similarity_threshold: 0.7,
            max_candidates: 25,
            use_semantic_similarity: false,
        };

        group.bench_with_input(
            BenchmarkId::new("lsh_context_creation", name),
            &entity_refs,
            |b, entities| {
                b.iter(|| {
                    let extractor = LshExtractor::new().with_lsh_config(lsh_config.clone());
                    let context = extractor.create_similarity_search_context(entities);
                    black_box(context.get_statistics());
                });
            },
        );
    }

    group.finish();
}

// Criterion benchmark groups
criterion_group!(
    benches,
    bench_phase1_weighted_shingling,
    bench_lsh_operations,
    bench_memory_scalability,
    bench_kgram_sizes,
    bench_lsh_configurations
);

criterion_main!(benches);
