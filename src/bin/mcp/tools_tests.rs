use super::*;
use crate::mcp::formatters::{
    create_markdown_report, format_analysis_results_with_temp_path,
};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::{tempdir, TempDir};
use valknut_rs::core::pipeline::{CodeDefinition, CodeDictionary};
use valknut_rs::core::scoring::Priority;

fn sample_results() -> AnalysisResults {
    let summary = valknut_rs::api::results::AnalysisSummary {
        files_processed: 2,
        entities_analyzed: 3,
        refactoring_needed: 2,
        high_priority: 1,
        critical: 1,
        avg_refactoring_score: 0.72,
        code_health_score: 0.58,
        total_files: 2,
        total_entities: 3,
        total_lines_of_code: 420,
        languages: vec!["Rust".to_string()],
        total_issues: 3,
        high_priority_issues: 2,
        critical_issues: 1,
        doc_health_score: 1.0,
        doc_issue_count: 0,
    };

    let candidate = valknut_rs::api::results::RefactoringCandidate {
        entity_id: "src/lib.rs::sample_fn".to_string(),
        name: "sample_fn".to_string(),
        file_path: "src/lib.rs".to_string(),
        line_range: Some((10, 40)),
        priority: Priority::Critical,
        score: 0.82,
        confidence: 0.93,
        issues: vec![
            valknut_rs::api::results::RefactoringIssue {
                code: "CMPLX".to_string(),
                category: "complexity".to_string(),
                severity: 2.1,
                contributing_features: vec![valknut_rs::api::results::FeatureContribution {
                    feature_name: "cyclomatic_complexity".to_string(),
                    value: 18.0,
                    normalized_value: 0.7,
                    contribution: 1.2,
                }],
            },
            valknut_rs::api::results::RefactoringIssue {
                code: "COUPL".to_string(),
                category: "coupling".to_string(),
                severity: 1.4,
                contributing_features: vec![valknut_rs::api::results::FeatureContribution {
                    feature_name: "fan_in".to_string(),
                    value: 12.0,
                    normalized_value: 0.6,
                    contribution: 0.8,
                }],
            },
        ],
        suggestions: vec![valknut_rs::api::results::RefactoringSuggestion {
            refactoring_type: "extract_method".to_string(),
            code: "XTRMTH".to_string(),
            priority: 0.9,
            effort: 0.4,
            impact: 0.85,
        }],
        issue_count: 2,
        suggestion_count: 1,
        coverage_percentage: None,
    };

    let mut code_dictionary = CodeDictionary::default();
    code_dictionary.issues.insert(
        "CMPLX".to_string(),
        CodeDefinition {
            code: "CMPLX".to_string(),
            title: "Complexity Too High".to_string(),
            summary: "Function exceeds complexity thresholds".to_string(),
            category: Some("complexity".to_string()),
        },
    );
    code_dictionary.issues.insert(
        "COUPL".to_string(),
        CodeDefinition {
            code: "COUPL".to_string(),
            title: "High Coupling".to_string(),
            summary: "Module has excessive dependencies".to_string(),
            category: Some("architecture".to_string()),
        },
    );

    AnalysisResults {
        project_root: std::path::PathBuf::new(),
        summary,
        normalized: None,
        passes: valknut_rs::api::results::StageResultsBundle::disabled(),
        refactoring_candidates: vec![candidate],
        statistics: valknut_rs::api::results::AnalysisStatistics {
            total_duration: std::time::Duration::from_secs(2),
            avg_file_processing_time: std::time::Duration::from_millis(150),
            avg_entity_processing_time: std::time::Duration::from_millis(20),
            features_per_entity: HashMap::new(),
            priority_distribution: HashMap::new(),
            issue_distribution: HashMap::new(),
            memory_stats: valknut_rs::api::results::MemoryStats {
                peak_memory_bytes: 1_000_000,
                final_memory_bytes: 500_000,
                efficiency_score: 0.7,
            },
        },
        health_metrics: None,
        clone_analysis: None,
        coverage_packs: Vec::new(),
        warnings: Vec::new(),
        code_dictionary,
        documentation: None,
    }
}

#[test]
fn default_parameter_helpers_match_expected_values() {
    assert!(default_include_suggestions());
    assert_eq!(default_format(), "json");
    assert_eq!(default_max_suggestions(), 10);
}

