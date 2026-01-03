//! Hierarchy building utilities for report generation.
//!
//! This module provides functions for building unified directory hierarchies
//! that combine directory health data with refactoring candidates.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::core::pipeline::{
    CodeDictionary, DirectoryHealthScore, DirectoryHealthTree, FileRefactoringGroup,
    RefactoringCandidate,
};
use crate::core::scoring::Priority;

/// Build a unified hierarchy combining directory health with refactoring candidates
pub fn build_unified_hierarchy(
    tree: &DirectoryHealthTree,
    file_groups: &[FileRefactoringGroup],
) -> Vec<serde_json::Value> {
    // Map directories for lookup
    let mut dir_map: HashMap<String, &DirectoryHealthScore> = HashMap::new();
    for (path_buf, dir) in &tree.directories {
        dir_map.insert(path_buf.to_string_lossy().to_string(), dir);
    }

    // Group files by directory
    let mut files_by_dir: BTreeMap<String, Vec<&FileRefactoringGroup>> = BTreeMap::new();
    for group in file_groups {
        let dir = Path::new(&group.file_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        files_by_dir.entry(dir).or_default().push(group);
    }

    // Recursively build nodes
    fn build_dir_node(
        path: &str,
        dir: &DirectoryHealthScore,
        dir_map: &HashMap<String, &DirectoryHealthScore>,
        files_by_dir: &BTreeMap<String, Vec<&FileRefactoringGroup>>,
    ) -> serde_json::Value {
        let mut children = Vec::new();

        // Child directories
        for (child_path, child_dir) in dir_map.iter() {
            if let Some(parent) = &child_dir.parent {
                if parent.to_string_lossy() == path {
                    children.push(build_dir_node(child_path, child_dir, dir_map, files_by_dir));
                }
            }
        }

        // File children
        if let Some(files) = files_by_dir.get(path) {
            for file_group in files {
                let total_issues: usize =
                    file_group.entities.iter().map(|e| e.issues.len()).sum();
                let entity_count = file_group.entities.len().max(1);
                let file_health =
                    (1.0 - (total_issues as f64 / entity_count as f64)).clamp(0.0, 1.0);

                let entities: Vec<serde_json::Value> = file_group
                    .entities
                    .iter()
                    .map(|entity| {
                        let mut v = serde_json::to_value(entity).unwrap_or_default();
                        v["type"] = serde_json::Value::String("entity".to_string());
                        // Ensure a stable id for tree rendering
                        if v.get("id").is_none() {
                            let id = if !entity.entity_id.is_empty() {
                                entity.entity_id.clone()
                            } else {
                                entity.name.clone()
                            };
                            v["id"] = serde_json::Value::String(format!(
                                "entity_{}",
                                id.replace('/', "_").replace(':', "_")
                            ));
                        }
                        v
                    })
                    .collect();

                children.push(serde_json::json!({
                    "id": format!("file_{}", file_group.file_path.replace('/', "_")),
                    "type": "file",
                    "path": file_group.file_path,
                    "name": file_group.file_name,
                    "entity_count": file_group.entity_count,
                    "avg_score": ((file_group.avg_score * 10.0).round() / 10.0),
                    "priority": file_group.highest_priority,
                    "health_score": file_health,
                    "total_issues": total_issues,
                    "children": entities
                }));
            }
        }

        children.sort_by(|a, b| {
            let name_a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let name_b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            name_a.cmp(name_b)
        });

        let display_name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());

        serde_json::json!({
            "id": format!("directory_{}", path.replace('/', "_")),
                    "type": "folder",
            "path": path,
            "name": display_name,
            "health_score": dir.health_score,
            "entity_count": dir.entity_count,
            "file_count": dir.file_count,
            "refactoring_needed": dir.refactoring_needed,
            "children": children
        })
    }

    let mut roots = Vec::new();
    for (path, dir) in dir_map.iter() {
        let is_root = dir
            .parent
            .as_ref()
            .map(|p| !dir_map.contains_key(&p.to_string_lossy().to_string()))
            .unwrap_or(true);
        if is_root {
            roots.push(build_dir_node(path, dir, &dir_map, &files_by_dir));
        }
    }

    if roots.is_empty() {
        for (path, dir) in dir_map.iter() {
            roots.push(build_dir_node(path, dir, &dir_map, &files_by_dir));
        }
    }

    roots
}

