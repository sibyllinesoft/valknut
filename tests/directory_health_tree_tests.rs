//! Comprehensive tests for DirectoryHealthTree functionality
//!
//! This test suite covers all aspects of the directory health tree implementation
//! to achieve >85% test coverage as required.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use valknut_rs::api::results::{
    AnalysisResults, AnalysisStatistics, AnalysisSummary, DirectoryHealthTree, FeatureContribution,
    MemoryStats, RefactoringCandidate, RefactoringIssue,
};
use valknut_rs::core::scoring::Priority;

/// Helper function to create a test refactoring candidate
fn create_test_candidate(
    entity_id: &str,
    name: &str,
    file_path: &str,
    priority: Priority,
    score: f64,
    confidence: f64,
    issues: Vec<RefactoringIssue>,
) -> RefactoringCandidate {
    RefactoringCandidate {
        entity_id: entity_id.to_string(),
        name: name.to_string(),
        file_path: file_path.to_string(),
        line_range: Some((1, 100)),
        priority,
        score,
        confidence,
        issues: issues.clone(),
        suggestions: vec![],
        issue_count: issues.len(),
        suggestion_count: 0,
    }
}

/// Helper function to create a test refactoring issue
fn create_test_issue(category: &str, description: &str, severity: f64) -> RefactoringIssue {
    RefactoringIssue {
        category: category.to_string(),
        description: description.to_string(),
        severity,
        contributing_features: vec![FeatureContribution {
            feature_name: "test_feature".to_string(),
            value: 10.0,
            normalized_value: 0.8,
            contribution: severity * 0.5,
        }],
    }
}

#[cfg(test)]
mod directory_health_tree_tests {
    use super::*;

    #[test]
    fn test_directory_health_tree_from_empty_candidates() {
        let candidates = vec![];
        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Should create a tree with at least a root directory
        assert!(!health_tree.directories.is_empty());
        assert_eq!(health_tree.root.path, PathBuf::from("."));
        assert_eq!(health_tree.root.health_score, 1.0); // Perfect health for empty
        assert_eq!(health_tree.root.file_count, 0);
        assert_eq!(health_tree.root.entity_count, 0);
        assert_eq!(health_tree.root.refactoring_needed, 0);
        assert_eq!(health_tree.root.critical_issues, 0);
        assert_eq!(health_tree.root.high_priority_issues, 0);

        // Tree statistics should reflect empty state
        assert_eq!(
            health_tree.tree_statistics.total_directories,
            health_tree.directories.len()
        );
        assert_eq!(health_tree.tree_statistics.avg_health_score, 1.0);
        assert!(health_tree.tree_statistics.hotspot_directories.is_empty());
    }