#[test]
fn parse_entity_id_handles_delimiters_and_errors() {
    assert_eq!(
        parse_entity_id("src/lib.rs:sample_fn").unwrap(),
        ("src/lib.rs".to_string(), Some("sample_fn".to_string()))
    );
    assert_eq!(
        parse_entity_id("src/lib.rs#sample_fn").unwrap(),
        ("src/lib.rs".to_string(), Some("sample_fn".to_string()))
    );
    assert_eq!(
        parse_entity_id("src/lib.rs").unwrap(),
        ("src/lib.rs".to_string(), None)
    );
    let error = parse_entity_id("");
    assert!(error.is_err());
}

#[test]
fn filter_refactoring_suggestions_limits_results() {
    let results = sample_results();
    let response = filter_refactoring_suggestions(&results, "src/lib.rs", 5);
    assert_eq!(response["suggestions_count"], 1);
    assert_eq!(response["entity_id"], "src/lib.rs");
    assert!(response["suggestions"][0]["suggested_actions"][0]
        .as_str()
        .unwrap()
        .contains("Immediate"));
}

#[test]
fn extract_suggested_actions_reflects_priority_and_issue_categories() {
    let mut candidate = sample_results().refactoring_candidates[0].clone();
    candidate.priority = Priority::Medium;
    candidate
        .issues
        .push(valknut_rs::api::results::RefactoringIssue {
            code: "DUP".to_string(),
            category: "duplication".to_string(),
            severity: 1.0,
            contributing_features: Vec::new(),
        });

    let actions = extract_suggested_actions(&candidate);
    assert!(
        actions
            .iter()
            .any(|action| action.contains("Consider refactoring")),
        "expected medium priority guidance in actions: {actions:?}"
    );
    assert!(
        actions
            .iter()
            .any(|action| action.contains("Extract common code")),
        "expected duplication hint in actions: {actions:?}"
    );
}

#[test]
fn create_file_quality_report_respects_suggestion_flag() {
    let results = sample_results();
    let with_suggestions = create_file_quality_report(&results, "src/lib.rs", true);
    assert!(
        with_suggestions["refactoring_suggestions"].is_array(),
        "expected suggestions array when include_suggestions=true"
    );

    let without_suggestions = create_file_quality_report(&results, "src/lib.rs", false);
    assert!(
        without_suggestions.get("refactoring_suggestions").is_none(),
        "suggestions key should be absent when include_suggestions=false"
    );
}

#[test]
fn evaluate_quality_gates_detects_threshold_violations() {
    let mut results = sample_results();
    results.summary.code_health_score = 0.4;
    results.summary.avg_refactoring_score = 0.9;
    results.summary.high_priority = 2;
    results.summary.critical = 1;

    let params = ValidateQualityGatesParams {
        path: ".".to_string(),
        max_complexity: Some(60.0),
        min_health: Some(0.6),
        max_debt: Some(50.0),
        max_issues: Some(1),
    };

    let report = evaluate_quality_gates(&results, &params);
    assert!(!report["quality_gates_passed"].as_bool().unwrap());
    let violations = report["violations"].as_array().unwrap();
    assert!(violations.iter().any(|v| v["rule"] == "Min Health Score"));
    assert!(violations.iter().any(|v| v["rule"] == "Max Complexity"));
    assert!(violations.iter().any(|v| v["rule"] == "Max Issues"));
    assert!(violations.iter().any(|v| v["rule"] == "Max Technical Debt"));
}

#[test]
fn evaluate_quality_gates_allows_passing_when_within_limits() {
    let results = sample_results();
    let params = ValidateQualityGatesParams {
        path: ".".to_string(),
        max_complexity: Some(90.0),
        min_health: Some(0.5),
        max_debt: Some(90.0),
        max_issues: Some(5),
    };

    let report = evaluate_quality_gates(&results, &params);
    assert!(report["quality_gates_passed"].as_bool().unwrap());
    assert!(report["violations"].as_array().unwrap().is_empty());
}

#[test]
fn format_analysis_results_defaults_to_json_for_unknown_formats() {
    let results = sample_results();
    let serialized = format_analysis_results(&results, "yaml").expect("fallback should work");
    let parsed: serde_json::Value =
        serde_json::from_str(&serialized).expect("result should be valid JSON");
    assert_eq!(parsed["summary"]["files_processed"], 2);
}

