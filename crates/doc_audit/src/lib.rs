use anyhow::{Context, Result};
use chrono::{DateTime, FixedOffset, TimeZone};
use git2::{DiffOptions, Oid, Repository};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_COMPLEXITY_THRESHOLD: usize = 8;
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

#[derive(Clone, Debug)]
pub struct DocAuditConfig {
    pub root: PathBuf,
    pub complexity_threshold: usize,
    pub max_readme_commits: usize,
    pub ignore_dirs: HashSet<String>,
    pub ignore_suffixes: HashSet<String>,
}

impl DocAuditConfig {
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
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Serialize)]
pub struct DocIssue {
    pub category: String,
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct AuditResult {
    pub documentation_issues: Vec<DocIssue>,
    pub missing_readmes: Vec<DocIssue>,
    pub stale_readmes: Vec<DocIssue>,
}

impl AuditResult {
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

pub fn run_audit(config: &DocAuditConfig) -> Result<AuditResult> {
    let (dir_info, files) = walk_repository(config)?;
    let documentation_issues = scan_documentation(&files, config);
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
            out.push_str(&format(&issue));
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

pub fn render_json(result: &AuditResult) -> Result<String> {
    serde_json::to_string_pretty(result).context("Failed to serialize audit results to JSON")
}

fn walk_repository(
    config: &DocAuditConfig,
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
                if should_ignore_dir(&path, config) {
                    continue;
                }
                dir_subdirs.push(path.clone());
                stack.push(path);
            } else if file_type.is_file() {
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

fn should_ignore_dir(path: &Path, config: &DocAuditConfig) -> bool {
    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
        config.ignore_dirs.contains(name)
    } else {
        false
    }
}

fn scan_documentation(files: &[PathBuf], config: &DocAuditConfig) -> Vec<DocIssue> {
    let mut issues = Vec::new();

    for file_path in files {
        if should_ignore_file(file_path, config) {
            continue;
        }

        match file_path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if matches!(ext.to_ascii_lowercase().as_str(), "py") => {
                match fs::read_to_string(file_path) {
                    Ok(contents) => {
                        let mut file_issues =
                            python::scan_python(&contents, file_path, &config.root);
                        issues.append(&mut file_issues);
                    }
                    Err(err) => issues.push(DocIssue {
                        category: "decode_error".to_string(),
                        path: relative_path(file_path, &config.root),
                        line: None,
                        symbol: None,
                        detail: format!("Unable to read file using UTF-8: {err}"),
                    }),
                }
            }
            Some(ext) if matches!(ext.to_ascii_lowercase().as_str(), "rs") => {
                match fs::read_to_string(file_path) {
                    Ok(contents) => {
                        let mut file_issues = rust::scan_rust(&contents, file_path, &config.root);
                        issues.append(&mut file_issues);
                    }
                    Err(err) => issues.push(DocIssue {
                        category: "decode_error".to_string(),
                        path: relative_path(file_path, &config.root),
                        line: None,
                        symbol: None,
                        detail: format!("Unable to read file using UTF-8: {err}"),
                    }),
                }
            }
            Some(ext)
                if matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "ts" | "tsx" | "js" | "jsx"
                ) =>
            {
                match fs::read_to_string(file_path) {
                    Ok(contents) => {
                        let mut file_issues =
                            typescript::scan_typescript(&contents, file_path, &config.root);
                        issues.append(&mut file_issues);
                    }
                    Err(err) => issues.push(DocIssue {
                        category: "decode_error".to_string(),
                        path: relative_path(file_path, &config.root),
                        line: None,
                        symbol: None,
                        detail: format!("Unable to read file using UTF-8: {err}"),
                    }),
                }
            }
            _ => {}
        }
    }

    issues
}

fn should_ignore_file(path: &Path, config: &DocAuditConfig) -> bool {
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
            let name = line["class ".len()..]
                .split(|c: char| c == '(' || c == ':' || c.is_whitespace())
                .next()?;
            return Some((name.to_string(), "Class"));
        } else if line.starts_with("async def ") {
            let name = line["async def ".len()..]
                .split(|c: char| c == '(' || c == ':' || c.is_whitespace())
                .next()?;
            return Some((name.to_string(), "Function"));
        } else if line.starts_with("def ") {
            let name = line["def ".len()..]
                .split(|c: char| c == '(' || c == ':' || c.is_whitespace())
                .next()?;
            return Some((name.to_string(), "Function"));
        }
        None
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

