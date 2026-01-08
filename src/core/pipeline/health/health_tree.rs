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

    /// Overlay precomputed health scores onto the tree.
    /// This uses health values computed from ALL scoring results (not just refactoring candidates)
    /// to ensure consistency with overall project health.
    /// Health scores are on 0-100 scale (same as overall project health).
    pub fn apply_health_overlays(&mut self, health_scores: &HashMap<String, f64>) {
        for (path, score) in health_scores {
            let key = PathBuf::from(path);
            // Convert from 0-100 scale to 0-1 scale for internal storage
            let health = (*score / 100.0).clamp(0.0, 1.0);
            if let Some(dir) = self.directories.get_mut(&key) {
                dir.health_score = health;
            } else if *path == self.root.path.to_string_lossy() {
                self.root.health_score = health;
            }
        }

        // Update tree statistics with the new average health
        if !self.directories.is_empty() {
            let total_health: f64 = self.directories.values().map(|d| d.health_score).sum();
            let count = self.directories.len() as f64;
            self.tree_statistics.avg_health_score =
                (self.root.health_score + total_health) / (count + 1.0);
        }
    }

    /// Create a minimal directory health tree from refactoring candidates.
    pub fn from_candidates(candidates: &[RefactoringCandidate]) -> Self {
        let avg_score = Self::calculate_average_score(candidates);
        let mut root = Self::build_root_from_candidates(candidates, avg_score);
        let mut directories = Self::build_directories_from_candidates(candidates);

        Self::finalize_candidate_directories(&mut directories);
        Self::link_children(&mut directories, &mut root);

        DirectoryHealthTree {
            root: root.clone(),
            tree_statistics: TreeStatistics {
                total_directories: directories.len() + 1,
                max_depth: 2,
                avg_health_score: root.health_score,
                health_score_std_dev: 0.0,
                hotspot_directories: Vec::new(),
                health_by_depth: HashMap::new(),
            },
            directories,
        }
    }

    fn calculate_average_score(candidates: &[RefactoringCandidate]) -> f64 {
        if candidates.is_empty() {
            0.0
        } else {
            candidates.iter().map(|c| c.score).sum::<f64>() / candidates.len() as f64
        }
    }

    fn build_root_from_candidates(
        candidates: &[RefactoringCandidate],
        avg_score: f64,
    ) -> DirectoryHealthScore {
        let count = candidates.len();
        let mut root = Self::new_directory_score(PathBuf::from("."), None);
        root.file_count = count;
        root.entity_count = count;
        root.refactoring_needed = count;
        root.health_score = if count > 0 {
            ((100.0 - avg_score) / 100.0).clamp(0.0, 1.0)
        } else {
            1.0
        };
        root.critical_issues = candidates
            .iter()
            .filter(|c| matches!(c.priority, Priority::Critical))
            .count();
        root.high_priority_issues = candidates
            .iter()
            .filter(|c| matches!(c.priority, Priority::High | Priority::Critical))
            .count();
        root.avg_refactoring_score = avg_score;
        root.weight = (count as f64).max(1.0);
        root
    }

    fn build_directories_from_candidates(
        candidates: &[RefactoringCandidate],
    ) -> HashMap<PathBuf, DirectoryHealthScore> {
        let mut directories = HashMap::new();

        for candidate in candidates {
            let path = PathBuf::from(&candidate.file_path);
            let dir_path = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            let entry = directories
                .entry(dir_path.clone())
                .or_insert_with(|| Self::new_directory_score(dir_path, None));

            entry.file_count += 1;
            entry.entity_count += 1;
            entry.refactoring_needed += 1;
            entry.critical_issues += usize::from(matches!(candidate.priority, Priority::Critical));
            entry.high_priority_issues +=
                usize::from(matches!(candidate.priority, Priority::High | Priority::Critical));
            entry.avg_refactoring_score += candidate.score;
            entry.weight += 1.0;
        }

        directories
    }

    fn finalize_candidate_directories(directories: &mut HashMap<PathBuf, DirectoryHealthScore>) {
        for entry in directories.values_mut() {
            if entry.entity_count > 0 {
                entry.avg_refactoring_score /= entry.entity_count as f64;
                entry.health_score =
                    ((100.0 - entry.avg_refactoring_score) / 100.0).clamp(0.0, 1.0);
            }
            entry.parent = Some(
                entry
                    .path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from(".")),
            );
        }
    }

    /// Create a directory health tree from file health scores.
    /// This builds a complete tree structure from all analyzed files.
    pub fn from_file_health(file_health: &HashMap<String, f64>) -> Self {
        if file_health.is_empty() {
            return Self::empty();
        }

        let mut directories: HashMap<PathBuf, DirectoryHealthScore> = HashMap::new();
        let mut root = Self::new_directory_score(PathBuf::from("."), None);

        Self::build_leaf_directories(file_health, &mut directories);
        Self::finalize_directory_averages(&mut directories);
        Self::build_intermediate_directories(&mut directories);
        Self::link_children(&mut directories, &mut root);
        Self::calculate_root_stats(&directories, &mut root);

        DirectoryHealthTree {
            root: root.clone(),
            tree_statistics: TreeStatistics {
                total_directories: directories.len() + 1,
                max_depth: 5,
                avg_health_score: root.health_score,
                health_score_std_dev: 0.0,
                hotspot_directories: Vec::new(),
                health_by_depth: HashMap::new(),
            },
            directories,
        }
    }

    /// Create a new directory score with default values.
    fn new_directory_score(path: PathBuf, parent: Option<PathBuf>) -> DirectoryHealthScore {
        DirectoryHealthScore {
            path,
            health_score: 1.0,
            file_count: 0,
            entity_count: 0,
            refactoring_needed: 0,
            critical_issues: 0,
            high_priority_issues: 0,
            avg_refactoring_score: 0.0,
            weight: 0.0,
            children: Vec::new(),
            parent,
            issue_categories: HashMap::new(),
            doc_health_score: 1.0,
            doc_issue_count: 0,
        }
    }

    /// Build initial directory entries from file health scores.
    fn build_leaf_directories(
        file_health: &HashMap<String, f64>,
        directories: &mut HashMap<PathBuf, DirectoryHealthScore>,
    ) {
        for (file_path, health_score) in file_health {
            let path = PathBuf::from(file_path);
            let dir_path = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            let health = (*health_score / 100.0).clamp(0.0, 1.0);

            let entry = directories
                .entry(dir_path.clone())
                .or_insert_with(|| Self::new_directory_score(dir_path, None));

            entry.file_count += 1;
            entry.entity_count += 1;
            entry.health_score += health;
            entry.weight += 1.0;
            if health < 0.8 {
                entry.refactoring_needed += 1;
            }
        }
    }

    /// Finalize averages and set parent paths.
    fn finalize_directory_averages(directories: &mut HashMap<PathBuf, DirectoryHealthScore>) {
        for entry in directories.values_mut() {
            if entry.file_count > 0 {
                entry.health_score /= entry.file_count as f64;
            }
            entry.parent = Some(
                entry
                    .path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from(".")),
            );
        }
    }

    /// Build intermediate directories to create full tree.
    fn build_intermediate_directories(directories: &mut HashMap<PathBuf, DirectoryHealthScore>) {
        let leaf_dirs: Vec<PathBuf> = directories.keys().cloned().collect();
        for dir_path in leaf_dirs {
            let mut current = dir_path.parent().map(|p| p.to_path_buf());
            while let Some(parent_path) = current {
                if parent_path.as_os_str().is_empty() || parent_path == PathBuf::from(".") {
                    break;
                }
                if !directories.contains_key(&parent_path) {
                    let parent = parent_path.parent().map(|p| p.to_path_buf());
                    directories.insert(
                        parent_path.clone(),
                        Self::new_directory_score(parent_path.clone(), parent),
                    );
                }
                current = parent_path.parent().map(|p| p.to_path_buf());
            }
        }
    }

    /// Link children to their parent directories.
    fn link_children(
        directories: &mut HashMap<PathBuf, DirectoryHealthScore>,
        root: &mut DirectoryHealthScore,
    ) {
        let keys: Vec<PathBuf> = directories.keys().cloned().collect();
        for dir_path in &keys {
            let parent_path = directories
                .get(dir_path)
                .and_then(|d| d.parent.clone())
                .unwrap_or_else(|| PathBuf::from("."));

            if let Some(parent_dir) = directories.get_mut(&parent_path) {
                parent_dir.children.push(dir_path.clone());
            } else if parent_path == PathBuf::from(".") || parent_path.as_os_str().is_empty() {
                root.children.push(dir_path.clone());
            }
        }
    }

    /// Calculate root directory statistics.
    fn calculate_root_stats(
        directories: &HashMap<PathBuf, DirectoryHealthScore>,
        root: &mut DirectoryHealthScore,
    ) {
        root.file_count = directories.values().map(|d| d.file_count).sum();
        root.entity_count = root.file_count;
        root.refactoring_needed = directories.values().map(|d| d.refactoring_needed).sum();

        if !directories.is_empty() {
            let total_health: f64 = directories
                .values()
                .map(|d| d.health_score * d.file_count as f64)
                .sum();
            let total_files: f64 = directories.values().map(|d| d.file_count as f64).sum();
            root.health_score = if total_files > 0.0 {
                total_health / total_files
            } else {
                1.0
            };
        }
    }

    /// Create an empty tree
    fn empty() -> Self {
        DirectoryHealthTree {
            root: Self::new_directory_score(PathBuf::from("."), None),
            directories: HashMap::new(),
            tree_statistics: TreeStatistics {
                total_directories: 1,
                max_depth: 0,
                avg_health_score: 1.0,
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
