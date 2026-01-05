    use super::*;
    use super::partitioning::GraphPartitioner;
    use super::reorganization::ReorganizationPlanner;
    use crate::detectors::structure::config::{
        EntityHealthConfig, FsDirectoryConfig, FsFileConfig, PartitioningConfig, StructureConfig,
        StructureToggles,
    };
    use crate::lang::registry::adapter_for_language;
    use petgraph::graph::Graph;
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
                huge_loc: 800,
                huge_bytes: 128_000,
                min_split_loc: 200,
                min_entities_per_split: 3,
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

    fn setup_test_directory() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create test files with different sizes
        fs::write(dir_path.join("small.py"), "# Small file\nprint('hello')").unwrap();
        fs::write(dir_path.join("medium.py"), "# Medium file\n".repeat(50)).unwrap();
        fs::write(dir_path.join("large.py"), "# Large file\n".repeat(200)).unwrap();
        fs::write(
            dir_path.join("test.js"),
            "// JavaScript file\nconsole.log('test');",
        )
        .unwrap();
        fs::write(
            dir_path.join("app.rs"),
            "// Rust file\nfn main() { println!(\"Hello\"); }",
        )
        .unwrap();

        // Create subdirectory
        fs::create_dir(dir_path.join("subdir")).unwrap();
        fs::write(dir_path.join("subdir/nested.py"), "# Nested file").unwrap();

        temp_dir
    }

    #[test]
    fn test_directory_analyzer_new() {
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config.clone());

        assert_eq!(
            analyzer.config.fsdir.max_files_per_dir,
            config.fsdir.max_files_per_dir
        );
        assert!(analyzer.metrics_cache.is_empty());
    }

    #[test]
    fn test_is_code_file() {
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        assert!(analyzer.is_code_file("py"));
        assert!(analyzer.is_code_file("js"));
        assert!(analyzer.is_code_file("ts"));
        assert!(analyzer.is_code_file("rs"));
        assert!(!analyzer.is_code_file("txt"));
        assert!(!analyzer.is_code_file("md"));
    }

    #[test]
    fn test_distribution_score_at_optimal() {
        // Score should be 1.0 when value equals optimal
        let score = calculate_distribution_score(7, 7, 2.0);
        assert!((score - 1.0).abs() < 0.0001, "Score at optimal should be 1.0");
    }

    #[test]
    fn test_distribution_score_one_stddev_away() {
        // At 1 stddev away, score should be exp(-0.5) ≈ 0.6065
        let score_above = calculate_distribution_score(9, 7, 2.0); // 7 + 2 = 9
        let score_below = calculate_distribution_score(5, 7, 2.0); // 7 - 2 = 5
        let expected = (-0.5_f64).exp();

        assert!(
            (score_above - expected).abs() < 0.0001,
            "Score at +1 stddev should be ~0.6065"
        );
        assert!(
            (score_below - expected).abs() < 0.0001,
            "Score at -1 stddev should be ~0.6065"
        );
    }

    #[test]
    fn test_distribution_score_two_stddev_away() {
        // At 2 stddev away, score should be exp(-2) ≈ 0.1353
        let score = calculate_distribution_score(11, 7, 2.0); // 7 + 4 = 11
        let expected = (-2.0_f64).exp();

        assert!(
            (score - expected).abs() < 0.0001,
            "Score at +2 stddev should be ~0.1353"
        );
    }

    #[test]
    fn test_distribution_score_zero_stddev() {
        // With zero stddev, only exact match should score 1.0
        assert_eq!(calculate_distribution_score(7, 7, 0.0), 1.0);
        assert_eq!(calculate_distribution_score(8, 7, 0.0), 0.0);
    }

    #[test]
    fn test_directory_metrics_include_distribution_scores() {
        let temp_dir = setup_test_directory();
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        let metrics = analyzer
            .calculate_directory_metrics(temp_dir.path())
            .unwrap();

        // Scores should be in valid range [0, 1]
        assert!(metrics.file_count_score >= 0.0 && metrics.file_count_score <= 1.0);
        assert!(metrics.subdir_count_score >= 0.0 && metrics.subdir_count_score <= 1.0);
    }

    #[test]
    fn test_count_lines_of_code() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");

        let content = r#"# Comment line
