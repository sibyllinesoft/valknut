//! Agent-first JSON report construction.
//!
//! The artifact retains the public analysis result fields for compatibility and
//! adds flat, queryable entity, metric, and finding collections. It also embeds
//! the exact effective configuration used for the run.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::core::config::ValknutConfig;
use crate::core::pipeline::AnalysisResults;
use crate::detectors::complexity::{ComplexityAnalysisResult, ComplexityThresholds};

/// Current schema of the agent-first JSON additions.
pub const AGENT_REPORT_SCHEMA: &str = "valknut.analysis/2";

#[derive(Debug, Serialize)]
struct RunMetadata {
    timestamp: DateTime<Utc>,
    config_hash: String,
    config: Value,
}

#[derive(Debug, Serialize)]
struct AgentEntity {
    id: String,
    name: String,
    kind: String,
    file_path: String,
    start_line: usize,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    context: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize)]
struct MetricRow {
    subject_id: String,
    metric: &'static str,
    value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    threshold_difference: Option<f64>,
}

#[derive(Debug, Serialize)]
struct FindingRow {
    subject_id: String,
    code: String,
    severity: String,
    metric: &'static str,
    value: f64,
    threshold_difference: f64,
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct DependencyRow {
    source: String,
    module: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbols: Option<Vec<String>>,
    line: usize,
}

#[derive(Debug, Serialize)]
struct MetricDefinition {
    unit: &'static str,
    scope: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'static str>,
}

/// Build a self-describing JSON artifact for CLI and embedded HTML use.
pub fn build_agent_report(
    results: &AnalysisResults,
    config: &ValknutConfig,
    timestamp: DateTime<Utc>,
) -> Result<Value, serde_json::Error> {
    let canonical_config = canonical_json(serde_json::to_value(config)?);
    let config_bytes = serde_json::to_vec(&canonical_config)?;
    let config_hash = format!("sha256:{:x}", Sha256::digest(&config_bytes));

    let mut report = match serde_json::to_value(results)? {
        Value::Object(map) => map,
        _ => Map::new(),
    };

    report.insert(
        "schema_version".into(),
        Value::String(AGENT_REPORT_SCHEMA.into()),
    );
    report.insert(
        "run".into(),
        serde_json::to_value(RunMetadata {
            timestamp,
            config_hash,
            config: canonical_config,
        })?,
    );
    report.insert(
        "analysis_status".into(),
        serde_json::to_value(analysis_status(results))?,
    );
    report.insert("catalog".into(), report_catalog());

    let complexity = &results.passes.complexity.detailed_results;
    report.insert(
        "entities".into(),
        serde_json::to_value(agent_entities(complexity, &results.project_root))?,
    );
    report.insert(
        "metrics".into(),
        serde_json::to_value(metric_rows(complexity, config, &results.project_root))?,
    );
    report.insert(
        "findings".into(),
        serde_json::to_value(finding_rows(complexity, &results.project_root))?,
    );
    report.insert(
        "dependencies".into(),
        serde_json::to_value(dependency_rows(&results.project_root))?,
    );

    Ok(Value::Object(report))
}

fn dependency_rows(project_root: &std::path::Path) -> Vec<DependencyRow> {
    if !project_root.is_dir() {
        return Vec::new();
    }
    let mut rows = Vec::new();
    let walker = ignore::WalkBuilder::new(project_root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();
    for entry in walker.filter_map(Result::ok) {
        let path = entry.path();
        if !entry.file_type().is_some_and(|kind| kind.is_file())
            || crate::lang::language_key_for_path(path).is_none()
        {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(mut adapter) = crate::lang::adapter_for_file(path) else {
            continue;
        };
        let Ok(imports) = adapter.extract_imports(&source) else {
            continue;
        };
        let source_path = project_relative(&path.to_string_lossy(), project_root);
        rows.extend(imports.into_iter().map(|import| DependencyRow {
            source: source_path.clone(),
            module: import.module,
            kind: import.import_type,
            symbols: import.imports,
            line: import.line_number,
        }));
    }
    rows.sort();
    rows.dedup();
    rows
}

fn canonical_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<_, _> = map
                .into_iter()
                .map(|(key, value)| (key, canonical_json(value)))
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonical_json).collect()),
        other => other,
    }
}

fn agent_entities(
    results: &[ComplexityAnalysisResult],
    project_root: &std::path::Path,
) -> Vec<AgentEntity> {
    let mut seen = BTreeSet::new();
    results
        .iter()
        .filter(|entity| seen.insert(agent_entity_id(entity, project_root)))
        .map(|entity| AgentEntity {
            id: agent_entity_id(entity, project_root),
            name: entity.entity_name.clone(),
            kind: entity.entity_type.to_ascii_lowercase(),
            file_path: project_relative(&entity.file_path, project_root),
            start_line: entity.start_line,
            context: entity
                .semantic_context
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        })
        .collect()
}

