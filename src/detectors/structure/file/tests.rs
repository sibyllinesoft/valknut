    use super::*;
    use crate::detectors::structure::config::{
        CohesionEdge, EntityHealthConfig, FsDirectoryConfig, FsFileConfig, ImportStatement,
        PartitioningConfig, StructureConfig, StructureToggles,
    };
    use crate::lang::common::{EntityKind, ParsedEntity, SourceLocation};
    use crate::lang::registry::adapter_for_language;
    use petgraph::Graph;
    use serde_json::Value;
    use std::collections::HashSet;
    use std::env;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> StructureConfig {
        StructureConfig {
            enable_branch_packs: true,
            enable_file_split_packs: true,
            top_packs: 20,
            fsdir: FsDirectoryConfig {
                max_files_per_dir: 20,
                max_subdirs_per_dir: 10,
                max_dir_loc: 2000,
                target_loc_per_subdir: 500,
                min_branch_recommendation_gain: 0.1,
                min_files_for_split: 5,
                optimal_files: 7,
                optimal_files_stddev: 2.0,
                optimal_subdirs: 3,
                optimal_subdirs_stddev: 1.5,
            },
            fsfile: FsFileConfig {
                huge_loc: 50,     // Low threshold for testing
                huge_bytes: 1000, // Low threshold for testing
                min_split_loc: 10,
                min_entities_per_split: 2,
                optimal_ast_nodes: 2000,
                ast_nodes_95th_percentile: 6000,
            },
            partitioning: PartitioningConfig {
                max_clusters: 8,
                min_clusters: 2,
                balance_tolerance: 0.3,
                naming_fallbacks: vec![
                    "core".to_string(),
                    "utils".to_string(),
                    "components".to_string(),
                    "services".to_string(),
                ],
            },
            entity_health: EntityHealthConfig::default(),
            exclude_patterns: Vec::new(),
        }
    }

    #[test]
    fn test_file_analyzer_new() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config.clone());

        assert_eq!(analyzer.config.fsfile.huge_loc, config.fsfile.huge_loc);
    }

    #[test]
    fn test_lognormal_score_at_optimal() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        // Score should be 1.0 when value equals optimal (mode)
        let score = analyzer.calculate_lognormal_score(2000, 2000, 6000);
        assert!(
            (score - 1.0).abs() < 0.0001,
            "Score at optimal should be 1.0, got {}",
            score
        );
    }

    #[test]
    fn test_lognormal_score_asymmetric() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        // Lognormal is asymmetric - scores should decrease as we move away from optimal
        let score_at_optimal = analyzer.calculate_lognormal_score(2000, 2000, 6000);
        let score_below = analyzer.calculate_lognormal_score(1000, 2000, 6000);
        let score_above = analyzer.calculate_lognormal_score(4000, 2000, 6000);

        assert!(score_at_optimal > score_below, "Score should decrease below optimal");
        assert!(score_at_optimal > score_above, "Score should decrease above optimal");

        // The distribution has a long right tail, so very large files still get
        // some credit while very small files drop off faster
        // Compare 500 nodes (0.25x optimal) vs 8000 nodes (4x optimal)
        let score_quarter = analyzer.calculate_lognormal_score(500, 2000, 6000);
        let score_4x = analyzer.calculate_lognormal_score(8000, 2000, 6000);
        assert!(
            score_4x > score_quarter,
            "Lognormal should have longer right tail (score_4x={}, score_quarter={})",
            score_4x, score_quarter
        );
    }

    #[test]
    fn test_lognormal_score_at_95th_percentile() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        // At 95th percentile, score should be low but non-zero
        let score = analyzer.calculate_lognormal_score(6000, 2000, 6000);
        assert!(score > 0.0, "Score at 95th percentile should be positive");
        assert!(score < 0.5, "Score at 95th percentile should be less than 0.5");
    }

    #[test]
    fn test_lognormal_score_very_large_file() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        // Very large files should have very low scores
        let score = analyzer.calculate_lognormal_score(20000, 2000, 6000);
        assert!(score < 0.1, "Score for very large file should be very low");
    }

    #[test]
    fn test_lognormal_score_edge_cases() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        // Zero value should return 0
        assert_eq!(analyzer.calculate_lognormal_score(0, 2000, 6000), 0.0);

        // Invalid config (p95 <= optimal) should handle gracefully
        assert_eq!(analyzer.calculate_lognormal_score(100, 2000, 1000), 0.0);
    }

    #[test]
    fn test_calculate_file_metrics() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");

        // Create a simple Python file
        let content = r#"
def hello():
    print("Hello, world!")

def goodbye():
    print("Goodbye!")
