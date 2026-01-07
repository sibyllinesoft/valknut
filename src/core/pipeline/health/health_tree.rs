//! Directory health tree implementation.
//!
//! This module contains the implementation logic for `DirectoryHealthTree`,
//! separating the tree-building and query methods from the type definitions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::scoring::Priority;

use crate::core::pipeline::results::result_types::{
    DirectoryHealthScore, DirectoryHealthTree, RefactoringCandidate, TreeStatistics,
};

/// Tree manipulation and overlay methods for [`DirectoryHealthTree`].
impl DirectoryHealthTree {
    /// Overlay documentation health/issue data onto the tree
    pub fn apply_doc_overlays(
        &mut self,
        doc_scores: &HashMap<String, f64>,
        doc_issues: &HashMap<String, usize>,
    ) {
        for (path, score) in doc_scores {
            let key = PathBuf::from(path);
            if let Some(dir) = self.directories.get_mut(&key) {
                dir.doc_health_score = (*score / 100.0).clamp(0.0, 1.0);
            } else if key == self.root.path {
                self.root.doc_health_score = (*score / 100.0).clamp(0.0, 1.0);
            }
        }

        for (path, issues) in doc_issues {
            let key = PathBuf::from(path);
            if let Some(dir) = self.directories.get_mut(&key) {
                dir.doc_issue_count = *issues;
            } else if key == self.root.path {
                self.root.doc_issue_count = *issues;
            }
        }
    }

    /// Create a minimal directory health tree from refactoring candidates.
    pub fn from_candidates(candidates: &[RefactoringCandidate]) -> Self {
        let file_count = candidates.len();
        let entity_count = candidates.len();
        let refactoring_needed = candidates.len();
        let avg_score = if candidates.is_empty() {
            0.0
        } else {
            candidates.iter().map(|c| c.score).sum::<f64>() / candidates.len() as f64
        };

        let mut root = DirectoryHealthScore {
            path: PathBuf::from("."),
            health_score: if entity_count > 0 {
                // Convert refactoring score (0-100, higher=worse) to health (0-1, higher=better)
                // Score of 0 = 100% health, score of 50 = 50% health, score of 100 = 0% health
                ((100.0 - avg_score) / 100.0).clamp(0.0, 1.0)
            } else {
                1.0
            },
            file_count,
            entity_count,
            refactoring_needed,
            critical_issues: candidates
                .iter()
                .filter(|c| matches!(c.priority, Priority::Critical))
                .count(),
            high_priority_issues: candidates
                .iter()
                .filter(|c| matches!(c.priority, Priority::High | Priority::Critical))
                .count(),
            avg_refactoring_score: avg_score,
            weight: (entity_count as f64).max(1.0),
            children: Vec::new(),
            parent: None,
            issue_categories: HashMap::new(),
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };
        // Build per-directory aggregates
        let mut directories: HashMap<PathBuf, DirectoryHealthScore> = HashMap::new();

        for candidate in candidates {
            let path = PathBuf::from(&candidate.file_path);
            let dir_path = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            let entry = directories
                .entry(dir_path.clone())
                .or_insert(DirectoryHealthScore {
                    path: dir_path.clone(),
                    health_score: 1.0,
                    file_count: 0,
                    entity_count: 0,
                    refactoring_needed: 0,
                    critical_issues: 0,
                    high_priority_issues: 0,
                    avg_refactoring_score: 0.0,
                    weight: 0.0,
                    children: Vec::new(),
                    parent: None,
                    issue_categories: HashMap::new(),
                    doc_health_score: 1.0,
                    doc_issue_count: 0,
                });

            entry.file_count += 1;
            entry.entity_count += 1;
            entry.refactoring_needed += 1;
            entry.critical_issues += usize::from(matches!(candidate.priority, Priority::Critical));
            entry.high_priority_issues += usize::from(matches!(
                candidate.priority,
                Priority::High | Priority::Critical
            ));
            entry.avg_refactoring_score += candidate.score;
            entry.weight += 1.0;
        }

        // Finalize averages and parent/child links
        for entry in directories.values_mut() {
            if entry.entity_count > 0 {
                entry.avg_refactoring_score /= entry.entity_count as f64;
                // Convert refactoring score (0-100, higher=worse) to health (0-1, higher=better)
                entry.health_score =
                    ((100.0 - entry.avg_refactoring_score) / 100.0).clamp(0.0, 1.0);
            } else {
                entry.health_score = 1.0;
            }

            let parent_path = entry
                .path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(PathBuf::from("."));
            entry.parent = Some(parent_path.clone());
        }

        // Populate children vectors
        let keys: Vec<PathBuf> = directories.keys().cloned().collect();
        for dir_path in keys {
            let parent_path = directories
                .get(&dir_path)
                .and_then(|d| d.parent.clone())
                .unwrap_or(PathBuf::from("."));
            if let Some(parent_dir) = directories.get_mut(&parent_path) {
                parent_dir.children.push(dir_path.clone());
            } else if parent_path == PathBuf::from(".") {
                // Attach to root
                root.children.push(dir_path.clone());
            }
        }

        let total_dirs = directories.len() + 1;

        DirectoryHealthTree {
            root: root.clone(),
            directories,
            tree_statistics: TreeStatistics {
                total_directories: total_dirs,
                max_depth: 2,
                avg_health_score: root.health_score,
                health_score_std_dev: 0.0,
                hotspot_directories: Vec::new(),
                health_by_depth: HashMap::new(),
            },
        }
    }

    /// Get the health score for a directory path, defaulting to root.
    pub fn get_health_score(&self, path: &Path) -> f64 {
        if let Some(dir) = self.directories.get(path) {
            dir.health_score
        } else if path == self.root.path {
            self.root.health_score
        } else {
            self.root.health_score
        }
    }

    /// Get all children directories for a given path (empty in minimal tree).
    pub fn get_children(&self, path: &Path) -> Vec<&DirectoryHealthScore> {
        let mut children = Vec::new();

        // Match root
        let path_buf = path.to_path_buf();
        for dir in self.directories.values() {
            if let Some(parent) = &dir.parent {
                if *parent == path_buf {
                    children.push(dir);
                }
            }
        }

        // If asking for root and no directory entries, return empty
        children
    }

    /// Generate a simple tree representation as text.
    pub fn to_tree_string(&self) -> String {
        let mut dirs: Vec<String> = self
            .directories
            .keys()
            .map(|p| p.display().to_string())
            .collect();
        dirs.sort();
        format!(
            "root: {} (health: {:.1}%) dirs: {:?}",
            self.root.path.display(),
            self.root.health_score * 100.0,
            dirs
        )
    }
}
