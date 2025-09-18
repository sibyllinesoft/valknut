//! Factory utilities for working with language adapters based on file extensions.

use std::path::Path;

use crate::core::errors::{Result, ValknutError};
use crate::lang::common::LanguageAdapter;
use crate::lang::python::PythonAdapter;

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
        "py" => Ok(Box::new(PythonAdapter::new()?)),
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
        let python = adapter_for_language("py");
        assert!(python.is_ok());
    }

    #[test]
    fn test_adapter_creation_unknown_language() {
        for lang in ["js", "ts", "rs", "go", "unknown"] {
            let adapter = adapter_for_language(lang);
            assert!(adapter.is_err());
        }
    }
}
