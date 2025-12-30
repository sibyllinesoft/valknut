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
    let filter_context = build_filter_context(pipeline_config, valknut_config)?;
    let (tracked_files, repo_root) = find_repository(&canonical_roots)?;

    let collected = if let Some(tracked) = tracked_files {
        collect_from_git_tracked(tracked, &canonical_roots, repo_root.as_deref(), &filter_context)
    } else {
        collect_from_filesystem_walk(&canonical_roots, &filter_context)
    };

    log_discovery_results(&collected);
    Ok(collected)
}

/// Build the filter context with compiled glob patterns.
fn build_filter_context(
    pipeline_config: &PipelineAnalysisConfig,
    valknut_config: Option<&ValknutConfig>,
) -> Result<(Option<GlobSet>, Option<GlobSet>, Option<GlobSet>, HashSet<String>, u64)> {
    let (include_patterns, mut exclude_patterns, ignore_patterns) =
        gather_patterns(pipeline_config, valknut_config);
    exclude_patterns.push("**/.git/**".to_string());

    let include_glob = compile_globset(&include_patterns)?;
    let exclude_glob = compile_globset(&exclude_patterns)?;
    let ignore_glob = compile_globset(&ignore_patterns)?;
    let allowed_extensions = allowed_extensions_from(pipeline_config, valknut_config);

    Ok((include_glob, exclude_glob, ignore_glob, allowed_extensions, pipeline_config.max_file_size_bytes))
}

/// Collect files from git-tracked file list.
fn collect_from_git_tracked(
    tracked: Vec<PathBuf>,
    canonical_roots: &[PathBuf],
    repo_root: Option<&Path>,
    filter_context: &(Option<GlobSet>, Option<GlobSet>, Option<GlobSet>, HashSet<String>, u64),
) -> Vec<PathBuf> {
    let (include_glob, exclude_glob, ignore_glob, allowed_extensions, max_file_size) = filter_context;

    info!(
        "Found git repository at '{}'. Using git index for file discovery.",
        repo_root.map(|p| p.display().to_string()).unwrap_or_else(|| "unknown".to_string())
    );
    info!("Discovered {} tracked files from git index", tracked.len());

    let mut unique = HashSet::new();
    let mut collected = Vec::new();

    for file in tracked {
        if !is_within_requested_roots(canonical_roots, &file) {
            continue;
        }

        let base = repo_root.unwrap_or_else(|| default_base_for(&file));
        if should_keep(&file, base, include_glob.as_ref(), exclude_glob.as_ref(), ignore_glob.as_ref(), allowed_extensions, *max_file_size) {
            add_unique(&mut unique, &mut collected, file);
        }
    }

    collected.sort();
    collected
}

/// Collect files via filesystem walk when git metadata isn't available.
fn collect_from_filesystem_walk(
    canonical_roots: &[PathBuf],
    filter_context: &(Option<GlobSet>, Option<GlobSet>, Option<GlobSet>, HashSet<String>, u64),
) -> Vec<PathBuf> {
    let (include_glob, exclude_glob, ignore_glob, allowed_extensions, max_file_size) = filter_context;

    warn!(
        "No git repository found for paths: {:?}. Falling back to filesystem walk. This may be slower.",
        canonical_roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
    );
    info!("Using filesystem traversal with ignore rules for file discovery");

    let mut unique = HashSet::new();
    let mut collected = Vec::new();

    for root in canonical_roots {
        if root.is_file() {
            if should_keep(root, default_base_for(root), include_glob.as_ref(), exclude_glob.as_ref(), ignore_glob.as_ref(), allowed_extensions, *max_file_size) {
                add_unique(&mut unique, &mut collected, root.clone());
            }
            continue;
        }

        walk_directory(root, &mut unique, &mut collected, include_glob, exclude_glob, ignore_glob, allowed_extensions, *max_file_size);
    }

    collected.sort();
    collected
}

/// Walk a directory and collect matching files.
fn walk_directory(
    root: &Path,
    unique: &mut HashSet<PathBuf>,
    collected: &mut Vec<PathBuf>,
    include_glob: &Option<GlobSet>,
    exclude_glob: &Option<GlobSet>,
    ignore_glob: &Option<GlobSet>,
    allowed_extensions: &HashSet<String>,
    max_file_size: u64,
) {
    let walker = WalkBuilder::new(root)
        .standard_filters(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .hidden(false)
        .build();

    for entry in walker {
        let Ok(dir_entry) = entry else {
            if let Err(err) = entry {
                warn!("Failed to walk directory: {err}");
            }
            continue;
        };

        let is_file = dir_entry.file_type().map(|ft| ft.is_file()).unwrap_or(false);
        if !is_file {
            continue;
        }

        let path = dir_entry.path();
        if should_keep(path, root, include_glob.as_ref(), exclude_glob.as_ref(), ignore_glob.as_ref(), allowed_extensions, max_file_size) {
            add_unique(unique, collected, path.to_path_buf());
        }
    }
}

/// Add a path to the collection if not already present.
fn add_unique(unique: &mut HashSet<PathBuf>, collected: &mut Vec<PathBuf>, path: PathBuf) {
    if unique.insert(path.clone()) {
        collected.push(path);
    }
}

/// Log discovery completion results.
fn log_discovery_results(collected: &[PathBuf]) {
    info!("File discovery completed: {} files selected for analysis", collected.len());
    if collected.len() > 100 {
        info!(
            "Large file set detected ({} files). Consider using more specific include/exclude patterns for better performance",
            collected.len()
        );
    }
}

fn canonicalize_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .map(|root| fs::canonicalize(root).unwrap_or_else(|_| root.clone()))
        .collect()
}

