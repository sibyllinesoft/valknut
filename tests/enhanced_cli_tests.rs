#!/usr/bin/env rust
//! Enhanced Integration tests for the Valknut CLI with correct configuration format
//!
//! This test suite validates the command-line interface and end-to-end functionality
//! using the correct ValknutConfig format after fixing configuration issues.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

/// Test helper to get the CLI binary
fn valknut_cmd() -> Command {
    Command::cargo_bin("valknut").unwrap()
}

/// Creates a correct test configuration using the ValknutConfig format
fn create_working_config() -> String {
    r#"analysis:
  enable_scoring: true
  enable_graph_analysis: true
  enable_lsh_analysis: true
  enable_refactoring_analysis: true
  enable_coverage_analysis: true
  enable_structure_analysis: true
  enable_names_analysis: true
  confidence_threshold: 0.7
  max_files: 100
  exclude_patterns:
    - "*/node_modules/*"
    - "*/venv/*" 
    - "*/target/*"
    - "*/__pycache__/*"
    - "*.min.js"
  include_patterns:
    - "**/*.py"
    - "**/*.rs"
    - "**/*.js"
    - "**/*.ts"
scoring:
  normalization_scheme: z_score
  use_bayesian_fallbacks: true
  confidence_reporting: false
  weights:
    complexity: 1.0
    graph: 0.8
    structure: 0.9
    style: 0.5
    coverage: 0.7
  statistical_params:
    confidence_level: 0.95
    min_sample_size: 10
    outlier_threshold: 3.0
graph:
  enable_betweenness: true
  enable_closeness: false
  enable_cycle_detection: true
  max_exact_size: 10000
  use_approximation: true
  approximation_sample_rate: 0.1
lsh:
  num_hashes: 128
  num_bands: 16
  shingle_size: 3
  similarity_threshold: 0.7
  max_candidates: 100
  use_semantic_similarity: false
languages:
  python:
    enabled: true
    file_extensions: [".py", ".pyi"]
    tree_sitter_language: "python"
    max_file_size_mb: 10.0
    complexity_threshold: 10.0
    additional_settings: {}
  rust:
    enabled: true
    file_extensions: [".rs"]
    tree_sitter_language: "rust"
    max_file_size_mb: 10.0
    complexity_threshold: 15.0
    additional_settings: {}
io:
  cache_dir: null
  enable_caching: true
  cache_ttl_seconds: 3600
  report_dir: null
  report_format: json
performance:
  max_threads: null
  memory_limit_mb: null
  file_timeout_seconds: 30
  total_timeout_seconds: null
  enable_simd: false
  batch_size: 100
structure:
  min_branch_recommendation_gain: 0.15
  min_files_for_split: 5
  target_loc_per_subdir: 1000
coverage:
  auto_discover: true
  search_paths: ["./coverage/", "./target/coverage/"]
  file_patterns: ["coverage.xml", "lcov.info", "coverage.json"]
  max_age_days: 7
  coverage_file: null"#
        .to_string()
}

/// Creates a test directory with sample source files
fn create_test_source_files(dir: &std::path::Path) -> std::io::Result<()> {
    // Create a Python file
    fs::write(
        dir.join("simple.py"),
        r#"
def fibonacci(n):
    """Calculate the nth Fibonacci number."""
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)

class Calculator:
    def add(self, a, b):
        return a + b
    
    def multiply(self, a, b):
        return a * b
"#,
    )?;

    // Create a Rust file
    fs::write(
        dir.join("lib.rs"),
        r#"
pub fn factorial(n: u32) -> u32 {
    match n {
        0 | 1 => 1,
        _ => n * factorial(n - 1),
    }
}

pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
    
    pub fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}
"#,
    )?;

    // Create a JavaScript file
    fs::write(
        dir.join("utils.js"),
        r#"
function isPrime(n) {
    if (n <= 1) return false;
    if (n <= 3) return true;
    if (n % 2 === 0 || n % 3 === 0) return false;
    
    for (let i = 5; i * i <= n; i += 6) {
        if (n % i === 0 || n % (i + 2) === 0) {
            return false;
        }
    }
    return true;
}

class UserManager {
    constructor() {
        this.users = new Map();
    }
    
    addUser(id, user) {
        this.users.set(id, user);
    }
    
    getUser(id) {
        return this.users.get(id);
    }
}
"#,
    )?;

    Ok(())
}

