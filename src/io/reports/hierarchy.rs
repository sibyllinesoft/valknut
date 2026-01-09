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
use crate::core::pipeline::health::normalize_dir_path;
use crate::core::scoring::Priority;

/// Build a unified hierarchy combining directory health with refactoring candidates
pub fn build_unified_hierarchy(
    tree: &DirectoryHealthTree,
    file_groups: &[FileRefactoringGroup],
) -> Vec<serde_json::Value> {
    build_unified_hierarchy_with_health(tree, file_groups, &std::collections::HashMap::new(), &std::collections::HashMap::new())
}

/// Build a unified hierarchy with precomputed file and directory health scores
pub fn build_unified_hierarchy_with_health(
    tree: &DirectoryHealthTree,
    file_groups: &[FileRefactoringGroup],
    file_health: &std::collections::HashMap<String, f64>,
    directory_health: &std::collections::HashMap<String, f64>,
) -> Vec<serde_json::Value> {
    let dir_map = build_directory_map(tree);
    let files_by_dir = group_files_by_directory(file_groups);

    let mut roots = Vec::new();
    for (path, dir) in dir_map.iter() {
        if is_root_directory(dir, &dir_map) {
            roots.push(build_dir_node(path, dir, &dir_map, &files_by_dir, file_health, directory_health));
        }
    }

    if roots.is_empty() {
        for (path, dir) in dir_map.iter() {
            roots.push(build_dir_node(path, dir, &dir_map, &files_by_dir, file_health, directory_health));
        }
    }

    roots
}

/// Build a lookup map of directory paths to health scores.
/// Paths are normalized to ensure consistent matching.
fn build_directory_map(tree: &DirectoryHealthTree) -> HashMap<String, &DirectoryHealthScore> {
    tree.directories.iter()
        .map(|(path_buf, dir)| (normalize_dir_path(&path_buf.to_string_lossy()), dir))
        .collect()
}