fn gather_patterns(
    _pipeline_config: &PipelineAnalysisConfig,
    valknut_config: Option<&ValknutConfig>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut include_patterns = vec!["**/*".to_string()]; // Default baseline include
    let mut exclude_patterns = default_exclude_patterns();
    let mut ignore_patterns = Vec::new();

    if let Some(cfg) = valknut_config {
        include_patterns.extend(cfg.analysis.include_patterns.clone());
        exclude_patterns.extend(cfg.analysis.exclude_patterns.clone());
        ignore_patterns.extend(cfg.analysis.ignore_patterns.clone());
    }

    exclude_patterns.sort();
    exclude_patterns.dedup();

    ignore_patterns.sort();
    ignore_patterns.dedup();

    (include_patterns, exclude_patterns, ignore_patterns)
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
    ignore_glob: Option<&GlobSet>,
    allowed_extensions: &HashSet<String>,
    max_file_size_bytes: u64,
) -> bool {
    let extension = match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => ext.to_ascii_lowercase(),
        None => return false,
    };

    if !allowed_extensions.is_empty() && !allowed_extensions.contains(&extension) {
        return false;
    }

    // Check file size limit (0 means unlimited)
    if max_file_size_bytes > 0 {
        if let Ok(metadata) = fs::metadata(path) {
            if metadata.len() > max_file_size_bytes {
                return false;
            }
        }
    }

    let relative = path.strip_prefix(base).unwrap_or(path);

    if let Some(exclude) = exclude_glob {
        if exclude.is_match(relative) {
            return false;
        }
    }

    if let Some(ignore) = ignore_glob {
        if ignore.is_match(relative) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::ValknutConfig;
    use crate::core::pipeline::pipeline_config::AnalysisConfig as PipelineAnalysisConfig;

    #[test]
    fn allowed_extensions_prioritises_language_config() {
        let pipeline_config = PipelineAnalysisConfig::default();
        let mut valknut_config = ValknutConfig::default();
        let extensions = allowed_extensions_from(&pipeline_config, Some(&valknut_config));
        assert!(extensions.contains("rs"));
        assert!(extensions.contains("py"));

        // Disable all languages so the pipeline config extensions are used instead
        valknut_config
            .languages
            .values_mut()
            .for_each(|lang| lang.enabled = false);
        let pipeline_extensions = allowed_extensions_from(&pipeline_config, Some(&valknut_config));
        for ext in &pipeline_config.file_extensions {
            assert!(pipeline_extensions.contains(&ext.trim_start_matches('.').to_string()));
        }
    }

    #[test]
    fn compile_globset_rejects_invalid_patterns() {
        let result = compile_globset(&["[invalid".to_string()]);
        assert!(result.is_err());

        let valid = compile_globset(&["**/*.rs".to_string()]).unwrap();
        assert!(valid.unwrap().is_match("src/lib.rs"));
    }

    #[test]
    fn should_keep_respects_include_exclude_and_extension() {
        let include = compile_globset(&["**/*.rs".to_string()]).unwrap();
        let exclude = compile_globset(&["**/generated/**".to_string()]).unwrap();
        let ignore = compile_globset(&["**/ignored.rs".to_string()]).unwrap();

        let mut allowed = HashSet::new();
        allowed.insert("rs".to_string());

        let base = Path::new("workspace");
        let keep_path = base.join("src/lib.rs");
        assert!(should_keep(
            &keep_path,
            base,
            include.as_ref(),
            exclude.as_ref(),
            ignore.as_ref(),
            &allowed,
            0, // unlimited file size
        ));

        let generated_path = base.join("generated/file.rs");
        assert!(!should_keep(
            &generated_path,
            base,
            include.as_ref(),
            exclude.as_ref(),
            ignore.as_ref(),
            &allowed,
            0,
        ));

        let ignored_path = base.join("src/ignored.rs");
        assert!(!should_keep(
            &ignored_path,
            base,
            include.as_ref(),
            exclude.as_ref(),
            ignore.as_ref(),
            &allowed,
            0,
        ));

        let wrong_extension = base.join("src/lib.ts");
        assert!(!should_keep(
            &wrong_extension,
            base,
            include.as_ref(),
            exclude.as_ref(),
            ignore.as_ref(),
            &allowed,
            0,
        ));
    }

    #[test]
    fn roots_membership_detects_files_and_directories() {
        let roots = vec![PathBuf::from("src"), PathBuf::from("README.md")];
        let file_in_src = Path::new("src/lib.rs");
        let exact_file = Path::new("README.md");
        let outside = Path::new("other/mod.rs");

        assert!(is_within_requested_roots(&roots, file_in_src));
        assert!(is_within_requested_roots(&roots, exact_file));
        assert!(!is_within_requested_roots(&roots, outside));
    }

    #[test]
    fn default_base_for_returns_parent_when_available() {
        let path = Path::new("src/lib.rs");
        assert_eq!(default_base_for(path), Path::new("src"));

        let standalone = Path::new("workspace");
        assert_eq!(default_base_for(standalone), Path::new(""));
    }
}
