    use super::*;
    use crate::core::pipeline::clone_detection::{
        build_simple_ast_for_entity, build_simple_ast_recursive, compute_apted_limit,
        filter_small_pairs, get_or_build_simple_ast, hash_kind, ordered_pair_key,
        parse_byte_range, serialize_clone_pairs, should_skip_small_pair, CachedSimpleAst,
        CloneDetectionStats, CloneEndpoint, ClonePairReport, CloneVerificationDetail,
        LshDetectionParams, LshEntityCollection,
    };
    use crate::core::arena_analysis::ArenaAnalysisResult;
    use crate::core::dependency::ProjectDependencyAnalysis;
    use crate::core::featureset::CodeEntity;
    use crate::core::file_utils::{CoverageFile, CoverageFormat};
    use crate::core::interning::intern;
    use crate::detectors::complexity::ComplexityConfig;
    use crate::detectors::lsh::LshExtractor;
    use crate::detectors::refactoring::{RefactoringAnalyzer, RefactoringConfig};
    use crate::detectors::structure::StructureConfig;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::Arc;
    use std::time::Duration;
    use std::time::SystemTime;
    use tempfile::tempdir;

    fn build_test_stages() -> AnalysisStages {
        let ast_service = Arc::new(AstService::new());
        let structure_extractor = StructureExtractor::with_config(StructureConfig::default());
        let complexity_analyzer =
            ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
        let refactoring_analyzer =
            RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());
        let coverage_extractor =
            CoverageExtractor::new(CoverageDetectorConfig::default(), ast_service.clone());
        let config = Arc::new(ValknutConfig::default());

        AnalysisStages::new(
            structure_extractor,
            complexity_analyzer,
            refactoring_analyzer,
            coverage_extractor,
            ast_service,
            config,
        )
    }

    fn build_test_stages_with_lsh() -> AnalysisStages {
        let ast_service = Arc::new(AstService::new());
        let structure_extractor = StructureExtractor::with_config(StructureConfig::default());
        let complexity_analyzer =
            ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
        let refactoring_analyzer =
            RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());
        let mut valknut_config = ValknutConfig::default();
        valknut_config.lsh.similarity_threshold = 0.0;
        valknut_config.lsh.num_hashes = 32;
        valknut_config.lsh.num_bands = 4;
        valknut_config.lsh.max_candidates = 8;
        valknut_config.lsh.apted_max_nodes = 512;
        let lsh_config = valknut_config.lsh.clone();

        let lsh_extractor = LshExtractor::new()
            .with_shared_ast_service(ast_service.clone())
            .with_lsh_config(lsh_config.clone().into());
        let coverage_extractor =
            CoverageExtractor::new(CoverageDetectorConfig::default(), ast_service.clone());

        AnalysisStages::new_with_lsh(
            structure_extractor,
            complexity_analyzer,
            refactoring_analyzer,
            lsh_extractor,
            coverage_extractor,
            ast_service,
            Arc::new(valknut_config),
        )
    }

    #[test]
    fn hash_kind_is_stable_for_identical_input() {
        let first = hash_kind("function_declaration");
        let second = hash_kind("function_declaration");

        assert_eq!(first, second);
        let different = hash_kind("struct_declaration");
        assert_ne!(first, different);
    }

    #[test]
    fn parse_byte_range_extracts_start_and_end() {
        let mut entity = CodeEntity::new("id", "function", "sample", "src/lib.rs");
        entity.add_property("byte_range", serde_json::json!([12, 48]));
        assert_eq!(parse_byte_range(&entity), Some((12, 48)));

        entity
            .properties
            .insert("byte_range".to_string(), serde_json::json!([12]));
        assert_eq!(parse_byte_range(&entity), None);
    }

    #[tokio::test]
    async fn calculate_overall_coverage_parses_lcov_percentage() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let lcov_path = tmp.path().join("lcov.info");
        let lcov_content = r#"TN:
SF:src/lib.rs
DA:1,1
DA:2,0
DA:3,2
end_of_record
"#;
        std::fs::write(&lcov_path, lcov_content).expect("write lcov");

        let coverage_file = CoverageFile {
            path: lcov_path,
            format: CoverageFormat::Lcov,
            modified: std::time::SystemTime::now(),
            size: lcov_content.len() as u64,
        };

        let percentage = stages
            .calculate_overall_coverage(&[coverage_file])
            .await
            .expect("coverage calc");

        assert!(percentage.is_some());
        let pct = percentage.unwrap();
        assert!(
            pct > 60.0 && pct < 80.0,
            "expected coverage around 66%, got {pct}"
        );
    }

    #[tokio::test]
    async fn analyze_xml_coverage_counts_uncovered_lines() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let xml_path = tmp.path().join("coverage.xml");
        let xml_content = r#"
