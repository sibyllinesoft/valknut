use crate::core::errors::{Result, ValknutError};
use crate::detectors::coverage::types::{CoverageFormat, FileCoverage, LineCoverage};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// Parse a coverage report, returning the detected format and extracted file coverage.
pub fn parse_report(path: &Path) -> Result<(CoverageFormat, Vec<FileCoverage>)> {
    let bytes = fs::read(path).map_err(|err| {
        ValknutError::io(
            format!("Failed to read coverage report at {}", path.display()),
            err,
        )
    })?;

    let format = detect_format(path, &bytes);

    let files = match format {
        CoverageFormat::CoveragePyXml | CoverageFormat::Cobertura => {
            parse_cobertura_like_xml(&bytes)
        }
        CoverageFormat::JaCoCo => parse_jacoco_xml(&bytes),
        CoverageFormat::Lcov => parse_lcov(&bytes),
        CoverageFormat::IstanbulJson => parse_istanbul_json(&bytes),
        CoverageFormat::Tarpaulin => parse_tarpaulin_json(&bytes),
        CoverageFormat::Unknown => Err(ValknutError::validation(format!(
            "Unsupported or unknown coverage report format: {}",
            path.display()
        ))),
    }?;

    Ok((format, files))
}

/// Attempt to detect the coverage report format using the file extension and the leading content bytes.
fn detect_format(path: &Path, bytes: &[u8]) -> CoverageFormat {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_ascii_lowercase();
        match ext_lower.as_str() {
            "info" => return CoverageFormat::Lcov,
            "json" => {
                // For JSON files, we need to detect between Istanbul and Tarpaulin
                if let Some(format) = detect_json_format(bytes) {
                    return format;
                }
                return CoverageFormat::IstanbulJson;
            }
            "xml" => { /* fall through to content-based detection */ }
            _ => {}
        }
    }

    let snippet = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let trimmed = snippet.trim_start();

    if trimmed.starts_with('{') {
        // Content-based JSON detection
        if let Some(format) = detect_json_format(bytes) {
            return format;
        }
        return CoverageFormat::IstanbulJson;
    }

    if trimmed.contains("TN:") || trimmed.contains("SF:") {
        return CoverageFormat::Lcov;
    }

    // Basic XML detection by peeking at the first start tag
    let mut reader = Reader::from_reader(bytes);
    reader.trim_text(true);
    let mut buf = Vec::new();
    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(tag) | Event::Empty(tag) => {
                return match tag.name().as_ref() {
                    b"coverage" => CoverageFormat::CoveragePyXml,
                    b"report" => CoverageFormat::JaCoCo,
                    _ => CoverageFormat::Cobertura,
                };
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    CoverageFormat::Unknown
}

fn normalize_report_path(path: &str) -> PathBuf {
    let trimmed = path.trim().trim_matches('"');
    let without_prefix = trimmed.strip_prefix("./").unwrap_or(trimmed);
    let normalized = without_prefix.replace('\\', "/");
    PathBuf::from(normalized)
}

fn insert_line(
    files: &mut HashMap<PathBuf, BTreeMap<usize, LineCoverage>>,
    path: PathBuf,
    line: LineCoverage,
) {
    let entry = files.entry(path).or_default();
    entry
        .entry(line.line_number)
        .and_modify(|existing| {
            existing.hits = existing.hits.max(line.hits);
            existing.is_covered |= line.is_covered;
        })
        .or_insert(line);
}

fn finalize_files_map(files: HashMap<PathBuf, BTreeMap<usize, LineCoverage>>) -> Vec<FileCoverage> {
    let mut result = Vec::with_capacity(files.len());
    for (path, lines_map) in files {
        let lines: Vec<_> = lines_map.into_values().collect();
        result.push(FileCoverage { path, lines });
    }
    result
}

fn parse_condition_coverage(value: &str) -> Option<(usize, usize)> {
    let start = value.find('(')?;
    let end = value[start..].find(')')? + start;
    let fraction = value[(start + 1)..end].trim();
    let mut parts = fraction.split('/');
    let covered = parts.next()?.trim().parse::<usize>().ok()?;
    let total = parts.next()?.trim().parse::<usize>().ok()?;
    Some((covered, total))
}