/// Create real file groups from individual refactoring candidates
pub fn create_file_groups_from_candidates(
    candidates: &[RefactoringCandidate],
) -> Vec<FileRefactoringGroup> {
    let mut file_map: HashMap<String, Vec<&RefactoringCandidate>> = HashMap::new();

    // Group candidates by file path
    for candidate in candidates {
        file_map
            .entry(candidate.file_path.clone())
            .or_insert_with(Vec::new)
            .push(candidate);
    }

    // Convert to FileRefactoringGroup format
    file_map
        .into_iter()
        .map(|(file_path, candidates)| {
            let file_name = std::path::Path::new(&file_path)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("unknown"))
                .to_string_lossy()
                .to_string();

            let entity_count = candidates.len();
            let avg_score = if entity_count > 0 {
                candidates.iter().map(|c| c.score).sum::<f64>() / entity_count as f64
            } else {
                0.0
            };

            let highest_priority = candidates
                .iter()
                .map(|c| &c.priority)
                .max()
                .cloned()
                .unwrap_or(Priority::Low);

            let total_issues = candidates.iter().map(|c| c.issue_count).sum::<usize>();

            // Use the candidates directly as entities
            let entities: Vec<RefactoringCandidate> = candidates.into_iter().cloned().collect();

            FileRefactoringGroup {
                file_path,
                file_name,
                entity_count,
                entities,
                avg_score,
                highest_priority,
                total_issues,
            }
        })
        .collect()
}

/// Build a lookup map from entity ID to refactoring candidate
pub fn build_candidate_lookup(
    candidates: &[RefactoringCandidate],
) -> HashMap<String, RefactoringCandidate> {
    let mut map = HashMap::with_capacity(candidates.len());
    for candidate in candidates {
        map.insert(candidate.entity_id.clone(), candidate.clone());
    }
    map
}

/// Merge file data into the hierarchical directory structure
pub fn add_files_to_hierarchy(
    hierarchy: &[serde_json::Value],
    file_groups: &[FileRefactoringGroup],
    code_dictionary: &CodeDictionary,
    candidate_lookup: &HashMap<String, RefactoringCandidate>,
) -> Vec<serde_json::Value> {
    // Build a map of directory path -> file groups for quick lookup
    let mut files_by_dir: HashMap<String, Vec<&FileRefactoringGroup>> = HashMap::new();

    for file_group in file_groups {
        let file_path = Path::new(&file_group.file_path);
        let dir_path = if let Some(parent) = file_path.parent() {
            parent.to_string_lossy().to_string()
        } else {
            ".".to_string()
        };

        files_by_dir
            .entry(dir_path)
            .or_insert_with(Vec::new)
            .push(file_group);
    }

    // Recursively add files to hierarchy nodes
    hierarchy
        .iter()
        .map(|node| add_files_to_node(node, &files_by_dir, code_dictionary, candidate_lookup))
        .collect()
}

