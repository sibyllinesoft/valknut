//! Import resolution and project dependency scanning.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::core::errors::Result;
use crate::core::file_utils::FileReader;
use crate::lang::common::{EntityKind, ParsedEntity};
use crate::lang::registry::adapter_for_file;

use crate::detectors::structure::config::{
    is_code_extension, should_skip_directory, ImportStatement, CODE_EXTENSIONS,
};

/// Snapshot of project imports for dependency analysis
#[derive(Default, Debug)]
pub struct ProjectImportSnapshot {
    pub imports_by_file: HashMap<PathBuf, Vec<PathBuf>>,
    pub reverse_imports: HashMap<PathBuf, HashSet<PathBuf>>,
}

/// Metrics about a file's dependencies
#[derive(Default, Debug, Clone)]
pub struct FileDependencyMetrics {
    pub exports: Vec<ExportedEntity>,
    pub outgoing_dependencies: HashSet<PathBuf>,
    pub incoming_importers: HashSet<PathBuf>,
}

/// An exported entity from a file
#[derive(Debug, Clone)]
pub struct ExportedEntity {
    pub name: String,
    pub kind: EntityKind,
}

/// Import resolver for project dependency scanning
pub struct ImportResolver {
    project_import_cache: Arc<RwLock<HashMap<PathBuf, Arc<ProjectImportSnapshot>>>>,
}