import os

def hello():
    print("Hello world")
    # Another comment
    return True

    # Empty line above
"#;
        fs::write(&file_path, content).unwrap();

        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);
        let loc = analyzer.count_lines_of_code(&file_path).unwrap();

        // Should count non-empty, non-comment lines
        assert!(loc > 0);
        assert!(loc < content.lines().count()); // Less than total lines due to comments
    }

    #[test]
    fn test_gather_directory_stats() {
        let temp_dir = setup_test_directory();
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        let (files, subdirs, loc_distribution) =
            analyzer.gather_directory_stats(temp_dir.path()).unwrap();

        assert_eq!(files, 5); // 5 code files
        assert_eq!(subdirs, 1); // 1 subdirectory
        assert_eq!(loc_distribution.len(), 5);
        assert!(loc_distribution.iter().all(|&loc| loc > 0));
    }

    #[test]
    fn test_calculate_gini_coefficient_empty() {
        let gini = calculate_gini_coefficient(&[]);
        assert_eq!(gini, 0.0);
    }

    #[test]
    fn test_calculate_gini_coefficient_single_value() {
        let gini = calculate_gini_coefficient(&[100]);
        assert_eq!(gini, 0.0);
    }

    #[test]
    fn test_calculate_gini_coefficient_equal_values() {
        let gini = calculate_gini_coefficient(&[50, 50, 50, 50]);
        assert!(gini < 0.1); // Should be close to 0 for equal distribution
    }

    #[test]
    fn test_calculate_gini_coefficient_unequal_values() {
        let gini = calculate_gini_coefficient(&[10, 20, 30, 100]);
        assert!(gini > 0.1); // Should be higher for unequal distribution
    }

    #[test]
    fn test_calculate_entropy_empty() {
        let entropy = calculate_entropy(&[]);
        assert_eq!(entropy, 0.0);
    }

    #[test]
    fn test_calculate_entropy_single_value() {
        let entropy = calculate_entropy(&[100]);
        assert_eq!(entropy, 0.0);
    }

    #[test]
    fn test_calculate_entropy_equal_values() {
        let entropy = calculate_entropy(&[25, 25, 25, 25]);
        assert!(entropy > 1.0); // Should be high for uniform distribution
    }

    #[test]
    fn test_calculate_size_normalization_factor() {
        let factor1 = calculate_size_normalization_factor(5, 500);
        let factor2 = calculate_size_normalization_factor(10, 1000);
        let factor3 = calculate_size_normalization_factor(20, 2000);

        // Normalization factor should be within reasonable range
        assert!(factor1 >= 0.5 && factor1 <= 1.5);
        assert!(factor2 >= 0.5 && factor2 <= 1.5);
        assert!(factor3 >= 0.5 && factor3 <= 1.5);
    }

    #[test]
    fn test_calculate_directory_metrics() {
        let temp_dir = setup_test_directory();
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        let metrics = analyzer
            .calculate_directory_metrics(temp_dir.path())
            .unwrap();

        assert_eq!(metrics.files, 5);
        assert_eq!(metrics.subdirs, 1);
        assert!(metrics.loc > 0);
        assert!(metrics.gini >= 0.0 && metrics.gini <= 1.0);
        assert!(metrics.entropy >= 0.0);
        assert!(metrics.file_pressure >= 0.0 && metrics.file_pressure <= 1.0);
        assert!(metrics.branch_pressure >= 0.0 && metrics.branch_pressure <= 1.0);
        assert!(metrics.size_pressure >= 0.0 && metrics.size_pressure <= 1.0);
        assert!(metrics.dispersion >= 0.0 && metrics.dispersion <= 1.0);
        assert!(metrics.imbalance >= 0.0);
    }

    #[test]
    fn test_calculate_directory_metrics_caching() {
        let temp_dir = setup_test_directory();
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        // First call
        let metrics1 = analyzer
            .calculate_directory_metrics(temp_dir.path())
            .unwrap();

        // Second call should return cached result
        let metrics2 = analyzer
            .calculate_directory_metrics(temp_dir.path())
            .unwrap();

        assert_eq!(metrics1.files, metrics2.files);
        assert_eq!(metrics1.subdirs, metrics2.subdirs);
        assert_eq!(metrics1.loc, metrics2.loc);
        assert!(!analyzer.metrics_cache.is_empty());
    }

    #[test]
    fn test_should_skip_directory() {
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);
        let root = Path::new("/project");

        assert!(analyzer.should_skip_directory(Path::new("node_modules"), root));
        assert!(analyzer.should_skip_directory(Path::new("target"), root));
        assert!(analyzer.should_skip_directory(Path::new(".git"), root));
        assert!(analyzer.should_skip_directory(Path::new("__pycache__"), root));
        assert!(!analyzer.should_skip_directory(Path::new("src"), root));
        assert!(!analyzer.should_skip_directory(Path::new("lib"), root));
    }

    #[test]
    fn test_extract_python_imports_basic() {
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
        assert!(imports[2]
            .imports
            .as_ref()
            .unwrap()
            .contains(&"Path".to_string()));
    }

    #[test]
    fn test_extract_python_imports_star_import() {
        let content = "from module import *";
        let mut adapter = adapter_for_language("py").unwrap();
        let imports = adapter.extract_imports(content).unwrap();

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].import_type, "star");
        assert!(imports[0].imports.is_none());
    }

    #[test]
    fn test_extract_javascript_imports_basic() {
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
    fn test_extract_rust_imports_basic() {
        let content = r#"use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use serde::{Serialize, Deserialize};
"#;

        let mut adapter = adapter_for_language("rs").unwrap();
        let imports = adapter.extract_imports(content).unwrap();

        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].module, "std::collections::HashMap");
        assert_eq!(imports[0].import_type, "module");

        assert_eq!(imports[1].module, "std::fs::");
        assert_eq!(imports[1].import_type, "named");
        assert!(imports[1]
            .imports
            .as_ref()
            .unwrap()
            .contains(&"File".to_string()));
    }

    #[test]
    fn test_generate_partition_name_with_common_tokens() {
        let files = vec![
            PathBuf::from("user_service.py"),
            PathBuf::from("user_model.py"),
            PathBuf::from("user_controller.py"),
        ];

        let fallbacks = vec!["core".to_string(), "utils".to_string()];
        let name = partitioning::generate_partition_name(&files, 0, &fallbacks);
        assert_eq!(name, "user");
    }

    #[test]
    fn test_generate_partition_name_fallback() {
        let files = vec![PathBuf::from("a.py"), PathBuf::from("b.py")];

        let fallbacks = vec!["core".to_string(), "utils".to_string()];
        let name = partitioning::generate_partition_name(&files, 0, &fallbacks);
        assert_eq!(name, "core"); // First fallback name
    }

    #[test]
    fn test_calculate_cut_size_simple() {
        let config = create_test_config();
        let partitioner = GraphPartitioner::new(&config);

        // Create a simple graph for testing
        let mut graph: DependencyGraph = petgraph::Graph::new();
        let node1 = graph.add_node(FileNode {
            path: PathBuf::from("file1.py"),
            loc: 100,
            size_bytes: 1000,
        });
        let node2 = graph.add_node(FileNode {
            path: PathBuf::from("file2.py"),
            loc: 200,
            size_bytes: 2000,
        });

        graph.add_edge(
            node1,
            node2,
            DependencyEdge {
                weight: 3,
                relationship_type: "import".to_string(),
            },
        );

        let part1 = vec![node1];
        let part2 = vec![node2];

        let cut_size = partitioner.calculate_cut_size(&graph, &part1, &part2);
        assert_eq!(cut_size, 3);
    }

    #[test]
    fn test_partition_distributes_nodes() {
        let config = create_test_config();
        let partitioner = GraphPartitioner::new(&config);

        // Create test node indices
        let mut graph: DependencyGraph = petgraph::Graph::new();
        let _nodes: Vec<_> = (0..6)
            .map(|i| {
                graph.add_node(FileNode {
                    path: PathBuf::from(format!("file{}.py", i)),
                    loc: 100,
                    size_bytes: 1000,
                })
            })
            .collect();

        let metrics = DirectoryMetrics {
            files: 6,
            subdirs: 0,
            loc: 600,
            gini: 0.0,
            entropy: 2.6,
            file_pressure: 0.3,
            branch_pressure: 0.0,
            size_pressure: 0.3,
            dispersion: 0.0,
            file_count_score: 0.7,
            subdir_count_score: 0.5,
            imbalance: 0.3,
        };

        let partitions = partitioner.partition_directory(&graph, &metrics).unwrap();
        let total_files: usize = partitions.iter().map(|p| p.files.len()).sum();
        assert_eq!(total_files, 6);
    }

    #[tokio::test]
    async fn test_discover_directories() {
        let temp_dir = TempDir::new().unwrap();
        let root_path = temp_dir.path();

        // Create nested directory structure
        fs::create_dir(root_path.join("src")).unwrap();
        fs::create_dir(root_path.join("src/lib")).unwrap();
        fs::create_dir(root_path.join("tests")).unwrap();
        fs::create_dir(root_path.join("node_modules")).unwrap(); // Should be skipped

        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        let directories = analyzer.discover_directories(root_path).await.unwrap();

        // Should find src, src/lib, and tests, but not node_modules
        assert!(directories.len() >= 3);
        assert!(directories.iter().any(|d| d.file_name().unwrap() == "src"));
        assert!(directories
            .iter()
            .any(|d| d.file_name().unwrap() == "tests"));
        assert!(!directories
            .iter()
            .any(|d| d.file_name().unwrap() == "node_modules"));
    }

    #[test]
    fn test_analyze_directory_for_reorg_low_imbalance() {
        let temp_dir = setup_test_directory();
        let mut config = create_test_config();
        // Set very high thresholds so imbalance will be low
        config.fsdir.max_files_per_dir = 1000;
        config.fsdir.max_dir_loc = 100000;

        let analyzer = DirectoryAnalyzer::new(config);

        let result = analyzer
            .analyze_directory_for_reorg(temp_dir.path())
            .unwrap();

        // Should return None due to low imbalance
        assert!(result.is_none());
    }

    #[test]
    fn test_calculate_reorganization_effort() {
        let config = create_test_config();
        let planner = ReorganizationPlanner::new(&config);

        let partitions = vec![
            DirectoryPartition {
                name: "partition1".to_string(),
                files: vec![PathBuf::from("file1.py"), PathBuf::from("file2.py")],
                loc: 200,
            },
            DirectoryPartition {
                name: "partition2".to_string(),
                files: vec![PathBuf::from("file3.py")],
                loc: 100,
            },
        ];

        let effort = planner.calculate_reorganization_effort(&partitions).unwrap();

        assert_eq!(effort.files_moved, 3);
        assert_eq!(effort.import_updates_est, 6); // 2 * files_moved
    }

    #[test]
    fn test_generate_file_moves() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        let planner = ReorganizationPlanner::new(&config);

        let partitions = vec![DirectoryPartition {
            name: "core".to_string(),
            files: vec![
                temp_dir.path().join("file1.py"),
                temp_dir.path().join("file2.py"),
            ],
            loc: 200,
        }];

        let moves = planner
            .generate_file_moves(&partitions, temp_dir.path())
            .unwrap();

        assert_eq!(moves.len(), 2);
        assert!(moves[0].to.starts_with(temp_dir.path().join("core")));
        assert!(moves[1].to.starts_with(temp_dir.path().join("core")));
    }

    #[test]
    fn test_resolve_import_to_local_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

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
        let analyzer = DirectoryAnalyzer::new(config);

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
    fn test_resolve_import_relative_import_skipped() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        let import = ImportStatement {
            module: ".relative_module".to_string(),
            imports: None,
            import_type: "module".to_string(),
            line_number: 1,
        };

        let resolved = analyzer.resolve_import_to_local_file(&import, temp_dir.path());
        assert!(resolved.is_none()); // Relative imports are skipped
    }

    #[test]
    fn test_calculate_gini_coefficient_large_array_parallel() {
        // Create array with >= 32 elements to trigger parallel computation
        let values: Vec<usize> = (1..50).collect();
        let gini = calculate_gini_coefficient(&values);

        assert!(gini >= 0.0 && gini <= 1.0);
        assert!(gini > 0.1); // Should show some inequality
    }

    #[test]
    fn test_calculate_gini_coefficient_sum_zero() {
        let gini = calculate_gini_coefficient(&[0, 0, 0, 0]);
        assert_eq!(gini, 0.0);
    }

    #[test]
    fn test_calculate_entropy_large_array_parallel() {
        // Create array with >= 100 elements to trigger parallel computation
        let values: Vec<usize> = (1..150).collect();
        let entropy = calculate_entropy(&values);

        assert!(entropy > 0.0);
    }

    #[test]
    fn test_calculate_entropy_total_zero() {
        let entropy = calculate_entropy(&[0, 0, 0, 0]);
        assert_eq!(entropy, 0.0);
    }

    #[test]
    fn test_analyze_directory_for_reorg_meets_conditions() {
        // Create a directory with multiple files to ensure imbalance and meet size requirements
        let temp_dir = TempDir::new().unwrap();

        // Create files with extreme imbalance to ensure imbalance >= 0.6
        let files = [
            ("file1.py", "# Very large file\n".repeat(100)), // 100 lines
            ("file2.py", "# Tiny file\npass\n".to_string()), // 2 lines
            ("file3.py", "# Small file\npass\n".to_string()), // 2 lines
            ("file4.py", "# Small file\npass\n".to_string()), // 2 lines
            ("file5.py", "# Small file\npass\n".to_string()), // 2 lines
            ("file6.py", "# Small file\npass\n".to_string()), // 2 lines
        ];

        for (name, content) in &files {
            std::fs::write(temp_dir.path().join(name), content).unwrap();
        }

        let mut config = create_test_config();
        // Set thresholds to ensure conditions are met
        config.fsdir.max_files_per_dir = 4; // Less than 6 files created
        config.fsdir.max_dir_loc = 90; // Less than total LOC (~110)

        let analyzer = DirectoryAnalyzer::new(config);

        let result = analyzer
            .analyze_directory_for_reorg(temp_dir.path())
            .unwrap();

        // Should return Some since conditions are met (high imbalance from mixed file sizes)
        assert!(result.is_some());
        let reorg_pack = result.unwrap();
        assert!(!reorg_pack.proposal.is_empty());
    }

    #[test]
    fn test_analyze_directory_for_reorg_small_directory_skipped() {
        let temp_dir = TempDir::new().unwrap();
        // Create a very small directory
        fs::write(
            temp_dir.path().join("small.py"),
            "# Small file\nprint('hi')",
        )
        .unwrap();

        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        let result = analyzer
            .analyze_directory_for_reorg(temp_dir.path())
            .unwrap();

        // Should return None for small directory
        assert!(result.is_none());
    }

    #[test]
    fn test_build_dependency_graph_basic() {
        let temp_dir = TempDir::new().unwrap();

        // Create files with imports
        fs::write(
            temp_dir.path().join("main.py"),
            "import utils\nfrom helpers import helper",
        )
        .unwrap();
        fs::write(temp_dir.path().join("utils.py"), "def utility(): pass").unwrap();
        fs::write(temp_dir.path().join("helpers.py"), "def helper(): pass").unwrap();

        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        let graph = analyzer.build_dependency_graph(temp_dir.path()).unwrap();

        assert!(graph.node_count() > 0);
        // Graph may have edges if imports are resolved - no need to check >= 0 for unsigned
    }

    #[test]
    fn test_build_dependency_graph_records_edges() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("main.py"),
            "import helpers\nfrom helpers import helper\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("helpers.py"),
            "def helper():\n    return 42\n",
        )
        .unwrap();

        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        let graph = analyzer.build_dependency_graph(temp_dir.path()).unwrap();
        assert_eq!(graph.node_count(), 2);
        assert!(
            graph.edge_count() > 0,
            "expected at least one dependency edge between modules"
        );
    }

    #[test]
    fn test_partition_directory_with_real_files() {
        let temp_dir = setup_test_directory();
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config.clone());

        let graph = analyzer.build_dependency_graph(temp_dir.path()).unwrap();
        let metrics = analyzer
            .calculate_directory_metrics(temp_dir.path())
            .unwrap();

        let partitioner = GraphPartitioner::new(&config);
        let partitions = partitioner.partition_directory(&graph, &metrics).unwrap();

        assert!(!partitions.is_empty());
        assert!(partitions.iter().all(|p| !p.files.is_empty()));
    }

    #[test]
    fn test_partition_directory_basic() {
        let config = create_test_config();
        let partitioner = GraphPartitioner::new(&config);

        let mut graph: DependencyGraph = Graph::new();
        let _nodes: Vec<_> = (0..4)
            .map(|i| {
                graph.add_node(FileNode {
                    path: PathBuf::from(format!("file{i}.py")),
                    loc: 10,
                    size_bytes: 100,
                })
            })
            .collect();

        let metrics = DirectoryMetrics {
            files: 4,
            subdirs: 0,
            loc: 40,
            gini: 0.0,
            entropy: 2.0,
            file_pressure: 0.2,
            branch_pressure: 0.0,
            size_pressure: 0.02,
            dispersion: 0.0,
            file_count_score: 0.8,
            subdir_count_score: 0.5,
            imbalance: 0.3,
        };

        let partitions = partitioner.partition_directory(&graph, &metrics).unwrap();
        assert!(!partitions.is_empty());
    }

    #[test]
    fn test_partition_preserves_nodes() {
        let mut config = create_test_config();
        config.partitioning.balance_tolerance = 0.5; // Allow more tolerance

        let mut graph: DependencyGraph = Graph::new();
        let _node_a = graph.add_node(FileNode {
            path: PathBuf::from("a.py"),
            loc: 10,
            size_bytes: 100,
        });
        let _node_b = graph.add_node(FileNode {
            path: PathBuf::from("b.py"),
            loc: 40,
            size_bytes: 400,
        });

        let metrics = DirectoryMetrics {
            files: 2,
            subdirs: 0,
            loc: 50,
            gini: 0.3,
            entropy: 1.0,
            file_pressure: 0.1,
            branch_pressure: 0.0,
            size_pressure: 0.025,
            dispersion: 0.3,
            file_count_score: 0.9,
            subdir_count_score: 0.5,
            imbalance: 0.2,
        };

        let partitioner = GraphPartitioner::new(&config);
        let partitions = partitioner.partition_directory(&graph, &metrics).unwrap();
        let total_files: usize = partitions.iter().map(|p| p.files.len()).sum();
        assert_eq!(total_files, 2);
    }

    #[test]
    fn test_calculate_cut_size() {
        let config = create_test_config();
        let partitioner = GraphPartitioner::new(&config);
        let mut graph: DependencyGraph = Graph::new();

        let node_a = graph.add_node(FileNode {
            path: PathBuf::from("a.py"),
            loc: 10,
            size_bytes: 100,
        });
        let node_b = graph.add_node(FileNode {
            path: PathBuf::from("b.py"),
            loc: 12,
            size_bytes: 120,
        });
        let node_c = graph.add_node(FileNode {
            path: PathBuf::from("c.py"),
            loc: 15,
            size_bytes: 150,
        });
        let node_d = graph.add_node(FileNode {
            path: PathBuf::from("d.py"),
            loc: 18,
            size_bytes: 180,
        });

        // Add edges between partitions
        graph.add_edge(
            node_a,
            node_c,
            DependencyEdge {
                weight: 3,
                relationship_type: "module".to_string(),
            },
        );
        graph.add_edge(
            node_b,
            node_d,
            DependencyEdge {
                weight: 2,
                relationship_type: "module".to_string(),
            },
        );

        let part1 = vec![node_a, node_b];
        let part2 = vec![node_c, node_d];

        let cut_size = partitioner.calculate_cut_size(&graph, &part1, &part2);
        assert_eq!(cut_size, 5); // 3 + 2 edges crossing
    }

    #[test]
    fn test_partition_empty_graph() {
        let config = create_test_config();
        let partitioner = GraphPartitioner::new(&config);

        let graph: DependencyGraph = petgraph::Graph::new();
        let metrics = DirectoryMetrics {
            files: 0,
            subdirs: 0,
            loc: 0,
            gini: 0.0,
            entropy: 0.0,
            file_pressure: 0.0,
            branch_pressure: 0.0,
            size_pressure: 0.0,
            dispersion: 0.0,
            file_count_score: 1.0,
            subdir_count_score: 1.0,
            imbalance: 0.0,
        };

        let result = partitioner.partition_directory(&graph, &metrics).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_partition_small_graph() {
        let config = create_test_config();
        let partitioner = GraphPartitioner::new(&config);

        // Create test node indices
        let mut graph: DependencyGraph = petgraph::Graph::new();
        let _nodes: Vec<_> = (0..4)
            .map(|i| {
                graph.add_node(FileNode {
                    path: PathBuf::from(format!("file{}.py", i)),
                    loc: 100,
                    size_bytes: 1000,
                })
            })
            .collect();

        let metrics = DirectoryMetrics {
            files: 4,
            subdirs: 0,
            loc: 400,
            gini: 0.0,
            entropy: 2.0,
            file_pressure: 0.2,
            branch_pressure: 0.0,
            size_pressure: 0.2,
            dispersion: 0.0,
            file_count_score: 0.8,
            subdir_count_score: 0.5,
            imbalance: 0.3,
        };

        let partitions = partitioner.partition_directory(&graph, &metrics).unwrap();
        let total_files: usize = partitions.iter().map(|p| p.files.len()).sum();
        assert_eq!(total_files, 4);
    }

    #[test]
    fn test_partition_balanced_bipartition() {
        let config = create_test_config();
        let partitioner = GraphPartitioner::new(&config);

        // Create a simple connected graph
        let mut graph: DependencyGraph = petgraph::Graph::new();
        let node1 = graph.add_node(FileNode {
            path: PathBuf::from("file1.py"),
            loc: 100,
            size_bytes: 1000,
        });
        let node2 = graph.add_node(FileNode {
            path: PathBuf::from("file2.py"),
            loc: 100,
            size_bytes: 1000,
        });
        let node3 = graph.add_node(FileNode {
            path: PathBuf::from("file3.py"),
            loc: 100,
            size_bytes: 1000,
        });

        graph.add_edge(
            node1,
            node2,
            DependencyEdge {
                weight: 1,
                relationship_type: "import".to_string(),
            },
        );

        let metrics = DirectoryMetrics {
            files: 3,
            subdirs: 0,
            loc: 300,
            gini: 0.0,
            entropy: 1.58,
            file_pressure: 0.15,
            branch_pressure: 0.0,
            size_pressure: 0.15,
            dispersion: 0.0,
            file_count_score: 0.85,
            subdir_count_score: 0.5,
            imbalance: 0.25,
        };

        let partitions = partitioner.partition_directory(&graph, &metrics).unwrap();
        let total_files: usize = partitions.iter().map(|p| p.files.len()).sum();
        assert_eq!(total_files, 3);
    }

    #[test]
    fn test_extract_imports_by_extension() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        let analyzer = DirectoryAnalyzer::new(config);

        // Test Python file
        let py_file = temp_dir.path().join("test.py");
        fs::write(&py_file, "import os\nfrom sys import path").unwrap();

        let imports = analyzer.extract_imports(&py_file).unwrap();
        assert_eq!(imports.len(), 2);

        // Test JavaScript file
        let js_file = temp_dir.path().join("test.js");
        fs::write(
            &js_file,
            "import React from 'react';\nimport {useState} from 'react';",
        )
        .unwrap();

        let imports = analyzer.extract_imports(&js_file).unwrap();
        assert_eq!(imports.len(), 2);

        // Test Rust file
        let rs_file = temp_dir.path().join("test.rs");
        fs::write(
            &rs_file,
            "use std::collections::HashMap;\nuse serde::Serialize;",
        )
        .unwrap();

        let imports = analyzer.extract_imports(&rs_file).unwrap();
        assert_eq!(imports.len(), 2);

        // Test unsupported extension
        let txt_file = temp_dir.path().join("test.txt");
        fs::write(&txt_file, "Some text content").unwrap();

        let imports = analyzer.extract_imports(&txt_file).unwrap();
        assert!(imports.is_empty());
    }

    // Note: test_estimate_cross_edges_reduced was removed as the method
    // is now internal to the reorganization module