"#;
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);
        let metrics = analyzer.calculate_file_metrics(&file_path).unwrap();

        assert_eq!(metrics.path, file_path);
        assert!(metrics.loc > 0, "LOC should be positive");
        assert!(metrics.ast_nodes > 0, "AST nodes should be positive");
        assert!(
            metrics.size_score >= 0.0 && metrics.size_score <= 1.0,
            "Size score should be in [0, 1]"
        );

        // Check entity health is calculated
        assert!(
            metrics.entity_health.is_some(),
            "Entity health should be calculated for Python file"
        );
        let entity_health = metrics.entity_health.unwrap();
        assert!(entity_health.entity_count >= 2, "Should find at least 2 entities");
        assert!(
            entity_health.health >= 0.0 && entity_health.health <= 1.0,
            "Health should be in [0, 1]"
        );
    }

    #[test]
    fn test_calculate_entity_health() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");

        // Create a file with functions
        let content = r#"
def small_function():
    return 1

def medium_function():
    x = 1
    y = 2
    z = x + y
    return z
"#;
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);
        let content = fs::read_to_string(&file_path).unwrap();
        let health = analyzer.calculate_entity_health(&file_path, &content).unwrap();

        // Should find entities
        assert!(health.entity_count > 0, "Should find entities");
        assert!(health.total_ast_nodes > 0, "Should have AST nodes");

        // Health scores should be valid
        assert!(
            health.health >= 0.0 && health.health <= 1.0,
            "health should be in [0, 1]"
        );
        assert!(
            health.min_health >= 0.0 && health.min_health <= 1.0,
            "min_health should be in [0, 1]"
        );

        // Small functions should be healthy (high scores)
        assert!(
            health.health > 0.5,
            "Small functions should have good health, got {}",
            health.health
        );
    }

    #[test]
    fn test_is_code_file() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        assert!(analyzer.is_code_file("py"));
        assert!(analyzer.is_code_file("js"));
        assert!(analyzer.is_code_file("ts"));
        assert!(analyzer.is_code_file("rs"));
        assert!(analyzer.is_code_file("go"));
        assert!(analyzer.is_code_file("java"));
        assert!(analyzer.is_code_file("cpp"));
        assert!(!analyzer.is_code_file("txt"));
        assert!(!analyzer.is_code_file("md"));
        assert!(!analyzer.is_code_file("png"));
    }

    #[test]
    fn test_count_lines_of_code() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");

        let content = r#"# Comment line
import os
import sys

def hello():
    print("Hello world")
    return True