/// Factory, caching, and resolution methods for [`ImportResolver`].
impl ImportResolver {
    /// Creates a new import resolver with an empty cache.
    pub fn new() -> Self {
        Self {
            project_import_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if file extension indicates a code file
    pub fn is_code_file(&self, extension: &str) -> bool {
        is_code_extension(extension)
    }

    /// Collect dependency metrics for a file
    pub fn collect_dependency_metrics(
        &self,
        file_path: &Path,
        project_root: Option<&Path>,
    ) -> Result<FileDependencyMetrics> {
        let mut metrics = FileDependencyMetrics::default();
        let content = FileReader::read_to_string(file_path)?;

        if let Ok(mut adapter) = adapter_for_file(file_path) {
            if let Ok(parse_index) = adapter.parse_source(&content, &file_path.to_string_lossy()) {
                metrics.exports = self.extract_exported_entities(file_path, &parse_index, &content);
            }
        }

        if let Some(root) = project_root {
            let snapshot = self.get_project_import_snapshot(root)?;
            let canonical_file = self.canonicalize_path(file_path);

            if let Some(targets) = snapshot.imports_by_file.get(&canonical_file) {
                metrics
                    .outgoing_dependencies
                    .extend(targets.iter().cloned());
            }

            if let Some(importers) = snapshot.reverse_imports.get(&canonical_file) {
                metrics.incoming_importers.extend(importers.iter().cloned());
            }
        }

        Ok(metrics)
    }

    /// Extract exported entities from a file
    pub fn extract_exported_entities(
        &self,
        file_path: &Path,
        parse_index: &crate::lang::common::ParseIndex,
        content: &str,
    ) -> Vec<ExportedEntity> {
        let file_key = file_path.to_string_lossy();
        parse_index
            .get_entities_in_file(&file_key)
            .into_iter()
            .filter(|entity| entity.parent.is_none())
            .filter(|entity| {
                matches!(
                    entity.kind,
                    EntityKind::Function
                        | EntityKind::Class
                        | EntityKind::Struct
                        | EntityKind::Enum
                        | EntityKind::Interface
                )
            })
            .filter(|entity| self.is_entity_exported(entity, file_path, content))
            .map(|entity| ExportedEntity {
                name: entity.name.clone(),
                kind: entity.kind,
            })
            .collect()
    }

    /// Check if an entity is exported based on language conventions
    pub fn is_entity_exported(&self, entity: &ParsedEntity, file_path: &Path, content: &str) -> bool {
        let ext = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default();

        match ext {
            "rs" => entity
                .metadata
                .get("visibility")
                .and_then(|value| value.as_str())
                .map(|vis| vis.contains("pub"))
                .unwrap_or(false),
            "py" | "pyi" => {
                if entity.name.starts_with('_') {
                    return false;
                }
                entity.parent.is_none()
            }
            "go" => entity
                .name
                .chars()
                .next()
                .map(|ch| ch.is_ascii_uppercase())
                .unwrap_or(false),
            "ts" | "tsx" | "js" | "jsx" => {
                self.line_has_export_keyword(content, entity.location.start_line)
            }
            "java" => self.line_has_keyword(content, entity.location.start_line, "public"),
            _ => entity.parent.is_none(),
        }
    }

    /// Check if a line has an export keyword (JS/TS).
    fn line_has_export_keyword(&self, content: &str, start_line: usize) -> bool {
        self.line_has_keyword(content, start_line, "export")
    }

    /// Check if a line has a specific keyword
    pub fn line_has_keyword(&self, content: &str, start_line: usize, keyword: &str) -> bool {
        if start_line == 0 {
            return false;
        }

        let lines: Vec<&str> = content.lines().collect();
        let line_idx = start_line.saturating_sub(1);

        if let Some(line) = lines.get(line_idx) {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                return false;
            }
            if trimmed.starts_with(keyword) || trimmed.contains(&format!("{keyword} ")) {
                return true;
            }
        }

        if line_idx > 0 {
            if let Some(previous) = lines.get(line_idx - 1) {
                if previous.trim_end().ends_with(keyword) {
                    return true;
                }
            }
        }

        false
    }

    /// Get or build the project import snapshot
    pub fn get_project_import_snapshot(
        &self,
        project_root: &Path,
    ) -> Result<Arc<ProjectImportSnapshot>> {
        let canonical_root = self.canonicalize_path(project_root);

        if let Some(snapshot) = self
            .project_import_cache
            .read()
            .unwrap()
            .get(&canonical_root)
            .cloned()
        {
            return Ok(snapshot);
        }

        let snapshot = Arc::new(self.build_project_import_snapshot(&canonical_root)?);
        self.project_import_cache
            .write()
            .unwrap()
            .insert(canonical_root, snapshot.clone());

        Ok(snapshot)
    }

    /// Build a fresh project import snapshot by scanning all code files.
    fn build_project_import_snapshot(&self, project_root: &Path) -> Result<ProjectImportSnapshot> {
        let mut snapshot = ProjectImportSnapshot::default();
        for file in self.collect_project_code_files(project_root)? {
            let canonical_file = self.canonicalize_path(&file);
            let imports = self.extract_imports(&file)?;

            for import in imports {
                if let Some(resolved) =
                    self.resolve_import_to_project_file(&import, &file, project_root)
                {
                    let canonical_target = self.canonicalize_path(&resolved);
                    snapshot
                        .imports_by_file
                        .entry(canonical_file.clone())
                        .or_default()
                        .push(canonical_target.clone());
                    snapshot
                        .reverse_imports
                        .entry(canonical_target)
                        .or_default()
                        .insert(canonical_file.clone());
                }
            }
        }

        Ok(snapshot)
    }

    /// Collect all code files in a project
    pub fn collect_project_code_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_project_code_files_recursive(root, &mut files)?;
        Ok(files)
    }

    /// Recursively collect code files from a directory.
    fn collect_project_code_files_recursive(
        &self,
        path: &Path,
        files: &mut Vec<PathBuf>,
    ) -> Result<()> {
        if self.should_skip_directory(path) {
            return Ok(());
        }

        for entry in std::fs::read_dir(path)? {
            let child_path = entry?.path();

            if child_path.is_dir() {
                self.collect_project_code_files_recursive(&child_path, files)?;
                continue;
            }

            let is_code = child_path
                .extension()
                .and_then(|e| e.to_str())
                .map_or(false, |ext| self.is_code_file(ext));

            if is_code {
                files.push(child_path);
            }
        }

        Ok(())
    }