        for index in 0..lines.len() {
            let line = lines[index];
            let trimmed = line.trim_start();
            if trimmed.starts_with("mod ") {
                if trimmed.ends_with(';') {
                    continue;
                }
                if let Some(name) = extract_identifier(trimmed, "mod") {
                    if is_doc_missing(&lines, index) {
                        push_issue(
                            &mut issues,
                            path,
                            root,
                            index + 1,
                            Some(&name),
                            "undocumented_rust_module",
                            format!("Module '{}' lacks module-level docs", name),
                        );
                    }
                }
            } else if let Some(name) = detect_function_name(trimmed) {
                if let Some(doc) = extract_comment_text(&lines, index) {
                    if is_incomplete_doc(&doc) {
                        push_issue(
                            &mut issues,
                            path,
                            root,
                            index + 1,
                            Some(&name),
                            "undocumented_rust_fn",
                            format!("Function '{}' has incomplete rustdoc", name),
                        );
                    }
                } else {
                    push_issue(
                        &mut issues,
                        path,
                        root,
                        index + 1,
                        Some(&name),
                        "undocumented_rust_fn",
                        format!("Function '{}' lacks rustdoc", name),
                    );
                }
            } else if let Some((kind, name)) = detect_type(trimmed) {
                if let Some(doc) = extract_comment_text(&lines, index) {
                    if is_incomplete_doc(&doc) {
                        push_issue(
                            &mut issues,
                            path,
                            root,
                            index + 1,
                            Some(&name),
                            "undocumented_rust_item",
                            format!("{} '{}' has incomplete rustdoc", kind, name),
                        );
                    }
                } else {
                    push_issue(
                        &mut issues,
                        path,
                        root,
                        index + 1,
                        Some(&name),
                        "undocumented_rust_item",
                        format!("{} '{}' lacks rustdoc", kind, name),
                    );
                }
            } else if let Some(target) = detect_impl(trimmed) {
                if let Some(doc) = extract_comment_text(&lines, index) {
                    if is_incomplete_doc(&doc) {
                        push_issue(
                            &mut issues,
                            path,
                            root,
                            index + 1,
                            Some(&target),
                            "undocumented_rust_impl",
                            format!("impl block for '{}' has incomplete docs", target),
                        );
                    }
                } else {
                    push_issue(
                        &mut issues,
                        path,
                        root,
                        index + 1,
                        Some(&target),
                        "undocumented_rust_impl",
                        format!("impl block for '{}' lacks overview docs", target),
                    );
                }
            }
        }

        issues
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
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_relative_path() {
        let root = PathBuf::from("/tmp/project");
        let file = PathBuf::from("/tmp/project/src/lib.rs");
        assert_eq!(relative_path(&file, &root), PathBuf::from("src/lib.rs"));
    }

    #[test]
    fn test_is_incomplete_doc_empty() {
        assert!(is_incomplete_doc(""));
    }

    #[test]
    fn test_is_incomplete_doc_todo() {
        assert!(is_incomplete_doc("TODO: fill in"));
    }

    #[test]
    fn test_is_incomplete_doc_ok() {
        assert!(!is_incomplete_doc("Describe behavior"));
    }

    #[test]
    fn audit_reports_python_doc_gap() -> Result<()> {
        let dir = tempdir()?;
        let root = dir.path().to_path_buf();
        let file_path = root.join("sample.py");

        fs::write(
            &file_path,
            r#"def important_function():
    return 42
"#,
        )?;

        let mut config = DocAuditConfig::new(root);
        config.complexity_threshold = usize::MAX; // avoid README enforcement noise

        let result = run_audit(&config)?;
        let issue = result
            .documentation_issues
            .iter()
            .find(|issue| issue.path.ends_with("sample.py"));
        assert!(issue.is_some(), "expected missing docstring to be reported");
        Ok(())
    }

