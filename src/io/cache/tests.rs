    use super::*;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::{tempdir, TempDir};

    fn sample_codebase_info() -> CodebaseInfo {
        let mut file_info = HashMap::new();
        let hash = Sha256::digest(b"fn sample() {}").to_vec();
        file_info.insert(
            "sample.rs".to_string(),
            FileInfo {
                line_count: 2,
                content_hash: hash,
            },
        );

        CodebaseInfo {
            functions: vec![FunctionInfo {
                id: "sample".to_string(),
                source_code: "fn sample() {\n    let value = 42;\n}".to_string(),
                file_path: "sample.rs".to_string(),
                line_count: 2,
            }],
            total_lines: 2,
            file_info,
        }
    }

    fn write_cache(manager: &StopMotifCacheManager, cache: &StopMotifCache) {
        let cache_path = manager.get_cache_path();
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let serialized = serde_json::to_string_pretty(cache).unwrap();
        fs::write(cache_path, serialized).unwrap();
    }

    #[test]
    fn test_get_valid_cache_returns_none_when_expired() {
        let temp_dir = TempDir::new().unwrap();
        let mut policy = CacheRefreshPolicy::default();
        policy.max_age_days = 1;
        let manager = StopMotifCacheManager::new(temp_dir.path(), policy.clone());

        let codebase = sample_codebase_info();
        let signature = manager.compute_codebase_signature(&codebase);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expired_cache = StopMotifCache {
            version: 1,
            k_gram_size: policy.k_gram_size,
            token_grams: Vec::new(),
            pdg_motifs: Vec::new(),
            ast_patterns: Vec::new(),
            last_updated: now - (policy.max_age_days * 24 * 60 * 60) - 1,
            codebase_signature: signature,
            mining_stats: MiningStats::default(),
        };

        write_cache(&manager, &expired_cache);

        let result = manager.get_valid_cache(&codebase).unwrap();
        assert!(result.is_none(), "expected expired cache to be invalidated");
    }

    #[test]
    fn test_get_valid_cache_returns_none_on_large_signature_change() {
        let temp_dir = TempDir::new().unwrap();
        let mut policy = CacheRefreshPolicy::default();
        policy.change_threshold_percent = 1.0;
        let manager = StopMotifCacheManager::new(temp_dir.path(), policy.clone());

        let original = sample_codebase_info();
        let signature = manager.compute_codebase_signature(&original);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let cache = StopMotifCache {
            version: 1,
            k_gram_size: policy.k_gram_size,
            token_grams: Vec::new(),
            pdg_motifs: Vec::new(),
            ast_patterns: Vec::new(),
            last_updated: now,
            codebase_signature: signature,
            mining_stats: MiningStats::default(),
        };

        write_cache(&manager, &cache);

        let mut updated = sample_codebase_info();
        updated.total_lines = 10;

        let result = manager.get_valid_cache(&updated).unwrap();
        assert!(
            result.is_none(),
            "expected cache to be refreshed when signature diverges"
        );
    }

    #[test]
    fn test_get_valid_cache_returns_cache_when_fresh() {
        let temp_dir = TempDir::new().unwrap();
        let policy = CacheRefreshPolicy::default();
        let manager = StopMotifCacheManager::new(temp_dir.path(), policy.clone());

        let codebase = sample_codebase_info();
        let signature = manager.compute_codebase_signature(&codebase);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let cache = StopMotifCache {
            version: 1,
            k_gram_size: policy.k_gram_size,
            token_grams: Vec::new(),
            pdg_motifs: Vec::new(),
            ast_patterns: Vec::new(),
            last_updated: now,
            codebase_signature: signature,
            mining_stats: MiningStats::default(),
        };

        write_cache(&manager, &cache);

        let result = manager.get_valid_cache(&codebase).unwrap();
        assert!(result.is_some(), "expected fresh cache to remain valid");
    }

    #[test]
    fn test_pattern_miner_extracts_kgrams_and_motifs() {
        let mut policy = CacheRefreshPolicy::default();
        policy.k_gram_size = 2;
        let miner = PatternMiner::new(policy);

        let function = FunctionInfo {
            id: "f".to_string(),
            source_code: "if value == 10 {\n    println!(\"value\");\n    total += value;\n}"
                .to_string(),
            file_path: "sample.rs".to_string(),
            line_count: 4,
        };

        let kgrams = miner.extract_function_kgrams(&function);
        assert!(
            !kgrams.is_empty(),
            "expected k-grams when token window threshold is satisfied"
        );

        let motifs = miner.extract_function_motifs(&function).unwrap();
        assert!(
            motifs.keys().any(|key| key.contains("control")),
            "expected control flow motif"
        );
        assert!(
            motifs.keys().any(|key| key.contains("boiler")),
            "expected boilerplate motif from println!/unwrap"
        );
    }

    #[test]
    fn test_pattern_miner_select_stop_motifs_respects_percentile() {
        let mut policy = CacheRefreshPolicy::default();
        policy.stop_motif_percentile = 1.0;
        let mut miner = PatternMiner::new(policy);

        miner.kgram_frequencies = HashMap::from([
            ("alpha beta".to_string(), 10),
            ("beta gamma".to_string(), 5),
        ]);
        miner.motif_frequencies = HashMap::from([("call:helper".to_string(), 7)]);
        miner.total_documents = 20;

        let idf_scores = miner.calculate_idf_scores();
        let stop_motifs = miner.select_stop_motifs(&idf_scores).unwrap();
        assert_eq!(stop_motifs.len(), 1, "percentile should cap the selection");
        assert_eq!(stop_motifs[0].pattern, "alpha beta");
        assert_eq!(
            stop_motifs[0].category,
            PatternCategory::TokenGram,
            "expected token gram category for highest frequency k-gram"
        );
    }

    #[test]
    fn test_normalize_token_handles_literals_and_keywords() {
        let mut policy = CacheRefreshPolicy::default();
        policy.k_gram_size = 2;
        let miner = PatternMiner::new(policy);

        assert_eq!(miner.normalize_token("if"), "if");
        assert_eq!(miner.normalize_token("=="), "==");
        assert_eq!(miner.normalize_token("42"), "INT_LIT");
        assert_eq!(miner.normalize_token("3.14"), "FLOAT_LIT");
        assert_eq!(miner.normalize_token("\"text\""), "STR_LIT");
        assert_eq!(miner.normalize_token("variable_name"), "LOCAL_VAR");
        assert_eq!(miner.normalize_token("SOME_CONSTANT"), "SOME_CONSTANT");
    }

    #[test]
    fn test_compute_codebase_signature_deterministic() {
        let policy = CacheRefreshPolicy::default();
        let manager = StopMotifCacheManager::new("unused", policy);
        let info = sample_codebase_info();
        let sig1 = manager.compute_codebase_signature(&info);
        let sig2 = manager.compute_codebase_signature(&info);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_estimate_change_percentage_detects_difference() {
        let policy = CacheRefreshPolicy::default();
        let manager = StopMotifCacheManager::new("unused", policy);
        assert_eq!(manager.estimate_change_percentage("aaaa", "aaaa"), 0.0);
        assert!(
            manager.estimate_change_percentage("aaaa", "bbbb") >= 50.0,
            "expected large heuristic change"
        );
    }

    #[test]
    fn test_ast_stop_motif_miner_extracts_patterns() -> Result<()> {
        let mut miner = AstStopMotifMiner::new();
        let functions = vec![
            FunctionInfo {
                id: "py_func".to_string(),
                source_code: "def greet(name):\n    print(f\"hi {name}\")\n".to_string(),
                file_path: "greet.py".to_string(),
                line_count: 2,
            },
            FunctionInfo {
                id: "js_func".to_string(),
                source_code: "export function add(a, b) { return a + b; }\n".to_string(),
                file_path: "math.js".to_string(),
                line_count: 1,
            },
        ];

        let patterns = miner.mine_ast_stop_motifs(&functions)?;
        assert!(
            patterns.len() <= functions.len(),
            "stop-motif selection should not exceed number of functions"
        );

        Ok(())
    }

    #[test]
    fn test_stop_motif_cache_serialization() {
        let cache = StopMotifCache {
            version: 1,
            k_gram_size: 9,
            token_grams: vec![
                StopMotifEntry {
                    pattern: "if LOCAL_VAR == INT_LIT".to_string(),
                    support: 150,
                    idf_score: 2.5,
                    weight_multiplier: 0.2,
                    category: PatternCategory::TokenGram,
                },
                StopMotifEntry {
                    pattern: "println! ( STR_LIT )".to_string(),
                    support: 89,
                    idf_score: 1.8,
                    weight_multiplier: 0.2,
                    category: PatternCategory::TokenGram,
                },
            ],
            pdg_motifs: vec![
                StopMotifEntry {
                    pattern: "control:branch".to_string(),
                    support: 200,
                    idf_score: 3.2,
                    weight_multiplier: 0.2,
                    category: PatternCategory::ControlFlow,
                },
                StopMotifEntry {
                    pattern: "boiler:debug_print".to_string(),
                    support: 95,
                    idf_score: 1.9,
                    weight_multiplier: 0.2,
                    category: PatternCategory::Boilerplate,
                },
            ],
            ast_patterns: vec![
                AstStopMotifEntry {
                    pattern: "node_type:Function".to_string(),
                    support: 300,
                    idf_score: 2.1,
                    weight_multiplier: 0.2,
                    category: AstPatternCategory::NodeType,
                    language: "python".to_string(),
                    metadata: HashMap::new(),
                },
                AstStopMotifEntry {
                    pattern: "token_seq:import_os".to_string(),
                    support: 120,
                    idf_score: 1.8,
                    weight_multiplier: 0.2,
                    category: AstPatternCategory::TokenSequence,
                    language: "python".to_string(),
                    metadata: HashMap::new(),
                },
            ],
            last_updated: 1699123456,
            codebase_signature: "abc123def456".to_string(),
            mining_stats: MiningStats {
                functions_analyzed: 1500,
                unique_kgrams_found: 8000,
                unique_motifs_found: 1200,
                ast_patterns_found: 2,
                ast_node_types_found: 1,
                ast_subtree_patterns_found: 0,
                stop_motifs_selected: 6, // Updated to include AST patterns
                percentile_threshold: 0.5,
                mining_duration_ms: 2500,
                languages_processed: ["python".to_string(), "rust".to_string()]
                    .into_iter()
                    .collect(),
            },
        };

        // Test serialization
        let json = serde_json::to_string_pretty(&cache).expect("Failed to serialize cache");
        assert!(json.contains("\"version\": 1"));
        assert!(json.contains("\"k_gram_size\": 9"));
        assert!(json.contains("if LOCAL_VAR == INT_LIT"));
        assert!(json.contains("control:branch"));

        // Test deserialization
        let deserialized: StopMotifCache =
            serde_json::from_str(&json).expect("Failed to deserialize cache");
        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.token_grams.len(), 2);
        assert_eq!(deserialized.pdg_motifs.len(), 2);
        assert_eq!(deserialized.mining_stats.functions_analyzed, 1500);
    }

    #[test]
    fn test_pattern_miner_kgram_extraction() {
        let policy = CacheRefreshPolicy::default();
        let miner = PatternMiner::new(policy);

        let func = FunctionInfo {
            id: "test_func".to_string(),
            source_code: r#"
                fn test_function() {
                    if x == 42 {
                        println!("Hello world");
                    }
                    for i in 0..10 {
                        process_item(i);
                    }
                }
            "#
            .to_string(),
            file_path: "test.rs".to_string(),
            line_count: 8,
        };

        let kgrams = miner.extract_function_kgrams(&func);

        // Should have various k-grams including normalized patterns
        assert!(!kgrams.is_empty());

        // Check that normalization occurred
        let has_normalized = kgrams
            .keys()
            .any(|k| k.contains("LOCAL_VAR") || k.contains("INT_LIT") || k.contains("STR_LIT"));
        assert!(has_normalized, "Should contain normalized tokens");

        // Check for control flow patterns
        let has_control_flow = kgrams.keys().any(|k| k.contains("if") || k.contains("for"));
        assert!(has_control_flow, "Should contain control flow patterns");
    }

    #[test]
    fn test_pattern_miner_motif_extraction() -> Result<()> {
        let policy = CacheRefreshPolicy::default();
        let miner = PatternMiner::new(policy);

        let func = FunctionInfo {
            id: "test_func".to_string(),
            source_code: r#"
                fn complex_function() {
                    if condition {
                        println!("debug message");
                    }
                    for item in items {
                        let result = process(item).unwrap();
                        data.push(result);
                    }
                    while active {
                        update_state();
                    }
                }
            "#
            .to_string(),
            file_path: "test.rs".to_string(),
            line_count: 12,
        };

        let motifs = miner.extract_function_motifs(&func)?;

        // Should extract various motif types
        assert!(!motifs.is_empty());

        // Check for expected patterns
        let motif_keys: Vec<_> = motifs.keys().collect();
        let has_control = motif_keys
            .iter()
            .any(|k| k.contains("control:branch") || k.contains("control:loop"));
        let has_boilerplate = motif_keys
            .iter()
            .any(|k| k.contains("boiler:debug_print") || k.contains("boiler:error_unwrap"));
        let has_assignment = motif_keys.iter().any(|k| k.contains("assign:assign"));
        let has_calls = motif_keys.iter().any(|k| k.contains("call:call"));

        assert!(has_control, "Should extract control flow motifs");
        assert!(has_boilerplate, "Should extract boilerplate motifs");
        assert!(has_assignment, "Should extract assignment motifs");
        assert!(has_calls, "Should extract function call motifs");

        Ok(())
    }

    #[test]
    fn test_pattern_miner_stop_motif_selection() -> Result<()> {
        let policy = CacheRefreshPolicy {
            stop_motif_percentile: 50.0, // Top 50% for easier testing
            ..Default::default()
        };
        let mut miner = PatternMiner::new(policy);

        let codebase_info = CodebaseInfo {
            functions: vec![
                FunctionInfo {
                    id: "func1".to_string(),
                    source_code: "fn func1() { println!(\"test\"); }".to_string(),
                    file_path: "file1.rs".to_string(),
                    line_count: 1,
                },
                FunctionInfo {
                    id: "func2".to_string(),
                    source_code: "fn func2() { println!(\"test2\"); if x > 0 { process(); } }"
                        .to_string(),
                    file_path: "file2.rs".to_string(),
                    line_count: 1,
                },
                FunctionInfo {
                    id: "func3".to_string(),
                    source_code: "fn func3() { if condition { println!(\"debug\"); } }".to_string(),
                    file_path: "file3.rs".to_string(),
                    line_count: 1,
                },
            ],
            total_lines: 3,
            file_info: HashMap::new(),
        };

        let cache = miner.mine_stop_motifs(&codebase_info)?;

        // Verify cache structure
        assert_eq!(cache.version, 1);
        assert_eq!(cache.mining_stats.functions_analyzed, 3);
        assert!(cache.mining_stats.stop_motifs_selected > 0);

        // Should have both token grams and motifs
        assert!(!cache.token_grams.is_empty() || !cache.pdg_motifs.is_empty());

        // All stop motifs should have weight multiplier of 0.2
        for stop_motif in &cache.token_grams {
            assert_eq!(stop_motif.weight_multiplier, 0.2);
            assert!(stop_motif.support > 0);
        }

        for stop_motif in &cache.pdg_motifs {
            assert_eq!(stop_motif.weight_multiplier, 0.2);
            assert!(stop_motif.support > 0);
        }

        Ok(())
    }

    #[test]
    fn test_cache_manager_persistence() -> Result<()> {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_dir = temp_dir.path().to_path_buf();

        let policy = CacheRefreshPolicy::default();
        let cache_manager = StopMotifCacheManager::new(&cache_dir, policy);

        let codebase_info = CodebaseInfo {
            functions: vec![FunctionInfo {
                id: "test_func".to_string(),
                source_code: "fn test() { println!(\"test\"); }".to_string(),
                file_path: "test.rs".to_string(),
                line_count: 1,
            }],
            total_lines: 1,
            file_info: HashMap::new(),
        };

        // First call should create cache
        let cache1 = cache_manager.get_cache(&codebase_info)?;
        assert_eq!(cache1.mining_stats.functions_analyzed, 1);

        // Verify cache file was created
        let cache_path = cache_dir.join("stop_motifs.v1.json");
        assert!(cache_path.exists());

        // Second call should load from cache (same codebase signature)
        let cache2 = cache_manager.get_cache(&codebase_info)?;
        assert_eq!(cache2.mining_stats.functions_analyzed, 1);
        assert_eq!(cache1.codebase_signature, cache2.codebase_signature);

        Ok(())
    }

    #[test]
    fn test_cache_invalidation_by_change() -> Result<()> {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_dir = temp_dir.path().to_path_buf();

        let policy = CacheRefreshPolicy {
            change_threshold_percent: 1.0, // Very low threshold for testing
            ..Default::default()
        };
        let cache_manager = StopMotifCacheManager::new(&cache_dir, policy);

        let codebase_info1 = CodebaseInfo {
            functions: vec![FunctionInfo {
                id: "func1".to_string(),
                source_code: "fn func1() { println!(\"test\"); }".to_string(),
                file_path: "test.rs".to_string(),
                line_count: 1,
            }],
            total_lines: 1,
            file_info: HashMap::new(),
        };

        let codebase_info2 = CodebaseInfo {
            functions: vec![
                FunctionInfo {
                    id: "func1".to_string(),
                    source_code: "fn func1() { println!(\"test\"); }".to_string(),
                    file_path: "test.rs".to_string(),
                    line_count: 1,
                },
                FunctionInfo {
                    id: "func2".to_string(),
                    source_code: "fn func2() { if x > 0 { process(); } }".to_string(),
                    file_path: "test2.rs".to_string(),
                    line_count: 1,
                },
            ],
            total_lines: 2,
            file_info: HashMap::new(),
        };

        // Create initial cache
        let cache1 = cache_manager.get_cache(&codebase_info1)?;
        let sig1 = cache1.codebase_signature.clone();

        // Changed codebase should trigger refresh
        let cache2 = cache_manager.get_cache(&codebase_info2)?;
        let sig2 = cache2.codebase_signature.clone();

        assert_ne!(
            sig1, sig2,
            "Signatures should differ for different codebases"
        );
        assert_eq!(cache2.mining_stats.functions_analyzed, 2);

        Ok(())
    }

    #[test]
    fn test_cache_retains_when_change_below_threshold() -> Result<()> {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_dir = temp_dir.path().to_path_buf();

        let policy = CacheRefreshPolicy {
            change_threshold_percent: 75.0,
            ..Default::default()
        };
        let cache_manager = StopMotifCacheManager::new(&cache_dir, policy);

        let mut base_file_info = HashMap::new();
        base_file_info.insert(
            "src/lib.rs".to_string(),
            FileInfo {
                line_count: 10,
                content_hash: vec![1, 2, 3, 4],
            },
        );

        let base_info = CodebaseInfo {
            functions: vec![FunctionInfo {
                id: "func1".to_string(),
                source_code: "fn func1() {}".to_string(),
                file_path: "src/lib.rs".to_string(),
                line_count: 1,
            }],
            total_lines: 10,
            file_info: base_file_info.clone(),
        };

        let cache1 = cache_manager.get_cache(&base_info)?;
        assert_eq!(cache1.mining_stats.functions_analyzed, 1);

        let mut changed_info = base_info.clone();
        changed_info.functions.push(FunctionInfo {
            id: "func2".to_string(),
            source_code: "fn func2() {}".to_string(),
            file_path: "src/new.rs".to_string(),
            line_count: 1,
        });
        changed_info.total_lines = 11;
        let mut changed_file_info = base_file_info;
        changed_file_info.insert(
            "src/new.rs".to_string(),
            FileInfo {
                line_count: 5,
                content_hash: vec![9, 9, 9, 9],
            },
        );
        changed_info.file_info = changed_file_info;

        let cache2 = cache_manager.get_cache(&changed_info)?;
        assert_eq!(
            cache2.codebase_signature, cache1.codebase_signature,
            "expected cache reuse when change below threshold"
        );
        assert_eq!(
            cache2.mining_stats.functions_analyzed, 1,
            "expected mining stats unchanged for reused cache"
        );

        Ok(())
    }

    #[test]
    fn test_cache_expires_when_past_max_age() -> Result<()> {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_dir = temp_dir.path().to_path_buf();

        let policy = CacheRefreshPolicy {
            max_age_days: 0,
            ..Default::default()
        };
        let cache_manager = StopMotifCacheManager::new(&cache_dir, policy);

        let codebase_info = CodebaseInfo {
            functions: vec![FunctionInfo {
                id: "func1".to_string(),
                source_code: "fn func1() {}".to_string(),
                file_path: "src/lib.rs".to_string(),
                line_count: 1,
            }],
            total_lines: 1,
            file_info: HashMap::new(),
        };

        let cache1 = cache_manager.get_cache(&codebase_info)?;
        let cache_path = cache_dir.join("stop_motifs.v1.json");
        assert!(cache_path.exists());

        let mut cache_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&cache_path)?).expect("parse cache json");
        if let Some(obj) = cache_json.as_object_mut() {
            obj.insert("last_updated".to_string(), json!(0));
        }
        fs::write(&cache_path, serde_json::to_string_pretty(&cache_json)?)?;

        let refreshed = cache_manager.get_cache(&codebase_info)?;
        let refreshed_file: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&cache_path)?)
                .expect("parse refreshed cache json");
        let refreshed_disk = refreshed_file["last_updated"]
            .as_u64()
            .expect("last_updated should be number");
        assert_eq!(refreshed_disk, refreshed.last_updated);
        assert!(refreshed.last_updated >= cache1.last_updated);
        assert!(
            refreshed.last_updated > 0,
            "expected refreshed cache timestamp to be non-zero"
        );

        Ok(())
    }

    #[test]
    fn test_compute_codebase_signature_order_independent() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_manager =
            StopMotifCacheManager::new(temp_dir.path(), CacheRefreshPolicy::default());

        let mut file_info_a = HashMap::new();
        file_info_a.insert(
            "b.rs".to_string(),
            FileInfo {
                line_count: 20,
                content_hash: vec![2, 3, 4],
            },
        );
        file_info_a.insert(
            "a.rs".to_string(),
            FileInfo {
                line_count: 10,
                content_hash: vec![1, 2, 3],
            },
        );
        let info_a = CodebaseInfo {
            functions: vec![],
            total_lines: 30,
            file_info: file_info_a,
        };

        let mut file_info_b = HashMap::new();
        file_info_b.insert(
            "a.rs".to_string(),
            FileInfo {
                line_count: 10,
                content_hash: vec![1, 2, 3],
            },
        );
        file_info_b.insert(
            "b.rs".to_string(),
            FileInfo {
                line_count: 20,
                content_hash: vec![2, 3, 4],
            },
        );
        let info_b = CodebaseInfo {
            functions: vec![],
            total_lines: 30,
            file_info: file_info_b,
        };

        let sig_a = cache_manager.compute_codebase_signature(&info_a);
        let sig_b = cache_manager.compute_codebase_signature(&info_b);
        assert_eq!(
            sig_a, sig_b,
            "expected signature independence from file ordering"
        );
    }

    #[test]
    fn test_pattern_normalization() {
        let policy = CacheRefreshPolicy::default();
        let miner = PatternMiner::new(policy);

        // Test token normalization
        assert_eq!(miner.normalize_token("42"), "INT_LIT");
        assert_eq!(miner.normalize_token("3.14"), "FLOAT_LIT");
        assert_eq!(miner.normalize_token("\"hello\""), "STR_LIT");
        assert_eq!(miner.normalize_token("'c'"), "STR_LIT");
        assert_eq!(miner.normalize_token("local_var"), "LOCAL_VAR");
        assert_eq!(miner.normalize_token("CONSTANT"), "CONSTANT");
        assert_eq!(miner.normalize_token("function_name"), "LOCAL_VAR");
    }

    #[test]
    fn test_motif_categorization() {
        let policy = CacheRefreshPolicy::default();
        let miner = PatternMiner::new(policy);

        // Test motif categorization
        assert_eq!(
            miner.categorize_motif("control:branch"),
            PatternCategory::ControlFlow
        );
        assert_eq!(
            miner.categorize_motif("control:loop"),
            PatternCategory::ControlFlow
        );
        assert_eq!(
            miner.categorize_motif("assign:assign"),
            PatternCategory::Assignment
        );
        assert_eq!(
            miner.categorize_motif("call:call"),
            PatternCategory::FunctionCall
        );
        assert_eq!(
            miner.categorize_motif("data:collection"),
            PatternCategory::DataStructure
        );
        assert_eq!(
            miner.categorize_motif("boiler:debug_print"),
            PatternCategory::Boilerplate
        );
        assert_eq!(
            miner.categorize_motif("boiler:error_unwrap"),
            PatternCategory::Boilerplate
        );
        assert_eq!(
            miner.categorize_motif("unknown:pattern"),
            PatternCategory::Boilerplate
        );
    }

    #[test]
    fn test_cache_new() {
        let cache = Cache::new();
        // Basic test to ensure new() works
        assert_eq!(std::mem::size_of_val(&cache), std::mem::size_of::<Cache>());
    }

    #[test]
    fn test_cache_default() {
        let cache = Cache::default();
        // Basic test to ensure default() works
        assert_eq!(std::mem::size_of_val(&cache), std::mem::size_of::<Cache>());
    }

    #[test]
    fn test_cache_debug() {
        let cache = Cache::new();
        let debug_str = format!("{:?}", cache);
        assert_eq!(debug_str, "Cache");
    }