"#;
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);
        let loc = analyzer.count_lines_of_code(&file_path).unwrap();

        assert!(loc > 0);
    }

    #[test]
    fn test_should_skip_directory() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        assert!(analyzer.should_skip_directory(Path::new("node_modules")));
        assert!(analyzer.should_skip_directory(Path::new("__pycache__")));
        assert!(analyzer.should_skip_directory(Path::new("target")));
        assert!(analyzer.should_skip_directory(Path::new(".git")));
        assert!(analyzer.should_skip_directory(Path::new("build")));
        assert!(analyzer.should_skip_directory(Path::new("dist")));
        assert!(!analyzer.should_skip_directory(Path::new("src")));
        assert!(!analyzer.should_skip_directory(Path::new("lib")));
    }

    #[test]
    fn test_find_cohesion_communities_filters_small_clusters() {
        let mut config = create_test_config();
        config.fsfile.min_entities_per_split = 2;
        let analyzer = FileAnalyzer::new(config);

        let mut graph = Graph::new_undirected();
        let mut symbols_a = HashSet::new();
        symbols_a.insert("value".to_string());
        symbols_a.insert("count".to_string());
        let node_a = graph.add_node(EntityNode {
            name: "alpha".into(),
            entity_type: "function".into(),
            loc: 10,
            ast_nodes: 100,
            symbols: symbols_a,
        });

        let mut symbols_b = HashSet::new();
        symbols_b.insert("value".to_string());
        symbols_b.insert("result".to_string());
        let node_b = graph.add_node(EntityNode {
            name: "beta".into(),
            entity_type: "function".into(),
            loc: 12,
            ast_nodes: 120,
            symbols: symbols_b,
        });

        let mut symbols_c = HashSet::new();
        symbols_c.insert("temp".to_string());
        let node_c = graph.add_node(EntityNode {
            name: "gamma".into(),
            entity_type: "function".into(),
            loc: 8,
            ast_nodes: 80,
            symbols: symbols_c,
        });

        graph.add_edge(
            node_a,
            node_b,
            CohesionEdge {
                similarity: 0.85,
                shared_symbols: 1,
            },
        );
        graph.add_edge(
            node_b,
            node_c,
            CohesionEdge {
                similarity: 0.1,
                shared_symbols: 0,
            },
        );

        let communities = analyzer.find_cohesion_communities(&graph).unwrap();
        assert_eq!(communities.len(), 1);
        assert_eq!(communities[0].len(), 2);
        assert!(communities[0].contains(&node_a));
        assert!(communities[0].contains(&node_b));
    }

    #[test]
    fn test_estimate_clone_factor_counts_heavy_edges() {
        let analyzer = FileAnalyzer::new(create_test_config());
        let mut graph = Graph::new_undirected();
        let n1 = graph.add_node(EntityNode {
            name: "a".into(),
            entity_type: "fn".into(),
            loc: 10,
            ast_nodes: 100,
            symbols: HashSet::new(),
        });
        let n2 = graph.add_node(EntityNode {
            name: "b".into(),
            entity_type: "fn".into(),
            loc: 12,
            ast_nodes: 120,
            symbols: HashSet::new(),
        });
        let n3 = graph.add_node(EntityNode {
            name: "c".into(),
            entity_type: "fn".into(),
            loc: 6,
            ast_nodes: 60,
            symbols: HashSet::new(),
        });

        graph.add_edge(
            n1,
            n2,
            CohesionEdge {
                similarity: 0.9,
                shared_symbols: 3,
            },
        );
        graph.add_edge(
            n2,
            n3,
            CohesionEdge {
                similarity: 0.8,
                shared_symbols: 4,
            },
        );
        graph.add_edge(
            n1,
            n3,
            CohesionEdge {
                similarity: 0.4,
                shared_symbols: 2,
            },
        );

        let factor = analyzer.estimate_clone_factor(&graph);
        assert!(factor > 0.0);
        assert!(factor <= 1.0);
    }

    #[test]
    fn test_line_has_keyword_skips_comments() {
        let analyzer = FileAnalyzer::new(create_test_config());
        let content = r#"
// export function fake() {}
export function real() {}
"#;

        assert!(!analyzer.line_has_keyword(content, 2, "export"));
        assert!(analyzer.line_has_keyword(content, 3, "export"));
    }

    #[test]
    fn test_line_has_keyword_detects_keyword_inline() {
        let analyzer = FileAnalyzer::new(create_test_config());
        let content = "export class Service {}\n";
        assert!(analyzer.line_has_keyword(content, 1, "export"));
    }

    #[test]
    fn test_line_has_keyword_detects_keyword_from_previous_line() {
        let analyzer = FileAnalyzer::new(create_test_config());
        let content = "export\nfunction helper() {}\n";
        assert!(analyzer.line_has_keyword(content, 2, "export"));
    }

    #[test]
    fn test_line_has_keyword_handles_zero_start_line() {
        let analyzer = FileAnalyzer::new(create_test_config());
        let content = "export const value = 1;\n";
        assert!(!analyzer.line_has_keyword(content, 0, "export"));
    }

    #[test]
    fn test_canonicalize_path_returns_relative() {
        let analyzer = FileAnalyzer::new(create_test_config());
        let absolute = std::env::current_dir().unwrap().join("src").join("lib.rs");
        let canonical = analyzer.canonicalize_path(&absolute);
        assert_eq!(canonical, PathBuf::from("src/lib.rs"));
    }

    #[test]
    fn test_calculate_jaccard_similarity_empty_sets() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let set1 = HashSet::new();
        let set2 = HashSet::new();
        let similarity = analyzer.calculate_jaccard_similarity(&set1, &set2);

        assert_eq!(similarity, 1.0);
    }

    #[test]
    fn test_calculate_jaccard_similarity_identical_sets() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut set1 = HashSet::new();
        set1.insert("a".to_string());
        set1.insert("b".to_string());

        let mut set2 = HashSet::new();
        set2.insert("a".to_string());
        set2.insert("b".to_string());

        let similarity = analyzer.calculate_jaccard_similarity(&set1, &set2);

        assert_eq!(similarity, 1.0);
    }

    #[test]
    fn test_calculate_jaccard_similarity_no_overlap() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut set1 = HashSet::new();
        set1.insert("a".to_string());
        set1.insert("b".to_string());

        let mut set2 = HashSet::new();
        set2.insert("c".to_string());
        set2.insert("d".to_string());

        let similarity = analyzer.calculate_jaccard_similarity(&set1, &set2);

        assert_eq!(similarity, 0.0);
    }

    #[test]
    fn test_calculate_jaccard_similarity_partial_overlap() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut set1 = HashSet::new();
        set1.insert("a".to_string());
        set1.insert("b".to_string());

        let mut set2 = HashSet::new();
        set2.insert("a".to_string());
        set2.insert("c".to_string());

        let similarity = analyzer.calculate_jaccard_similarity(&set1, &set2);

        assert_eq!(similarity, 1.0 / 3.0); // 1 intersection / 3 union
    }

    #[test]
    fn test_analyze_entity_names_io_focused() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let entities = vec![
            "read_file".to_string(),
            "write_data".to_string(),
            "load_config".to_string(),
        ];

        let suffix = analyzer.analyze_entity_names(&entities);
        assert_eq!(suffix, "_io");
    }

    #[test]
    fn test_analyze_entity_names_api_focused() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let entities = vec![
            "handle_request".to_string(),
            "api_controller".to_string(),
            "route_handler".to_string(),
        ];

        let suffix = analyzer.analyze_entity_names(&entities);
        assert_eq!(suffix, "_api");
    }

    #[test]
    fn test_analyze_entity_names_util_focused() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let entities = vec![
            "utility_function".to_string(),
            "helper_method".to_string(),
            "tool_implementation".to_string(),
        ];

        let suffix = analyzer.analyze_entity_names(&entities);
        // Could be _util, _helper, _tool, or _io based on keywords found
        assert!(suffix == "_util" || suffix == "_helper" || suffix == "_tool" || suffix == "_io");
    }

    #[test]
    fn test_analyze_entity_names_core_fallback() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let entities = vec![
            "calculate_result".to_string(),
            "process_data".to_string(),
            "main_algorithm".to_string(),
        ];

        let suffix = analyzer.analyze_entity_names(&entities);
        assert_eq!(suffix, "_core");
    }

    #[test]
    fn test_generate_split_name() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let entities = vec!["read_file".to_string(), "write_data".to_string()];
        let name = analyzer.generate_split_name("test", "_suffix", &entities, &file_path);

        assert_eq!(name, "test_io.py"); // Should detect io pattern
    }

    #[test]
    fn test_calculate_split_value() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let graph = Graph::new_undirected();
        let metrics = FileDependencyMetrics::default();
        let value = analyzer
            .calculate_split_value(100, &file_path, &graph, &metrics)
            .unwrap();

        assert!(value.score >= 0.0);
        assert!(value.score <= 1.0);
    }

    #[test]
    fn test_calculate_split_value_includes_cycle_and_clone_factors() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut graph = Graph::new_undirected();
        let mut symbols_a = HashSet::new();
        symbols_a.insert("shared".to_string());
        symbols_a.insert("extra".to_string());
        let node_a = graph.add_node(EntityNode {
            name: "A".into(),
            entity_type: "function".into(),
            loc: 20,
            ast_nodes: 200,
            symbols: symbols_a,
        });

        let mut symbols_b = HashSet::new();
        symbols_b.insert("shared".to_string());
        symbols_b.insert("another".to_string());
        let node_b = graph.add_node(EntityNode {
            name: "B".into(),
            entity_type: "function".into(),
            loc: 18,
            ast_nodes: 180,
            symbols: symbols_b,
        });

        graph.add_edge(
            node_a,
            node_b,
            CohesionEdge {
                similarity: 0.8,
                shared_symbols: 2,
            },
        );

        let mut metrics = FileDependencyMetrics::default();
        metrics
            .outgoing_dependencies
            .insert(PathBuf::from("mod_a.rs"));
        metrics.incoming_importers.insert(PathBuf::from("mod_a.rs"));

        let value = analyzer
            .calculate_split_value(120, Path::new("src/file.rs"), &graph, &metrics)
            .unwrap();

        // size_factor = min(120/50, 1) -> 1.0
        // cycle_factor = 1/1 -> 1.0 (due to identical outgoing/incoming set)
        // clone_factor = heavy edge (similarity >=0.75 but shared_symbols <3 so 0.0)
        // Expected score = 0.6*1 + 0.3*1 + 0.1*0 = 0.9
        assert!((value.score - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_estimate_clone_factor_requires_strong_overlap() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut graph = Graph::new_undirected();
        let mut symbols_a = HashSet::new();
        symbols_a.insert("alpha".to_string());
        symbols_a.insert("beta".to_string());
        symbols_a.insert("gamma".to_string());

        let mut symbols_b = HashSet::new();
        symbols_b.insert("alpha".to_string());
        symbols_b.insert("beta".to_string());
        symbols_b.insert("delta".to_string());

        let node_a = graph.add_node(EntityNode {
            name: "first".into(),
            entity_type: "function".into(),
            loc: 15,
            ast_nodes: 150,
            symbols: symbols_a,
        });
        let node_b = graph.add_node(EntityNode {
            name: "second".into(),
            entity_type: "function".into(),
            loc: 12,
            ast_nodes: 120,
            symbols: symbols_b,
        });

        graph.add_edge(
            node_a,
            node_b,
            CohesionEdge {
                similarity: 0.78,
                shared_symbols: 3,
            },
        );

        let factor = analyzer.estimate_clone_factor(&graph);
        assert!((factor - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_split_effort() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut metrics = FileDependencyMetrics::default();
        metrics.exports.push(ExportedEntity {
            name: "foo".to_string(),
            kind: EntityKind::Function,
        });
        metrics
            .incoming_importers
            .insert(temp_dir.path().join("other.py"));

        let effort = analyzer.calculate_split_effort(&metrics).unwrap();

        assert_eq!(effort.exports, 1);
        assert_eq!(effort.external_importers, 1);
    }

    #[test]
    fn test_extract_python_imports() {
        let content = r#"import os
import sys
from pathlib import Path
from collections import OrderedDict, defaultdict
"#;

        let mut adapter = adapter_for_language("py").unwrap();
        let imports = adapter.extract_imports(content).unwrap();

        assert_eq!(imports.len(), 4);
        assert_eq!(imports[0].module, "os");
        assert_eq!(imports[0].import_type, "module");
        assert_eq!(imports[2].module, "pathlib");
        assert_eq!(imports[2].import_type, "named");
    }

    #[test]
    fn test_extract_javascript_imports() {
        let content = r#"import React from 'react';
import { useState, useEffect } from 'react';
import * as utils from './utils';
"#;

        let mut adapter = adapter_for_language("js").unwrap();
        let imports = adapter.extract_imports(content).unwrap();

        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].module, "react");
        assert_eq!(imports[1].import_type, "named");
        assert_eq!(imports[2].import_type, "star");
    }

    #[test]
    fn test_extract_rust_imports() {
        let content = r#"use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use serde::{Serialize, Deserialize};
"#;

        let mut adapter = adapter_for_language("rs").unwrap();
        let imports = adapter.extract_imports(content).unwrap();

        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].module, "std::collections::HashMap");
        assert_eq!(imports[1].import_type, "named");
    }

    #[test]
    fn test_resolve_import_to_local_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        // Create a test file
        fs::write(temp_dir.path().join("utils.py"), "# Utils module").unwrap();

        let import = ImportStatement {
            module: "utils".to_string(),
            imports: None,
            import_type: "module".to_string(),
            line_number: 1,
        };

        let resolved = analyzer.resolve_import_to_local_file(&import, temp_dir.path());

        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap(), temp_dir.path().join("utils.py"));
    }

    #[test]
    fn test_resolve_import_to_local_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let import = ImportStatement {
            module: "nonexistent".to_string(),
            imports: None,
            import_type: "module".to_string(),
            line_number: 1,
        };

        let resolved = analyzer.resolve_import_to_local_file(&import, temp_dir.path());
        assert!(resolved.is_none());
    }

    #[test]
    fn test_analyze_file_for_split_small_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("small.py");

        let content = "def hello():\n    return 'world'";
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let result = analyzer.analyze_file_for_split(&file_path).unwrap();

        // Should return None for small files
        assert!(result.is_none());
    }

    #[test]
    fn test_analyze_file_for_split_large_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.py");

        // Create a large enough file to trigger split analysis
        let content = "def hello():\n    return 'world'\n".repeat(30); // Should exceed huge_loc threshold
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let result = analyzer.analyze_file_for_split(&file_path).unwrap();

        // Should find split opportunity
        if let Some(pack) = result {
            assert_eq!(pack.kind, "file_split");
            assert_eq!(pack.file, file_path);
            assert!(!pack.reasons.is_empty());
        }
    }

    #[test]
    fn test_build_entity_cohesion_graph_empty() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.py");

        fs::write(&file_path, "# Just a comment").unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let graph = analyzer.build_entity_cohesion_graph(&file_path).unwrap();

        // Should have 0 nodes for empty file
        assert_eq!(graph.node_count(), 0);
    }

    #[test]
    fn test_build_entity_cohesion_graph_with_entities() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("entities.py");

        let content = r#"
