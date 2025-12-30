use super::*;
use tempfile::tempdir;

#[tokio::test]
async fn coverage_extractor_default_builds() {
    let extractor = CoverageExtractor::with_ast(Arc::new(AstService::new()));
    let packs = extractor.build_coverage_packs(Vec::new()).await.unwrap();
    assert!(packs.is_empty());
}

fn make_extractor(mut config: CoverageConfig) -> CoverageExtractor {
    config.enabled = true;
    CoverageExtractor::new(config, Arc::new(AstService::new()))
}

#[tokio::test]
async fn builds_coverage_pack_from_minimal_lcov_report() {
    let tmp = tempdir().expect("temp dir");
    let source_path = tmp.path().join("sample.rs");
    let source = r#"pub fn add(a: i32, b: i32) -> i32 {
    if a > 0 {
        a + b
    } else {
        b - a
    }
}
"#;
    fs::write(&source_path, source).expect("write source file");

    let lcov_path = tmp.path().join("coverage.lcov");
    let lcov_report = format!(
        "TN:\nSF:{}\nDA:1,1\nDA:2,0\nDA:3,0\nDA:4,0\nDA:5,0\nDA:6,0\nDA:7,0\nDA:8,1\nend_of_record\n",
        source_path.display()
    );
    fs::write(&lcov_path, lcov_report).expect("write lcov file");

    let mut config = CoverageConfig::default();
    config.min_gap_loc = 1;
    config.snippet_context_lines = 1;
    config.long_gap_head_tail = 1;

    let extractor = make_extractor(config);
    let packs = extractor
        .build_coverage_packs(vec![lcov_path])
        .await
        .expect("pack generation");

    let pack = packs
        .iter()
        .find(|pack| pack.path == source_path)
        .expect("pack for source file");

    assert!(!pack.gaps.is_empty());
    let gap = &pack.gaps[0];
    assert_eq!(gap.span.start, 2);
    assert!(gap.span.end >= gap.span.start);
    assert!(gap.features.gap_loc >= 1);
    // Preview head contains all uncovered lines in the gap
    assert!(!gap.preview.head.is_empty());
}

#[test]
fn lines_to_spans_respects_min_gap_and_merges_runs() {
    let mut config = CoverageConfig::default();
    config.min_gap_loc = 3;
    let extractor = make_extractor(config);

    let lines = vec![
        LineCoverage {
            line_number: 1,
            hits: 0,
            is_covered: false,
        },
        LineCoverage {
            line_number: 2,
            hits: 0,
            is_covered: false,
        },
        LineCoverage {
            line_number: 3,
            hits: 0,
            is_covered: false,
        },
        LineCoverage {
            line_number: 5,
            hits: 0,
            is_covered: false,
        },
        LineCoverage {
            line_number: 10,
            hits: 0,
            is_covered: false,
        },
    ];
    let path = PathBuf::from("fake.rs");
    let spans = extractor
        .lines_to_spans(&path, &lines)
        .expect("compute spans");

    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].start, 1);
    assert_eq!(spans[0].end, 3);
}

#[test]
fn chunk_spans_python_splits_on_function_boundaries() {
    let tmp = tempdir().expect("temp dir");
    let path = tmp.path().join("module.py");
    let python_source = r#"
def a():
    return 1


def b():
    return 2
"#;
    fs::write(&path, python_source).expect("write python file");

    let mut config = CoverageConfig::default();
    config.min_gap_loc = 1;
    let extractor = make_extractor(config);

    let span = UncoveredSpan {
        path: path.clone(),
        start: 1,
        end: 6,
        hits: Some(0),
    };

    let chunked = extractor
        .chunk_spans_python(&path, &[span])
        .expect("python chunking");

    assert!(chunked.len() >= 2);
    assert_eq!(chunked[0].start, 1);
    assert!(chunked.iter().any(|s| s.start > 1));
}