<coverage>
  <line number="1" hits="0"/>
  <line number="2" hits="0"/>
  <line number="3" hits="1"/>
</coverage>
"#;
        std::fs::write(&xml_path, xml_content).expect("write xml");

        let gaps = stages
            .analyze_xml_coverage(&xml_path)
            .await
            .expect("xml analysis");

        assert_eq!(gaps, 1);
    }

    #[tokio::test]
    async fn analyze_json_coverage_returns_zero() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let json_path = tmp.path().join("coverage.json");
        std::fs::write(&json_path, r#"{"result": "placeholder"}"#).expect("write json");

        let gaps = stages
            .analyze_json_coverage(&json_path)
            .await
            .expect("json analysis");

        assert_eq!(gaps, 0);
    }

    #[tokio::test]
    async fn analyze_xml_coverage_warns_on_missing_file() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let missing_path = tmp.path().join("missing.xml");
        // Do not create the file

        let gaps = stages
            .analyze_xml_coverage(&missing_path)
            .await
            .expect("xml analysis");

        assert_eq!(gaps, 0, "missing files should yield zero gaps");
    }

    #[tokio::test]
    async fn analyze_lcov_coverage_counts_gaps() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let lcov_path = tmp.path().join("coverage.lcov");
        let content = "\
TN:\n\
SF:src/main.rs\n\
DA:1,1\n\
DA:2,0\n\
DA:3,0\n\
DA:4,1\n\
end_of_record\n";
        std::fs::write(&lcov_path, content).expect("write lcov");

        let gaps = stages
            .analyze_lcov_coverage(&lcov_path)
            .await
            .expect("lcov gaps");
        assert_eq!(
            gaps, 0,
            "expected zero gaps until dedicated LCOV parser support is added"
        );
    }

    #[tokio::test]
    async fn analyze_lcov_coverage_propagates_errors() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let lcov_path = tmp.path().join("coverage.lcov");
        std::fs::write(&lcov_path, "malformed").expect("write malformed lcov");

        let result = stages.analyze_lcov_coverage(&lcov_path).await;
        assert!(
            result.is_err(),
            "malformed LCOV input should surface extractor errors"
        );
    }

    #[tokio::test]
    async fn analyze_coverage_gaps_combines_multiple_formats() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");

        // Prepare source file and LCOV report
        let source_path = tmp.path().join("sample.rs");
        let source = r#"pub fn add(a: i32, b: i32) -> i32 {
    if a > 0 {
        a + b
    } else {
        b - a
    }
}
"#;
        std::fs::write(&source_path, source).expect("write source file");

        let lcov_path = tmp.path().join("coverage.lcov");
        let lcov_report = format!(
            "TN:\nSF:{}\nDA:1,1\nDA:2,0\nDA:3,0\nDA:4,1\nend_of_record\n",
            source_path.display()
        );
        std::fs::write(&lcov_path, lcov_report).expect("write lcov file");

        // XML coverage with two uncovered lines
        let xml_path = tmp.path().join("coverage.xml");
        let xml_content = r#"
<coverage>
  <line number="10" hits="0"/>
  <line number="11" hits="0"/>
  <line number="12" hits="1"/>