/// Group file refactoring groups by their parent directory.
fn group_files_by_directory(file_groups: &[FileRefactoringGroup]) -> BTreeMap<String, Vec<&FileRefactoringGroup>> {
    let mut files_by_dir: BTreeMap<String, Vec<&FileRefactoringGroup>> = BTreeMap::new();
    for group in file_groups {
        let dir = Path::new(&group.file_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        files_by_dir.entry(dir).or_default().push(group);
    }
    files_by_dir
}

/// Check if a directory is a root (no parent in the map).
fn is_root_directory(dir: &DirectoryHealthScore, dir_map: &HashMap<String, &DirectoryHealthScore>) -> bool {
    dir.parent.as_ref()
        .map(|p| !dir_map.contains_key(&normalize_dir_path(&p.to_string_lossy())))
        .unwrap_or(true)
}

/// Recursively build a directory node with children.
fn build_dir_node(
    path: &str,
    dir: &DirectoryHealthScore,
    dir_map: &HashMap<String, &DirectoryHealthScore>,
    files_by_dir: &BTreeMap<String, Vec<&FileRefactoringGroup>>,
    file_health: &std::collections::HashMap<String, f64>,
    directory_health: &std::collections::HashMap<String, f64>,
) -> serde_json::Value {
    let mut children = collect_child_directories(path, dir_map, files_by_dir, file_health, directory_health);
    children.extend(collect_file_children(path, files_by_dir, file_health));
    children.sort_by(|a, b| {
        let name_a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let name_b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        name_a.cmp(name_b)
    });

    let display_name = Path::new(path).file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    // Use precomputed directory health if available (0-100 scale)
    // For intermediate directories not in the HashMap, compute from children
    let normalized_path = normalize_dir_path(path);
    let health_score_01 = directory_health.get(&normalized_path)
        .map(|h| h / 100.0)
        .or_else(|| {
            // Compute from children's health scores if this is an intermediate directory
            let child_scores: Vec<f64> = children.iter()
                .filter_map(|c| c.get("healthScore").and_then(|v| v.as_f64()))
                .filter(|&s| s > 0.0)
                .collect();
            if !child_scores.is_empty() {
                Some(child_scores.iter().sum::<f64>() / child_scores.len() as f64)
            } else {
                None
            }
        })
        .unwrap_or(0.0);
    let health_score_100 = health_score_01 * 100.0;

    serde_json::json!({
        "id": format!("directory_{}", path.replace('/', "_")),
        "type": "folder",
        "path": path,
        "name": display_name,
        "health_score": health_score_100,
        "healthScore": health_score_01,
        "entity_count": dir.entity_count,
        "file_count": dir.file_count,
        "refactoring_needed": dir.refactoring_needed,
        "children": children
    })
}

/// Collect child directory nodes for a given path.
fn collect_child_directories(
    path: &str,
    dir_map: &HashMap<String, &DirectoryHealthScore>,
    files_by_dir: &BTreeMap<String, Vec<&FileRefactoringGroup>>,
    file_health: &std::collections::HashMap<String, f64>,
    directory_health: &std::collections::HashMap<String, f64>,
) -> Vec<serde_json::Value> {
    let normalized_path = normalize_dir_path(path);
    dir_map.iter()
        .filter(|(_, child_dir)| {
            child_dir.parent.as_ref()
                .map(|p| normalize_dir_path(&p.to_string_lossy()) == normalized_path)
                .unwrap_or(false)
        })
        .map(|(child_path, child_dir)| build_dir_node(child_path, child_dir, dir_map, files_by_dir, file_health, directory_health))
        .collect()
}

/// Collect file nodes for a given directory path.
fn collect_file_children(
    path: &str,
    files_by_dir: &BTreeMap<String, Vec<&FileRefactoringGroup>>,
    file_health: &std::collections::HashMap<String, f64>,
) -> Vec<serde_json::Value> {
    files_by_dir.get(path)
        .map(|files| files.iter().map(|fg| build_unified_file_node(fg, file_health)).collect())
        .unwrap_or_default()
}

/// Build a file node for the unified hierarchy.
fn build_unified_file_node(
    file_group: &FileRefactoringGroup,
    file_health_map: &std::collections::HashMap<String, f64>,
) -> serde_json::Value {
    let total_issues: usize = file_group.entities.iter().map(|e| e.issues.len()).sum();

    // Use precomputed file health if available (same formula as project health),
    // otherwise fall back to issue-count based calculation
    let file_health = file_health_map
        .get(&file_group.file_path)
        .map(|h| *h / 100.0) // Convert from 0-100 to 0-1 scale
        .unwrap_or_else(|| {
            let entity_count = file_group.entities.len().max(1);
            (1.0 - (total_issues as f64 / entity_count as f64)).clamp(0.0, 1.0)
        });

    let entities: Vec<serde_json::Value> = file_group.entities.iter()
        .map(build_unified_entity_value)
        .collect();

    serde_json::json!({
        "id": format!("file_{}", file_group.file_path.replace('/', "_")),
        "type": "file",
        "path": file_group.file_path,
        "file_path": file_group.file_path,  // Also add file_path for entity health lookup
        "name": file_group.file_name,
        "entity_count": file_group.entity_count,
        "avg_score": ((file_group.avg_score * 10.0).round() / 10.0),
        "priority": file_group.highest_priority,
        "health_score": file_health,
        "healthScore": file_health,  // React expects camelCase (already 0-1 scale)
        "total_issues": total_issues,
        "children": entities
    })
}

/// Build an entity value for the unified hierarchy.
fn build_unified_entity_value(entity: &RefactoringCandidate) -> serde_json::Value {
    let mut v = serde_json::to_value(entity).unwrap_or_default();
    v["type"] = serde_json::Value::String("entity".to_string());

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
}

/// Create minimal file groups from file_health map (for files without refactoring candidates)
pub fn create_file_groups_from_health(
    file_health: &std::collections::HashMap<String, f64>,
) -> Vec<FileRefactoringGroup> {
    file_health
        .keys()
        .map(|file_path| {
            let file_name = std::path::Path::new(file_path)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("unknown"))
                .to_string_lossy()
                .to_string();

            FileRefactoringGroup {
                file_path: file_path.clone(),
                file_name,
                entity_count: 0,
                entities: Vec::new(),
                avg_score: 0.0,
                highest_priority: Priority::None,
                total_issues: 0,
            }
        })
        .collect()
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
    let node_path = extract_node_path(node);

    let existing_children = node
        .get("children")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();

    let mut new_children: Vec<serde_json::Value> = existing_children
        .iter()
        .map(|child| add_files_to_node(child, files_by_dir, code_dictionary, candidate_lookup))
        .collect();

    if let Some(file_groups) = files_by_dir.get(&node_path) {
        for file_group in file_groups {
            new_children.push(build_file_node(file_group, code_dictionary, candidate_lookup));
        }
    }

    if let serde_json::Value::Object(ref mut obj) = new_node {
        obj.insert("children".to_string(), serde_json::Value::Array(new_children));
    }

    new_node
}

/// Extract directory path from a hierarchy node.
fn extract_node_path(node: &serde_json::Value) -> String {
    if let Some(path) = node.get("path").and_then(|p| p.as_str()) {
        return path.to_string();
    }
    if let Some(id) = node.get("id").and_then(|id| id.as_str()) {
        if id.starts_with("directory_") {
            return id.strip_prefix("directory_")
                .unwrap_or(id)
                .replace("_", "/")
                .replace("root", ".");
        }
    }
    ".".to_string()
}

/// Build a file node with entity children.
fn build_file_node(
    file_group: &FileRefactoringGroup,
    code_dictionary: &CodeDictionary,
    candidate_lookup: &HashMap<String, RefactoringCandidate>,
) -> serde_json::Value {
    let file_name = Path::new(&file_group.file_path)
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("unknown"))
        .to_string_lossy()
        .to_string();

    let file_children: Vec<serde_json::Value> = file_group.entities
        .iter()
        .map(|entity| build_entity_node(entity, code_dictionary, candidate_lookup))
        .collect();

    serde_json::json!({
        "id": format!("file_{}", file_group.file_path.replace("/", "_").replace(".", "root")),
        "type": "file",
        "name": file_name,
        "path": file_group.file_path,
        "entity_count": file_group.entity_count,
        "avg_score": ((file_group.avg_score * 10.0).round() / 10.0),
        "highest_priority": format!("{:?}", file_group.highest_priority),
        "total_issues": file_group.total_issues,
        "children": file_children
    })
}