#[test]
fn filter_refactoring_suggestions_handles_non_matches() {
    let results = sample_results();
    let response = filter_refactoring_suggestions(&results, "other/file.rs", 3);
    assert_eq!(response["suggestions_count"], 0);
    assert_eq!(response["suggestions"], serde_json::json!([]));
    assert_eq!(response["summary"]["total_files_analyzed"], 2);
}

#[test]
fn extract_suggested_actions_reflects_priority_and_issues() {
    let results = sample_results();
    let candidate = &results.refactoring_candidates[0];
    let actions = extract_suggested_actions(candidate);
    assert!(actions.iter().any(|a| a.contains("Immediate")));
    assert!(actions.iter().any(|a| a.contains("Break down")));
    assert!(actions.iter().any(|a| a.contains("Reduce dependencies")));
}

#[test]
fn extract_suggested_actions_handles_low_priority_duplication() {
    let mut results = sample_results();
    let mut candidate = results.refactoring_candidates[0].clone();
    candidate.priority = Priority::Low;
    candidate
        .issues
        .push(valknut_rs::api::results::RefactoringIssue {
            code: "DUPL".to_string(),
            category: "duplication".to_string(),
            severity: 1.1,
            contributing_features: vec![],
        });
    let actions = extract_suggested_actions(&candidate);
    assert!(actions.iter().any(|a| a.contains("optional")));
    assert!(actions.iter().any(|a| a.contains("Extract common code")));
}

#[test]
fn evaluate_quality_gates_reports_violations() {
    let results = sample_results();
    let params = ValidateQualityGatesParams {
        path: ".".to_string(),
        max_complexity: Some(50.0),
        min_health: Some(0.75),
        max_debt: Some(60.0),
        max_issues: Some(1),
    };

    let evaluation = evaluate_quality_gates(&results, &params);
    assert!(!evaluation["quality_gates_passed"].as_bool().unwrap());
    assert!(evaluation["violations"].as_array().unwrap().len() >= 3);
}

#[test]
fn evaluate_quality_gates_passes_within_thresholds() {
    let results = sample_results();
    let params = ValidateQualityGatesParams {
        path: ".".to_string(),
        max_complexity: Some(99.0),
        min_health: Some(0.4),
        max_debt: Some(95.0),
        max_issues: Some(5),
    };

    let evaluation = evaluate_quality_gates(&results, &params);
    assert!(evaluation["quality_gates_passed"].as_bool().unwrap());
    assert!(evaluation["violations"].as_array().unwrap().is_empty());
}

#[test]
fn create_file_quality_report_includes_optional_suggestions() {
    let results = sample_results();
    let report = create_file_quality_report(&results, "src/lib.rs", true);
    assert_eq!(report["file_path"], "src/lib.rs");
    assert!(
        report["quality_metrics"]["refactoring_score"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        report
            .get("refactoring_suggestions")
            .expect("expected suggestions")
            .as_array()
            .unwrap()
            .len()
            > 0
    );

    let minimal = create_file_quality_report(&results, "src/lib.rs", false);
    assert!(minimal.get("refactoring_suggestions").is_none());
}

#[test]
fn create_file_quality_report_handles_missing_file() {
    let results = sample_results();
    let report = create_file_quality_report(&results, "does/not/exist.rs", true);
    assert_eq!(report["file_path"], "does/not/exist.rs");
    assert!(!report["file_exists"].as_bool().unwrap());
    assert_eq!(report["refactoring_opportunities_count"], 0);
    assert_eq!(
        report["quality_metrics"]["refactoring_score"]
            .as_f64()
            .unwrap(),
        0.0
    );
    assert!(report.get("refactoring_suggestions").is_none());
}

#[test]
fn format_analysis_results_supports_json_and_markdown() {
    let results = sample_results();
    let json_output = format_analysis_results(&results, "json").unwrap();
    assert!(json_output.contains("\"files_processed\": 2"));

    let markdown_output = format_analysis_results(&results, "markdown").unwrap();
    assert!(markdown_output.contains("# Code Analysis Report"));
    assert!(markdown_output.contains("Refactoring Candidates"));

    let fallback_output = format_analysis_results(&results, "unknown").unwrap();
    assert!(fallback_output.contains("\"entities_analyzed\": 3"));
}

#[test]
fn format_analysis_results_supports_html() {
    let results = sample_results();
    let temp_file = tempfile::NamedTempFile::new().expect("temp file");
    let html_output =
        format_analysis_results_with_temp_path(&results, "html", temp_file.path())
            .expect("html generation should succeed");
    assert!(
        html_output.to_lowercase().contains("<html"),
        "html output should include root tag"
    );
    assert!(
        temp_file.path().exists(),
        "html report should be written to disk"
    );
}

#[tokio::test]
async fn analyze_with_session_cache_uses_warm_entry() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let path = temp_dir.path();
    let canonical_path = path.canonicalize().expect("canonicalize temp dir");

    let cached_results = Arc::new(sample_results());
    let cache: AnalysisCacheRef = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    {
        let mut guard = cache.lock().await;
        guard.insert(
            canonical_path.clone(),
            AnalysisCache {
                path: canonical_path.clone(),
                results: cached_results.clone(),
                timestamp: std::time::Instant::now(),
            },
        );
    }

    let config = AnalysisConfig::default();
    let returned = analyze_with_session_cache(&config, path, &cache)
        .await
        .expect("cache hit should succeed");

    assert!(
        Arc::ptr_eq(&returned, &cached_results),
        "should return the cached Arc"
    );
}

