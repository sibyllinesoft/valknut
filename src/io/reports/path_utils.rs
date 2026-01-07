//! Path cleaning utilities for report generation.
//!
//! This module provides functions for cleaning path prefixes and normalizing
//! paths in refactoring candidates, file groups, and directory health trees.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::pipeline::{
    DirectoryHealthTree, FileRefactoringGroup, RefactoringCandidate,
};

/// Clean path strings by removing absolute path prefixes and "./" prefixes.
pub fn clean_path_string(path: &str) -> String {
    clean_path_with_root(path, None)
}

/// Clean path strings using an optional explicit root path.
/// If root is None, attempts to detect the project root from the path itself.
pub fn clean_path_with_root(path: &str, root: Option<&str>) -> String {
    // If explicit root provided, use it
    if let Some(root_path) = root {
        if !root_path.is_empty() && path.starts_with(root_path) {
            let relative = &path[root_path.len()..];
            let cleaned = relative.strip_prefix('/').unwrap_or(relative);
            if !cleaned.is_empty() {
                return cleaned.to_string();
            }
        }
    }

    // Try current_dir for backwards compatibility
    if let Ok(current_dir) = std::env::current_dir() {
        let current_dir_str = current_dir.to_string_lossy();
        if path.starts_with(&current_dir_str.as_ref()) {
            let relative = &path[current_dir_str.len()..];
            let cleaned = relative.strip_prefix('/').unwrap_or(relative);
            if !cleaned.is_empty() {
                return cleaned.to_string();
            }
        }
    }

    // Handle arbitrary absolute paths by finding common project markers
    if path.starts_with('/') {
        // Look for common project root markers
        const PROJECT_MARKERS: &[&str] = &[
            "/src/", "/tests/", "/test/", "/lib/", "/bin/", "/pkg/",
            "/cmd/", "/internal/", "/examples/", "/benchmarks/",
            "/templates/", "/docs/", "/scripts/", "/config/",
        ];

        for marker in PROJECT_MARKERS {
            if let Some(idx) = path.find(marker) {
                // Return path starting from the marker (without leading slash)
                return path[idx + 1..].to_string();
            }
        }

        // If no marker found, try to find a reasonable cutoff
        // Look for typical repo directory patterns (random hash dirs, repo names)
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() > 3 {
            // Skip /tmp/repos/hash or similar patterns
            // Find first part that looks like a source directory
            for (i, part) in parts.iter().enumerate() {
                if i > 0
                    && (*part == "src"
                        || *part == "lib"
                        || *part == "tests"
                        || *part == "pkg"
                        || part.ends_with(".rs")
                        || part.ends_with(".py")
                        || part.ends_with(".ts")
                        || part.ends_with(".go"))
                {
                    return parts[i..].join("/");
                }
            }
        }
    }

    // Handle "./" prefixes
    if path.starts_with("./") {
        path[2..].to_string()
    } else {
        path.to_string()
    }
}

/// Detect the common root path from a collection of paths.
/// Returns the longest common prefix that ends at a directory boundary.
pub fn detect_project_root<'a>(paths: impl Iterator<Item = &'a str>) -> Option<String> {
    let abs_paths: Vec<&str> = paths.filter(|p| p.starts_with('/')).collect();

    if abs_paths.is_empty() {
        return None;
    }

    // Find common prefix
    let first = abs_paths[0];
    let mut common_len = first.len();

    for path in &abs_paths[1..] {
        let matching = first
            .chars()
            .zip(path.chars())
            .take_while(|(a, b)| a == b)
            .count();
        common_len = common_len.min(matching);
    }

    if common_len == 0 {
        return None;
    }

    // Trim to last directory boundary
    let common = &first[..common_len];
    if let Some(last_slash) = common.rfind('/') {
        if last_slash > 0 {
            return Some(first[..last_slash].to_string());
        }
    }

    None
}

/// Clean path prefixes like "./" from refactoring candidates.
pub fn clean_path_prefixes(candidates: &[RefactoringCandidate]) -> Vec<RefactoringCandidate> {
    candidates
        .iter()
        .cloned()
        .map(|mut candidate| {
            candidate.file_path = clean_path_string(&candidate.file_path);
            candidate.entity_id = clean_path_string(&candidate.entity_id);
            candidate.name = clean_path_string(&candidate.name);
            candidate
        })
        .collect()
}

/// Clean entity references by removing path prefixes.
pub fn clean_entity_refs(entities: &[RefactoringCandidate]) -> Vec<RefactoringCandidate> {
    entities
        .iter()
        .cloned()
        .map(|mut entity| {
            entity.entity_id = clean_path_string(&entity.entity_id);
            entity.name = clean_path_string(&entity.name);
            entity.file_path = clean_path_string(&entity.file_path);
            entity
        })
        .collect()
}

/// Clean path prefixes in file refactoring groups.
pub fn clean_path_prefixes_in_file_groups(
    file_groups: &[FileRefactoringGroup],
) -> Vec<FileRefactoringGroup> {
    file_groups
        .iter()
        .cloned()
        .map(|mut group| {
            group.file_path = clean_path_string(&group.file_path);
            group.entities = clean_entity_refs(&group.entities);
            group
        })
        .collect()
}

