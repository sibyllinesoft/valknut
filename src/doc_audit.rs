//! Documentation and README audit utilities.
//!
//! Scans codebases for missing documentation (docstrings in Python, rustdoc in Rust,
//! JSDoc in TypeScript/JavaScript), missing READMEs in complex directories, and
//! stale READMEs that haven't been updated alongside the code.

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
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
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

mod python {
    use super::{is_incomplete_doc, relative_path, DocIssue};
    use std::path::Path;

    pub fn scan_python(source: &str, path: &Path, root: &Path) -> Vec<DocIssue> {
        let lines: Vec<&str> = source.lines().collect();
        let mut issues = Vec::new();
        let mut stack: Vec<(usize, String)> = Vec::new();
        let mut index = 0usize;

        while index < lines.len() {
            let line = lines[index];
            let trimmed = line.trim_start();

            if trimmed.starts_with("def ")
                || trimmed.starts_with("async def ")
                || trimmed.starts_with("class ")
            {
                let indent = indentation(line);
                while let Some((current_indent, _)) = stack.last() {
                    if *current_indent >= indent {
                        stack.pop();
                    } else {
                        break;
                    }
                }

                if let Some((symbol, kind)) = parse_symbol(trimmed) {
                    let mut full_name = stack
                        .iter()
                        .map(|(_, name)| name.as_str())
                        .collect::<Vec<&str>>();
                    full_name.push(&symbol);
                    let symbol_name = full_name.join(".");

                    match find_docstring(&lines, index + 1, indent) {
                        Some((docstring, end_index)) => {
                            if is_incomplete_doc(&docstring) {
                                issues.push(build_issue(
                                    path,
                                    root,
                                    index + 1,
                                    kind,
                                    &symbol_name,
                                    format!("{} '{}' has incomplete docstring", kind, symbol_name),
                                ));
                            }
                            stack.push((indent, symbol));
                            index = end_index;
                        }
                        None => {
                            issues.push(build_issue(
                                path,
                                root,
                                index + 1,
                                kind,
                                &symbol_name,
                                format!("{} '{}' is missing a docstring", kind, symbol_name),
                            ));
                            stack.push((indent, symbol));
                        }
                    }
                }
            }

            index += 1;
        }

        issues
    }

    fn build_issue(
        path: &Path,
        root: &Path,
        line: usize,
        kind: &str,
        symbol: &str,
        detail: String,
    ) -> DocIssue {
        DocIssue {
            category: "undocumented_python".to_string(),
            path: relative_path(path, root),
            line: Some(line),
            symbol: Some(symbol.to_string()),
            detail,
        }
    }

    fn indentation(line: &str) -> usize {
        line.chars()
            .take_while(|ch| ch.is_ascii_whitespace())
            .count()
    }

    fn parse_symbol(line: &str) -> Option<(String, &'static str)> {
        if line.starts_with("class ") {
            return extract_symbol_name(line, "class ", "Class");
        }
        if line.starts_with("async def ") {
            return extract_symbol_name(line, "async def ", "Function");
        }
        if line.starts_with("def ") {
            return extract_symbol_name(line, "def ", "Function");
        }
        None
    }

    fn extract_symbol_name(line: &str, prefix: &str, kind: &'static str) -> Option<(String, &'static str)> {
        let name = line[prefix.len()..]
            .split(|c: char| c == '(' || c == ':' || c.is_whitespace())
            .next()?;
        Some((name.to_string(), kind))
    }

    fn find_docstring(lines: &[&str], mut index: usize, indent: usize) -> Option<(String, usize)> {
        while index < lines.len() {
            let line = lines[index];
            let trimmed = line.trim_start();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                index += 1;
                continue;
            }

            if indentation(line) <= indent {
                return None;
            }

            if let Some(doc) = extract_docstring(lines, index) {
                return Some(doc);
            }

            break;
        }

        None
    }

    fn extract_docstring(lines: &[&str], index: usize) -> Option<(String, usize)> {
        let line = lines[index].trim_start();
        let (prefix_len, quote_char) = find_string_prefix(line)?;
        let marker = match quote_char {
            '\'' => "'''",
            '"' => "\"\"\"",
            _ => return None,
        };

        let remainder = &line[prefix_len..];
        if !remainder.starts_with(marker) {
            return None;
        }

        let after_marker = &remainder[marker.len()..];
        if let Some(end_pos) = after_marker.find(marker) {
            let content = after_marker[..end_pos].to_string();
            return Some((content, index));
        }

        let mut collected = vec![after_marker.to_string()];
        let mut current = index + 1;

        while current < lines.len() {
            let current_line = lines[current];
            if let Some(end_pos) = current_line.find(marker) {
                let before = &current_line[..end_pos];
                collected.push(before.to_string());
                let doc = collected.join("\n");
                return Some((doc, current));
            } else {
                collected.push(current_line.to_string());
            }
            current += 1;
        }

        None
    }

    fn find_string_prefix(line: &str) -> Option<(usize, char)> {
        let mut index = 0;
        let chars: Vec<char> = line.chars().collect();
        while index < chars.len() {
            let ch = chars[index];
            if ch == '\'' || ch == '"' {
                return Some((index, ch));
            }
            if ch.is_ascii_alphabetic() {
                index += 1;
                continue;
            }
            break;
        }
        None
    }
}