#[tokio::test]
async fn analyze_with_session_cache_recomputes_expired_entry() {
    let project = tempdir().expect("temp dir");
    let project_path = project.path();
    let file_path = project_path.join("lib.rs");
    fs::write(
        &file_path,
        r#"
pub fn coverage_demo() -> i32 {
    41 + 1
}
"#,
    )
    .expect("should write sample source");

    let cache: AnalysisCacheRef = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let canonical_path = project_path
        .canonicalize()
        .expect("canonicalize project path");
    let expired_results = Arc::new(sample_results());
    {
        let mut guard = cache.lock().await;
        guard.insert(
            canonical_path.clone(),
            AnalysisCache {
                path: canonical_path.clone(),
                results: expired_results.clone(),
                timestamp: Instant::now() - Duration::from_secs(600),
            },
        );
    }

    let config = AnalysisConfig::default()
        .with_languages(vec!["rust".to_string()])
        .with_max_files(1);

    let refreshed = analyze_with_session_cache(&config, project_path, &cache)
        .await
        .expect("expired cache entry should trigger fresh analysis");

    assert!(
        !Arc::ptr_eq(&refreshed, &expired_results),
        "fresh analysis should replace the expired Arc"
    );

    let cache_guard = cache.lock().await;
    let cached_entry = cache_guard
        .get(&canonical_path)
        .expect("cache should contain refreshed entry");
    assert!(
        Arc::ptr_eq(&cached_entry.results, &refreshed),
        "cache should store the refreshed analysis results"
    );
}

#[test]
fn insert_analysis_into_cache_enforces_capacity() {
    let mut cache = HashMap::new();
    let base = Instant::now();
    for idx in 0..10 {
        let path = PathBuf::from(format!("cache_entry_{idx}.json"));
        cache.insert(
            path.clone(),
            AnalysisCache {
                path,
                results: Arc::new(sample_results()),
                timestamp: base - Duration::from_secs((idx + 1) as u64),
            },
        );
    }

    assert!(cache.contains_key(&PathBuf::from("cache_entry_9.json")));

    let new_path = PathBuf::from("cache_entry_new.json");
    let result_arc = Arc::new(sample_results());
    insert_analysis_into_cache(&mut cache, new_path.clone(), result_arc.clone());

    assert_eq!(cache.len(), 10, "capacity should remain capped");
    assert!(
        cache.contains_key(&new_path),
        "new entry should be present after insertion"
    );
    assert!(
        !cache.contains_key(&PathBuf::from("cache_entry_9.json")),
        "oldest entry should be evicted"
    );
    let stored = cache.get(&new_path).expect("new entry should exist");
    assert!(
        Arc::ptr_eq(&stored.results, &result_arc),
        "stored results should reuse the supplied Arc"
    );
}

