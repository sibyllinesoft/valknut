#!/usr/bin/env rust
//! Integration tests for the Valknut CLI

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

/// Test helper to get the CLI binary
fn valknut_cmd() -> Command {
    Command::cargo_bin("valknut").unwrap()
}

#[test]
fn cli_help_command() {
    let mut cmd = valknut_cmd();
    cmd.arg("--help");
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Analyze your codebase for technical debt"))
        .stdout(predicate::str::contains("analyze"))
        .stdout(predicate::str::contains("structure"))
        .stdout(predicate::str::contains("Commands:"));
}

#[test]  
fn cli_version_command() {
    let mut cmd = valknut_cmd();
    cmd.arg("--version");
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn analyze_help_command() {
    let mut cmd = valknut_cmd();
    cmd.args(["analyze", "--help"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Analyze code repositories for refactorability"))
        .stdout(predicate::str::contains("[PATHS]"))
        .stdout(predicate::str::contains("--config"))
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn analyze_nonexistent_path() {
    let mut cmd = valknut_cmd();
    cmd.args(["analyze", "/nonexistent/path"]);
    
    cmd.assert()
        .failure();
}

#[test]
fn analyze_empty_directory() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args(["analyze", temp_path, "--format", "json"]);
    
    cmd.assert()
        .success();
}

#[test]
fn analyze_with_config_file() {
    // Create a simple test config
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("valknut.yml");
    
    std::fs::write(&config_path, r#"
structure:
  enable_branch_packs: true
  enable_file_split_packs: false
  top_packs: 5
fsdir:
  max_files_per_dir: 20
  max_subdirs_per_dir: 8
  max_dir_loc: 1500
  min_branch_recommendation_gain: 0.15
  min_files_for_split: 5
  target_loc_per_subdir: 1000
fsfile:
  huge_loc: 600
  huge_bytes: 100000
  min_split_loc: 200
  min_entities_per_split: 3
partitioning:
  balance_tolerance: 0.25
  max_clusters: 4
  min_clusters: 2
  naming_fallbacks: ["core", "api", "util"]
"#).unwrap();

    // Create a simple test directory structure
    let test_code_dir = temp_dir.path().join("test-code");
    std::fs::create_dir(&test_code_dir).unwrap();
    
    // Create a simple Python file
    let simple_file = test_code_dir.join("simple.py");
    std::fs::write(&simple_file, "def hello(): return 'world'").unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze",
        test_code_dir.to_str().unwrap(),
        "--config",
        config_path.to_str().unwrap(),
        "--format",
        "json"
    ]);
    
    cmd.assert()
        .success();
}

#[test]
fn analyze_quiet_flag() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze", 
        temp_path, 
        "--quiet",
        "--format", 
        "json"
    ]);
    
    cmd.assert()
        .success();
}

#[test]
fn analyze_verbose_flag() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze", 
        temp_path, 
        "--verbose",
        "--format", 
        "json"
    ]);
    
    cmd.assert()
        .success();
}

#[test]
fn analyze_quality_gate_flag() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze", 
        temp_path, 
        "--quality-gate",
        "--format", 
        "json"
    ]);
    
    cmd.assert()
        .success();
}

#[test]
fn analyze_yaml_output() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze", 
        temp_path, 
        "--format", 
        "yaml"
    ]);
    
    cmd.assert()
        .success();
}

#[test]
fn analyze_pretty_output() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "analyze", 
        temp_path, 
        "--format", 
        "pretty"
    ]);
    
    cmd.assert()
        .success();
}