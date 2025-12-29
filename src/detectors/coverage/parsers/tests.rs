use super::*;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;
use tempfile::TempDir;

#[test]
fn test_parse_lcov_report_basic() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "TN:\nSF:src/lib.rs\nDA:1,1\nDA:2,0\nend_of_record").unwrap();

    let (format, files) = parse_report(file.path()).unwrap();
    assert_eq!(format, CoverageFormat::Lcov);
    assert_eq!(files.len(), 1);

    let coverage = &files[0];
    assert!(coverage
        .lines
        .iter()
        .any(|line| line.line_number == 1 && line.is_covered));
    assert!(coverage
        .lines
        .iter()
        .any(|line| line.line_number == 2 && !line.is_covered));
}

#[test]
fn test_parse_istanbul_json_basic() {
    let mut file = NamedTempFile::new().unwrap();
    let json = r#"{
        "src/app.js": {
            "path": "src/app.js",
            "l": {"1": 0, "2": 3}
        }
    }"#;
    write!(file, "{}", json).unwrap();

    let (format, files) = parse_report(file.path()).unwrap();
    assert_eq!(format, CoverageFormat::IstanbulJson);
    assert_eq!(files.len(), 1);
    let coverage = &files[0];
    assert_eq!(coverage.path, PathBuf::from("src/app.js"));
    assert_eq!(coverage.lines.len(), 2);
    assert!(coverage
        .lines
        .iter()
        .any(|line| line.line_number == 1 && line.hits == 0));
    assert!(coverage
        .lines
        .iter()
        .any(|line| line.line_number == 2 && line.hits == 3));
}

#[test]
fn test_parse_cobertura_with_package_and_conditions() {
    let xml = r#"
        <coverage>
          <packages>
            <package name="com.example">
              <classes>
                <class name="Foo" filename="Foo.py">
                  <lines>
                    <line number="10" hits="0" branch="true" condition-coverage="50% (1/2)" />
                  </lines>
                </class>
              </classes>
            </package>
          </packages>
        </coverage>
    "#;

    let files = parse_cobertura_like_xml(xml.as_bytes()).unwrap();
    assert_eq!(files.len(), 1);
    let coverage = &files[0];
    assert_eq!(coverage.path, PathBuf::from("com/example/Foo.py"));
    assert_eq!(coverage.lines.len(), 1);
    let line = &coverage.lines[0];
    assert_eq!(line.line_number, 10);
    assert_eq!(line.hits, 1);
    assert!(line.is_covered);
}

#[test]
fn test_parse_istanbul_nested_data_array() {
    let json = r#"{
        "data": [
            {
                "path": "lib/index.js",
                "lines": {"5": 2, "6": 0}
            }
        ]
    }"#;

    let files = parse_istanbul_json(json.as_bytes()).unwrap();
    assert_eq!(files.len(), 1);
    let coverage = &files[0];
    assert_eq!(coverage.path, PathBuf::from("lib/index.js"));
    assert_eq!(coverage.lines.len(), 2);
    assert!(coverage
        .lines
        .iter()
        .any(|line| line.line_number == 5 && line.hits == 2 && line.is_covered));
    assert!(coverage
        .lines
        .iter()
        .any(|line| line.line_number == 6 && line.hits == 0 && !line.is_covered));
}

#[test]
fn test_detect_format_unknown_returns_error() {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "garbage").unwrap();

    let err = parse_report(file.path()).unwrap_err();
    assert!(matches!(err, ValknutError::Validation { .. }));
}

