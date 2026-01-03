//! Git utilities for documentation audit.
//!
//! This module provides git-based analysis for detecting stale READMEs
//! by tracking commit history relative to documentation files.

use chrono::{DateTime, FixedOffset, TimeZone};
use git2::{DiffOptions, Oid, Repository};
use std::path::{Path, PathBuf};

/// Information about a commit.
#[derive(Clone)]
pub struct CommitInfo {
    /// Commit object ID.
    pub oid: Oid,
    /// Timestamp of the commit.
    pub timestamp: DateTime<FixedOffset>,
}

/// Helper for git repository operations.
pub struct GitHelper {
    repo: Option<Repository>,
    repo_root: PathBuf,
}

/// Git operations for repository analysis.
impl GitHelper {
    /// Create a new GitHelper for the given root path.
    pub fn new(root: &Path) -> Self {
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

    /// Get the underlying repository reference.
    pub fn repo(&self) -> Option<&Repository> {
        self.repo.as_ref()
    }

    /// Convert an absolute path to a path relative to the repository root.
    pub fn relative_to_repo(&self, path: &Path) -> Option<PathBuf> {
        path.strip_prefix(&self.repo_root).map(PathBuf::from).ok()
    }

    /// Get information about the last commit that touched the given path.
    pub fn last_commit_info(&self, path: &Path) -> Option<CommitInfo> {
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

    /// Count commits that touched a directory since a given commit,
    /// optionally excluding a specific path.
    pub fn commits_since(
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

/// Check if a commit touched a specific file path.
pub fn commit_touches_path(repo: &Repository, commit: &git2::Commit<'_>, path: &Path) -> bool {
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

/// Check if a commit touched files in a directory, optionally excluding a path.
pub fn commit_touches_directory(
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

/// Convert git2::Time to chrono DateTime with timezone offset.
pub fn to_datetime(time: git2::Time) -> DateTime<FixedOffset> {
    let seconds = time.seconds();
    let offset_minutes = time.offset_minutes();
    let offset = FixedOffset::east_opt(offset_minutes * 60).unwrap_or_else(|| FixedOffset::east(0));
    let naive = chrono::NaiveDateTime::from_timestamp_opt(seconds, 0)
        .unwrap_or_else(|| chrono::NaiveDateTime::from_timestamp(0, 0));
    offset.from_utc_datetime(&naive)
}