fn metric_rows(
    results: &[ComplexityAnalysisResult],
    config: &ValknutConfig,
    project_root: &std::path::Path,
) -> Vec<MetricRow> {
    let thresholds = &config.complexity;
    let mut rows = Vec::with_capacity(results.len() * 7);
    for entity in results {
        let metrics = &entity.metrics;
        push_metric(
            &mut rows,
            entity,
            "cyclomatic_complexity",
            metrics.cyclomatic_complexity,
            Some(&thresholds.cyclomatic_thresholds),
            project_root,
        );
        push_metric(
            &mut rows,
            entity,
            "cognitive_complexity",
            metrics.cognitive_complexity,
            Some(&thresholds.cognitive_thresholds),
            project_root,
        );
        push_metric(
            &mut rows,
            entity,
            "max_nesting_depth",
            metrics.max_nesting_depth,
            Some(&thresholds.nesting_thresholds),
            project_root,
        );
        push_metric(
            &mut rows,
            entity,
            "parameter_count",
            metrics.parameter_count,
            Some(&thresholds.parameter_thresholds),
            project_root,
        );
        push_metric(
            &mut rows,
            entity,
            "lines_of_code",
            metrics.lines_of_code,
            Some(&thresholds.function_length_thresholds),
            project_root,
        );
        push_metric(
            &mut rows,
            entity,
            "technical_debt_score",
            metrics.technical_debt_score,
            None,
            project_root,
        );
        push_metric(
            &mut rows,
            entity,
            "maintainability_index",
            metrics.maintainability_index,
            None,
            project_root,
        );
    }
    rows
}

fn push_metric(
    rows: &mut Vec<MetricRow>,
    entity: &ComplexityAnalysisResult,
    metric: &'static str,
    value: f64,
    thresholds: Option<&ComplexityThresholds>,
    project_root: &std::path::Path,
) {
    rows.push(MetricRow {
        subject_id: agent_entity_id(entity, project_root),
        metric,
        value,
        threshold_difference: thresholds.map(|configured| value - configured.high),
    });
}

fn finding_rows(
    results: &[ComplexityAnalysisResult],
    project_root: &std::path::Path,
) -> Vec<FindingRow> {
    results
        .iter()
        .flat_map(|entity| {
            entity.issues.iter().map(move |issue| FindingRow {
                subject_id: agent_entity_id(entity, project_root),
                code: issue.issue_type.clone(),
                severity: issue.severity.clone(),
                metric: metric_for_issue(&issue.issue_type),
                value: issue.metric_value,
                threshold_difference: issue.metric_value - issue.threshold,
            })
        })
        .collect()
}

fn agent_entity_id(entity: &ComplexityAnalysisResult, project_root: &std::path::Path) -> String {
    format!(
        "{}:{}:{}",
        project_relative(&entity.file_path, project_root),
        entity.entity_name,
        entity.start_line
    )
}

fn project_relative(file_path: &str, project_root: &std::path::Path) -> String {
    let path = std::path::Path::new(file_path);
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn metric_for_issue(issue_type: &str) -> &'static str {
    let lower = issue_type.to_ascii_lowercase();
    if lower.contains("cyclomatic") {
        "cyclomatic_complexity"
    } else if lower.contains("cognitive") {
        "cognitive_complexity"
    } else if lower.contains("nesting") {
        "max_nesting_depth"
    } else if lower.contains("parameter") {
        "parameter_count"
    } else if lower.contains("file") || lower.contains("function") {
        "lines_of_code"
    } else {
        "technical_debt_score"
    }
}

fn metric_catalog() -> BTreeMap<&'static str, MetricDefinition> {
    BTreeMap::from([
        (
            "cognitive_complexity",
            metric(
                "count",
                Some("Control-flow complexity with nesting penalties."),
            ),
        ),
        (
            "cyclomatic_complexity",
            metric("count", Some("Independent control-flow paths.")),
        ),
        ("lines_of_code", metric("lines", None)),
        (
            "maintainability_index",
            metric("score", Some("Composite maintainability index.")),
        ),
        ("max_nesting_depth", metric("levels", None)),
        ("parameter_count", metric("count", None)),
        (
            "technical_debt_score",
            metric("score", Some("Valknut composite debt estimate.")),
        ),
    ])
}

fn report_catalog() -> Value {
    serde_json::json!({
        "metrics": metric_catalog(),
        "dependency_kinds": {
            "import": "Direct module/package import.",
            "aliased_import": "Import bound to a local alias.",
            "named": "Selected symbols imported from a module.",
            "star": "All exported symbols imported.",
            "side_effect": "Module loaded without local bindings.",
            "require": "CommonJS require dependency.",
            "include": "C/C++ preprocessor include.",
            "module_import": "C++20 module import.",
            "export_import": "Re-exported C++20 module import.",
            "using": "C# namespace import.",
            "using_alias": "C# namespace/type alias.",
            "using_static": "C# static member import.",
            "module": "Language module path dependency."
        }
    })
}

