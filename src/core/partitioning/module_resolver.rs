//! Module resolution utilities for import graph construction.
//!
//! This module provides functions to map import statements to file paths,
//! supporting multiple languages (Rust, Python, JavaScript/TypeScript, Go).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::types::FileNode;

/// Build mapping from module names to file paths.
/// Creates multiple keys for each file to enable flexible resolution.
pub(crate) fn build_module_map(nodes: &HashMap<PathBuf, FileNode>) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();

    for path in nodes.keys() {
        let path_str = path.to_string_lossy();

        // Get the path without extension
        let without_ext = strip_extension(&path_str);

        // For Rust: handle mod.rs specially
        // e.g., "src/core/pipeline/mod.rs" -> "core::pipeline" and "core.pipeline"
        if path_str.ends_with("mod.rs") {
            if let Some(parent) = path.parent() {
                let parent_str = parent.to_string_lossy();
                let rust_module = path_to_rust_module(&parent_str);
                let dot_module = rust_module.replace("::", ".");
                map.insert(rust_module.clone(), path.clone());
                map.insert(dot_module.clone(), path.clone());
                // Also add crate:: prefixed version
                map.insert(format!("crate::{}", rust_module), path.clone());
                map.insert(format!("crate.{}", dot_module), path.clone());
            }
        }

        // Standard module path: "src/core/config.rs" -> "core::config", "core.config", "config"
        let rust_module = path_to_rust_module(&without_ext);
        let dot_module = rust_module.replace("::", ".");
        map.insert(rust_module.clone(), path.clone());
        map.insert(dot_module.clone(), path.clone());

        // Add crate:: prefixed version
        map.insert(format!("crate::{}", rust_module), path.clone());
        map.insert(format!("crate.{}", dot_module), path.clone());

        // Add just the file stem for simple resolution
        if let Some(stem) = path.file_stem() {
            let stem_str = stem.to_string_lossy().to_string();
            if stem_str != "mod" && stem_str != "lib" && stem_str != "main" {
                map.insert(stem_str, path.clone());
            }
        }

        // For TypeScript/JavaScript: handle relative paths
        // e.g., "./foo" or "../bar"
        if path_str.ends_with(".ts")
            || path_str.ends_with(".tsx")
            || path_str.ends_with(".js")
            || path_str.ends_with(".jsx")
        {
            // Add the path without src prefix
            let no_src = without_ext.strip_prefix("src/").unwrap_or(&without_ext);
            map.insert(format!("./{}", no_src), path.clone());
        }

        // For Python: handle dot-separated module paths
        if path_str.ends_with(".py") {
            let py_module = without_ext.replace('/', ".").replace('\\', ".");
            map.insert(py_module, path.clone());
        }

        // For Go: the import path is typically the full package path
        if path_str.ends_with(".go") {
            // Just use the directory as the package
            if let Some(parent) = path.parent() {
                let parent_str = parent.to_string_lossy().to_string();
                map.insert(parent_str, path.clone());
            }
        }
    }

    map
}

/// Strip file extension from a path string.
pub(crate) fn strip_extension(path: &str) -> String {
    let extensions = [".rs", ".py", ".js", ".ts", ".tsx", ".jsx", ".go"];
    for ext in extensions {
        if let Some(stripped) = path.strip_suffix(ext) {
            return stripped.to_string();
        }
    }
    path.to_string()
}

/// Convert a file path to a Rust module path.
/// e.g., "src/core/config" -> "core::config"
pub(crate) fn path_to_rust_module(path: &str) -> String {
    path.strip_prefix("src/")
        .or_else(|| path.strip_prefix("src\\"))
        .unwrap_or(path)
        .replace(['/', '\\'], "::")
}

/// Try to resolve an import string to a file path.
pub(crate) fn resolve_import(
    import: &str,
    from_file: &Path,
    module_map: &HashMap<String, PathBuf>,
) -> Option<PathBuf> {
    // Handle Rust special prefixes
    let normalized = if import.starts_with("crate::") {
        // crate:: refers to the current crate root
        import.to_string()
    } else if import.starts_with("super::") {
        // super:: refers to parent module - resolve relative to from_file
        if let Some(parent) = from_file.parent() {
            if let Some(grandparent) = parent.parent() {
                let rest = import.strip_prefix("super::").unwrap();
                format!(
                    "{}::{}",
                    path_to_rust_module(&grandparent.to_string_lossy()),
                    rest
                )
            } else {
                import.to_string()
            }
        } else {
            import.to_string()
        }
    } else if import.starts_with("self::") {
        // self:: refers to current module
        if let Some(parent) = from_file.parent() {
            let rest = import.strip_prefix("self::").unwrap();
            format!(
                "{}::{}",
                path_to_rust_module(&parent.to_string_lossy()),
                rest
            )
        } else {
            import.to_string()
        }
    } else {
        import.to_string()
    };

    // Normalize separators
    let normalized = normalized
        .replace("::", ".")
        .replace('/', ".")
        .trim_start_matches('.')
        .to_string();

    // Try exact match first
    if let Some(path) = module_map.get(&normalized) {
        return Some(path.clone());
    }

    // Try with :: separator
    let rust_style = normalized.replace('.', "::");
    if let Some(path) = module_map.get(&rust_style) {
        return Some(path.clone());
    }

    // Try progressively shorter prefixes
    let parts: Vec<&str> = normalized.split('.').collect();
    for end in (1..=parts.len()).rev() {
        let prefix = parts[..end].join(".");
        if let Some(path) = module_map.get(&prefix) {
            return Some(path.clone());
        }
        let rust_prefix = parts[..end].join("::");
        if let Some(path) = module_map.get(&rust_prefix) {
            return Some(path.clone());
        }
    }

    // For mod declarations in Rust, try resolving relative to the file
    // e.g., `mod foo;` in `src/lib.rs` -> `src/foo.rs` or `src/foo/mod.rs`
    if parts.len() == 1 {
        if let Some(parent) = from_file.parent() {
            let parent_str = parent.to_string_lossy();
            let mod_name = parts[0];

            // Try sibling file: parent/mod_name.rs
            let sibling_path = format!("{}.{}", parent_str, mod_name);
            if let Some(path) = module_map.get(&sibling_path) {
                return Some(path.clone());
            }

            // Try nested module: parent/mod_name/mod.rs -> mapped as parent::mod_name
            let nested_path = format!("{}::{}", path_to_rust_module(&parent_str), mod_name);
            if let Some(path) = module_map.get(&nested_path) {
                return Some(path.clone());
            }
        }
    }

    // Try just the last component as a fallback
    if let Some(last) = parts.last() {
        if let Some(path) = module_map.get(*last) {
            return Some(path.clone());
        }
    }

    None
}
