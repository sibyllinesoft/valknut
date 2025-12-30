//! Documentation and README audit utilities.
//!
//! Scans codebases for missing documentation (docstrings in Python, rustdoc in Rust,
//! JSDoc in TypeScript/JavaScript), missing READMEs in complex directories, and
//! stale READMEs that haven't been updated alongside the code.

mod python;
mod rust;
mod typescript;

use anyhow::{Context, Result};
use chrono::{DateTime, FixedOffset, TimeZone};
use git2::{DiffOptions, Oid, Repository};
use globset::{Glob, GlobSet, GlobSetBuilder};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Default complexity threshold for requiring READMEs.
pub const DEFAULT_COMPLEXITY_THRESHOLD: usize = 8;

/// Default number of commits before a README is considered stale.
pub const DEFAULT_MAX_README_COMMITS: usize = 10;

static DEFAULT_IGNORED_DIR_NAMES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        ".git",
        ".hg",
        ".svn",
        ".idea",
        ".vscode",
        ".venv",
        "__pycache__",
        "node_modules",
        "target",
        "dist",
        "build",
        "coverage",
        "reports",
        ".mypy_cache",
        ".ruff_cache",
        "tmp",
        "temp",
        "datasets",
        "archive",
    ]
    .into_iter()
    .collect()
});

static DEFAULT_IGNORED_SUFFIXES: Lazy<HashSet<&'static str>> =
    Lazy::new(|| [".lock", ".min.js", ".min.css"].into_iter().collect());

static README_CANDIDATES: [&str; 6] = [
    "README",
    "README.md",
    "README.rst",
    "README.txt",
    "readme.md",
    "Readme.md",
];

static TODO_MARKERS: [&str; 3] = ["TODO", "FIXME", "TBD"];

static DEFAULT_IGNORED_GLOBS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        "**/tests/**",
        "**/*_test.*",
        "**/*-test.*",
        "**/*Test.*",
        "**/*Tests.*",
    ]
});

/// Configuration for documentation audit operations.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocAuditConfig {
    /// Repository root path.
    pub root: PathBuf,
    /// Threshold for requiring READMEs.
    pub complexity_threshold: usize,
    /// Commits before README is stale.
    pub max_readme_commits: usize,
    /// Directories to skip.
    pub ignore_dirs: HashSet<String>,
    /// File suffixes to skip.
    pub ignore_suffixes: HashSet<String>,
    /// Glob patterns to skip.
    pub ignore_globs: Vec<String>,
}

impl DocAuditConfig {
    /// Create a new configuration with defaults for the given root.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            complexity_threshold: DEFAULT_COMPLEXITY_THRESHOLD,
            max_readme_commits: DEFAULT_MAX_README_COMMITS,
            ignore_dirs: DEFAULT_IGNORED_DIR_NAMES
                .iter()
                .map(|item| item.to_string())
                .collect(),
            ignore_suffixes: DEFAULT_IGNORED_SUFFIXES
                .iter()
                .map(|item| item.to_string())
                .collect(),
            ignore_globs: DEFAULT_IGNORED_GLOBS
                .iter()
                .map(|g| g.to_string())
                .collect(),
        }
    }
}

/// Output format for audit results.
#[derive(Clone, Copy, Debug)]
pub enum OutputFormat {
    /// Plain text output.
    Text,
    /// JSON output.
    Json,
}

/// A single documentation issue found during audit.
#[derive(Debug, Serialize)]
pub struct DocIssue {
    /// Issue category (e.g., "undocumented_python", "missing_readme").
    pub category: String,
    /// Path to the file or directory.
    pub path: PathBuf,
    /// Line number if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// Symbol name if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Human-readable description.
    pub detail: String,
}

/// Complete audit results.
#[derive(Debug, Serialize)]
pub struct AuditResult {
    /// Undocumented code issues.
    pub documentation_issues: Vec<DocIssue>,
    /// Directories missing READMEs.
    pub missing_readmes: Vec<DocIssue>,
    /// READMEs not updated with recent changes.
    pub stale_readmes: Vec<DocIssue>,
}

