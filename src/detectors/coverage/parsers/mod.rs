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

/// Compute line coverage from condition-coverage attribute.
fn apply_condition_coverage(cond: &str, hits: &mut usize, is_covered: &mut bool) {
    if let Some((covered, total)) = parse_condition_coverage(cond) {
        *hits = (*hits).max(covered);
        *is_covered = if total == 0 || covered >= total {
            *is_covered || covered > 0
        } else {
            *is_covered || covered > 0
        };
    }
}

/// Extract line coverage from XML tag attributes.
fn extract_line_from_tag(tag: &BytesStart<'_>) -> Option<LineCoverage> {
    let line_number = attribute_value(tag, b"number").and_then(|v| v.parse::<usize>().ok())?;
    let mut hits = attribute_value(tag, b"hits")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let mut is_covered = hits > 0;

    if let Some(cond) = attribute_value(tag, b"condition-coverage") {
        apply_condition_coverage(&cond, &mut hits, &mut is_covered);
    }

    if let Some(branch) = attribute_value(tag, b"branch") {
        if branch.eq_ignore_ascii_case("true") && hits == 0 {
            is_covered = false;
        }
    }

    Some(LineCoverage { line_number, hits, is_covered })
}

/// Extract class file path from tag and package context.
fn extract_class_path(tag: &BytesStart<'_>, package: Option<&str>) -> Option<PathBuf> {
    let mut name = attribute_value(tag, b"filename").or_else(|| attribute_value(tag, b"name"))?;
    if let Some(pkg) = package {
        if !name.contains('/') && !name.contains('\\') {
            name = format!("{}/{}", pkg.replace('.', "/"), name);
        }
    }
    Some(normalize_report_path(&name))
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
                    current_file = extract_class_path(&tag, current_package.as_deref());
                }
                b"line" => {
                    if let Some((file, line)) = current_file.clone().zip(extract_line_from_tag(&tag)) {
                        insert_line(&mut files, file, line);
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

/// Extract line coverage from JaCoCo line tag attributes.
fn extract_jacoco_line(tag: &BytesStart<'_>) -> Option<LineCoverage> {
    let line_number = attribute_value(tag, b"nr").and_then(|v| v.parse::<usize>().ok())?;
    let covered_instr = attribute_value(tag, b"ci")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let missed_instr = attribute_value(tag, b"mi")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let covered_branches = attribute_value(tag, b"cb")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    let hits = covered_instr + covered_branches;
    let is_covered = hits > 0 && missed_instr == 0;

    Some(LineCoverage { line_number, hits, is_covered })
}

/// Extract sourcefile path from tag and package context.
fn extract_sourcefile_path(tag: &BytesStart<'_>, package: Option<&str>) -> Option<PathBuf> {
    let name = attribute_value(tag, b"name")?;
    let joined = match package {
        Some(pkg) => format!("{}/{}", pkg.replace('.', "/"), name),
        None => name,
    };
    Some(normalize_report_path(&joined))
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
                    current_file = extract_sourcefile_path(&tag, current_package.as_deref());
                }
                b"line" => {
                    if let Some((file, line)) = current_file.clone().zip(extract_jacoco_line(&tag)) {
                        insert_line(&mut files, file, line);
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

/// Parse a LCOV DA: line into LineCoverage.
fn parse_lcov_da_line(rest: &str) -> Option<LineCoverage> {
    let mut parts = rest.split(',');
    let line_number = parts.next()?.parse::<usize>().ok()?;
    let hits = parts.next()?.parse::<usize>().ok()?;
    Some(LineCoverage {
        line_number,
        hits,
        is_covered: hits > 0,
    })
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
            let Some(file) = current_file.clone() else { continue };
            let Some(coverage) = parse_lcov_da_line(rest) else { continue };
            insert_line(&mut files, file, coverage);
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
        Value::Object(map) => parse_istanbul_object(map, files),
        Value::Array(entries) => {
            for entry in entries {
                parse_istanbul_value(entry, files)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Keys to skip when recursively processing Istanbul objects.
const ISTANBUL_SKIP_KEYS: &[&str] = &["data", "l", "lines", "s", "statementMap"];

fn parse_istanbul_object(
    map: &serde_json::Map<String, Value>,
    files: &mut HashMap<PathBuf, BTreeMap<usize, LineCoverage>>,
) -> Result<()> {
    // Handle nested "data" field first
    if let Some(data) = map.get("data") {
        parse_istanbul_value(data, files)?;
    }

    // Try to extract file path and coverage data
    if let Some(path) = extract_istanbul_path(map) {
        extract_istanbul_coverage(map, &path, files)?;
    }

    // Recurse into child objects
    for (key, child) in map {
        if ISTANBUL_SKIP_KEYS.contains(&key.as_str()) {
            continue;
        }
        parse_istanbul_value(child, files)?;
    }
    Ok(())
}

fn extract_istanbul_path(map: &serde_json::Map<String, Value>) -> Option<PathBuf> {
    map.get("path")
        .or_else(|| map.get("file"))
        .or_else(|| map.get("url"))
        .and_then(|val| val.as_str())
        .map(normalize_report_path)
}

fn extract_istanbul_coverage(
    map: &serde_json::Map<String, Value>,
    path: &PathBuf,
    files: &mut HashMap<PathBuf, BTreeMap<usize, LineCoverage>>,
) -> Result<()> {
    if let Some(lines) = map.get("l").or_else(|| map.get("lines")) {
        parse_istanbul_lines(lines, path, files);
        return Ok(());
    }

    let Some(statements) = map.get("statementMap") else {
        return Ok(());
    };
    let Some(statement_hits) = map.get("s") else {
        return Ok(());
    };
    parse_istanbul_statements(statements, statement_hits, path, files)
}

/// Create LineCoverage from line number and JSON hits value.
fn line_coverage_from_hits(line_number: usize, hits_value: &Value) -> LineCoverage {
    let hits = hits_value.as_i64().unwrap_or(0).max(0) as usize;
    LineCoverage {
        line_number,
        hits,
        is_covered: hits > 0,
    }
}

fn parse_istanbul_lines(
    lines_value: &Value,
    path: &PathBuf,
    files: &mut HashMap<PathBuf, BTreeMap<usize, LineCoverage>>,
) {
    match lines_value {
        Value::Object(map) => {
            for (line_str, hits_value) in map {
                let Some(line_number) = line_str.parse::<usize>().ok() else { continue };
                insert_line(files, path.clone(), line_coverage_from_hits(line_number, hits_value));
            }
        }
        Value::Array(list) => {
            for (idx, hits_value) in list.iter().enumerate() {
                insert_line(files, path.clone(), line_coverage_from_hits(idx + 1, hits_value));
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
mod tests;
