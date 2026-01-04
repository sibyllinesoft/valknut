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

    let file_gaps = count_file_gaps(&result);
    let aggregation = aggregate_file_stats(&file_gaps, &audit_cfg, cfg);
    let readme_penalties = result.missing_readmes.len() + result.stale_readmes.len();
    let (dir_scores, dir_score_map, dir_issue_map) =
        compute_directory_scores(&aggregation, cfg);
    let file_health_map = compute_file_health_scores(
        &file_gaps,
        analyzed_files,
        &audit_cfg,
    );
    let overall_score = compute_overall_score(
        &dir_scores,
        aggregation.eligible_files + readme_penalties,
        aggregation.files_with_gaps + readme_penalties,
    );

    Some(DocHealthResult {
        score: overall_score,
        issue_count: aggregation.files_with_gaps + readme_penalties,
        file_issues: aggregation.file_issue_out,
        dir_scores: dir_score_map,
        dir_issues: dir_issue_map,
        file_health: file_health_map,
    })
}

/// Count documentation issues per file.
fn count_file_gaps(result: &crate::doc_audit::AuditResult) -> HashMap<PathBuf, usize> {
    let mut file_gaps: HashMap<PathBuf, usize> = HashMap::new();
    for issue in result.documentation_issues.iter() {
        *file_gaps.entry(issue.path.clone()).or_insert(0) += 1;
    }
    file_gaps
}

/// Aggregated statistics from file analysis.
struct FileAggregation {
    eligible_files: usize,
    files_with_gaps: usize,
    file_issue_out: HashMap<String, usize>,
    dir_eligible: HashMap<PathBuf, usize>,
    dir_gaps: HashMap<PathBuf, usize>,
}

/// Aggregate file statistics for eligibility and directory grouping.
fn aggregate_file_stats(
    file_gaps: &HashMap<PathBuf, usize>,
    audit_cfg: &DocAuditConfig,
    cfg: &DocHealthConfig,
) -> FileAggregation {
    let mut agg = FileAggregation {
        eligible_files: 0,
        files_with_gaps: 0,
        file_issue_out: HashMap::new(),
        dir_eligible: HashMap::new(),
        dir_gaps: HashMap::new(),
    };

    for (path, gaps) in file_gaps.iter() {
        let full_path = resolve_full_path(path, &audit_cfg.root);
        let loc = std::fs::read_to_string(&full_path)
            .map(|c| c.lines().count())
            .unwrap_or(0);
        if loc < cfg.min_file_nodes {
            continue;
        }

        agg.eligible_files += 1;
        if *gaps > 0 {
            agg.files_with_gaps += 1;
        }
        agg.file_issue_out.insert(path.display().to_string(), *gaps);

        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        *agg.dir_eligible.entry(dir.clone()).or_insert(0) += 1;
        if *gaps > 0 {
            *agg.dir_gaps.entry(dir).or_insert(0) += 1;
        }
    }

    agg
}

/// Resolve a path to its full form, joining with root if relative.
fn resolve_full_path(path: &Path, root: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// Compute directory-level health scores.
fn compute_directory_scores(
    agg: &FileAggregation,
    cfg: &DocHealthConfig,
) -> (Vec<f64>, HashMap<String, f64>, HashMap<String, usize>) {
    let mut dir_scores = Vec::new();
    let mut dir_score_map: HashMap<String, f64> = HashMap::new();
    let mut dir_issue_map: HashMap<String, usize> = HashMap::new();

    for (dir, eligible) in agg.dir_eligible.iter() {
        let dir_key = dir.display().to_string();
        if *eligible < cfg.min_files_per_dir {
            dir_scores.push(100.0);
            dir_score_map.insert(dir_key.clone(), 100.0);
            dir_issue_map.insert(dir_key, 0);
            continue;
        }
        let gaps = *agg.dir_gaps.get(dir).unwrap_or(&0);
        let coverage = 1.0 - (gaps as f64 / *eligible as f64);
        let score = (coverage * 100.0).clamp(0.0, 100.0);
        dir_scores.push(score);
        dir_score_map.insert(dir_key.clone(), score);
        dir_issue_map.insert(dir_key, gaps);
    }

    (dir_scores, dir_score_map, dir_issue_map)
}

/// Compute file-level health scores using logarithmic scaling.
fn compute_file_health_scores(
    file_gaps: &HashMap<PathBuf, usize>,
    analyzed_files: &[PathBuf],
    audit_cfg: &DocAuditConfig,
) -> HashMap<String, f64> {
    let mut file_health_map: HashMap<String, f64> = HashMap::new();
    let max_gaps = file_gaps.values().copied().max().unwrap_or(1).max(1) as f64;
    let log_max = (max_gaps + 1.0).log10();

    // Score files with issues
    for (path, gaps) in file_gaps.iter() {
        let score = compute_log_scaled_score(*gaps, log_max);
        insert_path_variants(&mut file_health_map, path, score, audit_cfg);
    }

    // Add analyzed files without issues (score = 100.0)
    let file_gaps_canonical = canonicalize_paths(file_gaps.keys(), &audit_cfg.root);
    for path in analyzed_files.iter().filter(|p| p.is_file()) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !file_gaps_canonical.contains(&canonical) {
            insert_path_variants(&mut file_health_map, path, 100.0, audit_cfg);
        }
    }

    file_health_map
}

/// Compute health score using logarithmic scaling.
fn compute_log_scaled_score(gaps: usize, log_max: f64) -> f64 {
    if gaps == 0 {
        100.0
    } else {
        let log_gaps = (gaps as f64 + 1.0).log10();
        let scaled = 1.0 - (log_gaps / log_max);
        (scaled * 100.0).clamp(0.0, 100.0)
    }
}

/// Canonicalize a collection of paths.
fn canonicalize_paths<'a>(
    paths: impl Iterator<Item = &'a PathBuf>,
    root: &Path,
) -> HashSet<PathBuf> {
    paths
        .filter_map(|p| resolve_full_path(p, root).canonicalize().ok())
        .collect()
}

/// Compute overall documentation health score.
fn compute_overall_score(dir_scores: &[f64], eligible_files: usize, files_with_gaps: usize) -> f64 {
    if !dir_scores.is_empty() {
        let avg = dir_scores.iter().sum::<f64>() / dir_scores.len() as f64;
        avg.clamp(0.0, 100.0)
    } else if eligible_files == 0 {
        100.0
    } else {
        let coverage = 1.0 - (files_with_gaps as f64 / eligible_files as f64);
        (coverage * 100.0).clamp(0.0, 100.0)
    }
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