    #[test]
    fn audit_reports_missing_readme_for_complex_directory() -> Result<()> {
        let dir = tempdir()?;
        let root = dir.path();
        let heavy = root.join("complex");
        fs::create_dir_all(&heavy)?;
        for idx in 0..3 {
            fs::write(heavy.join(format!("file{idx}.rs")), "pub fn f() {}")?;
        }
        let nested = heavy.join("inner");
        fs::create_dir_all(&nested)?;
        fs::write(nested.join("lib.rs"), "pub fn inner() {}")?;

        let mut config = DocAuditConfig::new(root.to_path_buf());
        config.complexity_threshold = 2;

        let result = run_audit(&config)?;
        assert!(result
            .missing_readmes
            .iter()
            .any(|issue| issue.path == PathBuf::from("complex")));
        Ok(())
    }

    #[test]
    fn audit_reports_stale_readme_after_commits() -> Result<()> {
        let dir = tempdir()?;
        let root = dir.path();
        let repo = Repository::init(root)?;

        // Initial README commit
        fs::write(root.join("README.md"), "# Project\n")?;
        stage_and_commit(&repo, &["README.md"], "initial");

        // Modify project contents in subsequent commits
        fs::create_dir_all(root.join("src"))?;
        fs::write(root.join("src/lib.rs"), "pub fn lib() {}")?;
        stage_and_commit(&repo, &["src/lib.rs"], "add lib");

        let mut config = DocAuditConfig::new(root.to_path_buf());
        config.complexity_threshold = 0;
        config.max_readme_commits = 0; // treat any subsequent change as stale

        let result = run_audit(&config)?;
        assert!(result
            .stale_readmes
            .iter()
            .any(|issue| issue.path == PathBuf::from("README.md")));
        Ok(())
    }

    #[test]
    fn render_helpers_format_output() -> Result<()> {
        let sample = AuditResult {
            documentation_issues: vec![DocIssue {
                category: "undocumented_python".into(),
                path: PathBuf::from("main.py"),
                line: Some(3),
                symbol: Some("main".into()),
                detail: "Function 'main' is missing a docstring".into(),
            }],
            missing_readmes: vec![DocIssue {
                category: "missing_readme".into(),
                path: PathBuf::from("services"),
                line: None,
                symbol: None,
                detail: "Directory exceeds complexity threshold (12 items) without README".into(),
            }],
            stale_readmes: vec![DocIssue {
                category: "stale_readme".into(),
                path: PathBuf::from("README.md"),
                line: None,
                symbol: None,
                detail: "5 commits touched '.' since README update on 2024-01-01T00:00:00+00:00"
                    .into(),
            }],
        };

        let text = render_text(&sample);
        assert!(text.contains("Documentation gaps"));
        assert!(text.contains("Missing READMEs"));
        assert!(text.contains("Stale READMEs"));

        let json = render_json(&sample)?;
        let parsed: serde_json::Value = serde_json::from_str(&json)?;
        assert_eq!(parsed["documentation_issues"].as_array().unwrap().len(), 1);
        Ok(())
    }

    #[test]
    fn compute_complexities_counts_files_and_subdirectories() {
        let root = PathBuf::from("/tmp/project");
        let child = root.join("child");

        let mut dir_info = HashMap::new();
        dir_info.insert(
            child.clone(),
            DirectoryInfo {
                files: vec![child.join("lib.rs")],
                subdirs: Vec::new(),
            },
        );
        dir_info.insert(
            root.clone(),
            DirectoryInfo {
                files: vec![root.join("main.py")],
                subdirs: vec![child.clone()],
            },
        );

        let complexities = compute_complexities(&dir_info);
        assert_eq!(complexities.get(&child), Some(&1));
        assert_eq!(complexities.get(&root), Some(&3));
    }

    #[test]
    fn detect_missing_readmes_skips_directories_with_existing_docs() -> Result<()> {
        let temp = tempdir()?;
        let root = temp.path();
        let component = root.join("services");
        fs::create_dir_all(&component)?;
        fs::write(component.join("README.md"), "# docs")?;

        let mut complexities = HashMap::new();
        complexities.insert(component.clone(), DEFAULT_COMPLEXITY_THRESHOLD + 5);

        let mut config = DocAuditConfig::new(root.to_path_buf());
        config.complexity_threshold = 1;

        let (missing, index) = detect_missing_readmes(&complexities, &config);
        assert!(missing.is_empty(), "expected no missing README issues");
        let readme_path = component.join("README.md");
        assert_eq!(index.get(&readme_path), Some(&component));
        Ok(())
    }