fn parse_cobertura_like_xml(bytes: &[u8]) -> Result<Vec<FileCoverage>> {
    let mut reader = Reader::from_reader(bytes);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut current_package: Option<String> = None;
    let mut current_file: Option<PathBuf> = None;
    let mut files: HashMap<PathBuf, BTreeMap<usize, LineCoverage>> = HashMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => match tag.name().as_ref() {
                b"package" => {
                    current_package =
                        attribute_value(&tag, b"name").or_else(|| attribute_value(&tag, b"path"));
                }
                b"class" => {
                    let mut filename = attribute_value(&tag, b"filename")
                        .or_else(|| attribute_value(&tag, b"name"));

                    if let Some(mut name) = filename.take() {
                        if let Some(package) = &current_package {
                            if !name.contains('/') && !name.contains('\\') {
                                name = format!("{}/{}", package.replace('.', "/"), name);
                            }
                        }
                        current_file = Some(normalize_report_path(&name));
                    }
                }
                b"line" => {
                    if let Some(file) = current_file.clone() {
                        let line_no =
                            attribute_value(&tag, b"number").and_then(|v| v.parse::<usize>().ok());
                        let hits = attribute_value(&tag, b"hits")
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(0);

                        if let Some(line_number) = line_no {
                            let mut line_hits = hits;
                            let mut is_covered = hits > 0;

                            if let Some(cond) = attribute_value(&tag, b"condition-coverage") {
                                if let Some((covered, total)) = parse_condition_coverage(&cond) {
                                    line_hits = line_hits.max(covered);
                                    if total == 0 {
                                        is_covered |= covered > 0;
                                    } else if covered >= total {
                                        is_covered = true;
                                    } else {
                                        is_covered |= covered > 0;
                                    }
                                }
                            }

                            if let Some(branch) = attribute_value(&tag, b"branch") {
                                if branch.eq_ignore_ascii_case("true") && line_hits == 0 {
                                    is_covered = false;
                                }
                            }

                            insert_line(
                                &mut files,
                                file,
                                LineCoverage {
                                    line_number,
                                    hits: line_hits,
                                    is_covered,
                                },
                            );
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(tag)) => match tag.name().as_ref() {
                b"package" => current_package = None,
                b"class" => current_file = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(ValknutError::parse(
                    "xml",
                    format!("Failed to parse Cobertura-style coverage XML: {}", err),
                ));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(finalize_files_map(files))
}

fn parse_jacoco_xml(bytes: &[u8]) -> Result<Vec<FileCoverage>> {
    let mut reader = Reader::from_reader(bytes);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut current_package: Option<String> = None;
    let mut current_file: Option<PathBuf> = None;
    let mut files: HashMap<PathBuf, BTreeMap<usize, LineCoverage>> = HashMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => match tag.name().as_ref() {
                b"package" => {
                    current_package = attribute_value(&tag, b"name");
                }
                b"sourcefile" => {
                    if let Some(name) = attribute_value(&tag, b"name") {
                        let joined = if let Some(package) = &current_package {
                            format!("{}/{}", package.replace('.', "/"), name)
                        } else {
                            name
                        };
                        current_file = Some(normalize_report_path(&joined));
                    }
                }
                b"line" => {
                    if let Some(file) = current_file.clone() {
                        let line_no =
                            attribute_value(&tag, b"nr").and_then(|v| v.parse::<usize>().ok());
                        if let Some(line_number) = line_no {
                            let covered_instr = attribute_value(&tag, b"ci")
                                .and_then(|v| v.parse::<usize>().ok())
                                .unwrap_or(0);
                            let missed_instr = attribute_value(&tag, b"mi")
                                .and_then(|v| v.parse::<usize>().ok())
                                .unwrap_or(0);
                            let covered_branches = attribute_value(&tag, b"cb")
                                .and_then(|v| v.parse::<usize>().ok())
                                .unwrap_or(0);

                            let hits = covered_instr + covered_branches;
                            let is_covered = hits > 0 && missed_instr == 0;

                            insert_line(
                                &mut files,
                                file,
                                LineCoverage {
                                    line_number,
                                    hits,
                                    is_covered,
                                },
                            );
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(tag)) => match tag.name().as_ref() {
                b"package" => current_package = None,
                b"sourcefile" => current_file = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(ValknutError::parse(
                    "xml",
                    format!("Failed to parse JaCoCo XML: {}", err),
                ));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(finalize_files_map(files))
}

fn parse_lcov(bytes: &[u8]) -> Result<Vec<FileCoverage>> {
    let content = String::from_utf8_lossy(bytes);
    let mut current_file: Option<PathBuf> = None;
    let mut files: HashMap<PathBuf, BTreeMap<usize, LineCoverage>> = HashMap::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("SF:") {
            current_file = Some(normalize_report_path(rest));
            continue;
        }
        if let Some(rest) = line.strip_prefix("DA:") {
            if let Some(file) = current_file.clone() {
                let mut parts = rest.split(',');
                if let (Some(line_str), Some(hit_str)) = (parts.next(), parts.next()) {
                    if let (Ok(line_number), Ok(hits)) =
                        (line_str.parse::<usize>(), hit_str.parse::<usize>())
                    {
                        insert_line(
                            &mut files,
                            file,
                            LineCoverage {
                                line_number,
                                hits,
                                is_covered: hits > 0,
                            },
                        );
                    }
                }
            }
        }
    }

    Ok(finalize_files_map(files))
}

fn parse_istanbul_json(bytes: &[u8]) -> Result<Vec<FileCoverage>> {
    let root: Value = serde_json::from_slice(bytes).map_err(|err| ValknutError::Serialization {
        message: format!("Failed to parse Istanbul JSON coverage: {}", err),
        data_type: Some("istanbul_json".to_string()),
        source: Some(Box::new(err)),
    })?;

    let mut files: HashMap<PathBuf, BTreeMap<usize, LineCoverage>> = HashMap::new();
    parse_istanbul_value(&root, &mut files)?;
    Ok(finalize_files_map(files))
}

fn parse_istanbul_value(
    value: &Value,
    files: &mut HashMap<PathBuf, BTreeMap<usize, LineCoverage>>,
) -> Result<()> {
    match value {
        Value::Object(map) => {
            if let Some(data) = map.get("data") {
                parse_istanbul_value(data, files)?;
            }

            let path_value = map
                .get("path")
                .or_else(|| map.get("file"))
                .or_else(|| map.get("url"))
                .and_then(|val| val.as_str());

            if let Some(path_str) = path_value {
                let path = normalize_report_path(path_str);

                if let Some(lines) = map.get("l").or_else(|| map.get("lines")) {
                    parse_istanbul_lines(lines, &path, files);
                } else if let Some(statements) = map.get("statementMap") {
                    if let Some(statement_hits) = map.get("s") {
                        parse_istanbul_statements(statements, statement_hits, &path, files)?;
                    }
                }
            }

            for (key, child) in map {
                if key == "data"
                    || key == "l"
                    || key == "lines"
                    || key == "s"
                    || key == "statementMap"
                {
                    continue;
                }
                parse_istanbul_value(child, files)?;
            }
        }
        Value::Array(entries) => {
            for entry in entries {
                parse_istanbul_value(entry, files)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_istanbul_lines(
    lines_value: &Value,
    path: &PathBuf,
    files: &mut HashMap<PathBuf, BTreeMap<usize, LineCoverage>>,
) {
    match lines_value {
        Value::Object(map) => {
            for (line_str, hits_value) in map {
                if let Ok(line_number) = line_str.parse::<usize>() {
                    let hits = hits_value.as_i64().unwrap_or(0).max(0) as usize;
                    insert_line(
                        files,
                        path.clone(),
                        LineCoverage {
                            line_number,
                            hits,
                            is_covered: hits > 0,
                        },
                    );
                }
            }
        }
        Value::Array(list) => {
            for (idx, hits_value) in list.iter().enumerate() {
                let line_number = idx + 1;
                let hits = hits_value.as_i64().unwrap_or(0).max(0) as usize;
                insert_line(
                    files,
                    path.clone(),
                    LineCoverage {
                        line_number,
                        hits,
                        is_covered: hits > 0,
                    },
                );
            }
        }
        _ => {}
    }
}

fn parse_istanbul_statements(
    statement_map: &Value,
    statement_hits: &Value,
    path: &PathBuf,
    files: &mut HashMap<PathBuf, BTreeMap<usize, LineCoverage>>,
) -> Result<()> {
    let map = statement_map
        .as_object()
        .ok_or_else(|| ValknutError::validation("Invalid statementMap payload"))?;
    let hits_map = statement_hits
        .as_object()
        .ok_or_else(|| ValknutError::validation("Invalid statement counts payload"))?;

    for (id, location) in map {
        if let Some(hits_value) = hits_map.get(id) {
            let hits = hits_value.as_i64().unwrap_or(0).max(0) as usize;
            let line_number = location
                .get("start")
                .and_then(|start| start.get("line"))
                .and_then(|line| line.as_u64())
                .unwrap_or(0) as usize;

            if line_number > 0 {
                insert_line(
                    files,
                    path.clone(),
                    LineCoverage {
                        line_number,
                        hits,
                        is_covered: hits > 0,
                    },
                );
            }
        }
    }

    Ok(())
}

fn attribute_value(tag: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    tag.attributes()
        .with_checks(false)
        .flatten()
        .find(|attr| attr.key.as_ref() == name)
        .and_then(|attr| String::from_utf8(attr.value.into_owned()).ok())
}

/// Detect JSON coverage format by inspecting the structure.
/// Returns Some(format) if detected, None to fall back to default.
fn detect_json_format(bytes: &[u8]) -> Option<CoverageFormat> {
    // Quick heuristic check using early patterns in the file
    // Note: Tarpaulin files include full source code as "content" which can be very large,
    // so "traces" may not appear until much later in the file. Instead, detect by:
    // 1. Presence of "files" array with "path" as an array (unique to tarpaulin)
    // 2. Look at first ~1KB which should contain the structure indicators
    let snippet = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);

    // Tarpaulin has "files":[{"path":[ - path as array is distinctive
    // Istanbul has "path":"string" (path as string)
    if snippet.contains("\"files\"") && snippet.contains("\"path\":[") {
        return Some(CoverageFormat::Tarpaulin);
    }

    // Also check for larger window with traces for files where content is small
    let larger_snippet = String::from_utf8_lossy(&bytes[..bytes.len().min(65536)]);
    if larger_snippet.contains("\"files\"") && larger_snippet.contains("\"traces\"") {
        return Some(CoverageFormat::Tarpaulin);
    }

    None
}

/// Parse Tarpaulin JSON coverage format.
/// Structure:
/// {
///   "files": [
///     {
///       "path": ["/", "home", "user", "project", "src", "lib.rs"],
///       "content": "...",
///       "traces": [
///         {"line": 10, "address": [...], "length": 1, "stats": {"Line": 5}},
///         {"line": 12, "address": [...], "length": 1, "stats": null}
///       ],
///       "covered": 10,
///       "coverable": 20
///     }
///   ]
/// }
fn parse_tarpaulin_json(bytes: &[u8]) -> Result<Vec<FileCoverage>> {
    let root: Value = serde_json::from_slice(bytes).map_err(|err| ValknutError::Serialization {
        message: format!("Failed to parse Tarpaulin JSON coverage: {}", err),
        data_type: Some("tarpaulin_json".to_string()),
        source: Some(Box::new(err)),
    })?;

    let mut files: HashMap<PathBuf, BTreeMap<usize, LineCoverage>> = HashMap::new();

    if let Some(files_array) = root.get("files").and_then(|v| v.as_array()) {
        for file_entry in files_array {
            // Path can be an array of path segments or a string
            let path = extract_tarpaulin_path(file_entry);
            if path.is_none() {
                continue;
            }
            let path = path.unwrap();

            // Parse traces array
            if let Some(traces) = file_entry.get("traces").and_then(|v| v.as_array()) {
                for trace in traces {
                    if let Some(line_number) = trace.get("line").and_then(|v| v.as_u64()) {
                        let line_number = line_number as usize;

                        // stats can be null (uncovered) or {"Line": hits}
                        let hits = trace
                            .get("stats")
                            .and_then(|s| s.get("Line"))
                            .and_then(|h| h.as_u64())
                            .unwrap_or(0) as usize;

                        insert_line(
                            &mut files,
                            path.clone(),
                            LineCoverage {
                                line_number,
                                hits,
                                is_covered: hits > 0,
                            },
                        );
                    }
                }
            }
        }
    }

    Ok(finalize_files_map(files))
}

/// Extract path from Tarpaulin file entry.
/// Path can be an array of segments or a string.
fn extract_tarpaulin_path(file_entry: &Value) -> Option<PathBuf> {
    if let Some(path_array) = file_entry.get("path").and_then(|v| v.as_array()) {
        // Path is array of segments: ["/", "home", "user", "project", "src", "lib.rs"]
        let segments: Vec<&str> = path_array.iter().filter_map(|s| s.as_str()).collect();

        if segments.is_empty() {
            return None;
        }

        // Join segments into path
        let path_str = if segments[0] == "/" {
            // Absolute path
            format!("/{}", segments[1..].join("/"))
        } else {
            segments.join("/")
        };

        Some(normalize_report_path(&path_str))
    } else if let Some(path_str) = file_entry.get("path").and_then(|v| v.as_str()) {
        // Path is a string
        Some(normalize_report_path(path_str))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
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
}
