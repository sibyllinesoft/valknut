use super::*;
use crate::core::scoring::Priority;

fn sample_candidate(path: &str, severity: f64, priority: Priority) -> RefactoringCandidate {
    RefactoringCandidate {
        entity_id: format!("{path}::entity"),
        name: "entity".to_string(),
        file_path: path.to_string(),
        line_range: Some((1, 20)),
        priority,
        score: severity * 20.0,
        confidence: 0.85,
        issues: vec![RefactoringIssue {
            code: "CMPLX".to_string(),
            category: "complexity".to_string(),
            severity,
            contributing_features: vec![FeatureContribution {
                feature_name: "cyclomatic_complexity".to_string(),
                value: 18.0,
                normalized_value: 0.7,
                contribution: 1.3,
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
        coverage_percentage: None,
    }
}

#[test]
fn code_dictionary_reports_when_empty() {
    let mut dictionary = CodeDictionary::default();
    assert!(dictionary.is_empty());

    dictionary.issues.insert(
        "CMPLX".to_string(),
        CodeDefinition {
            code: "CMPLX".to_string(),
            title: "High Complexity".to_string(),
            summary: "Cyclomatic complexity exceeded target".to_string(),
            category: Some("complexity".to_string()),
        },
    );
    assert!(!dictionary.is_empty());
}

#[test]
fn memory_stats_merge_preserves_extremes_and_averages() {
    let mut base = MemoryStats {
        peak_memory_bytes: 5_000_000,
        final_memory_bytes: 3_000_000,
        efficiency_score: 0.8,
    };
    let other = MemoryStats {
        peak_memory_bytes: 7_500_000,
        final_memory_bytes: 2_000_000,
        efficiency_score: 0.4,
    };

    base.merge(other);
    assert_eq!(base.peak_memory_bytes, 7_500_000);
    assert_eq!(base.final_memory_bytes, 3_000_000);
    assert!((base.efficiency_score - 0.6).abs() < f64::EPSILON);
}