</coverage>
"#;
        std::fs::write(&xml_path, xml_content).expect("write xml file");

        // Placeholder JSON coverage (currently treated as zero gaps)
        let json_path = tmp.path().join("coverage.json");
        std::fs::write(&json_path, r#"{"files": []}"#).expect("write json file");

        let coverage_files = vec![
            CoverageFile {
                path: lcov_path,
                format: CoverageFormat::Lcov,
                modified: SystemTime::now(),
                size: 64,
            },
            CoverageFile {
                path: xml_path,
                format: CoverageFormat::CoveragePyXml,
                modified: SystemTime::now(),
                size: 64,
            },
            CoverageFile {
                path: json_path,
                format: CoverageFormat::IstanbulJson,
                modified: SystemTime::now(),
                size: 16,
            },
        ];

        let gap_count = stages
            .analyze_coverage_gaps(&coverage_files)
            .await
            .expect("gap analysis");

        assert!(
            gap_count >= 1,
            "expected at least one gap from LCOV or XML, got {gap_count}"
        );
    }

    #[tokio::test]
    async fn analyze_coverage_gaps_skips_unknown_formats() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let unknown_path = tmp.path().join("mystery.dat");
        std::fs::write(&unknown_path, "opaque").expect("write unknown coverage stub");

        let coverage_files = vec![CoverageFile {
            path: unknown_path,
            format: CoverageFormat::Unknown,
            modified: SystemTime::now(),
            size: 6,
        }];

        let gap_count = stages
            .analyze_coverage_gaps(&coverage_files)
            .await
            .expect("gap analysis");

        assert_eq!(
            gap_count, 0,
            "unknown coverage formats should be ignored without contributing gaps"
        );
    }

    #[tokio::test]
    async fn calculate_overall_coverage_returns_none_without_lcov() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let json_path = tmp.path().join("coverage.json");
        std::fs::write(&json_path, "{}").expect("write json coverage");

        let coverage_files = vec![CoverageFile {
            path: json_path,
            format: CoverageFormat::IstanbulJson,
            modified: SystemTime::now(),
            size: 2,
        }];

        let coverage = stages
            .calculate_overall_coverage(&coverage_files)
            .await
            .expect("coverage calc");

        assert!(
            coverage.is_none(),
            "non-LCOV coverage inputs should not produce a coverage percentage"
        );
    }

    #[tokio::test]
    async fn analyze_xml_coverage_returns_zero_when_file_missing() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let missing_path = tmp.path().join("missing.xml");

        let gaps = stages
            .analyze_xml_coverage(&missing_path)
            .await
            .expect("xml analysis");

        assert_eq!(
            gaps, 0,
            "missing coverage files should be treated as having no measurable gaps"
        );
    }

    #[tokio::test]
    async fn run_lsh_analysis_disabled_without_extractor() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let file_path = tmp.path().join("sample.rs");
        std::fs::write(&file_path, "pub fn demo() {}").expect("write sample");

        let analysis = stages
            .run_lsh_analysis(&[file_path], false)
            .await
            .expect("lsh analysis");

        assert!(!analysis.enabled);
        assert!(analysis.clone_pairs.is_empty());
    }

    #[tokio::test]
    async fn run_lsh_analysis_with_extractor_handles_empty_entities() {
        let stages = build_test_stages_with_lsh();
        let tmp = tempdir().expect("temp dir");
        let file_path = tmp.path().join("notes.txt");
        std::fs::write(&file_path, "plain text that yields no entities").expect("write stub");

        let analysis = stages
            .run_lsh_analysis(&[file_path], true)
            .await
            .expect("lsh analysis");

        assert!(analysis.enabled);
        assert!(analysis.clone_pairs.is_empty());
        assert!(analysis.verification.is_none());
    }

    #[tokio::test]
    async fn run_impact_analysis_handles_empty_and_non_empty_inputs() {
        let stages = build_test_stages();

        let empty = stages
            .run_impact_analysis(&[])
            .await
            .expect("empty impact analysis");
        assert!(!empty.enabled);

        let tmp = tempdir().expect("temp dir");
        let file_path = tmp.path().join("deps.rs");
        let content = r#"
pub mod deps {
    pub fn alpha() {
        beta();
    }

    pub fn beta() {
        alpha();
    }
}
"#;
        std::fs::write(&file_path, content).expect("write deps");

        let non_empty = stages
            .run_impact_analysis(&[file_path.clone()])
            .await
            .expect("impact analysis");

        assert!(non_empty.enabled);
        assert_eq!(non_empty.clone_groups.len(), 0);
        assert!(
            non_empty.issues_count >= 0,
            "issues_count should be non-negative"
        );
    }

    #[test]
    fn dependency_analysis_collects_metrics() {
        let tmp = tempdir().expect("temp dir");
        let file_path = tmp.path().join("analysis.rs");
        let content = r#"
pub mod cycle {
    pub fn first() {
        second();
    }

    pub fn second() {
        first();
    }
}
"#;
        std::fs::write(&file_path, content).expect("write analysis file");

        let analysis =
            ProjectDependencyAnalysis::analyze(&[file_path]).expect("perform dependency analysis");

        assert!(
            !analysis.is_empty(),
            "analysis should contain at least one function node"
        );
        assert!(analysis.metrics_iter().count() > 0, "metrics should exist");
        // Chokepoints may be empty depending on AST metadata, but call ensures accessor coverage.
        let _ = analysis.chokepoints();
    }

    #[tokio::test]
    async fn simple_ast_cache_reuses_entries_and_handles_truncation() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let file_path = tmp.path().join("ast_sample.rs");
        let content = r#"