#[test]
fn test_analyze_with_working_config() {
    // Create test configuration using correct format
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("valknut.yml");

    fs::write(&config_path, create_working_config()).unwrap();

    // Create test source files
    let test_code_dir = temp_dir.path().join("test-code");
    fs::create_dir(&test_code_dir).unwrap();
    create_test_source_files(&test_code_dir).unwrap();

    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--config",
        config_path.to_str().unwrap(),
        "--format",
        "json",
    ]);

    cmd.assert().success();
}

#[test]
fn test_analyze_with_minimal_config() {
    let temp_dir = tempdir().unwrap();
    let test_code_dir = temp_dir.path().join("test-code");
    fs::create_dir(&test_code_dir).unwrap();
    create_test_source_files(&test_code_dir).unwrap();

    // Test with minimal configuration that only enables scoring
    let config_path = temp_dir.path().join("config.yml");
    fs::write(
        &config_path,
        r#"analysis:
  enable_scoring: true
  enable_graph_analysis: false
  enable_lsh_analysis: false
  enable_refactoring_analysis: false
  enable_coverage_analysis: false
  enable_structure_analysis: false
  enable_names_analysis: false
  confidence_threshold: 0.8
  max_files: 50
  include_patterns: ["**/*.py", "**/*.rs"]
  exclude_patterns: []
scoring:
  normalization_scheme: z_score
  use_bayesian_fallbacks: true
  confidence_reporting: false
  weights:
    complexity: 1.0
    graph: 0.0
    structure: 0.0
    style: 0.0
    coverage: 0.0
  statistical_params:
    confidence_level: 0.95
    min_sample_size: 10
    outlier_threshold: 3.0
graph:
  enable_betweenness: false
  enable_closeness: false
  enable_cycle_detection: false
  max_exact_size: 10000
  use_approximation: false
  approximation_sample_rate: 0.1
lsh:
  num_hashes: 64
  num_bands: 8
  similarity_threshold: 0.8
languages:
  python:
    enabled: true
    file_extensions: [".py"]
    tree_sitter_language: "python"
    max_file_size_mb: 5.0
    complexity_threshold: 12.0
    additional_settings: {}
io:
  enable_caching: false
  report_format: json
performance:
  file_timeout_seconds: 10
structure:
  min_files_for_split: 10
coverage:
  auto_discover: false"#,
    )
    .unwrap();

    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--config",
        config_path.to_str().unwrap(),
        "--format",
        "json",
    ]);

    cmd.assert().success();
}

#[test]
fn test_analyze_with_language_specific_config() {
    let temp_dir = tempdir().unwrap();
    let test_code_dir = temp_dir.path().join("test-code");
    fs::create_dir(&test_code_dir).unwrap();
    create_test_source_files(&test_code_dir).unwrap();

    // Test language-specific analysis
    let config_path = temp_dir.path().join("config.yml");
    fs::write(
        &config_path,
        r#"analysis:
  enable_scoring: true
  enable_graph_analysis: true
  enable_lsh_analysis: false
  enable_refactoring_analysis: true
  enable_coverage_analysis: false
  enable_structure_analysis: true
  enable_names_analysis: false
  confidence_threshold: 0.6
  max_files: 25
  include_patterns: ["**/*.py"]
  exclude_patterns: ["**/test_*.py"]
scoring:
  normalization_scheme: z_score
  use_bayesian_fallbacks: true
  confidence_reporting: false
  weights:
    complexity: 1.0
    graph: 0.8
    structure: 0.9
    style: 0.5
    coverage: 0.0
  statistical_params:
    confidence_level: 0.95
    min_sample_size: 10
    outlier_threshold: 3.0
graph:
  enable_betweenness: true
  enable_closeness: false
  enable_cycle_detection: true
lsh:
  num_hashes: 128
  num_bands: 16
  similarity_threshold: 0.7
languages:
  python:
    enabled: true
    file_extensions: [".py", ".pyi"]
    tree_sitter_language: "python"
    max_file_size_mb: 5.0
    complexity_threshold: 12.0
    additional_settings: {}
io:
  enable_caching: true
  report_format: json
performance:
  file_timeout_seconds: 60
  enable_simd: false
structure:
  min_files_for_split: 5
coverage:
  auto_discover: false"#,
    )
    .unwrap();

    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--config",
        config_path.to_str().unwrap(),
        "--format",
        "json",
    ]);

    cmd.assert().success();
}