#[test]
fn test_detect_format_from_extension_and_content() {
    assert_eq!(
        detect_format(Path::new("report.info"), b"SF:src/lib.rs"),
        CoverageFormat::Lcov
    );
    assert_eq!(
        detect_format(Path::new("report.json"), b"{\"path\":\"src/app.js\"}"),
        CoverageFormat::IstanbulJson
    );

    let coverage_xml = br#"<coverage></coverage>"#;
    assert_eq!(
        detect_format(Path::new("coverage.xml"), coverage_xml),
        CoverageFormat::CoveragePyXml
    );

    let jacoco_xml = br#"<report></report>"#;
    assert_eq!(
        detect_format(Path::new("coverage.xml"), jacoco_xml),
        CoverageFormat::JaCoCo
    );

    let fallback_xml = br#"<foo></foo>"#;
    assert_eq!(
        detect_format(Path::new("coverage.xml"), fallback_xml),
        CoverageFormat::Cobertura
    );

    let nonsense = br"garbage data";
    assert_eq!(
        detect_format(Path::new("coverage.dat"), nonsense),
        CoverageFormat::Unknown
    );
}

#[test]
fn test_normalize_report_path_removes_prefixes() {
    assert_eq!(
        normalize_report_path("./src\\app.rs"),
        PathBuf::from("src/app.rs")
    );
    assert_eq!(
        normalize_report_path("\"./lib/file.js\""),
        PathBuf::from("lib/file.js")
    );
}

#[test]
fn test_parse_condition_coverage_parses_fraction() {
    let fraction = parse_condition_coverage("branch (3/5)");
    assert_eq!(fraction, Some((3, 5)));
    assert!(parse_condition_coverage("no fraction").is_none());
}

#[test]
fn test_parse_cobertura_branch_without_hits_is_uncovered() {
    let xml = r#"
        <coverage>
          <classes>
            <class filename="foo.py">
              <lines>
                <line number="7" hits="0" branch="true" />
              </lines>
            </class>
          </classes>
        </coverage>
    "#;

    let files = parse_cobertura_like_xml(xml.as_bytes()).unwrap();
    let line = &files[0].lines[0];
    assert_eq!(line.line_number, 7);
    assert_eq!(line.hits, 0);
    assert!(!line.is_covered);
}

#[test]
fn test_parse_jacoco_xml_basic() {
    let xml = r#"
        <report>
          <package name="com.example">
            <sourcefile name="Foo.java">
              <line nr="12" ci="3" mi="0" cb="1" />
            </sourcefile>
          </package>
        </report>
    "#;

    let files = parse_jacoco_xml(xml.as_bytes()).unwrap();
    assert_eq!(files.len(), 1);
    let coverage = &files[0];
    assert_eq!(coverage.path, PathBuf::from("com/example/Foo.java"));
    assert_eq!(coverage.lines.len(), 1);
    let line = &coverage.lines[0];
    assert_eq!(line.line_number, 12);
    assert_eq!(line.hits, 4);
    assert!(line.is_covered);
}

#[test]
fn test_parse_lcov_merges_duplicate_entries() {
    let content = "TN:\nSF:src/lib.rs\nDA:10,1\nDA:10,3\nend_of_record\n";
    let files = parse_lcov(content.as_bytes()).unwrap();
    let line = &files[0].lines[0];
    assert_eq!(line.line_number, 10);
    assert_eq!(line.hits, 3);
    assert!(line.is_covered);
}

#[test]
fn test_parse_istanbul_statements_records_hits() {
    let statements = serde_json::json!({
        "0": { "start": { "line": 9 } },
        "1": { "start": { "line": 0 } }
    });
    let statement_hits = serde_json::json!({"0": 5, "1": -3});
    let mut files: HashMap<PathBuf, BTreeMap<usize, LineCoverage>> = HashMap::new();
    let path = PathBuf::from("src/app.js");

    parse_istanbul_statements(&statements, &statement_hits, &path, &mut files).unwrap();
    let coverage = finalize_files_map(files);
    assert_eq!(coverage.len(), 1);
    assert_eq!(coverage[0].path, path);
    assert_eq!(coverage[0].lines[0].line_number, 9);
    assert_eq!(coverage[0].lines[0].hits, 5);
    assert!(coverage[0].lines[0].is_covered);
}