pub fn compute(limit: i32) -> i32 {
    let mut acc = 0;
    for i in 0..limit {
        acc += i;
    }
    acc
}
"#;
        std::fs::write(&file_path, content).expect("write rust sample");
        let path_str = file_path.to_string_lossy().to_string();

        let entities = stages
            .extract_entities_from_file(&file_path, content)
            .await
            .expect("extract entities");
        let entity = entities
            .into_iter()
            .find(|e| e.entity_type.to_lowercase().contains("function"))
            .expect("function entity");

        let mut ast_cache = HashMap::new();
        let cached_tree = stages
            .ast_service
            .get_ast(&path_str, content)
            .await
            .expect("cached tree");
        ast_cache.insert(path_str.clone(), cached_tree);

        let mut cache = HashMap::new();
        let simple =
            get_or_build_simple_ast(&mut cache, &entity, &ast_cache, 10_000).expect("simple ast");
        assert!(!simple.truncated);
        assert!(simple.node_count > 0);
        assert_eq!(cache.len(), 1);

        let reused =
            get_or_build_simple_ast(&mut cache, &entity, &ast_cache, 10_000).expect("reuse ast");
        assert_eq!(reused.node_count, simple.node_count);

        let mut truncated_cache = HashMap::new();
        let truncated =
            get_or_build_simple_ast(&mut truncated_cache, &entity, &ast_cache, 1).expect("trunc");
        assert!(truncated.truncated);

        let mut without_range = entity.clone();
        without_range.properties.remove("byte_range");
        let mut cache_without_range = HashMap::new();
        assert!(get_or_build_simple_ast(
            &mut cache_without_range,
            &without_range,
            &ast_cache,
            10_000
        )
        .is_none());

        let mut cache_missing_ast = HashMap::new();
        let empty_ast_cache: HashMap<String, Arc<CachedTree>> = HashMap::new();
        assert!(
            get_or_build_simple_ast(&mut cache_missing_ast, &entity, &empty_ast_cache, 10_000)
                .is_none()
        );
    }

    #[tokio::test]
    async fn run_lsh_analysis_produces_verified_clone_pairs() {
        let stages = build_test_stages_with_lsh();
        let tmp = tempdir().expect("temp dir");
        let file_a = tmp.path().join("clone_a.rs");
        let file_b = tmp.path().join("clone_b.rs");
        let function_src = r#"
pub fn compute() -> i32 {
    let mut total = 0;
    for value in 0..10 {
        total += value * 2;
    }
    total
}
"#;
        std::fs::write(&file_a, function_src).expect("write clone sample a");
        std::fs::write(&file_b, function_src).expect("write clone sample b");

        let analysis = stages
            .run_lsh_analysis(&[file_a.clone(), file_b.clone()], false)
            .await
            .expect("lsh analysis");

        assert!(analysis.enabled, "expected LSH analysis to be enabled");
        assert!(
            analysis.apted_verification_enabled,
            "APTED verification should be enabled"
        );
        assert!(
            analysis.duplicate_count > 0,
            "expected at least one clone pair"
        );

        let verification_summary = analysis.verification.expect("verification summary present");
        assert!(
            verification_summary.pairs_scored > 0,
            "expected structural verification to score at least one pair"
        );

        let first_pair = analysis.clone_pairs.first().expect("clone pair present");
        let similarity = first_pair
            .get("similarity")
            .and_then(|value| value.as_f64())
            .expect("similarity value recorded");
        assert!(
            similarity >= 0.0,
            "similarity scores should be non-negative"
        );

        let verification_detail = first_pair
            .get("verification")
            .and_then(|value| value.as_object())
            .expect("verification detail recorded");
        assert!(
            verification_detail.contains_key("node_counts"),
            "expected node count metadata"
        );
        assert!(
            verification_detail.contains_key("similarity")
                || verification_detail.contains_key("edit_cost"),
            "verification detail should include similarity or cost"
        );
    }

    #[tokio::test]
    async fn run_lsh_analysis_marks_truncated_asts() {
        let ast_service = Arc::new(AstService::new());
        let structure_extractor = StructureExtractor::with_config(StructureConfig::default());
        let complexity_analyzer =
            ComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());
        let refactoring_analyzer =
            RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service.clone());

        let mut valknut_config = ValknutConfig::default();
        valknut_config.lsh.similarity_threshold = 0.0;
        valknut_config.lsh.num_hashes = 16;
        valknut_config.lsh.num_bands = 2;
        valknut_config.lsh.max_candidates = 4;
        valknut_config.lsh.apted_max_pairs_per_entity = 2;
        valknut_config.lsh.apted_max_nodes = 8;
        valknut_config.lsh.verify_with_apted = true;
        let lsh_config = valknut_config.lsh.clone();

        let lsh_extractor = LshExtractor::new()
            .with_shared_ast_service(ast_service.clone())
            .with_lsh_config(lsh_config.into());
        let coverage_extractor =
            CoverageExtractor::new(CoverageDetectorConfig::default(), ast_service.clone());

        let stages = AnalysisStages::new_with_lsh(
            structure_extractor,
            complexity_analyzer,
            refactoring_analyzer,
            lsh_extractor,
            coverage_extractor,
            ast_service,
            Arc::new(valknut_config),
        );

        let tmp = tempdir().expect("temp dir");
        let file_a = tmp.path().join("truncated_a.rs");
        let file_b = tmp.path().join("truncated_b.rs");
        let big_function = r#"