impl AuditResult {
    /// Returns true if any issues were found.
    pub fn has_issues(&self) -> bool {
        !self.documentation_issues.is_empty()
            || !self.missing_readmes.is_empty()
            || !self.stale_readmes.is_empty()
    }
}

struct DirectoryInfo {
    files: Vec<PathBuf>,
    subdirs: Vec<PathBuf>,
}

/// Run the documentation audit with the given configuration.
pub fn run_audit(config: &DocAuditConfig) -> Result<AuditResult> {
    let globset = build_ignore_globset(&config.ignore_globs)?;
    let (dir_info, files) = walk_repository(config, &globset)?;
    let documentation_issues = scan_documentation(&files, config, &globset);
    let complexity_map = compute_complexities(&dir_info);
    let (missing_readmes, readme_index) = detect_missing_readmes(&complexity_map, config);
    let git_helper = GitHelper::new(&config.root);
    let stale_readmes = detect_stale_readmes(&git_helper, &readme_index, config);

    Ok(AuditResult {
        documentation_issues,
        missing_readmes,
        stale_readmes,
    })
}

/// Render audit results as plain text.
pub fn render_text(result: &AuditResult) -> String {
    fn render_section<F>(title: &str, issues: &[DocIssue], format: F, out: &mut String)
    where
        F: Fn(&DocIssue) -> String,
    {
        out.push_str(title);
        out.push('\n');
        out.push_str(&"-".repeat(title.len()));
        out.push('\n');
        if issues.is_empty() {
            out.push_str("  None\n\n");
            return;
        }
        for issue in issues {
            out.push_str(&format(issue));
            out.push('\n');
        }
        out.push('\n');
    }

    let mut output = String::new();
    render_section(
        "Documentation gaps",
        &result.documentation_issues,
        |issue| {
            let line = issue
                .line
                .map(|line| line.to_string())
                .unwrap_or_else(|| "?".to_string());
            format!("  - {}:{} - {}", issue.path.display(), line, issue.detail)
        },
        &mut output,
    );

    render_section(
        "Missing READMEs",
        &result.missing_readmes,
        |issue| format!("  - {} - {}", issue.path.display(), issue.detail),
        &mut output,
    );

    render_section(
        "Stale READMEs",
        &result.stale_readmes,
        |issue| format!("  - {} - {}", issue.path.display(), issue.detail),
        &mut output,
    );

    let total = result.documentation_issues.len()
        + result.missing_readmes.len()
        + result.stale_readmes.len();
    output.push_str(&format!(
        "Summary: {} issue(s) detected across documentation and READMEs.\n",
        total
    ));

    output
}

/// Render audit results as JSON.
pub fn render_json(result: &AuditResult) -> Result<String> {
    serde_json::to_string_pretty(result).context("Failed to serialize audit results to JSON")
}

fn build_ignore_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern)
            .with_context(|| format!("Invalid glob pattern in doc audit ignore list: {pattern}"))?;
        builder.add(glob);
    }
    builder
        .build()
        .context("Failed to build globset for doc audit ignores")
}

