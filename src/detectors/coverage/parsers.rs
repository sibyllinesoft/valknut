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
            "json" => return CoverageFormat::IstanbulJson,
            "xml" => { /* fall through to content-based detection */ }
            _ => {}
        }
    }

    let snippet = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let trimmed = snippet.trim_start();

    if trimmed.starts_with('{') {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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
}