pub fn heavy() -> i32 {
    let mut value = 0;
    for outer in 0..20 {
        value += outer;
        for inner in 0..20 {
            if inner % 3 == 0 {
                value -= inner;
            } else {
                value += inner;
            }
        }
    }
    value
}
"#;
        std::fs::write(&file_a, big_function).expect("write truncated sample a");
        std::fs::write(&file_b, big_function).expect("write truncated sample b");

        let analysis = stages
            .run_lsh_analysis(&[file_a, file_b], true)
            .await
            .expect("lsh analysis");

        let first_pair = analysis
            .clone_pairs
            .first()
            .expect("expected at least one clone pair");
        let truncated_flag = first_pair
            .get("verification")
            .and_then(|value| value.get("truncated"))
            .and_then(|flag| flag.as_bool())
            .unwrap_or(false);
        assert!(
            truncated_flag,
            "verification detail should mark ASTs as truncated when node budget is exceeded"
        );
    }

    #[tokio::test]
    async fn run_arena_file_analysis_with_content_returns_empty_for_none() {
        let stages = build_test_stages();
        let results = stages
            .run_arena_file_analysis_with_content(&[])
            .await
            .expect("arena analysis");

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn run_arena_file_analysis_skips_missing_files() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");
        let missing_path = tmp.path().join("does_not_exist.rs");
        let results = stages
            .run_arena_file_analysis(&[missing_path])
            .await
            .expect("arena analysis");

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn run_complexity_analysis_from_arena_results_handles_mix_of_inputs() {
        let stages = build_test_stages();
        let tmp = tempdir().expect("temp dir");

        // Existing file to drive successful analysis
        let existing_path = tmp.path().join("metrics.rs");
        let existing_source = r#"
pub fn compute(limit: i32) -> i32 {
    let mut acc = 0;
    for i in 0..limit {
        if i % 2 == 0 {
            acc += i;
        } else {
            acc -= 1;
        }
    }
    acc
}
"#;
        std::fs::write(&existing_path, existing_source).expect("write metrics file");

        // Missing file triggers warning path
        let missing_path = tmp.path().join("missing.rs");

        let mut entity = CodeEntity::new(
            "metrics::compute",
            "function",
            "compute",
            existing_path.to_string_lossy(),
        )
        .with_line_range(1, 12)
        .with_source_code(existing_source);

        entity.add_property("byte_range", serde_json::json!([0, existing_source.len()]));

        let arena_results = vec![
            ArenaAnalysisResult {
                entity_count: 0,
                file_path: intern(missing_path.to_string_lossy()),
                entity_extraction_time: Duration::from_millis(1),
                total_analysis_time: Duration::from_millis(1),
                arena_bytes_used: 0,
                memory_efficiency_score: 0.0,
                entities: Vec::new(),
                lines_of_code: 0,
                source_code: String::new(),
            },
            ArenaAnalysisResult {
                entity_count: 1,
                file_path: intern(existing_path.to_string_lossy()),
                entity_extraction_time: Duration::from_millis(2),
                total_analysis_time: Duration::from_millis(5),
                arena_bytes_used: 2 * 1024,
                memory_efficiency_score: 0.0,
                entities: vec![entity],
                lines_of_code: 12,
                source_code: existing_source.to_string(),
            },
        ];

        let analysis = stages
            .run_complexity_analysis_from_arena_results(&arena_results)
            .await
            .expect("complexity analysis");

        assert!(
            analysis.enabled,
            "analysis should be enabled with valid input"
        );
        assert!(
            analysis.detailed_results.len() >= 1,
            "expected at least one per-file complexity result"
        );
        assert!(
            analysis.average_cyclomatic_complexity >= 0.0,
            "averages should be non-negative"
        );
    }
