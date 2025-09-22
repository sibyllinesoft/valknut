//! Git-aware file discovery utilities.
//!
//! This module centralizes file discovery so the analysis pipeline only
//! processes files that are actually tracked (or explicitly requested) while
//! respecting both repository ignore rules and Valknut configuration globs.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use git2::Repository;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use tracing::{info, warn};

use crate::core::config::ValknutConfig;
use crate::core::errors::{Result, ValknutError};

use super::pipeline_config::AnalysisConfig as PipelineAnalysisConfig;

/// Discover source files for analysis using git metadata when available.
pub fn discover_files(
    roots: &[PathBuf],
    pipeline_config: &PipelineAnalysisConfig,
    valknut_config: Option<&ValknutConfig>,
) -> Result<Vec<PathBuf>> {
    if roots.is_empty() {
        return Ok(Vec::new());
    }

    let canonical_roots = canonicalize_roots(roots);

    let (include_patterns, mut exclude_patterns) = gather_patterns(pipeline_config);
    exclude_patterns.push("**/.git/**".to_string());

    let include_glob = compile_globset(&include_patterns)?;
    let exclude_glob = compile_globset(&exclude_patterns)?;

    let allowed_extensions = allowed_extensions_from(pipeline_config, valknut_config);

    let (tracked_files, repo_root) = find_repository(&canonical_roots)?;

    let mut unique = HashSet::new();
    let mut collected = Vec::new();

    if let Some(tracked) = tracked_files {
        info!("Found git repository at '{}'. Using git index for file discovery.", 
              repo_root.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "unknown".to_string()));
        info!("Discovered {} tracked files from git index", tracked.len());
        for file in tracked {
            if !is_within_requested_roots(&canonical_roots, &file) {
                continue;
            }

            if should_keep(
                &file,
                repo_root
                    .as_deref()
                    .unwrap_or_else(|| default_base_for(&file)),
                include_glob.as_ref(),
                exclude_glob.as_ref(),
                &allowed_extensions,
            ) {
                if unique.insert(file.clone()) {
                    collected.push(file);
                }
            }
        }
    } else {
        warn!("No git repository found for paths: {:?}. Falling back to filesystem walk. This may be slower.", 
              canonical_roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>());
        info!("Using filesystem traversal with ignore rules for file discovery");
        // Fall back to filesystem walk with ignore rules when git metadata isn't available.
        for root in &canonical_roots {
            if root.is_file() {
                if should_keep(
                    root,
                    default_base_for(root),
                    include_glob.as_ref(),
                    exclude_glob.as_ref(),
                    &allowed_extensions,
                ) {
                    if unique.insert(root.clone()) {
                        collected.push(root.clone());
                    }
                }
                continue;
            }

            let walker = WalkBuilder::new(root)
                .standard_filters(true)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .hidden(false)
                .build();

            for entry in walker {
                match entry {
                    Ok(dir_entry) => {
                        let path = dir_entry.path();
                        if !dir_entry
                            .file_type()
                            .map(|ft| ft.is_file())
                            .unwrap_or(false)
                        {
                            continue;
                        }

                        if should_keep(
                            path,
                            root,
                            include_glob.as_ref(),
                            exclude_glob.as_ref(),
                            &allowed_extensions,
                        ) {
                            let path = path.to_path_buf();
                            if unique.insert(path.clone()) {
                                collected.push(path);
                            }
                        }
                    }
                    Err(err) => warn!("Failed to walk directory: {err}"),
                }
            }
        }
    }

    collected.sort();
    info!("File discovery completed: {} files selected for analysis", collected.len());
    if collected.len() > 100 {
        info!("Large file set detected ({} files). Consider using more specific include/exclude patterns for better performance", collected.len());
    }
    Ok(collected)
}

fn canonicalize_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .map(|root| fs::canonicalize(root).unwrap_or_else(|_| root.clone()))
        .collect()
}