    /// Resolve an import to a project file path
    pub fn resolve_import_to_project_file(
        &self,
        import: &ImportStatement,
        current_file: &Path,
        project_root: &Path,
    ) -> Option<PathBuf> {
        let module = import.module.trim();
        if module.is_empty() {
            return None;
        }

        let current_dir = current_file.parent().unwrap_or(project_root);
        let mut candidates: Vec<PathBuf> = Vec::new();

        if module.starts_with("./") || module.starts_with("../") {
            candidates.push(current_dir.join(module));
        } else if module.starts_with('.') {
            candidates.extend(self.resolve_python_relative_module(
                current_dir,
                project_root,
                module,
            ));
        } else {
            if module.contains('/') {
                candidates.push(project_root.join(module));
                candidates.push(current_dir.join(module));
            }

            if module.contains('.') {
                let mut from_root = project_root.to_path_buf();
                for part in module.split('.') {
                    if part.is_empty() {
                        continue;
                    }
                    from_root.push(part);
                }
                candidates.push(from_root);
            }

            candidates.push(current_dir.join(module));
        }

        for candidate in candidates {
            if let Some(resolved) = self.resolve_candidate_path(&candidate) {
                return Some(resolved);
            }
        }

        None
    }

    /// Resolve Python relative import (dot notation) to candidate paths.
    fn resolve_python_relative_module(
        &self,
        current_dir: &Path,
        project_root: &Path,
        module: &str,
    ) -> Vec<PathBuf> {
        let mut base = current_dir.to_path_buf();
        let mut parts = Vec::new();
        for part in module.split('.') {
            if part.is_empty() {
                if let Some(parent) = base.parent() {
                    base = parent.to_path_buf();
                } else {
                    base = project_root.to_path_buf();
                }
            } else {
                parts.push(part);
            }
        }

        if parts.is_empty() {
            vec![base]
        } else {
            let mut path = base;
            for part in parts {
                path.push(part);
            }
            vec![path]
        }
    }

    /// Resolve a candidate path to an existing file
    pub fn resolve_candidate_path(&self, candidate: &Path) -> Option<PathBuf> {
        let mut targets = Vec::new();

        if candidate.exists() {
            if candidate.is_file() {
                targets.push(candidate.to_path_buf());
            } else if candidate.is_dir() {
                targets.extend(self.directory_module_fallbacks(candidate));
            }
        }

        if candidate.extension().is_none() {
            for ext in Self::supported_extensions() {
                let candidate_with_ext = candidate.with_extension(ext);
                if candidate_with_ext.exists() {
                    targets.push(candidate_with_ext);
                }
            }
        }

        targets.into_iter().find(|path| path.exists())
    }

    /// Get directory module fallback paths
    pub fn directory_module_fallbacks(&self, dir: &Path) -> Vec<PathBuf> {
        [
            "mod.rs",
            "lib.rs",
            "__init__.py",
            "index.ts",
            "index.tsx",
            "index.js",
            "index.jsx",
        ]
        .iter()
        .map(|candidate| dir.join(candidate))
        .collect()
    }

    /// Supported file extensions for import resolution
    pub fn supported_extensions() -> &'static [&'static str] {
        CODE_EXTENSIONS
    }

    /// Canonicalize a path for consistent comparison
    pub fn canonicalize_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            if let Ok(current_dir) = std::env::current_dir() {
                if let Ok(relative) = path.strip_prefix(&current_dir) {
                    return relative.to_path_buf();
                }
            }
        }
        path.to_path_buf()
    }

    /// Extract imports from a file
    pub fn extract_imports(&self, file_path: &Path) -> Result<Vec<ImportStatement>> {
        let content = FileReader::read_to_string(file_path)?;
        let mut adapter = adapter_for_file(file_path)?;
        adapter.extract_imports(&content)
    }

    /// Resolve import statement to local file path
    pub fn resolve_import_to_local_file(
        &self,
        import: &ImportStatement,
        dir_path: &Path,
    ) -> Option<PathBuf> {
        let module_name = &import.module;

        if module_name.starts_with('.') {
            return None;
        }

        for ext in Self::supported_extensions() {
            let potential_path = dir_path.join(format!("{}.{}", module_name, ext));
            if potential_path.exists() {
                return Some(potential_path);
            }
        }

        None
    }

    /// Check if directory should be skipped during analysis
    pub fn should_skip_directory(&self, path: &Path) -> bool {
        should_skip_directory(path)
    }
}

/// Default implementation for [`ImportResolver`].
impl Default for ImportResolver {
    /// Returns a new import resolver with default settings.
    fn default() -> Self {
        Self::new()
    }
}