mod rust {
    use super::{extract_comment_text, is_incomplete_doc, relative_path, DocIssue};
    use std::path::Path;

    pub fn scan_rust(source: &str, path: &Path, root: &Path) -> Vec<DocIssue> {
        let lines: Vec<&str> = source.lines().collect();
        let mut issues = Vec::new();
        let mut pending_attrs: Vec<String> = Vec::new();
        let mut index = 0usize;

        while index < lines.len() {
            let line = lines[index];
            let trimmed = line.trim_start();

            if trimmed.is_empty() || trimmed.starts_with("///") || trimmed.starts_with("//!") {
                index += 1;
                continue;
            }

            // Handle attribute lines
            if trimmed.starts_with("#[") {
                let effective_line = handle_attribute_line(trimmed, &mut pending_attrs);
                if let Some(remainder) = effective_line {
                    let has_test_attr = has_test_attribute(&pending_attrs);
                    if let Some(new_index) = process_item_line(
                        &remainder, &lines, index, has_test_attr, &mut pending_attrs,
                        &mut issues, path, root,
                    ) {
                        index = new_index;
                        continue;
                    }
                }
                index += 1;
                continue;
            }

            let has_test_attr = has_test_attribute(&pending_attrs);
            if let Some(new_index) = process_item_line(
                trimmed, &lines, index, has_test_attr, &mut pending_attrs,
                &mut issues, path, root,
            ) {
                index = new_index;
            } else {
                pending_attrs.clear();
                index += 1;
            }
        }

        issues
    }

    /// Handle attribute line and return remainder if item follows on same line
    fn handle_attribute_line(trimmed: &str, pending_attrs: &mut Vec<String>) -> Option<String> {
        pending_attrs.push(trimmed.to_string());

        if let Some(pos) = trimmed.rfind(']') {
            let remainder = trimmed[pos + 1..].trim_start();
            if !remainder.is_empty() {
                return Some(remainder.to_string());
            }
        }
        None
    }

    /// Check if pending attributes include test-related markers
    fn has_test_attribute(pending_attrs: &[String]) -> bool {
        pending_attrs.iter().any(|attr| attr.contains("cfg(test)") || attr.contains("test"))
    }

    /// Process an item line and return the new index if handled
    fn process_item_line(
        trimmed: &str,
        lines: &[&str],
        index: usize,
        has_test_attr: bool,
        pending_attrs: &mut Vec<String>,
        issues: &mut Vec<DocIssue>,
        path: &Path,
        root: &Path,
    ) -> Option<usize> {
        if trimmed.starts_with("mod ") {
            pending_attrs.clear();
            return Some(handle_module_item(trimmed, lines, index, has_test_attr, issues, path, root));
        }

        if let Some(name) = detect_function_name(trimmed) {
            pending_attrs.clear();
            if !has_test_attr {
                check_item_docs(lines, index, &name, "undocumented_rust_fn", "Function", issues, path, root);
            }
            return Some(index + 1);
        }

        if let Some((kind, name)) = detect_type(trimmed) {
            pending_attrs.clear();
            if !has_test_attr {
                check_item_docs(lines, index, &name, "undocumented_rust_item", kind, issues, path, root);
            }
            return Some(index + 1);
        }

        if let Some(target) = detect_impl(trimmed) {
            pending_attrs.clear();
            if !has_test_attr {
                check_impl_docs(lines, index, &target, issues, path, root);
            }
            return Some(index + 1);
        }

        None
    }

    /// Handle module item and return new index
    fn handle_module_item(
        trimmed: &str,
        lines: &[&str],
        index: usize,
        has_test_attr: bool,
        issues: &mut Vec<DocIssue>,
        path: &Path,
        root: &Path,
    ) -> usize {
        if has_test_attr {
            return skip_block(lines, index).map(|end| end + 1).unwrap_or(index + 1);
        }
        if trimmed.ends_with(';') {
            return index + 1;
        }
        if let Some(name) = extract_identifier(trimmed, "mod") {
            if is_doc_missing(lines, index) {
                push_issue(issues, path, root, index + 1, Some(&name),
                    "undocumented_rust_module",
                    format!("Module '{}' lacks module-level docs", name));
            }
        }
        index + 1
    }