/// Build an entity node with issue and suggestion children.
fn build_entity_node(
    entity: &RefactoringCandidate,
    code_dictionary: &CodeDictionary,
    candidate_lookup: &HashMap<String, RefactoringCandidate>,
) -> serde_json::Value {
    let display_name = entity.name.split(':').last()
        .map(|part| part.to_string())
        .unwrap_or_else(|| entity.name.clone());

    let candidate = candidate_lookup.get(&entity.entity_id);
    let entity_children = candidate
        .map(|c| build_entity_children(c, &entity.entity_id, code_dictionary))
        .unwrap_or_default();

    // Build issues array for tooltip display
    let issues: Vec<serde_json::Value> = candidate
        .map(|c| {
            c.issues.iter().map(|issue| {
                let issue_meta = code_dictionary.issues.get(&issue.code);
                serde_json::json!({
                    "code": issue.code,
                    "category": issue.category,
                    "title": issue_meta.map(|def| def.title.clone()).unwrap_or_else(|| issue.category.clone()),
                    "summary": issue_meta.map(|def| def.summary.clone()).unwrap_or_default(),
                    "severity": issue.severity,
                    "contributing_features": issue.contributing_features
                })
            }).collect()
        })
        .unwrap_or_default();

    // Build suggestions array for tooltip display
    let suggestions: Vec<serde_json::Value> = candidate
        .map(|c| {
            c.suggestions.iter().map(|suggestion| {
                let suggestion_meta = code_dictionary.suggestions.get(&suggestion.refactoring_type);
                serde_json::json!({
                    "code": suggestion.refactoring_type.clone(),
                    "refactoring_type": suggestion.refactoring_type.clone(),
                    "title": suggestion_meta.map(|def| def.title.clone()).unwrap_or_else(|| suggestion.refactoring_type.clone()),
                    "summary": suggestion_meta.map(|def| def.summary.clone()).unwrap_or_default(),
                    "impact": suggestion.impact,
                    "effort": suggestion.effort,
                    "priority": suggestion.priority
                })
            }).collect()
        })
        .unwrap_or_default();

    serde_json::json!({
        "id": entity.entity_id.clone(),
        "type": "entity",
        "name": display_name,
        "score": ((entity.score * 10.0).round() / 10.0),
        "priority": format!("{:?}", entity.priority),
        "issue_count": entity.issue_count,
        "suggestion_count": entity.suggestion_count,
        "issues": issues,
        "suggestions": suggestions,
        "children": entity_children
    })
}

/// Build children nodes (issues and suggestions) for an entity.
fn build_entity_children(
    candidate: &RefactoringCandidate,
    entity_id: &str,
    code_dictionary: &CodeDictionary,
) -> Vec<serde_json::Value> {
    let mut children = Vec::new();

    for (i, issue) in candidate.issues.iter().enumerate() {
        children.push(build_issue_node(issue, entity_id, i, code_dictionary));
    }
    for (i, suggestion) in candidate.suggestions.iter().enumerate() {
        children.push(build_suggestion_node(suggestion, entity_id, i, code_dictionary));
    }

    children
}

/// Build an issue node.
fn build_issue_node(
    issue: &crate::core::pipeline::RefactoringIssue,
    entity_id: &str,
    index: usize,
    code_dictionary: &CodeDictionary,
) -> serde_json::Value {
    let issue_meta = code_dictionary.issues.get(&issue.code);
    let issue_title = issue_meta.map(|def| def.title.clone())
        .unwrap_or_else(|| issue.category.clone());
    let issue_summary = issue_meta.map(|def| def.summary.clone())
        .unwrap_or_else(|| format!("{} signals detected by analyzer.", issue.category));

    serde_json::json!({
        "id": format!("{}:issue:{}", entity_id, index),
        "type": "issue",
        "code": issue.code,
        "name": format!("{} – {}", issue.code, issue_title),
        "title": issue_title,
        "category": issue.category,
        "summary": issue_summary,
        "severity": (issue.severity * 10.0).round() / 10.0,
        "contributing_features": issue.contributing_features,
        "children": []
    })
}

/// Build a suggestion node.
fn build_suggestion_node(
    suggestion: &crate::core::pipeline::RefactoringSuggestion,
    entity_id: &str,
    index: usize,
    code_dictionary: &CodeDictionary,
) -> serde_json::Value {
    let suggestion_meta = code_dictionary.suggestions.get(&suggestion.code);
    let suggestion_title = suggestion_meta.map(|def| def.title.clone())
        .unwrap_or_else(|| suggestion.refactoring_type.clone());
    let suggestion_summary = suggestion_meta.map(|def| def.summary.clone())
        .unwrap_or_else(|| suggestion.refactoring_type.replace('_', " "));

    serde_json::json!({
        "id": format!("{}:suggestion:{}", entity_id, index),
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
    })
}
