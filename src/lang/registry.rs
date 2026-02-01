//! Factory utilities and metadata for language adapters.

use std::path::Path;
use tree_sitter::Language;

use crate::core::errors::{Result, ValknutError};
use crate::lang::common::LanguageAdapter;
use crate::lang::cpp::CppAdapter;
use crate::lang::go::GoAdapter;
use crate::lang::javascript::JavaScriptAdapter;
use crate::lang::python::PythonAdapter;
use crate::lang::rust_lang::RustAdapter;
use crate::lang::typescript::TypeScriptAdapter;

/// Stability indicator used for documentation and CLI output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageStability {
    Stable,
    Beta,
}

/// Metadata describing one of the built-in language adapters.
#[derive(Debug, Clone, Copy)]
pub struct LanguageInfo {
    /// Canonical short key (matches CLI/config usage, e.g. "py").
    pub key: &'static str,
    /// Human-friendly display name.
    pub name: &'static str,
    /// Supported file extensions (without leading dots).
    pub extensions: &'static [&'static str],
    /// Stability status.
    pub status: LanguageStability,
    /// Feature notes for documentation/UI.
    pub notes: &'static str,
}

const REGISTERED_LANGUAGES: &[LanguageInfo] = &[
    LanguageInfo {
        key: "py",
        name: "Python",
        extensions: &["py", "pyi"],
        status: LanguageStability::Stable,
        notes: "Full analysis & refactoring",
    },
    LanguageInfo {
        key: "ts",
        name: "TypeScript",
        extensions: &["ts", "tsx", "cts", "mts"],
        status: LanguageStability::Stable,
        notes: "Full analysis & type-aware heuristics",
    },
    LanguageInfo {
        key: "js",
        name: "JavaScript",
        extensions: &["js", "jsx", "mjs", "cjs"],
        status: LanguageStability::Stable,
        notes: "Full analysis & complexity metrics",
    },
    LanguageInfo {
        key: "rs",
        name: "Rust",
        extensions: &["rs"],
        status: LanguageStability::Stable,
        notes: "Ownership-aware analysis",
    },
    LanguageInfo {
        key: "go",
        name: "Go",
        extensions: &["go"],
        status: LanguageStability::Beta,
        notes: "AST parsing & structure checks",
    },
    LanguageInfo {
        key: "cpp",
        name: "C++",
        extensions: &["cpp", "cxx", "cc", "c++", "hpp", "hxx", "hh", "h++", "h"],
        status: LanguageStability::Beta,
        notes: "Classes, namespaces, templates",
    },
];

/// Return the languages that are compiled into this build.
pub fn registered_languages() -> &'static [LanguageInfo] {
    REGISTERED_LANGUAGES
}

/// Identify the canonical language key for a file path.
pub fn language_key_for_path(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    if ext.is_empty() {
        return None;
    }

    find_language_by_extension(&ext).map(|info| info.key.to_string())
}

/// Create a language adapter suitable for analysing the provided file.
pub fn adapter_for_file(path: &Path) -> Result<Box<dyn LanguageAdapter>> {
    let key = language_key_for_path(path).ok_or_else(|| {
        ValknutError::unsupported(format!(
            "Could not determine language for file: {}",
            path.display()
        ))
    })?;

    adapter_for_language(&key)
}

/// Create a language adapter for a specific language key (usually an extension).
pub fn adapter_for_language(language: &str) -> Result<Box<dyn LanguageAdapter>> {
    match normalize_language_key(language) {
        Some("py") => Ok(Box::new(PythonAdapter::new()?)),
        Some("js") => Ok(Box::new(JavaScriptAdapter::new()?)),
        Some("ts") => Ok(Box::new(TypeScriptAdapter::new()?)),
        Some("rs") => Ok(Box::new(RustAdapter::new()?)),
        Some("go") => Ok(Box::new(GoAdapter::new()?)),
        Some("cpp") => Ok(Box::new(CppAdapter::new()?)),
        _ => Err(ValknutError::unsupported(format!(
            "Language adapter for '{}' is not yet implemented",
            language
        ))),
    }
}

/// Get tree-sitter language for a given language key
pub fn get_tree_sitter_language(language_key: &str) -> Result<Language> {
    match normalize_language_key(language_key) {
        Some("py") => Ok(tree_sitter_python::LANGUAGE.into()),
        Some("rs") => Ok(tree_sitter_rust::LANGUAGE.into()),
        Some("js") => Ok(tree_sitter_javascript::LANGUAGE.into()),
        Some("ts") => Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        Some("go") => Ok(tree_sitter_go::LANGUAGE.into()),
        Some("cpp") => Ok(tree_sitter_cpp::LANGUAGE.into()),
        _ => Err(ValknutError::unsupported(format!(
            "No tree-sitter grammar for: {}",
            language_key
        ))),
    }
}