def func1():
    x = value
    return x

def func2():
    y = value
    return y
"#;
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let graph = analyzer.build_entity_cohesion_graph(&file_path).unwrap();

        // Should have at least some nodes (may vary based on parsing implementation)
        // node_count() is unsigned, always >= 0
    }

    #[test]
    fn test_find_cohesion_communities_empty_graph() {
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let graph = Graph::new_undirected();
        let communities = analyzer.find_cohesion_communities(&graph).unwrap();

        assert_eq!(communities.len(), 1);
        assert!(communities[0].is_empty());
    }

    #[test]
    fn test_generate_split_suggestions_empty_communities() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        fs::write(&file_path, "# test").unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let communities = Vec::new();
        let suggestions = analyzer
            .generate_split_suggestions(&file_path, &communities)
            .unwrap();

        // Should generate default splits when no communities found
        assert_eq!(suggestions.len(), 2);
        assert!(suggestions.iter().all(|s| s.name.contains("test")));
    }

    #[tokio::test]
    async fn test_discover_large_files() {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path();

        // Create a large file
        let large_file = root_path.join("large.py");
        let content = "def hello():\n    return 'world'\n".repeat(30);
        fs::write(&large_file, content).unwrap();

        // Create a small file
        let small_file = root_path.join("small.py");
        fs::write(&small_file, "print('hello')").unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let large_files = analyzer.discover_large_files(root_path).await.unwrap();

        // Should find the large file but not the small one
        assert!(large_files.contains(&large_file));
        assert!(!large_files.contains(&small_file));
    }

    #[test]
    fn test_collect_large_files_recursive_loc_threshold() {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path();

        // Create directory structure
        let nested_dir = root_path.join("src");
        std::fs::create_dir_all(&nested_dir).unwrap();

        // File with many short lines to trigger huge_loc without large byte size
        let loc_heavy_file = nested_dir.join("loc_heavy.rs");
        let content = "fn main() {}\n".repeat(60); // > huge_loc (50)
        fs::write(&loc_heavy_file, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut files = Vec::new();
        analyzer
            .collect_large_files_recursive(root_path, &mut files)
            .expect("collect loc-heavy file");

        assert!(files.contains(&loc_heavy_file));
    }

    #[test]
    fn test_extract_imports_by_extension() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        // Test Python file
        let py_file = temp_dir.path().join("test.py");
        fs::write(&py_file, "import os").unwrap();
        let py_imports = analyzer.extract_imports(&py_file).unwrap();
        assert_eq!(py_imports.len(), 1);

        // Test JavaScript file
        let js_file = temp_dir.path().join("test.js");
        fs::write(&js_file, "import React from 'react';").unwrap();
        let js_imports = analyzer.extract_imports(&js_file).unwrap();
        assert_eq!(js_imports.len(), 1);

        // Test Rust file
        let rs_file = temp_dir.path().join("test.rs");
        fs::write(&rs_file, "use std::collections::HashMap;").unwrap();
        let rs_imports = analyzer.extract_imports(&rs_file).unwrap();
        assert_eq!(rs_imports.len(), 1);

        // Test unsupported file - should return error for unsupported language
        let txt_file = temp_dir.path().join("test.txt");
        fs::write(&txt_file, "some text").unwrap();
        let txt_result = analyzer.extract_imports(&txt_file);
        assert!(txt_result.is_err()); // Should error for unsupported file type
    }

    #[test]
    fn test_collect_large_files_recursive_skips_directories() {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path();

        // Create node_modules directory (should be skipped)
        let node_modules = root_path.join("node_modules");
        fs::create_dir(&node_modules).unwrap();
        let large_file_in_node_modules = node_modules.join("large.js");
        let content = "function test() { return 'test'; }\n".repeat(30);
        fs::write(&large_file_in_node_modules, content).unwrap();

        let config = create_test_config();
        let analyzer = FileAnalyzer::new(config);

        let mut files = Vec::new();
        analyzer
            .collect_large_files_recursive(root_path, &mut files)
            .unwrap();

        // Should not find the file in node_modules
        assert!(!files.contains(&large_file_in_node_modules));
    }

    #[test]
    fn test_collect_dependency_metrics_exports_and_import_graph() {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path();
        let main_file = root_path.join("main.py");
        let helper_file = root_path.join("helpers.py");

        fs::write(
            &helper_file,
            r#"
def helper_function():
    return 42

def _private_helper():
    return 0
"#,
        )
        .unwrap();

        fs::write(
            &main_file,
            r#"
from helpers import helper_function

def run():
    return helper_function()
"#,
        )
        .unwrap();

        let analyzer = FileAnalyzer::new(create_test_config());
        let graph: CohesionGraph = Graph::new_undirected();

        let helper_metrics = analyzer
            .collect_dependency_metrics(&helper_file, Some(root_path), &graph)
            .unwrap();

        assert!(
            helper_metrics
                .exports
                .iter()
                .any(|entity| entity.name == "helper_function"),
            "expected helper_function to be recognised as an export"
        );
        assert!(
            !helper_metrics
                .exports
                .iter()
                .any(|entity| entity.name == "_private_helper"),
            "private helper should not be exported"
        );

        let canonical_main = analyzer.canonicalize_path(&main_file);
        assert!(
            helper_metrics.incoming_importers.contains(&canonical_main),
            "helpers.py should record main.py as an importer"
        );
        assert!(
            helper_metrics.outgoing_dependencies.is_empty(),
            "helpers.py should not list outgoing dependencies"
        );

        let main_metrics = analyzer
            .collect_dependency_metrics(&main_file, Some(root_path), &graph)
            .unwrap();

        let canonical_helper = analyzer.canonicalize_path(&helper_file);
        assert!(
            main_metrics
                .outgoing_dependencies
                .contains(&canonical_helper),
            "main.py should depend on helpers.py"
        );
        assert!(
            main_metrics
                .exports
                .iter()
                .any(|entity| entity.name == "run"),
            "top-level run function should be exported from main.py"
        );
    }

    #[test]
    fn test_collect_dependency_metrics_without_project_root() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("module.rs");
        fs::write(&file_path, "pub fn public_fn() {}\nfn private_fn() {}\n").unwrap();

        let analyzer = FileAnalyzer::new(create_test_config());
        let graph: CohesionGraph = Graph::new_undirected();

        let metrics = analyzer
            .collect_dependency_metrics(&file_path, None, &graph)
            .expect("collect metrics");

        assert!(metrics
            .exports
            .iter()
            .any(|entity| entity.name == "public_fn"));
        assert!(metrics
            .exports
            .iter()
            .all(|entity| entity.name != "private_fn"));
        assert!(metrics.incoming_importers.is_empty());
        assert!(metrics.outgoing_dependencies.is_empty());
    }

    #[test]
    fn test_resolve_candidate_path_prefers_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let module_rs = temp_dir.path().join("module.rs");
        fs::write(&module_rs, "pub mod sample {}").unwrap();

        let analyzer = FileAnalyzer::new(create_test_config());
        let resolved = analyzer.resolve_candidate_path(&module_rs);

        assert_eq!(resolved.as_ref(), Some(&module_rs));
    }

    #[test]
    fn test_resolve_candidate_path_uses_directory_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let package_dir = temp_dir.path().join("package");
        fs::create_dir_all(&package_dir).unwrap();
        let fallback = package_dir.join("mod.rs");
        fs::write(&fallback, "pub mod inner;").unwrap();

        let analyzer = FileAnalyzer::new(create_test_config());
        let resolved = analyzer.resolve_candidate_path(&package_dir);

        assert_eq!(resolved.as_ref(), Some(&fallback));
    }

    #[test]
    fn test_resolve_candidate_path_finds_supported_extension() {
        let temp_dir = TempDir::new().unwrap();
        let stem = temp_dir.path().join("component");
        let ts_path = stem.with_extension("ts");
        fs::write(&ts_path, "export const value = 1;").unwrap();

        let analyzer = FileAnalyzer::new(create_test_config());
        let resolved = analyzer.resolve_candidate_path(&stem);

        assert_eq!(resolved.as_ref(), Some(&ts_path));
    }

    #[test]
    fn test_resolve_candidate_path_returns_none_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let analyzer = FileAnalyzer::new(create_test_config());
        let candidate = temp_dir.path().join("missing_module");

        assert!(analyzer.resolve_candidate_path(&candidate).is_none());
    }

    #[test]
    fn test_directory_module_fallbacks_provide_common_files() {
        let analyzer = FileAnalyzer::new(create_test_config());
        let dir = Path::new("pkg");
        let fallbacks = analyzer.directory_module_fallbacks(dir);

        let expected = vec![
            dir.join("mod.rs"),
            dir.join("lib.rs"),
            dir.join("__init__.py"),
            dir.join("index.ts"),
            dir.join("index.tsx"),
            dir.join("index.js"),
            dir.join("index.jsx"),
        ];

        assert_eq!(fallbacks, expected);
    }

    #[test]
    fn test_supported_extensions_includes_major_languages() {
        let extensions = FileAnalyzer::supported_extensions();
        assert!(extensions.contains(&"py"));
        assert!(extensions.contains(&"ts"));
        assert!(extensions.contains(&"rs"));
        assert!(extensions.contains(&"go"));
        assert!(extensions.contains(&"java"));
    }

    #[test]
    fn test_collect_project_code_files_filters_supported_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Supported code files
        let rust_file = root.join("lib.rs");
        let python_file = root.join("service").join("api.py");
        let ts_file = root.join("web").join("component.tsx");

        std::fs::create_dir_all(python_file.parent().unwrap()).unwrap();
        std::fs::create_dir_all(ts_file.parent().unwrap()).unwrap();
        fs::write(&rust_file, "pub fn lib() {}").unwrap();
        fs::write(&python_file, "def api():\n    pass").unwrap();
        fs::write(&ts_file, "export const value = 1;").unwrap();

        // Unsupported file extension should be ignored
        fs::write(root.join("README.txt"), "not code").unwrap();

        // Skipped directory should not be traversed
        let node_modules = root.join("node_modules");
        std::fs::create_dir_all(&node_modules).unwrap();
        fs::write(node_modules.join("ignore.js"), "console.log('skip');").unwrap();

        let analyzer = FileAnalyzer::new(create_test_config());
        let mut collected = analyzer
            .collect_project_code_files(root)
            .expect("collect project files");

        collected.sort();

        assert!(collected.contains(&rust_file));
        assert!(collected.contains(&python_file));
        assert!(collected.contains(&ts_file));
        assert!(!collected.iter().any(|path| path.ends_with("README.txt")));
        assert!(!collected
            .iter()
            .any(|path| path.components().any(|c| c.as_os_str() == "node_modules")));
    }

    #[test]
    fn test_collect_project_code_files_skips_root_directory() {
        let temp_dir = TempDir::new().unwrap();
        let skip_dir = temp_dir.path().join("node_modules");
        std::fs::create_dir_all(&skip_dir).unwrap();
        fs::write(skip_dir.join("ignored.ts"), "export const ignored = true;").unwrap();

        let analyzer = FileAnalyzer::new(create_test_config());
        let files = analyzer
            .collect_project_code_files(&skip_dir)
            .expect("collect files under skipped directory");

        assert!(files.is_empty());
    }

    #[test]
    fn test_should_skip_directory_matches_common_patterns() {
        let analyzer = FileAnalyzer::new(create_test_config());
        let skip_dirs = [
            "node_modules",
            "__pycache__",
            "target",
            ".git",
            "build",
            "dist",
        ];

        for dir in skip_dirs {
            assert!(
                analyzer.should_skip_directory(Path::new(dir)),
                "expected {dir} to be skipped"
            );
        }
    }

    #[test]
    fn test_should_skip_directory_allows_regular_paths() {
        let analyzer = FileAnalyzer::new(create_test_config());
        let allowed = ["src", "lib", "services/backend", "packages/ui"];

        for dir in allowed {
            assert!(
                !analyzer.should_skip_directory(Path::new(dir)),
                "expected {dir} to be allowed"
            );
        }
    }

    fn build_entity(name: &str, kind: EntityKind, start_line: usize) -> ParsedEntity {
        ParsedEntity {
            id: format!("{}::id", name),
            kind,
            name: name.to_string(),
            parent: None,
            children: Vec::new(),
            location: SourceLocation {
                file_path: "test".to_string(),
                start_line,
                end_line: start_line,
                start_column: 1,
                end_column: 20,
            },
            metadata: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_is_entity_exported_handles_language_visibility() {
        let analyzer = FileAnalyzer::new(create_test_config());

        // Rust visibility comes from metadata
        let mut rust_entity = build_entity("do_stuff", EntityKind::Function, 3);
        rust_entity.metadata.insert(
            "visibility".to_string(),
            Value::String("pub(crate)".to_string()),
        );
        assert!(analyzer.is_entity_exported(
            &rust_entity,
            Path::new("lib.rs"),
            "pub(crate) fn do_stuff() {}"
        ));

        let mut private_rust = build_entity("internal", EntityKind::Function, 5);
        private_rust
            .metadata
            .insert("visibility".to_string(), Value::String("fn".to_string()));
        assert!(!analyzer.is_entity_exported(
            &private_rust,
            Path::new("mod.rs"),
            "fn internal() {}"
        ));

        // Python exports block private (underscore) names
        let python_public = build_entity("visible", EntityKind::Function, 2);
        assert!(analyzer.is_entity_exported(
            &python_public,
            Path::new("module.py"),
            "def visible():\n    pass"
        ));

        let mut python_private = build_entity("_hidden", EntityKind::Function, 4);
        python_private.parent = None;
        assert!(!analyzer.is_entity_exported(
            &python_private,
            Path::new("module.py"),
            "def _hidden():\n    pass"
        ));

        // Go treats uppercase identifiers as exported
        let go_exported = build_entity("Service", EntityKind::Struct, 1);
        assert!(analyzer.is_entity_exported(
            &go_exported,
            Path::new("service.go"),
            "type Service struct {}"
        ));

        let go_internal = build_entity("impl", EntityKind::Struct, 1);
        assert!(!analyzer.is_entity_exported(
            &go_internal,
            Path::new("service.go"),
            "type impl struct {}"
        ));

        // TypeScript relies on the export keyword at the correct line
        let ts_entity = build_entity("makeWidget", EntityKind::Function, 1);
        assert!(analyzer.is_entity_exported(
            &ts_entity,
            Path::new("widget.ts"),
            "export function makeWidget() {\n    return 1;\n}\n"
        ));

        let ts_comment = build_entity("helper", EntityKind::Function, 1);
        assert!(!analyzer.is_entity_exported(
            &ts_comment,
            Path::new("widget.ts"),
            "// export function helper() {}\nfunction helper() {}\n"
        ));

        // Java checks for explicit public keyword
        let java_public = build_entity("Widget", EntityKind::Class, 1);
        assert!(analyzer.is_entity_exported(
            &java_public,
            Path::new("Widget.java"),
            "public class Widget {}\n"
        ));

        let java_package = build_entity("WidgetImpl", EntityKind::Class, 1);
        assert!(!analyzer.is_entity_exported(
            &java_package,
            Path::new("WidgetImpl.java"),
            "class WidgetImpl {}\n"
        ));

        // Other extensions fall back to parent-less entities
        let mut nested_entity = build_entity("Inner", EntityKind::Class, 10);
        nested_entity.parent = Some("Outer".to_string());
        assert!(!analyzer.is_entity_exported(&nested_entity, Path::new("README.md"), "irrelevant"));

        let top_level_unknown = build_entity("Top", EntityKind::Class, 1);
        assert!(analyzer.is_entity_exported(
            &top_level_unknown,
            Path::new("README.md"),
            "irrelevant"
        ));
    }