fn walk_repository(
    config: &DocAuditConfig,
    globset: &GlobSet,
) -> Result<(HashMap<PathBuf, DirectoryInfo>, Vec<PathBuf>)> {
    let mut directories: HashMap<PathBuf, DirectoryInfo> = HashMap::new();
    let mut files = Vec::new();
    let mut stack = vec![config.root.clone()];

    while let Some(dir) = stack.pop() {
        let mut dir_files = Vec::new();
        let mut dir_subdirs = Vec::new();

        let entries = fs::read_dir(&dir)
            .with_context(|| format!("Failed to read directory {}", dir.display()))?;

        for entry in entries {
            let entry =
                entry.with_context(|| format!("Failed to read entry in {}", dir.display()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("Failed to determine file type for {}", path.display()))?;

            if file_type.is_dir() {
                if should_ignore_dir(&path, config, globset) {
                    continue;
                }
                dir_subdirs.push(path.clone());
                stack.push(path);
            } else if file_type.is_file() {
                if should_ignore_file(&path, config, globset) {
                    continue;
                }
                dir_files.push(path.clone());
                files.push(path);
            }
        }

        directories.insert(
            dir,
            DirectoryInfo {
                files: dir_files,
                subdirs: dir_subdirs,
            },
        );
    }

    Ok((directories, files))
}

fn should_ignore_dir(path: &Path, config: &DocAuditConfig, globset: &GlobSet) -> bool {
    let rel = relative_path(path, &config.root);
    if globset.is_match(&rel) {
        return true;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| config.ignore_dirs.contains(name))
        .unwrap_or(false)
}

/// Scan a file and collect documentation issues using the provided scanner function.
fn scan_file_with<F>(
    file_path: &Path,
    root: &Path,
    scanner: F,
    issues: &mut Vec<DocIssue>,
)
where
    F: FnOnce(&str, &Path, &Path) -> Vec<DocIssue>,
{
    match fs::read_to_string(file_path) {
        Ok(contents) => {
            issues.extend(scanner(&contents, file_path, root));
        }
        Err(err) => {
            issues.push(DocIssue {
                category: "decode_error".to_string(),
                path: relative_path(file_path, root),
                line: None,
                symbol: None,
                detail: format!("Unable to read file using UTF-8: {err}"),
            });
        }
    }
}

fn scan_documentation(
    files: &[PathBuf],
    config: &DocAuditConfig,
    globset: &GlobSet,
) -> Vec<DocIssue> {
    let mut issues = Vec::new();

    for file_path in files {
        if should_ignore_file(file_path, config, globset) {
            continue;
        }

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());

        match ext.as_deref() {
            Some("py") => scan_file_with(file_path, &config.root, python::scan_python, &mut issues),
            Some("rs") => scan_file_with(file_path, &config.root, rust::scan_rust, &mut issues),
            Some("ts" | "tsx" | "js" | "jsx") => {
                scan_file_with(file_path, &config.root, typescript::scan_typescript, &mut issues)
            }
            _ => {}
        }
    }

    issues
}

fn should_ignore_file(path: &Path, config: &DocAuditConfig, globset: &GlobSet) -> bool {
    let rel = relative_path(path, &config.root);

    if globset.is_match(&rel) {
        return true;
    }

    let path_str = path.to_string_lossy();
    config
        .ignore_suffixes
        .iter()
        .any(|suffix| path_str.ends_with(suffix))
}

fn compute_complexities(dir_info: &HashMap<PathBuf, DirectoryInfo>) -> HashMap<PathBuf, usize> {
    let mut complexities = HashMap::new();
    let mut directories: Vec<_> = dir_info.keys().collect();
    directories.sort_by(|a, b| {
        let len_a = a.components().count();
        let len_b = b.components().count();
        len_b.cmp(&len_a)
    });

    for directory in directories {
        if let Some(info) = dir_info.get(directory) {
            let mut total = info.files.len();
            for subdir in &info.subdirs {
                let subdir_complexity = complexities.get(subdir).copied().unwrap_or(0);
                total += subdir_complexity + 1;
            }
            complexities.insert(directory.clone(), total);
        }
    }

    complexities
}

fn detect_missing_readmes(
    complexities: &HashMap<PathBuf, usize>,
    config: &DocAuditConfig,
) -> (Vec<DocIssue>, HashMap<PathBuf, PathBuf>) {
    let mut issues = Vec::new();
    let mut readmes = HashMap::new();

    for (directory, complexity) in complexities {
        if *complexity <= config.complexity_threshold {
            continue;
        }

        if let Some(readme) = find_readme(directory) {
            readmes.insert(readme, directory.clone());
            continue;
        }

        issues.push(DocIssue {
            category: "missing_readme".to_string(),
            path: relative_path(directory, &config.root),
            line: None,
            symbol: None,
            detail: format!(
                "Directory exceeds complexity threshold ({} items) without README",
                complexity
            ),
        });
    }

    (issues, readmes)
}

fn find_readme(directory: &Path) -> Option<PathBuf> {
    for candidate in README_CANDIDATES {
        let candidate_path = directory.join(candidate);
        if candidate_path.exists() {
            return Some(candidate_path);
        }
    }
    None
}