    #[test]
    fn test_directory_health_tree_basic_structure() {
        let candidates = vec![
            create_test_candidate(
                "func1",
                "main_function",
                "src/main.rs",
                Priority::High,
                2.0,
                0.9,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
            create_test_candidate(
                "func2",
                "helper_function",
                "src/utils/helper.rs",
                Priority::Medium,
                1.5,
                0.8,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
            create_test_candidate(
                "func3",
                "api_handler",
                "src/api/handlers.rs",
                Priority::Critical,
                3.0,
                0.95,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
        ];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Verify tree has the expected directories
        let src_path = PathBuf::from("src");
        let utils_path = PathBuf::from("src/utils");
        let api_path = PathBuf::from("src/api");

        assert!(health_tree.directories.contains_key(&src_path));
        assert!(health_tree.directories.contains_key(&utils_path));
        assert!(health_tree.directories.contains_key(&api_path));

        // Verify parent-child relationships
        let src_dir = health_tree.directories.get(&src_path).unwrap();
        assert!(src_dir.children.contains(&utils_path));
        assert!(src_dir.children.contains(&api_path));

        let utils_dir = health_tree.directories.get(&utils_path).unwrap();
        assert_eq!(utils_dir.parent, Some(src_path.clone()));

        let api_dir = health_tree.directories.get(&api_path).unwrap();
        assert_eq!(api_dir.parent, Some(src_path.clone()));

        // Verify file and entity counts
        assert_eq!(src_dir.file_count, 1); // main.rs
        assert_eq!(utils_dir.file_count, 1); // helper.rs
        assert_eq!(api_dir.file_count, 1); // handlers.rs

        // Verify health scores are calculated (should be < 1.0 due to issues)
        assert!(src_dir.health_score < 1.0);
        assert!(utils_dir.health_score < 1.0);
        assert!(api_dir.health_score < 1.0);

        // Critical priority should have lowest health score
        assert!(api_dir.health_score <= utils_dir.health_score);
    }

    #[test]
    fn test_directory_health_score_calculation() {
        let high_severity_issue = create_test_issue("complexity", "Very high complexity", 2.5);
        let medium_severity_issue = create_test_issue("structure", "Poor structure", 1.2);

        let candidates = vec![
            create_test_candidate(
                "func1",
                "bad_function",
                "src/bad/terrible.rs",
                Priority::Critical,
                3.0,
                0.95,
                vec![high_severity_issue],
            ),
            create_test_candidate(
                "func2",
                "ok_function",
                "src/good/decent.rs",
                Priority::Low,
                0.5,
                0.6,
                vec![medium_severity_issue],
            ),
        ];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        let bad_dir = health_tree
            .directories
            .get(&PathBuf::from("src/bad"))
            .unwrap();
        let good_dir = health_tree
            .directories
            .get(&PathBuf::from("src/good"))
            .unwrap();

        // Bad directory should have lower health score
        assert!(bad_dir.health_score < good_dir.health_score);

        // Critical issues should be counted
        assert_eq!(bad_dir.critical_issues, 1);
        assert_eq!(good_dir.critical_issues, 0);

        // High priority issues (includes critical)
        assert_eq!(bad_dir.high_priority_issues, 1);
        assert_eq!(good_dir.high_priority_issues, 0);

        // Verify issue categories are tracked
        assert!(bad_dir.issue_categories.contains_key("complexity"));
        assert!(good_dir.issue_categories.contains_key("structure"));

        let complexity_summary = &bad_dir.issue_categories["complexity"];
        assert_eq!(complexity_summary.affected_entities, 1);
        assert_eq!(complexity_summary.max_severity, 2.5);
    }

    #[test]
    fn test_directory_issue_summary_aggregation() {
        let complexity_issue1 = create_test_issue("complexity", "High complexity", 2.0);
        let complexity_issue2 = create_test_issue("complexity", "Very high complexity", 2.5);
        let structure_issue = create_test_issue("structure", "Poor structure", 1.5);

        let candidates = vec![
            create_test_candidate(
                "func1",
                "func1",
                "src/module/file1.rs",
                Priority::High,
                2.0,
                0.9,
                vec![complexity_issue1],
            ),
            create_test_candidate(
                "func2",
                "func2",
                "src/module/file2.rs",
                Priority::Critical,
                2.5,
                0.95,
                vec![complexity_issue2],
            ),
            create_test_candidate(
                "func3",
                "func3",
                "src/module/file3.rs",
                Priority::Medium,
                1.5,
                0.8,
                vec![structure_issue],
            ),
        ];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);
        let module_dir = health_tree
            .directories
            .get(&PathBuf::from("src/module"))
            .unwrap();

        // Check complexity issue aggregation
        let complexity_summary = module_dir.issue_categories.get("complexity").unwrap();
        assert_eq!(complexity_summary.affected_entities, 2);
        assert_eq!(complexity_summary.max_severity, 2.5);
        assert!(complexity_summary.avg_severity > 0.0);
        assert!(complexity_summary.health_impact > 0.0);

        // Check structure issue
        let structure_summary = module_dir.issue_categories.get("structure").unwrap();
        assert_eq!(structure_summary.affected_entities, 1);
        assert_eq!(structure_summary.max_severity, 1.5);
    }

    #[test]
    fn test_tree_statistics_calculation() {
        let candidates = vec![
            create_test_candidate(
                "func1",
                "func1",
                "src/level1/level2/file1.rs",
                Priority::High,
                2.0,
                0.9,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
            create_test_candidate(
                "func2",
                "func2",
                "src/level1/file2.rs",
                Priority::Medium,
                1.5,
                0.8,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
            create_test_candidate(
                "func3",
                "func3",
                "src/file3.rs",
                Priority::Critical,
                3.0,
                0.95,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
        ];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);
        let stats = &health_tree.tree_statistics;

        // Verify basic statistics
        assert!(stats.total_directories >= 3); // At least src, src/level1, src/level1/level2
        assert!(stats.max_depth >= 3); // src/level1/level2 creates depth 3
        assert!(stats.avg_health_score > 0.0 && stats.avg_health_score <= 1.0);
        assert!(stats.health_score_std_dev >= 0.0);

        // Verify health by depth statistics
        assert!(!stats.health_by_depth.is_empty());

        // Check that each depth has valid statistics
        for (&depth, depth_stats) in &stats.health_by_depth {
            assert!(depth_stats.directory_count > 0);
            assert!(depth_stats.avg_health_score >= 0.0 && depth_stats.avg_health_score <= 1.0);
            assert!(depth_stats.min_health_score >= 0.0 && depth_stats.min_health_score <= 1.0);
            assert!(depth_stats.max_health_score >= 0.0 && depth_stats.max_health_score <= 1.0);
            assert!(depth_stats.min_health_score <= depth_stats.avg_health_score);
            assert!(depth_stats.avg_health_score <= depth_stats.max_health_score);
        }
    }

    #[test]
    fn test_hotspot_detection() {
        let candidates = vec![
            // Create a "hotspot" directory with many severe issues
            create_test_candidate(
                "func1",
                "terrible_func1",
                "src/hotspot/bad1.rs",
                Priority::Critical,
                3.0,
                0.95,
                vec![
                    create_test_issue("complexity", "Very high complexity", 3.0),
                    create_test_issue("structure", "Very poor structure", 2.5),
                ],
            ),
            create_test_candidate(
                "func2",
                "terrible_func2",
                "src/hotspot/bad2.rs",
                Priority::Critical,
                2.8,
                0.9,
                vec![
                    create_test_issue("complexity", "Very high complexity", 3.0),
                    create_test_issue("structure", "Very poor structure", 2.5),
                ],
            ),
            create_test_candidate(
                "func3",
                "terrible_func3",
                "src/hotspot/bad3.rs",
                Priority::High,
                2.5,
                0.85,
                vec![
                    create_test_issue("complexity", "Very high complexity", 3.0),
                    create_test_issue("structure", "Very poor structure", 2.5),
                ],
            ),
            // Create a "healthy" directory with minor issues
            create_test_candidate(
                "func4",
                "good_func",
                "src/healthy/good.rs",
                Priority::Low,
                0.5,
                0.6,
                vec![create_test_issue("complexity", "Low complexity", 0.5)],
            ),
        ];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Verify hotspot detection
        let hotspot_dirs = &health_tree.tree_statistics.hotspot_directories;
        assert!(!hotspot_dirs.is_empty());

        // The hotspot directory should be identified
        let hotspot_paths: Vec<&PathBuf> = hotspot_dirs.iter().map(|h| &h.path).collect();
        assert!(hotspot_paths
            .iter()
            .any(|p| p.to_string_lossy().contains("hotspot")));

        // Verify hotspot properties
        for hotspot in hotspot_dirs {
            assert!(hotspot.health_score < 0.6); // Should be below healthy threshold
            assert!(hotspot.rank >= 1);
            assert!(!hotspot.primary_issue_category.is_empty());
            assert!(!hotspot.recommendation.is_empty());
        }

        // Hotspots should be ranked by health score (worst first)
        if hotspot_dirs.len() > 1 {
            for i in 0..hotspot_dirs.len() - 1 {
                assert!(hotspot_dirs[i].health_score <= hotspot_dirs[i + 1].health_score);
                assert_eq!(hotspot_dirs[i].rank, i + 1);
            }
        }
    }

    #[test]
    fn test_tree_string_generation() {
        let candidates = vec![
            create_test_candidate(
                "func1",
                "func1",
                "project/src/main.rs",
                Priority::Medium,
                1.5,
                0.8,
                vec![create_test_issue("complexity", "Medium complexity", 1.5)],
            ),
            create_test_candidate(
                "func2",
                "func2",
                "project/src/utils/helper.rs",
                Priority::High,
                2.0,
                0.9,
                vec![create_test_issue("complexity", "Medium complexity", 1.5)],
            ),
            create_test_candidate(
                "func3",
                "func3",
                "project/tests/integration.rs",
                Priority::Low,
                0.8,
                0.7,
                vec![create_test_issue("complexity", "Medium complexity", 1.5)],
            ),
        ];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);
        let tree_string = health_tree.to_tree_string();

        // Verify tree string contains expected elements
        assert!(!tree_string.is_empty());
        assert!(tree_string.contains("health:")); // Health percentages
        assert!(tree_string.contains("%")); // Percentage symbols

        // Should contain health indicators
        assert!(
            tree_string.contains("✓") || tree_string.contains("!") || tree_string.contains("⚠")
        );

        // Should show directory hierarchy
        let lines: Vec<&str> = tree_string.lines().collect();
        assert!(lines.len() > 1); // Multiple directories should create multiple lines

        // Should have proper indentation for nested directories
        let indented_lines: Vec<&str> = lines
            .iter()
            .filter(|line| line.starts_with("  "))
            .map(|s| *s)
            .collect();
        assert!(!indented_lines.is_empty()); // Some lines should be indented for children

        println!("Tree string output:");
        println!("{}", tree_string);
    }

    #[test]
    fn test_get_health_score_method() {
        let candidates = vec![create_test_candidate(
            "func1",
            "func1",
            "src/main.rs",
            Priority::High,
            2.0,
            0.9,
            vec![create_test_issue("complexity", "High complexity", 2.0)],
        )];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Test direct path lookup
        let src_path = Path::new("src");
        let health_score = health_tree.get_health_score(src_path);
        assert!(health_score > 0.0 && health_score <= 1.0);

        // Test non-existent path (should traverse up to root)
        let non_existent_path = Path::new("non/existent/path");
        let fallback_score = health_tree.get_health_score(non_existent_path);
        assert_eq!(fallback_score, health_tree.root.health_score);

        // Test root path
        let root_path = Path::new(".");
        let root_score = health_tree.get_health_score(root_path);
        assert!(root_score > 0.0 && root_score <= 1.0);
    }

    #[test]
    fn test_get_children_method() {
        let candidates = vec![
            create_test_candidate(
                "func1",
                "func1",
                "src/utils/helper1.rs",
                Priority::High,
                2.0,
                0.9,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
            create_test_candidate(
                "func2",
                "func2",
                "src/utils/helper2.rs",
                Priority::Medium,
                1.5,
                0.8,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
            create_test_candidate(
                "func3",
                "func3",
                "src/api/handler.rs",
                Priority::High,
                2.0,
                0.9,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
        ];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Test getting children of src directory
        let src_path = Path::new("src");
        let children = health_tree.get_children(src_path);

        assert_eq!(children.len(), 2); // utils and api

        let child_paths: Vec<&PathBuf> = children.iter().map(|child| &child.path).collect();
        assert!(child_paths.contains(&&PathBuf::from("src/utils")));
        assert!(child_paths.contains(&&PathBuf::from("src/api")));

        // Test getting children of leaf directory (should be empty)
        let utils_path = Path::new("src/utils");
        let utils_children = health_tree.get_children(utils_path);
        assert!(utils_children.is_empty());

        // Test getting children of non-existent directory
        let non_existent_path = Path::new("non/existent");
        let no_children = health_tree.get_children(non_existent_path);
        assert!(no_children.is_empty());
    }

    #[test]
    fn test_edge_case_single_file() {
        let candidates = vec![create_test_candidate(
            "func1",
            "standalone_function",
            "standalone.rs",
            Priority::Medium,
            1.5,
            0.8,
            vec![create_test_issue("complexity", "High complexity", 2.0)],
        )];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Should handle files in root directory
        assert!(!health_tree.directories.is_empty());

        // Root directory should contain the file
        let root_key = health_tree
            .directories
            .keys()
            .find(|k| k.to_string_lossy() == ".")
            .cloned();
        if let Some(root_path) = root_key {
            let root_dir = health_tree.directories.get(&root_path).unwrap();
            assert!(root_dir.file_count > 0 || root_dir.entity_count > 0);
        }
    }

    #[test]
    fn test_edge_case_special_characters_in_paths() {
        let candidates = vec![
            create_test_candidate(
                "func1",
                "func_with_special_chars",
                "src/module-name/file_name.rs",
                Priority::Medium,
                1.5,
                0.8,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
            create_test_candidate(
                "func2",
                "func_with_numbers",
                "src/v2.0/handler123.rs",
                Priority::High,
                2.0,
                0.9,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
        ];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Should handle special characters and numbers in paths
        assert!(!health_tree.directories.is_empty());

        let module_path = PathBuf::from("src/module-name");
        let version_path = PathBuf::from("src/v2.0");

        // Should be able to find directories with special characters
        let has_special_chars = health_tree
            .directories
            .keys()
            .any(|p| p.to_string_lossy().contains("-") || p.to_string_lossy().contains("."));
        assert!(has_special_chars);

        // Tree string should handle special characters
        let tree_string = health_tree.to_tree_string();
        assert!(!tree_string.is_empty());
    }

    #[test]
    fn test_multiple_issues_per_entity() {
        let multiple_issues = vec![
            create_test_issue("complexity", "High complexity", 2.0),
            create_test_issue("structure", "Poor structure", 1.8),
            create_test_issue("graph", "High coupling", 1.5),
        ];

        let candidates = vec![create_test_candidate(
            "problematic_func",
            "very_problematic_function",
            "src/problems/bad.rs",
            Priority::Critical,
            3.0,
            0.95,
            multiple_issues,
        )];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);
        let problems_dir = health_tree
            .directories
            .get(&PathBuf::from("src/problems"))
            .unwrap();

        // Should track all issue categories
        assert_eq!(problems_dir.issue_categories.len(), 3);
        assert!(problems_dir.issue_categories.contains_key("complexity"));
        assert!(problems_dir.issue_categories.contains_key("structure"));
        assert!(problems_dir.issue_categories.contains_key("graph"));

        // All categories should have the same affected entities count
        for (_, issue_summary) in &problems_dir.issue_categories {
            assert_eq!(issue_summary.affected_entities, 1);
            assert!(issue_summary.avg_severity > 0.0);
            assert!(issue_summary.max_severity > 0.0);
            assert!(issue_summary.health_impact > 0.0);
        }

        // Health score should be impacted by multiple issues
        assert!(problems_dir.health_score < 0.5); // Should be quite low due to multiple severe issues
    }

    #[test]
    fn test_depth_health_stats() {
        let candidates = vec![
            // Depth 1: src
            create_test_candidate(
                "func1",
                "func1",
                "src/main.rs",
                Priority::Low,
                0.8,
                0.7,
                vec![create_test_issue("complexity", "Medium complexity", 1.5)],
            ),
            // Depth 2: src/level1
            create_test_candidate(
                "func2",
                "func2",
                "src/level1/file.rs",
                Priority::Medium,
                1.5,
                0.8,
                vec![create_test_issue("complexity", "Medium complexity", 1.5)],
            ),
            // Depth 3: src/level1/level2
            create_test_candidate(
                "func3",
                "func3",
                "src/level1/level2/deep.rs",
                Priority::High,
                2.0,
                0.9,
                vec![create_test_issue("complexity", "Medium complexity", 1.5)],
            ),
        ];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);
        let depth_stats = &health_tree.tree_statistics.health_by_depth;

        // Should have statistics for multiple depth levels
        assert!(!depth_stats.is_empty());
        assert!(depth_stats.len() >= 2); // At least depth 1 and 2

        // Verify each depth level has valid statistics
        for (&depth, stats) in depth_stats {
            assert!(depth >= 1); // Depth should be at least 1
            assert!(stats.directory_count > 0);
            assert!(stats.avg_health_score >= 0.0 && stats.avg_health_score <= 1.0);
            assert!(stats.min_health_score >= 0.0 && stats.min_health_score <= 1.0);
            assert!(stats.max_health_score >= 0.0 && stats.max_health_score <= 1.0);
            assert!(stats.min_health_score <= stats.max_health_score);
        }

        // Verify that the depth matches the expected depth level
        if let Some(depth_3_stats) = depth_stats.get(&3) {
            assert_eq!(depth_3_stats.depth, 3);
            assert!(depth_3_stats.directory_count >= 1); // At least src/level1/level2
        }
    }

    #[test]
    fn test_analysis_results_directory_integration() {
        let candidates = vec![
            create_test_candidate(
                "func1",
                "func1",
                "src/main.rs",
                Priority::Critical,
                3.0,
                0.95,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
            create_test_candidate(
                "func2",
                "func2",
                "src/utils/helper.rs",
                Priority::High,
                2.0,
                0.9,
                vec![create_test_issue("complexity", "High complexity", 2.0)],
            ),
        ];

        // Create directory health tree first
        let directory_health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Create AnalysisResults with directory health tree
        let results = AnalysisResults {
            summary: AnalysisSummary {
                files_processed: 2,
                entities_analyzed: 2,
                refactoring_needed: 2,
                high_priority: 2,
                critical: 1,
                avg_refactoring_score: 2.5,
                code_health_score: 0.6,
            },
            refactoring_candidates: candidates.clone(),
            refactoring_candidates_by_file: vec![],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(5),
                avg_file_processing_time: Duration::from_millis(2500),
                avg_entity_processing_time: Duration::from_millis(2500),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 1024,
                    final_memory_bytes: 512,
                    efficiency_score: 0.8,
                },
            },
            directory_health_tree: Some(directory_health_tree),
            clone_analysis: None,
            coverage_packs: vec![],
            unified_hierarchy: vec![],
            warnings: vec![],
        };

        // Test directory hotspot detection through AnalysisResults
        let hotspots = results.get_directory_hotspots();
        // May or may not have hotspots depending on health threshold, but should not panic
        assert!(
            hotspots.len()
                <= results
                    .directory_health_tree
                    .as_ref()
                    .unwrap()
                    .directories
                    .len()
        );

        // Test directory health lookup
        let src_health = results.get_directory_health(Path::new("src"));
        if let Some(health) = src_health {
            assert!(health >= 0.0 && health <= 1.0);
        }

        // Test directories by health sorting
        let dirs_by_health = results.get_directories_by_health();
        if dirs_by_health.len() > 1 {
            // Should be sorted by health score (worst first)
            for i in 0..dirs_by_health.len() - 1 {
                assert!(dirs_by_health[i].health_score <= dirs_by_health[i + 1].health_score);
            }
        }
    }

    #[test]
    fn test_json_serialization_with_directory_tree() {
        let candidates = vec![create_test_candidate(
            "func1",
            "test_function",
            "src/main.rs",
            Priority::High,
            2.0,
            0.9,
            vec![create_test_issue("complexity", "High complexity", 2.0)],
        )];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Test serialization to JSON
        let json = serde_json::to_string(&health_tree).expect("Should serialize to JSON");
        assert!(!json.is_empty());

        // Test deserialization from JSON
        let deserialized: DirectoryHealthTree =
            serde_json::from_str(&json).expect("Should deserialize from JSON");

        // Verify deserialized data matches original
        assert_eq!(deserialized.root.path, health_tree.root.path);
        assert_eq!(
            deserialized.directories.len(),
            health_tree.directories.len()
        );
        assert_eq!(
            deserialized.tree_statistics.total_directories,
            health_tree.tree_statistics.total_directories
        );

        // Test that all directory data is preserved
        for (path, original_dir) in &health_tree.directories {
            let deserialized_dir = deserialized
                .directories
                .get(path)
                .expect("Directory should exist after deserialization");

            assert_eq!(deserialized_dir.path, original_dir.path);
            assert_eq!(deserialized_dir.health_score, original_dir.health_score);
            assert_eq!(deserialized_dir.file_count, original_dir.file_count);
            assert_eq!(deserialized_dir.entity_count, original_dir.entity_count);
            assert_eq!(deserialized_dir.children, original_dir.children);
            assert_eq!(deserialized_dir.parent, original_dir.parent);
        }
    }

    #[test]
    fn test_hotspot_recommendation_generation() {
        // Test different primary issue categories for recommendation generation
        let complexity_issues = vec![create_test_issue("complexity", "High complexity", 2.5)];
        let structure_issues = vec![create_test_issue("structure", "Poor structure", 2.0)];
        let graph_issues = vec![create_test_issue("graph", "High coupling", 1.8)];

        let candidates = vec![
            create_test_candidate(
                "func1",
                "complex_func",
                "src/complexity_hotspot/bad.rs",
                Priority::Critical,
                3.0,
                0.95,
                complexity_issues,
            ),
            create_test_candidate(
                "func2",
                "struct_func",
                "src/structure_hotspot/bad.rs",
                Priority::High,
                2.5,
                0.9,
                structure_issues,
            ),
            create_test_candidate(
                "func3",
                "coupled_func",
                "src/coupling_hotspot/bad.rs",
                Priority::High,
                2.2,
                0.85,
                graph_issues,
            ),
        ];

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);
        let hotspots = &health_tree.tree_statistics.hotspot_directories;

        // Should have recommendations for different issue types
        for hotspot in hotspots {
            assert!(!hotspot.recommendation.is_empty());

            match hotspot.primary_issue_category.as_str() {
                "complexity" => {
                    assert!(
                        hotspot.recommendation.contains("complexity")
                            || hotspot.recommendation.contains("functions")
                            || hotspot.recommendation.contains("simplifying")
                    );
                }
                "structure" => {
                    assert!(
                        hotspot.recommendation.contains("structural")
                            || hotspot.recommendation.contains("architectural")
                            || hotspot.recommendation.contains("separation")
                    );
                }
                "graph" => {
                    assert!(
                        hotspot.recommendation.contains("coupling")
                            || hotspot.recommendation.contains("dependency")
                    );
                }
                _ => {
                    // Generic recommendation should mention the issue category
                    assert!(hotspot
                        .recommendation
                        .contains(&hotspot.primary_issue_category));
                }
            }
        }
    }

    #[test]
    fn test_large_directory_structure() {
        let mut candidates = Vec::new();

        // Create a larger directory structure to test scalability
        for i in 0..20 {
            for j in 0..5 {
                let file_path = format!("src/module{}/submodule{}/file{}.rs", i, j, j);
                candidates.push(create_test_candidate(
                    &format!("func_{}_{}", i, j),
                    &format!("function_{}_{}", i, j),
                    &file_path,
                    if i % 3 == 0 {
                        Priority::High
                    } else {
                        Priority::Medium
                    },
                    1.5 + (i as f64 * 0.1),
                    0.8,
                    vec![create_test_issue("complexity", "Medium complexity", 1.5)],
                ));
            }
        }

        let health_tree = DirectoryHealthTree::from_candidates(&candidates);

        // Should handle large directory structures efficiently
        assert!(health_tree.directories.len() >= 20); // At least 20 module directories
        assert!(health_tree.tree_statistics.total_directories >= 40); // Including submodules

        // Tree statistics should be calculated correctly for large structures
        assert!(health_tree.tree_statistics.max_depth >= 3); // src/module/submodule
        assert!(health_tree.tree_statistics.avg_health_score > 0.0);

        // Should be able to generate tree string without issues
        let tree_string = health_tree.to_tree_string();
        assert!(!tree_string.is_empty());
        assert!(tree_string.lines().count() >= 20); // Should have many lines for large structure
    }
}
