    use super::*;
    use crate::core::config::ValknutConfig;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::tempdir;

    fn entity(id: &str, code: &str) -> CodeEntity {
        CodeEntity::new(id, "function", id, format!("{id}.rs")).with_source_code(code)
    }

    #[tokio::test]
    async fn test_lsh_extractor() {
        let extractor = LshExtractor::new();

        assert_eq!(extractor.name(), "lsh");
        assert!(!extractor.features().is_empty());

        let entity = CodeEntity::new("test_function", "function", "test_func", "/test/file.py")
            .with_source_code("def test_func():\n    x = 1\n    y = 2\n    return x + y");

        let config = Arc::new(ValknutConfig::default());
        let context = ExtractionContext::new(config, "python");

        let features = extractor.extract(&entity, &context).await.unwrap();

        assert!(features.contains_key("clone_mass"));
        assert!(features.contains_key("max_similarity"));
        assert!(features.contains_key("avg_similarity"));
        assert!(features.contains_key("duplicate_count"));
    }

    #[test]
    fn test_shingle_creation() {
        let extractor = LshExtractor::with_params(64, 2);
        let code = "def func():\n    return 1";
        let shingles = extractor.create_shingles(code);

        assert!(!shingles.is_empty());
    }

    #[test]
    fn test_interned_shingle_creation() {
        let extractor = LshExtractor::with_params(64, 2);
        let code = "def func():\n    return 1";

        // Test interned shingles
        let interned_shingles = extractor.create_shingles_interned(code);
        assert!(!interned_shingles.is_empty());

        // Test normal shingles for comparison
        let normal_shingles = extractor.create_shingles(code);
        assert_eq!(interned_shingles.len(), normal_shingles.len());

        // Verify content matches by resolving interned strings
        for (interned, normal) in interned_shingles.iter().zip(normal_shingles.iter()) {
            let resolved = resolve(*interned);
            assert_eq!(resolved, normal);
        }
    }

    #[test]
    fn test_interned_minhash_signature() {
        let extractor = LshExtractor::with_params(16, 2);
        let code = "def test(): return 1";

        // Test interned signature
        let interned_signature = extractor.generate_minhash_signature_interned(code);
        assert_eq!(interned_signature.len(), 16);
        assert!(interned_signature.iter().any(|&x| x != u64::MAX));

        // Test normal signature for comparison
        let normal_signature = extractor.generate_minhash_signature(code);
        assert_eq!(interned_signature.len(), normal_signature.len());

        // Both should produce identical results
        assert_eq!(interned_signature, normal_signature);
    }

    #[test]
    fn test_minhash_signature() {
        let extractor = LshExtractor::with_params(16, 2);
        let code = "def test(): return 1";
        let signature = extractor.generate_minhash_signature(code);

        assert_eq!(signature.len(), 16);
        assert!(signature.iter().any(|&x| x != u64::MAX));
    }

    #[test]
    fn test_jaccard_similarity() {
        let sig1 = vec![1, 2, 3, 4];
        let sig2 = vec![1, 2, 5, 6];
        let sig3 = vec![1, 2, 3, 4];

        let extractor = LshExtractor::new();

        let sim12 = extractor.jaccard_similarity(&sig1, &sig2);
        let sim13 = extractor.jaccard_similarity(&sig1, &sig3);

        assert_eq!(sim12, 0.5); // 2 out of 4 match
        assert_eq!(sim13, 1.0); // Perfect match
    }

    #[test]
    fn test_lsh_index() {
        let mut index = LshIndex::new(4);

        let sig1 = MinHashSignature::new(vec![1, 2, 3, 4, 5, 6, 7, 8], 8, 2);
        let sig2 = MinHashSignature::new(vec![1, 2, 3, 4, 9, 10, 11, 12], 8, 2);

        index.add_entity("entity1".to_string(), sig1);
        index.add_entity("entity2".to_string(), sig2);

        let candidates = index.find_candidates("entity1");
        assert!(!candidates.is_empty());
    }

    #[test]
    fn test_lsh_index_returns_empty_for_missing_entity() {
        let index = LshIndex::new(2);
        assert!(index.find_candidates("unknown").is_empty());
    }

    #[test]
    fn test_weighted_shingle_analyzer() {
        let mut analyzer = WeightedShingleAnalyzer::new(3);

        // Create test entities
        let entity1 = entity("test1", "def func1():\n    x = 1\n    return x\n");
        let entity2 = entity("test2", "def func2():\n    y = 2\n    return y\n");

        let entities = vec![&entity1, &entity2];

        // Test IDF table construction
        let result = analyzer.build_idf_table(&entities);
        assert!(result.is_ok());

        // Test signature computation
        let signatures_result = analyzer.compute_weighted_signatures(&entities);
        assert!(signatures_result.is_ok());

        let signatures = signatures_result.unwrap();
        assert_eq!(signatures.len(), 2);
        assert!(signatures.contains_key("test1"));
        assert!(signatures.contains_key("test2"));

        let stats = analyzer.statistics();
        assert_eq!(stats.total_documents, 2);
        assert!(stats.unique_grams > 0);
        assert!(stats.top1pct_contribution >= 0.0);
    }

    #[test]
    fn test_weighted_jaccard_similarity() {
        let analyzer = WeightedShingleAnalyzer::new(2);

        let sig1 = WeightedMinHashSignature::new(vec![1.0, 2.0, 3.0, 4.0]);
        let sig2 = WeightedMinHashSignature::new(vec![1.0, 2.0, 5.0, 6.0]);
        let sig3 = WeightedMinHashSignature::new(vec![1.0, 2.0, 3.0, 4.0]);

        let sim12 = analyzer.weighted_jaccard_similarity(&sig1, &sig2);
        let sim13 = analyzer.weighted_jaccard_similarity(&sig1, &sig3);
        let sim_mismatch = analyzer
            .weighted_jaccard_similarity(&WeightedMinHashSignature::new(vec![1.0, 2.0]), &sig3);

        assert_eq!(sim12, 0.5); // 2 out of 4 match
        assert_eq!(sim13, 1.0); // Perfect match
        assert_eq!(sim_mismatch, 0.0);
    }

    #[test]
    fn test_kgram_generation() {
        let analyzer = WeightedShingleAnalyzer::new(2);
        let code = "def func():\n    return 1";
        let kgrams = analyzer.generate_kgrams(code);

        assert!(!kgrams.is_empty());
        // Should contain k-grams like "def func", "func (", etc.
    }

    #[test]
    fn test_weighted_shingle_analyzer_handles_edge_cases() {
        let mut analyzer = WeightedShingleAnalyzer::new(4);
        assert!(analyzer.build_idf_table(&[]).is_err());

        let short_entity = entity("short", "fn a() {}");
        let signature = analyzer
            .compute_weighted_signature_for_entity(&short_entity)
            .expect("signature for short entity");
        assert!(signature.signature.is_empty());
    }

    #[test]
    fn test_lsh_extractor_with_denoise() {
        let extractor = LshExtractor::new().with_denoise_enabled(true);

        // Should have weighted analyzer enabled
        assert!(extractor.weighted_analyzer.is_some());

        let extractor_disabled = LshExtractor::new().with_denoise_enabled(false);
        assert!(extractor_disabled.weighted_analyzer.is_none());
    }

    #[test]
    fn test_lsh_performance_metrics_validation_paths() {
        let mut metrics = LshPerformanceMetrics::new();
        metrics.entities_processed = 1;
        metrics.signature_generation_time = Duration::from_millis(150);
        metrics.comparisons_performed = 1;
        metrics.comparison_time = Duration::from_millis(40);
        metrics.log_summary();
        assert!(metrics.validate_performance().is_err());

        metrics.signature_generation_time = Duration::from_millis(50);
        assert!(metrics.validate_performance().is_ok());

        metrics.comparison_time = Duration::from_millis(80);
        assert!(metrics.validate_performance().is_err());
    }

    #[test]
    fn test_lsh_extractor_configuration_helpers() {
        let mut custom_config = LshConfig::default();
        custom_config.num_hashes = 64;
        custom_config.num_bands = 8;
        custom_config.shingle_size = 4;
        custom_config.similarity_threshold = 0.85;
        custom_config.max_candidates = 0;

        let extractor = LshExtractor::new().with_lsh_config(custom_config.clone());
        assert_eq!(extractor.similarity_threshold(), 0.85);
        assert!(extractor.max_candidates().is_none());

        let mut metrics_clone = extractor.get_performance_metrics().clone();
        metrics_clone.entities_processed = 1;
        metrics_clone.signature_generation_time = Duration::from_millis(10);
        metrics_clone.comparisons_performed = 1;
        metrics_clone.comparison_time = Duration::from_millis(5);
        metrics_clone.log_summary();

        let mut other_config = custom_config.clone();
        other_config.max_candidates = 5;
        let mut second_extractor = LshExtractor::new().with_lsh_config(other_config);
        assert_eq!(second_extractor.max_candidates(), Some(5));

        second_extractor.reset_performance_metrics();
        second_extractor.log_performance_statistics();
    }

    #[test]
    fn test_weighted_signature_statistics_helpers() {
        let extractor = LshExtractor::new().with_denoise_enabled(true);
        let entity1 = CodeEntity::new("w1", "function", "alpha", "alpha.rs")
            .with_source_code("fn alpha() { let value = 1; value }");
        let entity2 = CodeEntity::new("w2", "function", "beta", "beta.rs")
            .with_source_code("fn beta() { let other = 2; other }");

        let entities = vec![&entity1, &entity2];
        let (signatures, stats) = extractor
            .weighted_signatures_with_stats(&entities)
            .expect("compute weighted signatures");
        assert_eq!(signatures.len(), 2);
        assert!(stats.total_documents >= 2);

        let stats_only = extractor
            .weighted_statistics(&entities)
            .expect("weighted stats");
        assert_eq!(stats_only.total_documents, stats.total_documents);
    }

    #[tokio::test]
    async fn test_entity_threshold_short_circuit_behavior() {
        let extractor = LshExtractor::new();
        let entity = CodeEntity::new("e1", "function", "gamma", "gamma.rs")
            .with_source_code("fn gamma() -> usize { 1 }");

        assert!(extractor
            .entity_passes_thresholds(&entity)
            .await
            .expect("no config should bypass thresholds"));

        let config = DedupeConfig::default();
        let extractor_with_config = LshExtractor::with_dedupe_config(config);
        assert!(!extractor_with_config
            .entity_passes_thresholds(&entity)
            .await
            .expect("default thresholds should reject short snippet"));
    }

    #[test]
    fn test_similarity_context_cache_is_invalidateable() {
        let extractor = LshExtractor::new();
        let config = Arc::new(ValknutConfig::default());
        let mut context = ExtractionContext::new(config, "rust");

        let entity_a = CodeEntity::new("entity_a", "function", "entity_a", "a.rs")
            .with_source_code("fn alpha() { 1 + 2 }");
        let entity_b = CodeEntity::new("entity_b", "function", "entity_b", "b.rs")
            .with_source_code("fn beta() { 1 + 2 }");

        context.add_entity(entity_a.clone());
        context.add_entity(entity_b.clone());

        let cached = extractor
            .similarity_context(&context)
            .expect("context should be built");
        let cached_again = extractor
            .similarity_context(&context)
            .expect("context should be cached");
        assert!(Arc::ptr_eq(&cached, &cached_again));

        extractor.clear_caches();

        let rebuilt = extractor
            .similarity_context(&context)
            .expect("context should rebuild after clearing caches");
        assert!(
            !Arc::ptr_eq(&cached, &rebuilt),
            "clearing caches should invalidate similarity context"
        );
    }

    #[tokio::test]
    async fn test_candidate_filter_bruteforce_uses_weighted_cache() {
        let extractor = LshExtractor::new().with_denoise_enabled(true);
        let config = Arc::new(ValknutConfig::default());

        let entity_a = CodeEntity::new("entity_a", "function", "entity_a", "a.rs")
            .with_source_code("fn duplicated() { let value = 42; value }");
        let entity_b = CodeEntity::new("entity_b", "function", "entity_b", "b.rs")
            .with_source_code("fn duplicated() { let value = 42; value }");

        let mut context = ExtractionContext::new(config.clone(), "rust");
        context.add_entity(entity_a.clone());
        context.add_entity(entity_b.clone());

        let partitions = Arc::new(HashMap::from([
            (entity_a.id.clone(), vec![entity_b.id.clone()]),
            (entity_b.id.clone(), vec![entity_a.id.clone()]),
        ]));
        let context = context.with_candidate_partitions(partitions);

        let first = extractor
            .extract(&entity_a, &context)
            .await
            .expect("first extraction succeeds");
        assert!(first.get("duplicate_count").copied().unwrap_or_default() >= 1.0);

        let second = extractor
            .extract(&entity_a, &context)
            .await
            .expect("second extraction succeeds");
        assert_eq!(
            first.get("duplicate_count"),
            second.get("duplicate_count"),
            "cached weighted signatures should produce stable results"
        );
    }

    #[tokio::test]
    async fn test_partitions_without_entry_skip_similarity_search() {
        let extractor = LshExtractor::new().with_denoise_enabled(true);
        let config = Arc::new(ValknutConfig::default());

        let entity_a = CodeEntity::new("entity_a", "function", "entity_a", "a.rs")
            .with_source_code("fn alpha() { 1 + 2 }");
        let entity_b = CodeEntity::new("entity_b", "function", "entity_b", "b.rs")
            .with_source_code("fn beta() { 2 + 3 }");

        let mut context = ExtractionContext::new(config.clone(), "rust");
        context.add_entity(entity_a.clone());
        context.add_entity(entity_b.clone());

        let partitions = Arc::new(HashMap::from([(
            entity_b.id.clone(),
            vec![entity_a.id.clone()],
        )]));
        let context = context.with_candidate_partitions(partitions);

        let scores = extractor
            .extract(&entity_a, &context)
            .await
            .expect("extraction succeeds");

        assert_eq!(scores.get("max_similarity"), Some(&0.0));
        assert_eq!(scores.get("duplicate_count"), Some(&0.0));
    }

    #[tokio::test]
    async fn test_similarity_context_path_produces_matches() {
        let extractor = LshExtractor::new();
        let config = Arc::new(ValknutConfig::default());

        let entity_a = CodeEntity::new("entity_a", "function", "entity_a", "a.rs")
            .with_source_code("fn mirrored() { let n = 5; n * 2 }");
        let entity_b = CodeEntity::new("entity_b", "function", "entity_b", "b.rs")
            .with_source_code("fn mirrored() { let n = 5; n * 2 }");

        let mut context = ExtractionContext::new(config.clone(), "rust");
        context.add_entity(entity_a.clone());
        context.add_entity(entity_b.clone());

        let results = extractor
            .extract(&entity_a, &context)
            .await
            .expect("extraction succeeds");

        assert!(
            results.get("max_similarity").copied().unwrap_or_default()
                >= extractor.similarity_threshold()
        );
        assert!(results.get("duplicate_count").copied().unwrap_or_default() >= 1.0);
    }

    #[tokio::test]
    async fn test_meets_fragment_thresholds_respects_ast_stats() {
        let tmp = tempdir().expect("temp dir");
        let short_path = tmp.path().join("short.rs");
        fs::write(&short_path, "fn short() {}").expect("write short file");

        let detailed_source = "\
fn long_enough() {\n    let mut total = 0;\n    for value in 0..5 {\n        total += process(value);\n    }\n    if total > 3 {\n        finalize(total);\n    }\n}\n";
        let acceptable_path = tmp.path().join("long.rs");
        fs::write(&acceptable_path, detailed_source).expect("write long file");

        let mut config = DedupeConfig::default();
        config.min_function_tokens = 5;
        config.min_ast_nodes = 20;
        config.require_distinct_blocks = 1;

        let extractor = LshExtractor::with_dedupe_config(config.clone());

        let short_entity = CodeEntity::new(
            "short",
            "function",
            "short",
            short_path.to_string_lossy().into_owned(),
        )
        .with_source_code("fn short() {}");
        assert!(
            !extractor
                .entity_passes_thresholds(&short_entity)
                .await
                .expect("threshold evaluation"),
            "short snippet should be filtered out"
        );

        let mut acceptable = CodeEntity::new(
            "ok",
            "function",
            "ok",
            acceptable_path.to_string_lossy().into_owned(),
        )
        .with_source_code(detailed_source);
        let total_len = detailed_source.as_bytes().len();
        acceptable.add_property("start_byte", serde_json::json!(0));
        acceptable.add_property("end_byte", serde_json::json!(total_len));
        acceptable.add_property("ast_kind", serde_json::json!("function_item"));

        let stats = extractor
            .compute_entity_ast_stats(&acceptable)
            .await
            .expect("ast stats lookup")
            .expect("ast stats present");
        assert!(
            stats.node_count >= config.min_ast_nodes,
            "node_count {}",
            stats.node_count
        );
        assert!(
            stats.block_count >= config.require_distinct_blocks,
            "block_count {}",
            stats.block_count
        );
        assert!(!stats.has_stop_motif, "stop motif incorrectly detected");

        assert!(
            extractor
                .entity_passes_thresholds(&acceptable)
                .await
                .expect("threshold evaluation"),
            "entity meeting thresholds should be accepted"
        );
    }

    #[test]
    fn test_similarity_context_cache_reuses_last_context() {
        let extractor = LshExtractor::new();
        let config = Arc::new(ValknutConfig::default());
        let mut context = ExtractionContext::new(config, "rust");

        let entity_a = CodeEntity::new("entity_a", "function", "entity_a", "a.rs")
            .with_source_code("fn entity_a() { let x = 1; x + 2; }");
        let entity_b = CodeEntity::new("entity_b", "function", "entity_b", "b.rs")
            .with_source_code("fn entity_b() { let y = 2; y * 3; }");

        context.add_entity(entity_a);
        context.add_entity(entity_b);

        let first = extractor
            .similarity_context(&context)
            .expect("context should exist");
        let second = extractor
            .similarity_context(&context)
            .expect("cached context should exist");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn test_generate_cache_key_is_order_insensitive() {
        let extractor = LshExtractor::new().with_denoise_enabled(true);
        let entity_a = CodeEntity::new("alpha", "function", "alpha", "alpha.rs")
            .with_source_code("fn alpha() { 1 }");
        let entity_b = CodeEntity::new("beta", "function", "beta", "beta.rs")
            .with_source_code("fn beta() { 2 }");

        let forward_key = extractor.generate_cache_key(&[&entity_a, &entity_b]);
        let reverse_key = extractor.generate_cache_key(&[&entity_b, &entity_a]);

        assert_eq!(forward_key, reverse_key);
    }

    #[test]
    fn test_weighted_signature_cache_hits() {
        let extractor = LshExtractor::new().with_denoise_enabled(true);
        let entity_a = CodeEntity::new("w_alpha", "function", "alpha", "alpha.rs")
            .with_source_code("fn alpha() { let mut v = 0; v += 1; v }");
        let entity_b = CodeEntity::new("w_beta", "function", "beta", "beta.rs")
            .with_source_code("fn beta() { let mut v = 1; v += 2; v }");
        let entities = vec![&entity_a, &entity_b];

        let first = extractor
            .get_or_compute_weighted_signatures(&entities)
            .expect("initial signatures");

        {
            let cached = extractor
                .cached_weighted_signatures
                .read()
                .expect("cache guard");
            assert!(cached
                .as_ref()
                .map(|map| !map.is_empty())
                .unwrap_or_default());
        }

        let second = extractor
            .get_or_compute_weighted_signatures(&entities)
            .expect("cached signatures");

        assert_eq!(first.len(), second.len());
        for (key, sig) in &first {
            let cached = second
                .get(key)
                .expect("signature for key should be present on cache hit");
            assert_eq!(sig.signature, cached.signature);
        }
    }

    #[test]
    fn test_shingle_variants_produce_consistent_lengths() {
        let extractor = LshExtractor::with_params(32, 3);
        let code = "fn compute(value: i32) -> i32 { if value > 0 { value } else { -value } }";
        let standard = extractor.create_shingles(code);
        let interned = extractor.create_shingles_interned(code);

        assert_eq!(standard.len(), interned.len());
        assert!(!standard.is_empty());
    }