#[test]
fn test_analyze_with_coverage_config() {
    let temp_dir = tempdir().unwrap();
    let test_code_dir = temp_dir.path().join("test-code");
    fs::create_dir(&test_code_dir).unwrap();
    create_test_source_files(&test_code_dir).unwrap();

    // Create a mock coverage file
    let coverage_dir = test_code_dir.join("coverage");
    fs::create_dir(&coverage_dir).unwrap();
    fs::write(
        coverage_dir.join("lcov.info"),
        r#"
TN:
SF:simple.py
FN:1,fibonacci
FN:8,Calculator.__init__
FNDA:5,fibonacci
FNDA:2,Calculator.__init__
FNF:2
FNH:2
DA:1,5
DA:2,5
DA:3,2
DA:4,2
DA:5,3
LH:5
LF:5
end_of_record
"#,
    )
    .unwrap();

    let config_path = temp_dir.path().join("config.yml");
    let config_content = format!(
        r#"analysis:
  enable_scoring: true
  enable_graph_analysis: false
  enable_lsh_analysis: false
  enable_refactoring_analysis: false
  enable_coverage_analysis: true
  enable_structure_analysis: false
  enable_names_analysis: false
  confidence_threshold: 0.7
  max_files: 50
  include_patterns: ["**/*.py"]
  exclude_patterns: []
scoring:
  normalization_scheme: z_score
  use_bayesian_fallbacks: true
  confidence_reporting: false
  weights:
    complexity: 1.0
    graph: 0.0
    structure: 0.0
    style: 0.0
    coverage: 1.0
  statistical_params:
    confidence_level: 0.95
    min_sample_size: 10
    outlier_threshold: 3.0
graph:
  enable_betweenness: false
  enable_closeness: false
  enable_cycle_detection: false
  max_exact_size: 10000
  use_approximation: false
  approximation_sample_rate: 0.1
lsh:
  num_hashes: 64
  num_bands: 8
  similarity_threshold: 0.7
languages:
  python:
    enabled: true
    file_extensions: [".py"]
    tree_sitter_language: "python"
    max_file_size_mb: 10.0
    complexity_threshold: 10.0
    additional_settings: {{}}
io:
  enable_caching: true
  report_format: json
performance:
  file_timeout_seconds: 30
structure:
  min_files_for_split: 5
coverage:
  auto_discover: true
  search_paths: ["{}"]
  file_patterns: ["lcov.info"]
  max_age_days: 7
  coverage_file: "{}/lcov.info""#,
        coverage_dir.to_str().unwrap(),
        coverage_dir.to_str().unwrap()
    );
    fs::write(&config_path, config_content).unwrap();

    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--config",
        config_path.to_str().unwrap(),
        "--format",
        "json",
    ]);

    cmd.assert().success();
}

#[test]
fn test_analyze_multiple_formats() {
    let temp_dir = tempdir().unwrap();
    let test_code_dir = temp_dir.path().join("test-code");
    fs::create_dir(&test_code_dir).unwrap();
    create_test_source_files(&test_code_dir).unwrap();

    // Test JSON output
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--format",
        "json",
    ]);
    cmd.assert().success();

    // Test YAML output
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--format",
        "yaml",
    ]);
    cmd.assert().success();

    // Test pretty output
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--format",
        "pretty",
    ]);
    cmd.assert().success();
}

#[test]
fn test_analyze_with_flags() {
    let temp_dir = tempdir().unwrap();
    let test_code_dir = temp_dir.path().join("test-code");
    fs::create_dir(&test_code_dir).unwrap();
    create_test_source_files(&test_code_dir).unwrap();

    // Test verbose flag
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--verbose",
        "--format",
        "json",
    ]);
    cmd.assert().success();

    // Test quiet flag
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--quiet",
        "--format",
        "json",
    ]);
    cmd.assert().success();

    // Test quality gate flag
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--quality-gate",
        "--format",
        "json",
    ]);
    cmd.assert().success();
}

#[test]
fn test_cli_help_commands() {
    // Test main help
    let mut cmd = valknut_cmd();
    cmd.arg("--help");
    cmd.assert().success().stdout(predicate::str::contains(
        "Analyze your codebase for technical debt",
    ));

    // Test analyze help
    let mut cmd = valknut_cmd();
    cmd.args(["analyze", "--help"]);
    cmd.assert().success().stdout(predicate::str::contains(
        "Analyze code repositories for refactorability",
    ));

    // Test version
    let mut cmd = valknut_cmd();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_error_conditions() {
    // Test nonexistent path
    let mut cmd = valknut_cmd();
    cmd.args(["analyze", "/nonexistent/path"]);
    cmd.assert().failure();

    // Test invalid config
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("invalid.yml");
    fs::write(&config_path, "invalid: yaml: [missing_bracket").unwrap();

    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        temp_dir.path().to_str().unwrap(),
        "--config",
        config_path.to_str().unwrap(),
    ]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("configuration"));
}