/// Helper to clean a PathBuf using the path cleaning logic.
fn clean_pathbuf(path: &PathBuf, root: Option<&str>) -> PathBuf {
    PathBuf::from(clean_path_with_root(&path.to_string_lossy(), root))
}

/// Clean path prefixes from directory health tree paths.
/// Handles both "./" prefixes and arbitrary absolute paths.
pub fn clean_directory_health_tree_paths(tree: &DirectoryHealthTree) -> DirectoryHealthTree {
    // First, detect the project root from all paths in the tree
    let all_paths: Vec<String> = tree
        .directories
        .keys()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let root = detect_project_root(all_paths.iter().map(|s| s.as_str()));
    let root_ref = root.as_deref();

    let mut cleaned_tree = tree.clone();

    // Clean root path
    cleaned_tree.root.path = clean_pathbuf(&cleaned_tree.root.path, root_ref);

    // Clean parent path in root
    if let Some(ref parent) = cleaned_tree.root.parent {
        cleaned_tree.root.parent = Some(clean_pathbuf(parent, root_ref));
    }

    // Clean children paths in root
    cleaned_tree.root.children = cleaned_tree
        .root
        .children
        .iter()
        .map(|child| clean_pathbuf(child, root_ref))
        .collect();

    // Clean all directory paths and their contents
    let mut cleaned_directories = HashMap::new();
    for (path, dir_health) in &cleaned_tree.directories {
        let mut cleaned_dir = dir_health.clone();

        // Clean the directory path key
        let cleaned_path = clean_pathbuf(path, root_ref);

        // Clean the path field in the DirectoryHealthScore
        cleaned_dir.path = clean_pathbuf(&cleaned_dir.path, root_ref);

        // Clean parent path
        if let Some(ref parent) = cleaned_dir.parent {
            cleaned_dir.parent = Some(clean_pathbuf(parent, root_ref));
        }

        // Clean children paths
        cleaned_dir.children = cleaned_dir
            .children
            .iter()
            .map(|child| clean_pathbuf(child, root_ref))
            .collect();

        cleaned_directories.insert(cleaned_path, cleaned_dir);
    }
    cleaned_tree.directories = cleaned_directories;

    // Clean hotspot directory paths in tree statistics
    cleaned_tree.tree_statistics.hotspot_directories = cleaned_tree
        .tree_statistics
        .hotspot_directories
        .iter()
        .map(|hotspot| {
            let mut cleaned_hotspot = hotspot.clone();
            cleaned_hotspot.path = clean_pathbuf(&cleaned_hotspot.path, root_ref);
            cleaned_hotspot
        })
        .collect();

    cleaned_tree
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_path_string_removes_dot_slash() {
        assert_eq!(clean_path_string("./src/main.rs"), "src/main.rs");
        assert_eq!(clean_path_string("src/main.rs"), "src/main.rs");
    }

    #[test]
    fn test_clean_path_string_handles_nested() {
        assert_eq!(clean_path_string("./src/lib/utils.rs"), "src/lib/utils.rs");
    }

    #[test]
    fn test_clean_path_string_handles_absolute_with_src() {
        assert_eq!(
            clean_path_string("/tmp/repos/abc123/src/main.rs"),
            "src/main.rs"
        );
        assert_eq!(
            clean_path_string("/home/user/project/src/lib/utils.rs"),
            "src/lib/utils.rs"
        );
    }

    #[test]
    fn test_clean_path_string_handles_absolute_with_tests() {
        assert_eq!(
            clean_path_string("/tmp/repos/xyz/tests/integration.rs"),
            "tests/integration.rs"
        );
    }

    #[test]
    fn test_clean_path_with_explicit_root() {
        assert_eq!(
            clean_path_with_root(
                "/tmp/repos/cfKs9DL7NKlyCMOSd1nzv/src/main.rs",
                Some("/tmp/repos/cfKs9DL7NKlyCMOSd1nzv")
            ),
            "src/main.rs"
        );
    }

    #[test]
    fn test_detect_project_root() {
        let paths = vec![
            "/tmp/repos/abc/src/main.rs",
            "/tmp/repos/abc/src/lib.rs",
            "/tmp/repos/abc/tests/test.rs",
        ];
        let root = detect_project_root(paths.iter().map(|s| *s));
        assert_eq!(root, Some("/tmp/repos/abc".to_string()));
    }

    #[test]
    fn test_detect_project_root_no_common() {
        let paths = vec!["/home/user/a.rs", "/var/log/b.rs"];
        let root = detect_project_root(paths.iter().map(|s| *s));
        assert_eq!(root, None);
    }

    #[test]
    fn test_detect_project_root_empty() {
        let paths: Vec<&str> = vec![];
        let root = detect_project_root(paths.iter().map(|s| *s));
        assert_eq!(root, None);
    }
}