fn detect_stale_readmes(
    git_helper: &GitHelper,
    readme_index: &HashMap<PathBuf, PathBuf>,
    config: &DocAuditConfig,
) -> Vec<DocIssue> {
    let mut issues = Vec::new();

    for (readme_path, directory) in readme_index {
        if let Some(info) = git_helper.last_commit_info(readme_path) {
            if let Some(count) = git_helper.commits_since(info.oid, directory, Some(readme_path)) {
                if count > config.max_readme_commits {
                    let rel_directory = relative_path(directory, &config.root);
                    issues.push(DocIssue {
                        category: "stale_readme".to_string(),
                        path: relative_path(readme_path, &config.root),
                        line: None,
                        symbol: None,
                        detail: format!(
                            "{} commits touched '{}' since README update on {}",
                            count,
                            rel_directory.display(),
                            info.timestamp
                        ),
                    });
                }
            }
        }
    }

    issues
}

#[derive(Clone)]
struct CommitInfo {
    oid: Oid,
    timestamp: DateTime<FixedOffset>,
}

struct GitHelper {
    repo: Option<Repository>,
    repo_root: PathBuf,
}

impl GitHelper {
    fn new(root: &Path) -> Self {
        match Repository::discover(root) {
            Ok(repo) => {
                let repo_root = repo
                    .workdir()
                    .map(|path| path.to_path_buf())
                    .unwrap_or_else(|| root.to_path_buf());
                Self {
                    repo: Some(repo),
                    repo_root,
                }
            }
            Err(_) => Self {
                repo: None,
                repo_root: root.to_path_buf(),
            },
        }
    }

    fn repo(&self) -> Option<&Repository> {
        self.repo.as_ref()
    }

    fn relative_to_repo(&self, path: &Path) -> Option<PathBuf> {
        path.strip_prefix(&self.repo_root).map(PathBuf::from).ok()
    }

    fn last_commit_info(&self, path: &Path) -> Option<CommitInfo> {
        let repo = self.repo()?;
        let relative = self.relative_to_repo(path)?;

        let mut walker = repo.revwalk().ok()?;
        walker.push_head().ok()?;

        for oid in walker {
            let oid = oid.ok()?;
            let commit = repo.find_commit(oid).ok()?;
            if commit_touches_path(repo, &commit, &relative) {
                let timestamp = to_datetime(commit.time());
                return Some(CommitInfo { oid, timestamp });
            }
        }

        None
    }

    fn commits_since(
        &self,
        since: Oid,
        directory: &Path,
        exclude_path: Option<&Path>,
    ) -> Option<usize> {
        let repo = self.repo()?;
        let directory_rel = self.relative_to_repo(directory)?;
        let exclude_rel = exclude_path.and_then(|path| self.relative_to_repo(path));

        let mut walker = repo.revwalk().ok()?;
        walker.push_head().ok()?;

        let mut counter = 0usize;
        for oid in walker {
            let oid = oid.ok()?;
            if oid == since {
                break;
            }
            let commit = repo.find_commit(oid).ok()?;
            if commit_touches_directory(repo, &commit, &directory_rel, exclude_rel.as_deref()) {
                counter += 1;
            }
        }

        Some(counter)
    }
}

fn commit_touches_path(repo: &Repository, commit: &git2::Commit<'_>, path: &Path) -> bool {
    let mut diff_opts = DiffOptions::new();
    if let Some(path_str) = path.to_str() {
        diff_opts.pathspec(path_str);
    }

    let tree = match commit.tree() {
        Ok(tree) => tree,
        Err(_) => return false,
    };

    if commit.parent_count() == 0 {
        return repo
            .diff_tree_to_tree(None, Some(&tree), Some(&mut diff_opts))
            .map(|diff| diff.deltas().len() > 0)
            .unwrap_or(false);
    }

    for parent in commit.parents() {
        if let Ok(parent_tree) = parent.tree() {
            if repo
                .diff_tree_to_tree(Some(&parent_tree), Some(&tree), Some(&mut diff_opts))
                .map(|diff| diff.deltas().len() > 0)
                .unwrap_or(false)
            {
                return true;
            }
        }
    }

    false
}