    /// Check docs for a named item (function, struct, enum, trait)
    fn check_item_docs(
        lines: &[&str],
        index: usize,
        name: &str,
        category: &'static str,
        kind: &str,
        issues: &mut Vec<DocIssue>,
        path: &Path,
        root: &Path,
    ) {
        if let Some(doc) = extract_comment_text(lines, index) {
            if is_incomplete_doc(&doc) {
                push_issue(issues, path, root, index + 1, Some(name), category,
                    format!("{} '{}' has incomplete rustdoc", kind, name));
            }
        } else {
            push_issue(issues, path, root, index + 1, Some(name), category,
                format!("{} '{}' lacks rustdoc", kind, name));
        }
    }

    /// Check docs for impl blocks
    fn check_impl_docs(
        lines: &[&str],
        index: usize,
        target: &str,
        issues: &mut Vec<DocIssue>,
        path: &Path,
        root: &Path,
    ) {
        if let Some(doc) = extract_comment_text(lines, index) {
            if is_incomplete_doc(&doc) {
                push_issue(issues, path, root, index + 1, Some(target),
                    "undocumented_rust_impl",
                    format!("impl block for '{}' has incomplete docs", target));
            }
        } else {
            push_issue(issues, path, root, index + 1, Some(target),
                "undocumented_rust_impl",
                format!("impl block for '{}' lacks overview docs", target));
        }
    }

    fn skip_block(lines: &[&str], start: usize) -> Option<usize> {
        let mut depth: isize = 0;
        for (idx, line) in lines.iter().enumerate().skip(start) {
            depth += line.chars().filter(|c| *c == '{').count() as isize;
            depth -= line.chars().filter(|c| *c == '}').count() as isize;
            if depth == 0 && idx >= start {
                return Some(idx);
            }
        }
        None
    }

    fn push_issue(
        issues: &mut Vec<DocIssue>,
        path: &Path,
        root: &Path,
        line: usize,
        symbol: Option<&str>,
        category: &'static str,
        detail: String,
    ) {
        issues.push(DocIssue {
            category: category.to_string(),
            path: relative_path(path, root),
            line: Some(line),
            symbol: symbol.map(|s| s.to_string()),
            detail,
        });
    }

    fn detect_function_name(line: &str) -> Option<String> {
        let fn_pos = find_keyword(line, "fn")?;
        let prefix = line[..fn_pos].trim();
        if !is_valid_fn_prefix(prefix) {
            return None;
        }
        let remainder = line[fn_pos + 2..].trim_start();
        let name = remainder
            .split(|c: char| c == '(' || c == '<' || c.is_whitespace())
            .next()?;
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn detect_type(line: &str) -> Option<(&'static str, String)> {
        for keyword in ["struct", "enum", "trait"] {
            if let Some(name) = extract_identifier(line, keyword) {
                let kind = match keyword {
                    "struct" => "Struct",
                    "enum" => "Enum",
                    "trait" => "Trait",
                    _ => unreachable!(),
                };
                return Some((kind, name));
            }
        }
        None
    }

    fn detect_impl(line: &str) -> Option<String> {
        let impl_pos = find_keyword(line, "impl")?;
        let prefix = line[..impl_pos].trim();
        if !prefix.is_empty() && prefix != "unsafe" && prefix != "default" {
            return None;
        }
        let remainder = line[impl_pos + 4..].trim_start();
        let target = remainder
            .split(|c: char| c == '{' || c == ' ' || c == '\t' || c == '\n')
            .filter(|segment| !segment.is_empty())
            .next()?;
        Some(target.trim_matches(|c| c == '<' || c == '>').to_string())
    }

    fn extract_identifier(line: &str, keyword: &str) -> Option<String> {
        if !line.starts_with(keyword) && !line.starts_with(&format!("pub {}", keyword)) {
            return None;
        }
        let remainder = line[keyword.len()..].trim_start();
        let name = remainder
            .split(|c: char| c == '{' || c == '(' || c.is_whitespace())
            .next()?;
        if name.is_empty() {
            None
        } else {
            Some(name.trim_end_matches(';').to_string())
        }
    }

    fn find_keyword(line: &str, keyword: &str) -> Option<usize> {
        let mut offset = 0usize;
        while let Some(found) = line[offset..].find(keyword) {
            let idx = offset + found;
            let start_ok = idx == 0
                || !line
                    .chars()
                    .nth(idx - 1)
                    .map(|ch| ch.is_alphanumeric() || ch == '_')
                    .unwrap_or(false);
            let end_ok = line
                .chars()
                .nth(idx + keyword.len())
                .map(|ch| !ch.is_alphanumeric() && ch != '_')
                .unwrap_or(true);
            if start_ok && end_ok {
                return Some(idx);
            }
            offset = idx + keyword.len();
        }
        None
    }

    fn is_valid_fn_prefix(prefix: &str) -> bool {
        if prefix.is_empty() {
            return true;
        }
        let tokens = prefix.split_whitespace();
        for token in tokens {
            let token = token.trim();
            if token.starts_with("pub") || matches!(token, "async" | "unsafe" | "const") {
                continue;
            }
            if token.starts_with("extern") {
                continue;
            }
            return false;
        }
        true
    }

    fn is_doc_missing(lines: &[&str], index: usize) -> bool {
        extract_comment_text(lines, index).is_none()
    }
}

mod typescript {
    use super::{extract_comment_text, is_incomplete_doc, relative_path, DocIssue};
    use std::path::Path;

