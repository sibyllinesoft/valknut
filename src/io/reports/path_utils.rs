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
    // First handle absolute paths by converting to relative
    if let Ok(current_dir) = std::env::current_dir() {
        let current_dir_str = current_dir.to_string_lossy();
        if path.starts_with(&current_dir_str.as_ref()) {
            let relative = &path[current_dir_str.len()..];
            let cleaned = relative.strip_prefix('/').unwrap_or(relative);
            return cleaned.to_string();
        }
    }

    // Then handle "./" prefixes
    if path.starts_with("./") {
        path[2..].to_string()
    } else {
        path.to_string()
    }
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

/// Clean "./" prefixes from directory health tree paths.
pub fn clean_directory_health_tree_paths(tree: &DirectoryHealthTree) -> DirectoryHealthTree {
    let mut cleaned_tree = tree.clone();

    // Clean root path
    if cleaned_tree.root.path.to_string_lossy().starts_with("./") {
        cleaned_tree.root.path = PathBuf::from(&cleaned_tree.root.path.to_string_lossy()[2..]);
    }

    // Clean parent path in root
    if let Some(ref parent) = cleaned_tree.root.parent {
        if parent.to_string_lossy().starts_with("./") {
            cleaned_tree.root.parent = Some(PathBuf::from(&parent.to_string_lossy()[2..]));
        }
    }

    // Clean children paths in root
    cleaned_tree.root.children = cleaned_tree
        .root
        .children
        .iter()
        .map(|child| {
            if child.to_string_lossy().starts_with("./") {
                PathBuf::from(&child.to_string_lossy()[2..])
            } else {
                child.clone()
            }
        })
        .collect();

    // Clean all directory paths and their contents
    let mut cleaned_directories = HashMap::new();
    for (path, dir_health) in &cleaned_tree.directories {
        let mut cleaned_dir = dir_health.clone();

        // Clean the directory path key
        let cleaned_path = if path.to_string_lossy().starts_with("./") {
            PathBuf::from(&path.to_string_lossy()[2..])
        } else {
            path.clone()
        };

        // Clean the path field in the DirectoryHealthScore
        if cleaned_dir.path.to_string_lossy().starts_with("./") {
            cleaned_dir.path = PathBuf::from(&cleaned_dir.path.to_string_lossy()[2..]);
        }

        // Clean parent path
        if let Some(ref parent) = cleaned_dir.parent {
            if parent.to_string_lossy().starts_with("./") {
                cleaned_dir.parent = Some(PathBuf::from(&parent.to_string_lossy()[2..]));
            }
        }

        // Clean children paths
        cleaned_dir.children = cleaned_dir
            .children
            .iter()
            .map(|child| {
                if child.to_string_lossy().starts_with("./") {
                    PathBuf::from(&child.to_string_lossy()[2..])
                } else {
                    child.clone()
                }
            })
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
            if cleaned_hotspot.path.to_string_lossy().starts_with("./") {
                cleaned_hotspot.path =
                    PathBuf::from(&cleaned_hotspot.path.to_string_lossy()[2..]);
            }
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
}