fn commit_touches_directory(
    repo: &Repository,
    commit: &git2::Commit<'_>,
    directory: &Path,
    exclude_path: Option<&Path>,
) -> bool {
    let mut diff_opts = DiffOptions::new();
    if let Some(dir_str) = directory.to_str() {
        diff_opts.pathspec(dir_str);
    }

    let tree = match commit.tree() {
        Ok(tree) => tree,
        Err(_) => return false,
    };

    let mut touched = false;

    let compare = |diff: git2::Diff<'_>| {
        let mut relevant = false;
        for delta in diff.deltas() {
            let mut is_excluded = false;
            if let Some(exclude) = exclude_path {
                if delta
                    .new_file()
                    .path()
                    .and_then(|path| Some(path == exclude))
                    .unwrap_or(false)
                {
                    is_excluded = true;
                }
                if delta
                    .old_file()
                    .path()
                    .and_then(|path| Some(path == exclude))
                    .unwrap_or(false)
                {
                    is_excluded = true;
                }
            }

            if !is_excluded {
                relevant = true;
                break;
            }
        }
        relevant
    };

    if commit.parent_count() == 0 {
        if let Ok(diff) = repo.diff_tree_to_tree(None, Some(&tree), Some(&mut diff_opts)) {
            touched |= compare(diff);
        }
        return touched;
    }

    for parent in commit.parents() {
        if let Ok(parent_tree) = parent.tree() {
            if let Ok(diff) =
                repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), Some(&mut diff_opts))
            {
                if compare(diff) {
                    touched = true;
                    break;
                }
            }
        }
    }

    touched
}

fn to_datetime(time: git2::Time) -> DateTime<FixedOffset> {
    let seconds = time.seconds();
    let offset_minutes = time.offset_minutes();
    let offset = FixedOffset::east_opt(offset_minutes * 60).unwrap_or_else(|| FixedOffset::east(0));
    let naive = chrono::NaiveDateTime::from_timestamp_opt(seconds, 0)
        .unwrap_or_else(|| chrono::NaiveDateTime::from_timestamp(0, 0));
    offset.from_utc_datetime(&naive)
}

fn relative_path(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map(PathBuf::from)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn contains_todo(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    TODO_MARKERS.iter().any(|marker| upper.contains(marker))
}

fn is_incomplete_doc(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.is_empty() || contains_todo(trimmed)
}

fn extract_comment_text(lines: &[&str], mut index: usize) -> Option<String> {
    let mut collected = Vec::new();
    while index > 0 {
        index -= 1;
        let line = lines[index];
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("#[") {
            continue;
        }
        if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            collected.push(trimmed.trim_start_matches('/').trim().to_string());
            continue;
        }
        if trimmed.ends_with("*/") {
            collected.push(trimmed.to_string());
            while index > 0 {
                index -= 1;
                let inner = lines[index].trim_start();
                collected.push(inner.to_string());
                if inner.contains("/**") || inner.contains("/*!") {
                    break;
                }
            }
            break;
        }
        break;
    }

    if collected.is_empty() {
        None
    } else {
        collected.reverse();
        Some(normalize_doc_lines(&collected))
    }
}

fn normalize_doc_lines(lines: &[String]) -> String {
    let mut cleaned = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            cleaned.push(
                trimmed
                    .trim_start_matches('/')
                    .trim_start_matches('!')
                    .trim()
                    .to_string(),
            );
        } else if trimmed.starts_with("/*") || trimmed.starts_with('*') {
            let mut text = trimmed
                .trim_start_matches("/*")
                .trim_start_matches('*')
                .trim();
            text = text.trim_end_matches("*/").trim();
            if !text.is_empty() {
                cleaned.push(text.to_string());
            }
        } else {
            cleaned.push(trimmed.to_string());
        }
    }
    cleaned.join(" ")
}

#[cfg(test)]
#[path = "../doc_audit_tests.rs"]
mod tests;
