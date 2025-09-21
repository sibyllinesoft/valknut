//! Factory utilities for working with language adapters based on file extensions.

use std::path::Path;
use tree_sitter::Language;

use crate::core::errors::{Result, ValknutError};
use crate::lang::common::LanguageAdapter;
use crate::lang::go::GoAdapter;
use crate::lang::javascript::JavaScriptAdapter;
use crate::lang::python::PythonAdapter;
use crate::lang::rust_lang::RustAdapter;
use crate::lang::typescript::TypeScriptAdapter;

/// Identify the canonical language key for a file path.
pub fn language_key_for_path(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    if ext.is_empty() {
        return None;
    }

    // Normalise TypeScript/JavaScript extensions that have multiple variants.
    let key = match ext.as_str() {
        "jsx" | "js" | "mjs" | "cjs" => "js", // tree-sitter javascript (includes ES modules and CommonJS)
        "tsx" | "ts" => "ts", // tree-sitter typescript
        other => other,
    };

    Some(key.to_string())
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
    match language {
        "py" | "python" => Ok(Box::new(PythonAdapter::new()?)),
        "js" | "jsx" | "javascript" => Ok(Box::new(JavaScriptAdapter::new()?)),
        "ts" | "tsx" | "typescript" => Ok(Box::new(TypeScriptAdapter::new()?)),
        "rs" | "rust" => Ok(Box::new(RustAdapter::new()?)),
        "go" | "golang" => Ok(Box::new(GoAdapter::new()?)),
        other => Err(ValknutError::unsupported(format!(
            "Language adapter for '{}' is not yet implemented",
            other
        ))),
    }
}

/// Get tree-sitter language for a given language key
pub fn get_tree_sitter_language(language_key: &str) -> Result<Language> {
    match language_key {
        "py" | "pyw" => Ok(tree_sitter_python::LANGUAGE.into()),
        "rs" => Ok(tree_sitter_rust::LANGUAGE.into()),
        "js" | "jsx" | "mjs" | "cjs" => Ok(tree_sitter_javascript::LANGUAGE.into()),
        "ts" | "tsx" => Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "go" => Ok(tree_sitter_go::LANGUAGE.into()),
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
        .unwrap_or("txt")
        .to_string()
}

/// Create a new parser for the given language
pub fn create_parser_for_language(language_key: &str) -> Result<tree_sitter::Parser> {
    let mut parser = tree_sitter::Parser::new();
    let tree_sitter_language = get_tree_sitter_language(language_key)?;
    parser.set_language(&tree_sitter_language).map_err(|e| {
        ValknutError::parse(language_key, format!("Failed to set parser language: {}", e))
    })?;
    Ok(parser)
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
        for lang in ["py", "js", "ts", "rs", "go"] {
            let adapter = adapter_for_language(lang);
            assert!(adapter.is_ok(), "adapter for {} should be available", lang);
        }
    }

    #[test]
    fn test_adapter_creation_language_aliases() {
        for alias in ["python", "javascript", "typescript", "rust", "golang"] {
            let adapter = adapter_for_language(alias);
            assert!(
                adapter.is_ok(),
                "adapter for alias {} should be available",
                alias
            );
        }
    }

    #[test]
    fn test_adapter_creation_unknown_language() {
        let adapter = adapter_for_language("unknown");
        assert!(adapter.is_err());
    }

    #[test]
    fn test_tree_sitter_functions() {
        // Test get_tree_sitter_language
        for lang in ["py", "rs", "js", "ts", "go"] {
            let result = get_tree_sitter_language(lang);
            assert!(result.is_ok(), "Language {} should be supported", lang);
        }

        // Test create_parser_for_language
        for lang in ["py", "rs", "js", "ts", "go"] {
            let result = create_parser_for_language(lang);
            assert!(result.is_ok(), "Should create parser for {}", lang);
        }

        // Test detect_language_from_path
        assert_eq!(detect_language_from_path("test.py"), "py");
        assert_eq!(detect_language_from_path("test.rs"), "rs");
        assert_eq!(detect_language_from_path("test.js"), "js");
        assert_eq!(detect_language_from_path("test.mjs"), "mjs");
        assert_eq!(detect_language_from_path("test.cjs"), "cjs");
        assert_eq!(detect_language_from_path("test.ts"), "ts");
        assert_eq!(detect_language_from_path("test.go"), "go");
    }
}
