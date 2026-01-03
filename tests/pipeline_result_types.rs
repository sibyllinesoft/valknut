use std::path::Path;

use valknut_rs::core::pipeline::{
    AnalysisResults, CodeDefinition, CodeDictionary, DirectoryHealthTree, FeatureContribution,
    RefactoringCandidate, RefactoringIssue, RefactoringSuggestion,
};
// Use the 3-field MemoryStats with merge method
use valknut_rs::core::pipeline::results::result_types::MemoryStats;
use valknut_rs::core::scoring::Priority;

fn candidate(path: &str, severity: f64, priority: Priority) -> RefactoringCandidate {
    RefactoringCandidate {
        entity_id: format!("{path}::entity"),
        name: "entity".to_string(),
        file_path: path.to_string(),
        line_range: Some((10, 40)),
        coverage_percentage: None,
        priority,
        score: severity * 20.0,
        confidence: 0.8,
        issues: vec![RefactoringIssue {
            code: "CMPLX".to_string(),
            category: "complexity".to_string(),
            severity,
            contributing_features: vec![FeatureContribution {
                feature_name: "cyclomatic_complexity".to_string(),
                value: 18.0,
                normalized_value: 0.7,
                contribution: 1.2,
            }],
        }],
        suggestions: vec![RefactoringSuggestion {
            refactoring_type: "extract_method".to_string(),
            code: "XTRMTH".to_string(),
            priority: 0.9,
            effort: 0.4,
            impact: 0.85,
        }],
        issue_count: 1,
        suggestion_count: 1,
    }
}

#[test]
fn directory_health_tree_exposes_children_and_scores() {
    let candidates = vec![
        candidate("src/lib.rs", 2.0, Priority::Critical),
        candidate("src/utils/mod.rs", 1.4, Priority::High),
        candidate("src/utils/parsers.rs", 1.1, Priority::High),
    ];

    let tree = DirectoryHealthTree::from_candidates(&candidates);

    // Existing path lookups
    let root_score = tree.get_health_score(Path::new("src"));
    assert!(root_score >= 0.0 && root_score <= 1.0);
    let nested_score = tree.get_health_score(Path::new("src/utils"));
    assert!(nested_score <= root_score);

    // Missing directories should fall back to nearest parent/root
    let missing_score = tree.get_health_score(Path::new("src/unknown"));
    assert_eq!(missing_score, nested_score);

    // Child enumeration and tree string
    let children = tree.get_children(Path::new("src"));
    assert!(
        children.iter().any(|child| child.path.ends_with("utils")),
        "expected utils child directory"
    );
    let tree_str = tree.to_tree_string();
    assert!(tree_str.contains("src"));
    assert!(tree_str.contains("src/utils"));
}

#[test]
fn directory_health_tree_handles_empty_input() {
    let tree = DirectoryHealthTree::from_candidates(&[]);
    assert_eq!(tree.root.path, Path::new("."));
    assert_eq!(tree.get_health_score(Path::new("nonexistent")), 1.0);
    assert!(tree.to_tree_string().contains("."));
}

#[test]
fn memory_stats_merge_and_code_dictionary_helpers_work() {
    let mut stats = MemoryStats {
        peak_memory_bytes: 1_000_000,
        final_memory_bytes: 250_000,
        efficiency_score: 0.9,
    };
    stats.merge(MemoryStats {
        peak_memory_bytes: 2_000_000,
        final_memory_bytes: 500_000,
        efficiency_score: 0.5,
    });
    assert_eq!(stats.peak_memory_bytes, 2_000_000);
    assert_eq!(stats.final_memory_bytes, 500_000);
    assert!((stats.efficiency_score - 0.7).abs() < f64::EPSILON);

    let mut dictionary = CodeDictionary::default();
    assert!(dictionary.is_empty());
    dictionary.issues.insert(
        "CMPLX".to_string(),
        CodeDefinition {
            code: "CMPLX".to_string(),
            title: "Complexity Hotspot".to_string(),
            summary: "Cyclomatic complexity exceeded threshold".to_string(),
            category: Some("complexity".to_string()),
        },
    );
    assert!(!dictionary.is_empty());
}

#[test]
fn analysis_results_group_candidates_by_file_sorts_by_priority() {
    let mut c1 = candidate("src/lib.rs", 1.0, Priority::High);
    c1.score = 0.8;
    let mut c2 = candidate("src/lib.rs", 0.8, Priority::Critical);
    c2.score = 0.9;
    let c3 = candidate("src/utils/mod.rs", 0.6, Priority::Medium);

    let groups = AnalysisResults::group_candidates_by_file(&[c1.clone(), c2.clone(), c3.clone()]);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].file_path, "src/lib.rs");
    assert_eq!(groups[0].entity_count, 2);
    assert_eq!(groups[0].highest_priority, Priority::Critical);
    assert_eq!(groups[1].file_path, "src/utils/mod.rs");
}

#[test]
fn analysis_results_empty_initializes_defaults() {
    let empty = AnalysisResults::empty();
    assert_eq!(empty.summary.files_processed, 0);
    assert!(empty.refactoring_candidates.is_empty());
    assert!(empty.warnings.is_empty());
    assert_eq!(empty.statistics.memory_stats.efficiency_score, 1.0);
    assert!(empty.code_dictionary.is_empty());
}

#[test]
fn analysis_results_build_unified_hierarchy_prefers_directory_tree() {
    let candidates = vec![candidate("src/lib.rs", 1.0, Priority::High)];
    let tree = DirectoryHealthTree::from_candidates(&candidates);

    let hierarchy = AnalysisResults::build_unified_hierarchy_with_fallback(&candidates, &tree);
    assert!(
        hierarchy
            .first()
            .and_then(|root| root.get("name"))
            .is_some(),
        "directory tree hierarchy should produce nodes"
    );
}

#[test]
fn analysis_results_build_unified_hierarchy_falls_back_to_candidates() {
    let candidates = vec![candidate("src/lib.rs", 1.0, Priority::High)];
    let empty_tree = DirectoryHealthTree::from_candidates(&[]);

    let hierarchy =
        AnalysisResults::build_unified_hierarchy_with_fallback(&candidates, &empty_tree);

    assert!(
        !hierarchy.is_empty(),
        "should fallback to candidate-based hierarchy when tree is empty"
    );
}
