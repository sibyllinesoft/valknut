use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::runtime::{Builder, Runtime};
use valknut_rs::core::arena_analysis::{ArenaAnalysisResult, ArenaBatchResult, ArenaFileAnalyzer};
use valknut_rs::core::interning::intern;

fn test_runtime() -> Runtime {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime for arena analysis tests")
}

#[test]
fn analyze_files_in_arenas_processes_multiple_files() {
    let runtime = test_runtime();
    let analyzer = ArenaFileAnalyzer::new();

    let files: Vec<PathBuf> = vec![
        PathBuf::from("arena_multi_a.py"),
        PathBuf::from("arena_multi_b.py"),
    ];
    let file_refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();

    let sources = vec![
        String::from(
            r#"
def alpha():
    helper = 3
    if helper > 1:
        return helper
    return helper - 1
"#,
        ),
        String::from(
            r#"
class Example:
    def compute(self, base: int) -> int:
        total = 0
        for step in range(base):
            total += step
        return total
"#,
        ),
    ];
    let source_refs: Vec<&str> = sources.iter().map(|s| s.as_str()).collect();

    let results = runtime
        .block_on(async {
            analyzer
                .analyze_files_in_arenas(&file_refs, &source_refs)
                .await
        })
        .expect("multi-file arena analysis should succeed");

    assert_eq!(results.len(), 2, "each file should produce a result");
    assert!(
        results.iter().all(|res| res.entity_count > 0),
        "analyzer should extract entities from both files"
    );
    assert!(
        results
            .iter()
            .all(|res| res.memory_efficiency_score >= 0.0 && res.arena_kb_used() >= 0.0),
        "metrics should be populated for each file"
    );
    assert!(
        results
            .iter()
            .any(|res| res.file_path_str().ends_with("arena_multi_a.py")),
        "results should carry the original file path"
    );
}

#[test]
fn analyze_files_in_arenas_validates_lengths() {
    let runtime = test_runtime();
    let analyzer = ArenaFileAnalyzer::new();
    let file = PathBuf::from("arena_mismatch.py");
    let files = [file.as_path()];
    let empty_sources: [&str; 0] = [];

    let err = runtime
        .block_on(async {
            analyzer
                .analyze_files_in_arenas(&files, &empty_sources)
                .await
        })
        .expect_err("mismatched slices should yield a validation error");

    let message = err.to_string();
    assert!(
        message.contains("same length"),
        "error should mention mismatched lengths, got: {message}"
    );
}

#[test]
fn arena_batch_result_metrics_are_consistent() {
    let entity_count = 12;
    let arena_bytes = 12 * 1024;
    let analysis_time = Duration::from_millis(24);
    let memory_efficiency = (entity_count as f64) / (arena_bytes as f64 / 1024.0);

    let sample_result = ArenaAnalysisResult {
        entity_count,
        file_path: intern("batch/sample.py"),
        entity_extraction_time: Duration::from_millis(4),
        total_analysis_time: analysis_time,
        arena_bytes_used: arena_bytes,
        memory_efficiency_score: memory_efficiency,
        entities: Vec::new(),
        lines_of_code: 100,
        source_code: String::new(),
    };

    assert_eq!(sample_result.file_path_str(), "batch/sample.py");
    assert!(
        sample_result.entities_per_second() > 0.0,
        "entities/sec should be non-zero for positive durations"
    );
    assert!(
        (sample_result.arena_kb_used() - 12.0).abs() < f64::EPSILON,
        "arena_kb_used should convert bytes to kilobytes"
    );

    let batch = ArenaBatchResult {
        file_results: vec![sample_result.clone()],
        total_files: 1,
        total_entities: sample_result.entity_count,
        total_arena_bytes: sample_result.arena_bytes_used,
        total_analysis_time: analysis_time,
        average_entities_per_file: sample_result.entity_count as f64,
        arena_efficiency_score: sample_result.memory_efficiency_score,
    };

    assert!(
        batch.entities_per_second() > 0.0,
        "batch throughput should be positive"
    );
    assert!(
        (batch.total_arena_kb() - sample_result.arena_kb_used()).abs() < f64::EPSILON,
        "batch KB conversion should mirror individual results"
    );
    assert!(
        batch.estimated_malloc_savings() > 0.0,
        "savings estimate should be positive for non-empty batches"
    );
}