    pub fn scan_typescript(source: &str, path: &Path, root: &Path) -> Vec<DocIssue> {
        let lines: Vec<&str> = source.lines().collect();
        let mut issues = Vec::new();

        for index in 0..lines.len() {
            let line = lines[index];
            let trimmed = line.trim_start();

            if let Some(name) = detect_function(trimmed) {
                push_issue_if_needed(
                    &lines,
                    index,
                    path,
                    root,
                    "undocumented_ts_function",
                    &name,
                    format!("Function '{}' missing doc comment", name),
                    format!("Function '{}' has incomplete doc comment", name),
                    &mut issues,
                );
            } else if let Some(name) = detect_class(trimmed) {
                push_issue_if_needed(
                    &lines,
                    index,
                    path,
                    root,
                    "undocumented_ts_class",
                    &name,
                    format!("Class '{}' missing doc comment", name),
                    format!("Class '{}' has incomplete doc comment", name),
                    &mut issues,
                );
            } else if let Some(name) = detect_arrow_function(trimmed) {
                push_issue_if_needed(
                    &lines,
                    index,
                    path,
                    root,
                    "undocumented_ts_arrow",
                    &name,
                    format!("Function '{}' missing doc comment", name),
                    format!("Function '{}' has incomplete doc comment", name),
                    &mut issues,
                );
            }
        }

        issues
    }

    fn push_issue_if_needed(
        lines: &[&str],
        index: usize,
        path: &Path,
        root: &Path,
        category: &'static str,
        symbol: &str,
        missing_detail: String,
        incomplete_detail: String,
        issues: &mut Vec<DocIssue>,
    ) {
        match extract_comment_text(lines, index) {
            Some(doc) if !is_incomplete_doc(&doc) => {}
            Some(_) => issues.push(build_issue(
                path,
                root,
                index + 1,
                category,
                Some(symbol),
                incomplete_detail,
            )),
            None => issues.push(build_issue(
                path,
                root,
                index + 1,
                category,
                Some(symbol),
                missing_detail,
            )),
        }
    }

    fn build_issue(
        path: &Path,
        root: &Path,
        line: usize,
        category: &'static str,
        symbol: Option<&str>,
        detail: String,
    ) -> DocIssue {
        DocIssue {
            category: category.to_string(),
            path: relative_path(path, root),
            line: Some(line),
            symbol: symbol.map(|s| s.to_string()),
            detail,
        }
    }

    fn detect_function(line: &str) -> Option<String> {
        if !line.contains("function") {
            return None;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        for (idx, token) in tokens.iter().enumerate() {
            if *token == "function" {
                return tokens
                    .get(idx + 1)
                    .map(|name| name.trim_end_matches(|c| c == '(' || c == '{').to_string());
            }
        }
        None
    }

    fn detect_class(line: &str) -> Option<String> {
        if !line.contains("class ") {
            return None;
        }
        line.split_whitespace()
            .skip_while(|token| *token != "class")
            .nth(1)
            .map(|name| name.trim_end_matches(|c| c == '{' || c == '(').to_string())
    }

    fn detect_arrow_function(line: &str) -> Option<String> {
        if !(line.starts_with("const ") || line.starts_with("let ") || line.starts_with("var ")) {
            return None;
        }
        let lhs = line.split('=').next()?.trim();
        let name_token = lhs.split_whitespace().last()?;
        if line.contains("=>") {
            Some(name_token.to_string())
        } else {
            None
        }
    }
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
#[path = "doc_audit_tests.rs"]
mod tests;
