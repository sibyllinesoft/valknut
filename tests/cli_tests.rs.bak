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
        .stdout(predicate::str::contains("Valknut analyzes code structure"))
        .stdout(predicate::str::contains("structure"))
        .stdout(predicate::str::contains("--config"))
        .stdout(predicate::str::contains("--format"));
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
fn structure_help_command() {
    let mut cmd = valknut_cmd();
    cmd.args(["structure", "--help"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Analyze code structure"))
        .stdout(predicate::str::contains("<PATH>"))
        .stdout(predicate::str::contains("--branch-only"))
        .stdout(predicate::str::contains("--file-split-only"))
        .stdout(predicate::str::contains("--extensions"));
}

#[test]
fn structure_nonexistent_path() {
    let mut cmd = valknut_cmd();
    cmd.args(["structure", "/nonexistent/path"]);
    
    cmd.assert()
        .failure()
        .code(1);
}

#[test]
fn structure_empty_directory() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args(["structure", temp_path, "--format", "json"]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"packs\": []"));
}

#[test]
fn structure_config_file_loading() {
    // Create a simple test config
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test-config.yml");
    
    std::fs::write(&config_path, r#"
structure:
  enable_branch_packs: false
  enable_file_split_packs: true
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
  naming_fallbacks: ["core", "api", "util", "shared"]
"#).unwrap();

    // Create a simple test directory structure
    let test_code_dir = temp_dir.path().join("test-code");
    std::fs::create_dir(&test_code_dir).unwrap();
    
    // Create a file that should trigger analysis based on our config
    let large_file = test_code_dir.join("large.py");
    let mut content = String::new();
    for i in 0..700 {  // Create a file with >600 lines
        content.push_str(&format!("# Line {}\ndef function_{}(): pass\n", i, i));
    }
    std::fs::write(&large_file, content).unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "structure",
        test_code_dir.to_str().unwrap(),
        "--config",
        config_path.to_str().unwrap(),
        "--format",
        "json",
        "--verbose"
    ]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Loading configuration from"))
        .stdout(predicate::str::contains("Configuration: branch_packs=false, file_split_packs=true"));
}

#[test]
fn structure_branch_only_flag() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "structure", 
        temp_path, 
        "--branch-only",
        "--format", 
        "json"
    ]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration: branch_packs=true, file_split_packs=false"));
}

#[test]
fn structure_file_split_only_flag() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "structure", 
        temp_path, 
        "--file-split-only",
        "--format", 
        "json"
    ]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration: branch_packs=false, file_split_packs=true"));
}

#[test]
fn structure_top_limit_flag() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "structure", 
        temp_path, 
        "--top", 
        "3",
        "--format", 
        "json"
    ]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("top_packs=3"));
}

#[test]
fn structure_yaml_output() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "structure", 
        temp_path, 
        "--format", 
        "yaml"
    ]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("packs:"))
        .stdout(predicate::str::contains("summary:"));
}

#[test]
fn structure_pretty_output() {
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_str().unwrap();
    
    let mut cmd = valknut_cmd();
    cmd.args([
        "structure", 
        temp_path, 
        "--format", 
        "pretty"
    ]);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("🏗️  Valknut Structure Analysis Results"))
        .stdout(predicate::str::contains("No structural issues found!").or(predicate::str::contains("Found")));
}