#[test]
fn test_parse_report_handles_jacoco_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("jacoco.xml");
    let xml = r#"
        <report>
          <package name="demo">
            <sourcefile name="Foo.kt">
              <line nr="3" ci="1" mi="0" cb="0" />
            </sourcefile>
          </package>
        </report>
    "#;
    fs::write(&path, xml).unwrap();

    let (format, files) = parse_report(&path).unwrap();
    assert_eq!(format, CoverageFormat::JaCoCo);
    assert_eq!(files.len(), 1);
}

#[test]
fn test_detect_tarpaulin_format() {
    let tarpaulin_json = br#"{"files":[{"path":["src","lib.rs"],"traces":[]}]}"#;
    assert_eq!(
        detect_format(Path::new("tarpaulin-report.json"), tarpaulin_json),
        CoverageFormat::Tarpaulin
    );

    // Istanbul format should not be detected as Tarpaulin
    let istanbul_json = br#"{"src/app.js":{"path":"src/app.js","l":{"1":0}}}"#;
    assert_eq!(
        detect_format(Path::new("coverage.json"), istanbul_json),
        CoverageFormat::IstanbulJson
    );
}

#[test]
fn test_parse_tarpaulin_json_basic() {
    let json = r#"{
        "files": [
            {
                "path": ["src", "lib.rs"],
                "traces": [
                    {"line": 10, "stats": {"Line": 5}},
                    {"line": 12, "stats": null},
                    {"line": 15, "stats": {"Line": 0}}
                ]
            }
        ]
    }"#;

    let files = parse_tarpaulin_json(json.as_bytes()).unwrap();
    assert_eq!(files.len(), 1);

    let coverage = &files[0];
    assert_eq!(coverage.path, PathBuf::from("src/lib.rs"));
    assert_eq!(coverage.lines.len(), 3);

    // Line 10 is covered with 5 hits
    assert!(coverage
        .lines
        .iter()
        .any(|line| line.line_number == 10 && line.hits == 5 && line.is_covered));

    // Line 12 has null stats = uncovered
    assert!(coverage
        .lines
        .iter()
        .any(|line| line.line_number == 12 && line.hits == 0 && !line.is_covered));

    // Line 15 has 0 hits = uncovered
    assert!(coverage
        .lines
        .iter()
        .any(|line| line.line_number == 15 && line.hits == 0 && !line.is_covered));
}

#[test]
fn test_parse_tarpaulin_absolute_path() {
    let json = r#"{
        "files": [
            {
                "path": ["/", "home", "user", "project", "src", "main.rs"],
                "traces": [
                    {"line": 1, "stats": {"Line": 1}}
                ]
            }
        ]
    }"#;

    let files = parse_tarpaulin_json(json.as_bytes()).unwrap();
    assert_eq!(files.len(), 1);
    // Absolute path normalized - leading slash removed by normalize_report_path
    let path = &files[0].path;
    assert!(path
        .to_string_lossy()
        .contains("home/user/project/src/main.rs"));
}

#[test]
fn test_parse_tarpaulin_string_path() {
    // Tarpaulin can also use string paths
    let json = r#"{
        "files": [
            {
                "path": "src/lib.rs",
                "traces": [
                    {"line": 5, "stats": {"Line": 2}}
                ]
            }
        ]
    }"#;

    let files = parse_tarpaulin_json(json.as_bytes()).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, PathBuf::from("src/lib.rs"));
    assert_eq!(files[0].lines[0].hits, 2);
}

#[test]
fn test_parse_report_handles_tarpaulin_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("tarpaulin-report.json");
    let json = r#"{
        "files": [
            {
                "path": ["src", "main.rs"],
                "traces": [
                    {"line": 1, "stats": {"Line": 1}},
                    {"line": 2, "stats": null}
                ]
            }
        ]
    }"#;
    fs::write(&path, json).unwrap();

    let (format, files) = parse_report(&path).unwrap();
    assert_eq!(format, CoverageFormat::Tarpaulin);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].lines.len(), 2);
}