fn metric(unit: &'static str, description: Option<&'static str>) -> MetricDefinition {
    MetricDefinition {
        unit,
        scope: "entity",
        description,
    }
}

fn analysis_status(results: &AnalysisResults) -> BTreeMap<&'static str, Value> {
    let mut status = BTreeMap::new();
    status.insert(
        "complexity",
        module_status(results.passes.complexity.enabled, None),
    );
    status.insert(
        "structure",
        module_status(results.passes.structure.enabled, None),
    );
    status.insert(
        "refactoring",
        module_status(results.passes.refactoring.enabled, None),
    );
    status.insert("impact", module_status(results.passes.impact.enabled, None));
    status.insert("clones", module_status(results.passes.lsh.enabled, None));
    let coverage_reason = (results.passes.coverage.enabled
        && results.passes.coverage.coverage_files_used.is_empty())
    .then_some("no coverage input was available");
    status.insert(
        "coverage",
        module_status(results.passes.coverage.enabled, coverage_reason),
    );
    status.insert(
        "cohesion",
        module_status(results.passes.cohesion.enabled, None),
    );
    status
}

fn module_status(enabled: bool, unavailable_reason: Option<&str>) -> Value {
    if !enabled {
        serde_json::json!({ "status": "disabled" })
    } else if let Some(reason) = unavailable_reason {
        serde_json::json!({ "status": "unavailable", "reason": reason })
    } else {
        serde_json::json!({ "status": "complete" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detectors::complexity::{ComplexityMetrics, ComplexitySeverity, HalsteadMetrics};

    #[test]
    fn report_embeds_canonical_config_and_stable_hash() {
        let results = AnalysisResults::empty();
        let config = ValknutConfig::default();
        let timestamp = DateTime::parse_from_rfc3339("2026-08-03T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let first = build_agent_report(&results, &config, timestamp).unwrap();
        let second = build_agent_report(&results, &config, timestamp).unwrap();

        assert_eq!(first["schema_version"], AGENT_REPORT_SCHEMA);
        assert_eq!(first["run"]["config_hash"], second["run"]["config_hash"]);
        assert!(first["run"]["config"]["complexity"].is_object());
        assert!(first["metrics"].is_array());
    }

    #[test]
    fn metric_rows_use_signed_raw_threshold_difference() {
        let entity = ComplexityAnalysisResult {
            entity_id: "/repo/src/lib.rs:work:10".into(),
            file_path: "/repo/src/lib.rs".into(),
            line_number: 10,
            start_line: 10,
            entity_name: "work".into(),
            entity_type: "function".into(),
            semantic_context: std::collections::HashMap::from([
                ("parent_id".into(), serde_json::json!("parent")),
                ("parameters".into(), serde_json::json!(["value"])),
            ]),
            metrics: ComplexityMetrics {
                cyclomatic_complexity: 7.0,
                cognitive_complexity: 33.0,
                max_nesting_depth: 2.0,
                parameter_count: 1.0,
                lines_of_code: 20.0,
                statement_count: 10.0,
                halstead: HalsteadMetrics::default(),
                technical_debt_score: 4.0,
                maintainability_index: 80.0,
                decision_points: Vec::new(),
            },
            issues: Vec::new(),
            severity: ComplexitySeverity::High,
            recommendations: Vec::new(),
        };
        let rows = metric_rows(
            std::slice::from_ref(&entity),
            &ValknutConfig::default(),
            std::path::Path::new("/repo"),
        );
        let cyclomatic = rows
            .iter()
            .find(|row| row.metric == "cyclomatic_complexity")
            .unwrap();
        let cognitive = rows
            .iter()
            .find(|row| row.metric == "cognitive_complexity")
            .unwrap();

        assert_eq!(cyclomatic.subject_id, "src/lib.rs:work:10");
        assert_eq!(cyclomatic.threshold_difference, Some(-8.0));
        assert_eq!(cognitive.threshold_difference, Some(8.0));
        let entities = agent_entities(&[entity], std::path::Path::new("/repo"));
        assert_eq!(entities[0].context["parent_id"], "parent");
        assert_eq!(
            entities[0].context["parameters"],
            serde_json::json!(["value"])
        );
    }

    #[test]
    fn report_exposes_flat_dependency_rows() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("main.py"),
            "import os\nfrom pathlib import Path\n",
        )
        .unwrap();
        let mut results = AnalysisResults::empty();
        results.project_root = directory.path().to_path_buf();
        let report = build_agent_report(&results, &ValknutConfig::default(), Utc::now()).unwrap();
        assert_eq!(report["dependencies"].as_array().unwrap().len(), 2);
        assert_eq!(report["dependencies"][0]["source"], "main.py");
        assert!(report["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["module"] == "pathlib"));
    }
}