fn gather_patterns(pipeline_config: &PipelineAnalysisConfig) -> (Vec<String>, Vec<String>) {
    let include_patterns = vec!["**/*".to_string()]; // Always use default for now
    
    let mut exclude_patterns = default_exclude_patterns();

    exclude_patterns.sort();
    exclude_patterns.dedup();

    (include_patterns, exclude_patterns)
}

fn default_exclude_patterns() -> Vec<String> {
    vec![
        "**/node_modules/**".to_string(),
        "**/target/**".to_string(),
        "**/__pycache__/**".to_string(),
        "**/dist/**".to_string(),
        "**/build/**".to_string(),
    ]
}

fn allowed_extensions_from(
    pipeline_config: &PipelineAnalysisConfig,
    valknut_config: Option<&ValknutConfig>,
) -> HashSet<String> {
    if let Some(cfg) = valknut_config {
        let extensions: HashSet<String> = cfg
            .languages
            .values()
            .filter(|lang| lang.enabled)
            .flat_map(|lang| lang.file_extensions.iter())
            .map(|ext| ext.trim_start_matches('.').to_ascii_lowercase())
            .collect();

        if !extensions.is_empty() {
            return extensions;
        }
    }

    // Use file_extensions from the pipeline config
    pipeline_config
        .file_extensions
        .iter()
        .map(|ext| ext.trim_start_matches('.').to_ascii_lowercase())
        .collect()
}

fn compile_globset(patterns: &[String]) -> Result<Option<GlobSet>> {
    let mut builder = GlobSetBuilder::new();
    let mut added = false;

    for pattern in patterns {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }

        let glob = GlobBuilder::new(pattern)
            .literal_separator(false)
            .build()
            .map_err(|err| {
                ValknutError::config(format!("Invalid glob pattern '{pattern}': {err}"))
            })?;
        builder.add(glob);
        added = true;
    }

    if added {
        builder
            .build()
            .map(Some)
            .map_err(|err| ValknutError::config(format!("Failed to build glob set: {err}")))
    } else {
        Ok(None)
    }
}

fn find_repository(roots: &[PathBuf]) -> Result<(Option<Vec<PathBuf>>, Option<PathBuf>)> {
    for root in roots {
        if let Ok(repo) = Repository::discover(root) {
            if let Some(workdir) = repo.workdir() {
                info!("Located git repository: {}", workdir.display());
                let tracked = collect_tracked_files(&repo, workdir)?;
                return Ok((Some(tracked), Some(workdir.to_path_buf())));
            }
        }
    }

    info!("No git repository found in any of the provided paths");
    Ok((None, None))
}

fn collect_tracked_files(repo: &Repository, workdir: &Path) -> Result<Vec<PathBuf>> {
    let index = repo
        .index()
        .map_err(|err| ValknutError::internal(format!("Failed to read git index: {err}")))?;

    let mut files = Vec::with_capacity(index.len());

    for entry in index.iter() {
        let rel = String::from_utf8_lossy(entry.path.as_ref()).into_owned();
        let absolute = workdir.join(rel);

        // Skip entries that no longer exist in the working tree.
        if absolute.is_file() {
            files.push(absolute);
        }
    }

    Ok(files)
}

fn should_keep(
    path: &Path,
    base: &Path,
    include_glob: Option<&GlobSet>,
    exclude_glob: Option<&GlobSet>,
    allowed_extensions: &HashSet<String>,
) -> bool {
    let extension = match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => ext.to_ascii_lowercase(),
        None => return false,
    };

    if !allowed_extensions.is_empty() && !allowed_extensions.contains(&extension) {
        return false;
    }

    let relative = path.strip_prefix(base).unwrap_or(path);

    if let Some(exclude) = exclude_glob {
        if exclude.is_match(relative) {
            return false;
        }
    }

    if let Some(include) = include_glob {
        include.is_match(relative)
    } else {
        true
    }
}

fn is_within_requested_roots(roots: &[PathBuf], path: &Path) -> bool {
    roots.iter().any(|root| {
        if root.is_dir() {
            path.starts_with(root)
        } else {
            path == root
        }
    })
}

fn default_base_for(path: &Path) -> &Path {
    path.parent().unwrap_or(path)
}
