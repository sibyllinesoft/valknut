//! Documentation health computation for the analysis pipeline.
//!
//! This module provides functionality for computing documentation health scores
//! using doc_audit with directory-aware aggregation and eligibility thresholds.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::core::config::DocHealthConfig;
use crate::doc_audit::{run_audit, DocAuditConfig};

/// Result of documentation health computation.
pub struct DocHealthResult {
    /// Overall documentation health score (0-100)
    pub score: f64,
    /// Total count of documentation issues
    pub issue_count: usize,
    /// Per-file issue counts
    pub file_issues: HashMap<String, usize>,
    /// Per-directory health scores
    pub dir_scores: HashMap<String, f64>,
    /// Per-directory issue counts
    pub dir_issues: HashMap<String, usize>,
    /// Per-file health scores (with multiple path variants for lookup)
    pub file_health: HashMap<String, f64>,
}

/// Compute documentation health using doc_audit with directory-aware aggregation and eligibility thresholds.
///
/// Returns None if no valid root directory is found in the paths.
pub fn compute_doc_health(
    paths: &[PathBuf],
    analyzed_files: &[PathBuf],
    cfg: &DocHealthConfig,
) -> Option<DocHealthResult> {
    let root = paths.iter().find(|p| p.is_dir())?.clone();
    let audit_cfg = DocAuditConfig::new(root);
    let result = run_audit(&audit_cfg).ok()?;

    // Per-file issue counts
    let mut file_gaps: HashMap<PathBuf, usize> = HashMap::new();
    for issue in result.documentation_issues.iter() {
        *file_gaps.entry(issue.path.clone()).or_insert(0) += 1;
    }

    let mut file_issue_out: HashMap<String, usize> = HashMap::new();

    // Aggregate per-file eligibility and scores
    let mut eligible_files = 0usize;
    let mut files_with_gaps = 0usize;

    // Directory aggregation buckets
    let mut dir_eligible: HashMap<PathBuf, usize> = HashMap::new();
    let mut dir_gaps: HashMap<PathBuf, usize> = HashMap::new();

    for (path, gaps) in file_gaps.iter() {
        // Paths from doc_audit are relative to audit root, so join them
        let full_path = if path.is_absolute() {
            path.clone()
        } else {
            audit_cfg.root.join(path)
        };
        let loc = std::fs::read_to_string(&full_path)
            .map(|c| c.lines().count())
            .unwrap_or(0);
        if loc < cfg.min_file_nodes {
            continue;
        }
        eligible_files += 1;
        if *gaps > 0 {
            files_with_gaps += 1;
        }

        file_issue_out.insert(path.display().to_string(), *gaps);

        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        *dir_eligible.entry(dir.clone()).or_insert(0) += 1;
        if *gaps > 0 {
            *dir_gaps.entry(dir.clone()).or_insert(0) += 1;
        }
    }

    // README gaps counted as project-level penalties
    let readme_gap_files = result.missing_readmes.len() + result.stale_readmes.len();
    eligible_files += readme_gap_files;
    files_with_gaps += readme_gap_files;
    let total_doc_issues = files_with_gaps;

    // Directory-level doc health (only if enough files)
    let mut dir_scores = Vec::new();
    let mut dir_score_map: HashMap<String, f64> = HashMap::new();
    let mut dir_issue_map: HashMap<String, usize> = HashMap::new();
    let mut file_health_map: HashMap<String, f64> = HashMap::new();

    for (dir, eligible) in dir_eligible.iter() {
        if *eligible < cfg.min_files_per_dir {
            dir_scores.push(100.0);
            dir_score_map.insert(dir.display().to_string(), 100.0);
            dir_issue_map.insert(dir.display().to_string(), 0);
            continue;
        }
        let gaps = *dir_gaps.get(dir).unwrap_or(&0);
        let coverage = 1.0 - (gaps as f64 / *eligible as f64);
        let score = (coverage * 100.0).clamp(0.0, 100.0);
        dir_scores.push(score);
        dir_score_map.insert(dir.display().to_string(), score);
        dir_issue_map.insert(dir.display().to_string(), gaps);
    }

    // File-level doc health: files with issues get scaled score, files without issues get 100
    // Use logarithmic scaling so files with many issues don't all collapse to 0
    // Formula: health = 100 * (1 - log10(gaps + 1) / log10(max_gaps + 1))
    // This gives a gentler curve: 1 issue ~= 85, 10 issues ~= 50, 100 issues ~= 15
    let max_gaps = file_gaps.values().copied().max().unwrap_or(1).max(1) as f64;
    let log_max = (max_gaps + 1.0).log10();

    for (path, gaps) in file_gaps.iter() {
        let score = if *gaps == 0 {
            100.0
        } else {
            let log_gaps = (*gaps as f64 + 1.0).log10();
            let scaled = 1.0 - (log_gaps / log_max);
            (scaled * 100.0).clamp(0.0, 100.0)
        };
        insert_path_variants(&mut file_health_map, path, score, &audit_cfg);
    }

    // Then add all analyzed source files that don't have issues (score = 100.0)
    // We need to normalize paths before comparing since file_gaps paths may differ
    // file_gaps paths are relative to audit_cfg.root, so we need to join them
    let file_gaps_canonical: HashSet<PathBuf> = file_gaps
        .keys()
        .filter_map(|p| {
            let full_path = if p.is_absolute() {
                p.clone()
            } else {
                audit_cfg.root.join(p)
            };
            full_path.canonicalize().ok()
        })
        .collect();

    for path in analyzed_files.iter() {
        if path.is_file() {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !file_gaps_canonical.contains(&canonical) {
                insert_path_variants(&mut file_health_map, path, 100.0, &audit_cfg);
            }
        }
    }

    // Project score preference: directory weighted average if any eligible dirs; else file coverage; else 100.
    let overall_score = if !dir_scores.is_empty() {
        let avg = dir_scores.iter().sum::<f64>() / dir_scores.len() as f64;
        avg.clamp(0.0, 100.0)
    } else if eligible_files == 0 {
        100.0
    } else {
        let coverage = 1.0 - (files_with_gaps as f64 / eligible_files as f64);
        (coverage * 100.0).clamp(0.0, 100.0)
    };

    Some(DocHealthResult {
        score: overall_score,
        issue_count: total_doc_issues,
        file_issues: file_issue_out,
        dir_scores: dir_score_map,
        dir_issues: dir_issue_map,
        file_health: file_health_map,
    })
}