/// Recursively add files to a single hierarchy node
fn add_files_to_node(
    node: &serde_json::Value,
    files_by_dir: &HashMap<String, Vec<&FileRefactoringGroup>>,
    code_dictionary: &CodeDictionary,
    candidate_lookup: &HashMap<String, RefactoringCandidate>,
) -> serde_json::Value {
    let mut new_node = node.clone();

    // Get the path from the node
    let node_path = if let Some(path) = node.get("path").and_then(|p| p.as_str()) {
        path.to_string()
    } else if let Some(id) = node.get("id").and_then(|id| id.as_str()) {
        // Extract path from ID like "directory_src_detectors" -> "src/detectors"
        if id.starts_with("directory_") {
            id.strip_prefix("directory_")
                .unwrap_or(id)
                .replace("_", "/")
                .replace("root", ".")
        } else {
            ".".to_string()
        }
    } else {
        ".".to_string()
    };

    // Get existing children or create empty array
    let existing_children = node
        .get("children")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();

    // Recursively process existing children (directories)
    let mut new_children: Vec<serde_json::Value> = existing_children
        .iter()
        .map(|child| add_files_to_node(child, files_by_dir, code_dictionary, candidate_lookup))
        .collect();

    // Add files that belong to this directory
    if let Some(file_groups) = files_by_dir.get(&node_path) {
        for file_group in file_groups {
            let file_name = Path::new(&file_group.file_path)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("unknown"))
                .to_string_lossy()
                .to_string();

            // Create file node with entity children
            let mut file_children = Vec::new();

            for entity in &file_group.entities {
                // Extract entity name for better readability
                let display_name = entity
                    .name
                    .split(':')
                    .last()
                    .map(|part| part.to_string())
                    .unwrap_or_else(|| entity.name.clone());

                // Create children for issues and suggestions
                let mut entity_children = Vec::new();

                if let Some(candidate) = candidate_lookup.get(&entity.entity_id) {
                    for (i, issue) in candidate.issues.iter().enumerate() {
                        let issue_meta = code_dictionary.issues.get(&issue.code);
                        let issue_title = issue_meta
                            .map(|def| def.title.clone())
                            .unwrap_or_else(|| issue.category.clone());
                        let issue_summary = issue_meta
                            .map(|def| def.summary.clone())
                            .unwrap_or_else(|| {
                                format!("{} signals detected by analyzer.", issue.category)
                            });
                        let severity = (issue.severity * 10.0).round() / 10.0;

                        entity_children.push(serde_json::json!({
                            "id": format!("{}:issue:{}", entity.entity_id, i),
                            "type": "issue",
                            "code": issue.code,
                            "name": format!("{} – {}", issue.code, issue_title),
                            "title": issue_title,
                            "category": issue.category,
                            "summary": issue_summary,
                            "severity": severity,
                            "contributing_features": issue.contributing_features,
                            "children": []
                        }));
                    }

                    for (i, suggestion) in candidate.suggestions.iter().enumerate() {
                        let suggestion_meta = code_dictionary.suggestions.get(&suggestion.code);
                        let suggestion_title = suggestion_meta
                            .map(|def| def.title.clone())
                            .unwrap_or_else(|| suggestion.refactoring_type.clone());
                        let suggestion_summary = suggestion_meta
                            .map(|def| def.summary.clone())
                            .unwrap_or_else(|| suggestion.refactoring_type.replace('_', " "));

                        entity_children.push(serde_json::json!({
                            "id": format!("{}:suggestion:{}", entity.entity_id, i),
                            "type": "suggestion",
                            "code": suggestion.code,
                            "name": format!("{} – {}", suggestion.code, suggestion_title),
                            "title": suggestion_title,
                            "summary": suggestion_summary,
                            "priority": ((suggestion.priority * 10.0).round() / 10.0),
                            "effort": ((suggestion.effort * 10.0).round() / 10.0),
                            "impact": ((suggestion.impact * 10.0).round() / 10.0),
                            "refactoring_type": suggestion.refactoring_type.clone(),
                            "children": []
                        }));
                    }
                }

                let entity_node = serde_json::json!({
                    "id": entity.entity_id.clone(),
                    "type": "entity",
                    "name": display_name,
                    "score": ((entity.score * 10.0).round() / 10.0),
                    "priority": format!("{:?}", entity.priority),
                    "issue_count": entity.issue_count,
                    "suggestion_count": entity.suggestion_count,
                    "children": entity_children
                });
                file_children.push(entity_node);
            }

            let file_node = serde_json::json!({
                "id": format!("file_{}", file_group.file_path.replace("/", "_").replace(".", "root")),
                "type": "file",
                "name": file_name,
                "path": file_group.file_path,
                "entity_count": file_group.entity_count,
                "avg_score": ((file_group.avg_score * 10.0).round() / 10.0),
                "highest_priority": format!("{:?}", file_group.highest_priority),
                "total_issues": file_group.total_issues,
                "children": file_children
            });

            new_children.push(file_node);
        }
    }

    // Update the node with new children
    if let serde_json::Value::Object(ref mut obj) = new_node {
        obj.insert(
            "children".to_string(),
            serde_json::Value::Array(new_children),
        );
    }

    new_node
}