    #[test]
    fn should_ignore_file_respects_suffix_configuration() {
        let root = PathBuf::from("/tmp/project");
        let mut config = DocAuditConfig::new(root.clone());
        config.ignore_suffixes.insert(".ignore-me".into());

        let ignored = should_ignore_file(&root.join("bundle.ignore-me"), &config);
        let tracked = should_ignore_file(&root.join("lib.rs"), &config);

        assert!(ignored);
        assert!(!tracked);
    }

    #[test]
    fn python_scanner_detects_missing_and_incomplete_docstrings() {
        let root = PathBuf::from("/tmp/project");
        let path = root.join("analysis.py");
        let source = r#"
class Service:
    def ok(self):
        \"\"\"Performs the operation\"\"\"
        return 1

    def needs_docs(self):
        \"\"\"TODO: describe\"\"\"
        return 2

async def helper():
    return 3
"#;

        let issues = python::scan_python(source, &path, &root);
        let symbols: Vec<_> = issues
            .iter()
            .map(|issue| issue.symbol.clone().unwrap_or_default())
            .collect();

        assert!(
            symbols.iter().any(|symbol| symbol == "Service.needs_docs"),
            "expected incomplete docstring to be reported"
        );
        assert!(
            symbols.iter().any(|symbol| symbol == "helper"),
            "expected missing docstring for helper"
        );
    }

    #[test]
    fn rust_scanner_flags_undocumented_items() {
        let root = PathBuf::from("/tmp/project");
        let path = root.join("lib.rs");
        let source = r#"
mod utilities {
    pub fn helper() {}
}

/// TODO: finish docs
impl utilities::Helper {
    pub fn build() {}
}

pub struct Widget;

fn needs_docs() {}
"#;

        let issues = rust::scan_rust(source, &path, &root);
        let mut categories: Vec<_> = issues.iter().map(|issue| issue.category.as_str()).collect();
        categories.sort();

        assert!(
            categories.contains(&"undocumented_rust_module"),
            "module without docs should be reported"
        );
        assert!(
            categories.contains(&"undocumented_rust_fn"),
            "functions without docs should be reported"
        );
        assert!(
            categories.contains(&"undocumented_rust_impl"),
            "impl blocks with incomplete docs should be reported"
        );
        assert!(
            categories.contains(&"undocumented_rust_item"),
            "structs without docs should be reported"
        );
    }

    #[test]
    fn typescript_scanner_handles_functions_classes_and_arrows() {
        let root = PathBuf::from("/tmp/project");
        let path = root.join("metrics.ts");
        let source = r#"
/**
 * TODO: doc pending
 */
function summarized(): number {
    return 0;
}

class Widget {
    method() {}
}

const compute = (value: number) => {
    return value * 2;
};
"#;

        let issues = typescript::scan_typescript(source, &path, &root);
        let categories: HashSet<_> = issues.iter().map(|issue| issue.category.as_str()).collect();

        assert!(categories.contains("undocumented_ts_function"));
        assert!(categories.contains("undocumented_ts_class"));
        assert!(categories.contains("undocumented_ts_arrow"));
    }

    fn stage_and_commit(repo: &Repository, paths: &[&str], message: &str) {
        let mut index = repo.index().expect("index");
        for path in paths {
            index.add_path(Path::new(path)).expect("add path");
        }
        index.write().expect("write index");
        let tree_id = index.write_tree().expect("write tree");
        let tree = repo.find_tree(tree_id).expect("find tree");
        let sig = git2::Signature::now("Test", "test@example.com").expect("signature");

        let parents: Vec<git2::Commit> = repo
            .head()
            .ok()
            .and_then(|reference| reference.peel_to_commit().ok())
            .into_iter()
            .collect();

        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
            .expect("commit");
    }
}
