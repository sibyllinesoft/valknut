//! Documentation audit command implementation.
//!
//! This module handles the standalone `doc-audit` command for analyzing
//! documentation quality and coverage in a codebase.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

use crate::cli::args::{DocAuditArgs, DocAuditFormat};
use valknut_rs::doc_audit;

/// Optional YAML configuration for the standalone doc-audit command.
#[derive(Debug, Deserialize)]
pub struct DocAuditConfigFile {
    pub root: Option<PathBuf>,
    pub complexity_threshold: Option<usize>,
    pub max_readme_commits: Option<usize>,
    #[serde(default)]
    pub ignore_dir: Vec<String>,
    #[serde(default)]
    pub ignore_suffix: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

/// Run the standalone documentation audit command.
pub fn doc_audit_command(args: DocAuditArgs) -> anyhow::Result<()> {
    let file_config = find_doc_audit_config_file(&args.config)?;
    let root_path = resolve_doc_audit_root(&args.root, file_config.as_ref())?;

    let mut config = doc_audit::DocAuditConfig::new(root_path);

    if let Some(file_cfg) = file_config {
        apply_file_config_to_doc_audit(&mut config, file_cfg);
    }

    config.complexity_threshold = args.complexity_threshold;
    config.max_readme_commits = args.max_readme_commits;
    apply_cli_ignores_to_doc_audit(
        &mut config,
        &args.ignore_dir,
        &args.ignore_suffix,
        &args.ignore,
    );

    let result = doc_audit::run_audit(&config)?;
    render_doc_audit_output(&result, &args.format)?;

    if args.strict && result.has_issues() {
        anyhow::bail!("Documentation audit found issues");
    }

    Ok(())
}

/// Load doc-audit settings from a YAML file.
pub fn load_doc_audit_config_file(path: &Path) -> anyhow::Result<DocAuditConfigFile> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read doc audit config at {}", path.display()))?;
    serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse doc audit config {}", path.display()))
}

/// Find and load doc audit config file from explicit path or implicit locations.
pub fn find_doc_audit_config_file(
    explicit_path: &Option<PathBuf>,
) -> anyhow::Result<Option<DocAuditConfigFile>> {
    let implicit_config = [".valknut.docaudit.yml", ".valknut.docaudit.yaml"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists());

    match explicit_path.clone().or(implicit_config) {
        Some(path) => Ok(Some(load_doc_audit_config_file(&path)?)),
        None => Ok(None),
    }
}

/// Resolve and validate the doc audit root path.
pub fn resolve_doc_audit_root(
    cli_root: &Path,
    file_config: Option<&DocAuditConfigFile>,
) -> anyhow::Result<PathBuf> {
    let root_override = if cli_root != Path::new(".") {
        cli_root.to_path_buf()
    } else {
        file_config
            .and_then(|c| c.root.clone())
            .unwrap_or_else(|| cli_root.to_path_buf())
    };

    if !root_override.exists() {
        return Err(anyhow::anyhow!(
            "Audit root does not exist: {}",
            root_override.display()
        ));
    }

    let root_path = std::fs::canonicalize(&root_override).map_err(|err| {
        anyhow::anyhow!(
            "Failed to resolve audit root {}: {}",
            root_override.display(),
            err
        )
    })?;

    if !root_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Audit root must be a directory: {}",
            root_path.display()
        ));
    }

    Ok(root_path)
}

/// Apply file config settings to doc audit config.
pub fn apply_file_config_to_doc_audit(
    config: &mut doc_audit::DocAuditConfig,
    file_cfg: DocAuditConfigFile,
) {
    if let Some(threshold) = file_cfg.complexity_threshold {
        config.complexity_threshold = threshold;
    }
    if let Some(commits) = file_cfg.max_readme_commits {
        config.max_readme_commits = commits;
    }
    extend_ignore_set(&mut config.ignore_dirs, file_cfg.ignore_dir);
    extend_ignore_set(&mut config.ignore_suffixes, file_cfg.ignore_suffix);
    extend_ignore_vec(&mut config.ignore_globs, file_cfg.ignore);
}

/// Apply CLI ignore arguments to doc audit config.
pub fn apply_cli_ignores_to_doc_audit(
    config: &mut doc_audit::DocAuditConfig,
    ignore_dir: &[String],
    ignore_suffix: &[String],
    ignore: &[String],
) {
    extend_ignore_set(&mut config.ignore_dirs, ignore_dir.to_vec());
    extend_ignore_set(&mut config.ignore_suffixes, ignore_suffix.to_vec());
    extend_ignore_vec(&mut config.ignore_globs, ignore.to_vec());
}

/// Extend a HashSet with non-empty trimmed strings.
fn extend_ignore_set(set: &mut HashSet<String>, items: Vec<String>) {
    for item in items {
        if !item.trim().is_empty() {
            set.insert(item);
        }
    }
}

/// Extend a Vec with non-empty trimmed strings.
fn extend_ignore_vec(vec: &mut Vec<String>, items: Vec<String>) {
    for item in items {
        if !item.trim().is_empty() {
            vec.push(item);
        }
    }
}

/// Render doc audit output in the requested format.
pub fn render_doc_audit_output(
    result: &doc_audit::AuditResult,
    format: &DocAuditFormat,
) -> anyhow::Result<()> {
    match format {
        DocAuditFormat::Text => println!("{}", doc_audit::render_text(result)),
        DocAuditFormat::Json => println!("{}", doc_audit::render_json(result)?),
    }
    Ok(())
}
