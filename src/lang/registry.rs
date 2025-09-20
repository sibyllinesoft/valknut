//! Factory utilities for working with language adapters based on file extensions.

use std::path::Path;

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
        "jsx" | "js" => "js", // tree-sitter javascript
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
}