/// Detect language key from file path
pub fn detect_language_from_path(file_path: &str) -> String {
    std::path::Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| find_language_by_extension(&ext.to_ascii_lowercase()))
        .map(|info| info.key.to_string())
        .unwrap_or_else(|| "txt".to_string())
}

/// Create a new parser for the given language
pub fn create_parser_for_language(language_key: &str) -> Result<tree_sitter::Parser> {
    let mut parser = tree_sitter::Parser::new();
    let tree_sitter_language = get_tree_sitter_language(language_key)?;
    parser.set_language(&tree_sitter_language).map_err(|e| {
        ValknutError::parse(
            language_key,
            format!("Failed to set parser language: {}", e),
        )
    })?;
    Ok(parser)
}

/// Check whether a file extension (with or without leading dot) is supported.
pub fn extension_is_supported(ext: &str) -> bool {
    let normalized = ext.trim_start_matches('.').to_ascii_lowercase();
    find_language_by_extension(&normalized).is_some()
}

/// Finds the language info for a given file extension.
fn find_language_by_extension(ext: &str) -> Option<&'static LanguageInfo> {
    let target = ext.trim_start_matches('.').to_ascii_lowercase();
    registered_languages().iter().find(|info| {
        info.extensions
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&target))
    })
}

/// Normalizes a language identifier to its canonical key.
fn normalize_language_key(language: &str) -> Option<&'static str> {
    match language.to_ascii_lowercase().as_str() {
        "py" | "pyw" | "python" => Some("py"),
        "js" | "jsx" | "mjs" | "cjs" | "javascript" => Some("js"),
        "ts" | "tsx" | "cts" | "mts" | "typescript" => Some("ts"),
        "rs" | "rust" => Some("rs"),
        "go" | "golang" => Some("go"),
        "cpp" | "cxx" | "cc" | "c++" | "hpp" | "hxx" | "hh" | "h++" | "h" | "cplusplus" => {
            Some("cpp")
        }
        other => registered_languages()
            .iter()
            .find(|info| info.key == other)
            .map(|info| info.key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_key_detection() {
        assert_eq!(
            language_key_for_path(Path::new("src/main.py")),
            Some("py".to_string())
        );
        assert_eq!(
            language_key_for_path(Path::new("src/component.jsx")),
            Some("js".to_string())
        );
        assert_eq!(
            language_key_for_path(Path::new("src/module.mjs")),
            Some("js".to_string())
        );
        assert_eq!(
            language_key_for_path(Path::new("src/module.cjs")),
            Some("js".to_string())
        );
        assert_eq!(
            language_key_for_path(Path::new("src/component.tsx")),
            Some("ts".to_string())
        );
        assert_eq!(language_key_for_path(Path::new("README")), None);
    }

    #[test]
    fn test_adapter_creation_supported_languages() {
        for lang in ["py", "js", "ts", "rs", "go", "cpp"] {
            let adapter = adapter_for_language(lang);
            assert!(adapter.is_ok(), "adapter for {} should be available", lang);
        }
    }

    #[test]
    fn test_adapter_creation_language_aliases() {
        for alias in ["python", "javascript", "typescript", "rust", "golang", "cplusplus"] {
            let adapter = adapter_for_language(alias);
            assert!(
                adapter.is_ok(),
                "adapter for alias {} should be available",
                alias
            );
        }
    }

    #[test]
    fn test_extension_support() {
        for ext in ["py", ".pyi", "JSX", "mjs", "TS", "tsx", "rs", "go", "cpp", "hpp", "cc"] {
            assert!(
                extension_is_supported(ext),
                "extension {} should be supported",
                ext
            );
        }
        assert!(!extension_is_supported("java"));
    }

    #[test]
    fn test_tree_sitter_functions() {
        // Test get_tree_sitter_language
        for lang in ["py", "rs", "js", "ts", "go", "cpp"] {
            let result = get_tree_sitter_language(lang);
            assert!(result.is_ok(), "Language {} should be supported", lang);
        }

        // Test create_parser_for_language
        for lang in ["py", "rs", "js", "ts", "go", "cpp"] {
            let result = create_parser_for_language(lang);
            assert!(result.is_ok(), "Should create parser for {}", lang);
        }

        // Test detect_language_from_path
        assert_eq!(detect_language_from_path("test.py"), "py");
        assert_eq!(detect_language_from_path("test.rs"), "rs");
        assert_eq!(detect_language_from_path("test.js"), "js");
        assert_eq!(detect_language_from_path("test.mjs"), "js");
        assert_eq!(detect_language_from_path("test.cjs"), "js");
        assert_eq!(detect_language_from_path("test.ts"), "ts");
        assert_eq!(detect_language_from_path("test.go"), "go");
        assert_eq!(detect_language_from_path("test.cpp"), "cpp");
        assert_eq!(detect_language_from_path("test.hpp"), "cpp");
    }
}