/// Insert multiple path variants into the file health map for robust lookup.
fn insert_path_variants(
    file_health_map: &mut HashMap<String, f64>,
    path: &Path,
    score: f64,
    audit_cfg: &DocAuditConfig,
) {
    let abs = if path.is_absolute() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else {
        let joined = audit_cfg.root.join(path);
        joined.canonicalize().unwrap_or(joined)
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let rel_cwd = abs
        .strip_prefix(&cwd)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| abs.clone());
    let rel_root = abs
        .strip_prefix(&audit_cfg.root)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| rel_cwd.clone());

    let mut keys: Vec<PathBuf> = Vec::new();
    keys.push(abs.clone());
    keys.push(rel_cwd.clone());
    keys.push(rel_root.clone());
    if abs.starts_with(&audit_cfg.root) {
        let rel_to_root = abs.strip_prefix(&audit_cfg.root).unwrap_or(&abs);
        keys.push(rel_to_root.to_path_buf());
    }
    if rel_root.starts_with("src") {
        if let Ok(stripped) = rel_root.strip_prefix("src") {
            keys.push(stripped.to_path_buf());
            keys.push(PathBuf::from("src").join(stripped));
        }
    }
    keys.push(PathBuf::from("src").join(rel_root.clone()));
    if let Some(file_name) = abs.file_name() {
        keys.push(PathBuf::from(file_name));
    }

    for k in keys {
        let kstr = k.to_string_lossy().replace('\\', "/");
        file_health_map.insert(kstr.clone(), score);
        if !kstr.starts_with("./") {
            file_health_map.insert(format!("./{}", kstr), score);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_doc_health_no_dir() {
        // When no directory is provided, should return None
        let paths = vec![PathBuf::from("nonexistent_file.rs")];
        let result = compute_doc_health(&paths, &[], &DocHealthConfig::default());
        assert!(result.is_none());
    }
}