#[test]
fn evict_oldest_cache_entry_handles_empty_cache() {
    let mut cache = HashMap::new();
    assert!(
        evict_oldest_cache_entry(&mut cache).is_none(),
        "evict helper should return None when cache is empty"
    );
}

#[test]
fn cache_entry_is_fresh_detects_recent_entries() {
    let entry = AnalysisCache {
        path: PathBuf::from("recent"),
        results: Arc::new(sample_results()),
        timestamp: Instant::now() - Duration::from_secs(10),
    };

    assert!(
        cache_entry_is_fresh(&entry),
        "entries newer than 5 minutes should be considered fresh"
    );
}

#[test]
fn cache_entry_is_fresh_detects_expired_entries() {
    let entry = AnalysisCache {
        path: PathBuf::from("expired"),
        results: Arc::new(sample_results()),
        timestamp: Instant::now() - Duration::from_secs(600),
    };

    assert!(
        !cache_entry_is_fresh(&entry),
        "entries older than 5 minutes should expire"
    );
}

#[test]
fn create_markdown_report_includes_warnings_section() {
    let mut results = sample_results();
    results.warnings.push("First warning".to_string());
    results.warnings.push("Second warning".to_string());

    let markdown = create_markdown_report(&results).unwrap();
    assert!(markdown.contains("## Warnings"));
    assert!(markdown.contains("First warning"));
    assert!(markdown.contains("Second warning"));
}

#[tokio::test]
async fn execute_analyze_code_returns_invalid_params_for_missing_path() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let missing_path = std::env::temp_dir().join(format!("valknut_missing_{unique}"));

    let params = AnalyzeCodeParams {
        path: missing_path.to_string_lossy().into_owned(),
        format: "json".to_string(),
    };

    let err = execute_analyze_code(params)
        .await
        .expect_err("non-existent paths should be rejected early");

    assert_eq!(err.0, error_codes::INVALID_PARAMS);
    assert!(
        err.1.contains("does not exist"),
        "unexpected error message: {}",
        err.1
    );
}

#[tokio::test]
async fn execute_refactoring_suggestions_rejects_empty_entity_id() {
    let params = RefactoringSuggestionsParams {
        entity_id: String::new(),
        max_suggestions: 5,
    };

    let err = execute_refactoring_suggestions(params)
        .await
        .expect_err("empty entity ids should fail validation");

    assert_eq!(err.0, error_codes::INVALID_PARAMS);
    assert!(
        err.1.to_lowercase().contains("entity id"),
        "unexpected error message: {}",
        err.1
    );
}

#[tokio::test]
async fn execute_validate_quality_gates_requires_existing_path() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let missing_dir = std::env::temp_dir().join(format!("valknut_missing_dir_{unique}"));

    let params = ValidateQualityGatesParams {
        path: missing_dir.to_string_lossy().into_owned(),
        max_complexity: Some(50.0),
        min_health: Some(0.7),
        max_debt: None,
        max_issues: None,
    };

    let err = execute_validate_quality_gates(params)
        .await
        .expect_err("missing directories should yield validation errors");

    assert_eq!(err.0, error_codes::INVALID_PARAMS);
    assert!(
        err.1.contains("does not exist"),
        "unexpected error message: {}",
        err.1
    );
}

#[tokio::test]
async fn execute_analyze_file_quality_requires_real_files() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    let missing_file = std::env::temp_dir().join(format!("valknut_missing_file_{unique}.rs"));

    let params = AnalyzeFileQualityParams {
        file_path: missing_file.to_string_lossy().into_owned(),
        include_suggestions: true,
    };

    let err = execute_analyze_file_quality(params)
        .await
        .expect_err("missing files should be rejected");

    assert_eq!(err.0, error_codes::INVALID_PARAMS);
    assert!(
        err.1.contains("does not exist"),
        "unexpected error message: {}",
        err.1
    );
}

#[tokio::test]
async fn execute_analyze_file_quality_rejects_directory_paths() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let params = AnalyzeFileQualityParams {
        file_path: temp_dir.path().to_string_lossy().into_owned(),
        include_suggestions: false,
    };

    let err = execute_analyze_file_quality(params)
        .await
        .expect_err("directories should not be accepted as file inputs");

    assert_eq!(err.0, error_codes::INVALID_PARAMS);
    assert!(
        err.1.contains("not a file"),
        "unexpected error message: {}",
        err.1
    );
}
