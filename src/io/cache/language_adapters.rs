//! Language adapter implementations for AST analysis.

use std::collections::HashMap;

use crate::core::errors::Result;
use crate::lang::common::ParseIndex;

use super::types::{AstPattern, AstPatternType};

/// Language adapter trait for AST analysis.
///
/// Provides a unified interface for parsing source code and extracting
/// AST patterns across different programming languages. Each implementation
/// wraps a language-specific parser and adds pattern extraction capabilities.
pub trait LanguageAdapter: Send + Sync {
    /// Returns the name of the programming language this adapter handles.
    fn language_name(&self) -> &str;

    /// Parses source code and returns an index of parsed entities.
    ///
    /// # Arguments
    /// * `source_code` - The source code to parse
    /// * `file_path` - Path to the source file (used for error reporting)
    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex>;

    /// Extracts AST patterns from parsed code for similarity analysis.
    ///
    /// Patterns include node types, subtree signatures, and common token sequences
    /// that can be used to identify similar code structures across files.
    ///
    /// # Arguments
    /// * `parse_index` - The parsed entity index from `parse_source`
    /// * `source_code` - Original source code for token sequence extraction
    fn extract_ast_patterns(
        &self,
        parse_index: &ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>>;
}

/// Extract token sequence patterns from source code.
/// The `sanitize` closure transforms the sequence into a valid ID component.
fn extract_token_sequence_patterns<F>(
    source_code: &str,
    sequences: &[&str],
    language: &str,
    sanitize: F,
) -> Vec<AstPattern>
where
    F: Fn(&str) -> String,
{
    let mut patterns = Vec::new();
    for line in source_code.lines() {
        let line = line.trim();
        for sequence in sequences {
            if line.contains(sequence) {
                patterns.push(AstPattern {
                    id: format!("token_seq:{}", sanitize(sequence)),
                    pattern_type: AstPatternType::TokenSequence,
                    node_type: None,
                    subtree_signature: None,
                    token_sequence: Some(sequence.to_string()),
                    language: language.to_string(),
                    metadata: HashMap::new(),
                });
            }
        }
    }
    patterns
}

/// Python language adapter for AST pattern extraction.
///
/// Wraps the Python tree-sitter parser and provides pattern extraction
/// for Python-specific constructs like decorators, function parameters,
/// and common Python idioms.
pub struct PythonLanguageAdapter {
    adapter: crate::lang::python::PythonAdapter,
}

/// Python adapter constructor.
impl PythonLanguageAdapter {
    /// Creates a new Python language adapter.
    ///
    /// # Errors
    /// Returns an error if the tree-sitter Python parser fails to initialize.
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::python::PythonAdapter::new()?;
        Ok(Self { adapter })
    }
}

/// [`LanguageAdapter`] implementation for Python.
impl LanguageAdapter for PythonLanguageAdapter {
    /// Returns the language name ("python").
    fn language_name(&self) -> &str {
        "python"
    }

    /// Parses Python source code and returns a parse index.
    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

    /// Extracts AST patterns from parsed Python code.
    fn extract_ast_patterns(
        &self,
        parse_index: &ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        // Extract node type patterns from entities
        for (_id, entity) in &parse_index.entities {
            // Node type pattern
            let node_type = format!("{:?}", entity.kind);
            let node_pattern = AstPattern {
                id: format!("node_type:{}", node_type),
                pattern_type: AstPatternType::NodeType,
                node_type: Some(node_type),
                subtree_signature: None,
                token_sequence: None,
                language: "python".to_string(),
                metadata: HashMap::new(),
            };
            patterns.push(node_pattern);

            // Extract metadata-based patterns for Python-specific constructs
            if let Some(serde_json::Value::Bool(true)) = entity.metadata.get("has_decorators") {
                let decorator_pattern = AstPattern {
                    id: "decorator_usage".to_string(),
                    pattern_type: AstPatternType::FrameworkPattern,
                    node_type: None,
                    subtree_signature: Some("decorator_list".to_string()),
                    token_sequence: None,
                    language: "python".to_string(),
                    metadata: entity.metadata.clone(),
                };
                patterns.push(decorator_pattern);
            }

            // Extract function parameter patterns
            if let Some(serde_json::Value::Array(params)) = entity.metadata.get("parameters") {
                if !params.is_empty() {
                    let param_pattern = AstPattern {
                        id: format!("function_params:{}", params.len()),
                        pattern_type: AstPatternType::SubtreePattern,
                        node_type: None,
                        subtree_signature: Some(format!(
                            "function_definition->parameters[{}]",
                            params.len()
                        )),
                        token_sequence: None,
                        language: "python".to_string(),
                        metadata: HashMap::new(),
                    };
                    patterns.push(param_pattern);
                }
            }
        }

        // Extract token sequence patterns from source
        let token_patterns = self.extract_token_sequences(source_code)?;
        patterns.extend(token_patterns);

        Ok(patterns)
    }
}

/// Python-specific pattern extraction.
impl PythonLanguageAdapter {
    /// Extracts common Python token sequence patterns from source code.
    fn extract_token_sequences(&self, source_code: &str) -> Result<Vec<AstPattern>> {
        const COMMON_SEQUENCES: &[&str] = &[
            "if __name__ == \"__main__\":",
            "from typing import",
            "import os",
            "import sys",
            "def __init__(self",
            "self.",
            "return None",
            "raise ValueError",
            "except Exception",
            "with open(",
        ];
        Ok(extract_token_sequence_patterns(
            source_code,
            COMMON_SEQUENCES,
            "python",
            |s| s.replace(' ', "_"),
        ))
    }
}

/// JavaScript language adapter for AST pattern extraction.
///
/// Wraps the JavaScript tree-sitter parser and provides pattern extraction
/// for JavaScript-specific constructs including ES6+ features, async/await,
/// and common Node.js patterns.
pub struct JavaScriptLanguageAdapter {
    adapter: crate::lang::javascript::JavaScriptAdapter,
}

/// JavaScript adapter constructor.
impl JavaScriptLanguageAdapter {
    /// Creates a new JavaScript language adapter.
    ///
    /// # Errors
    /// Returns an error if the tree-sitter JavaScript parser fails to initialize.
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::javascript::JavaScriptAdapter::new()?;
        Ok(Self { adapter })
    }
}

/// [`LanguageAdapter`] implementation for JavaScript.
impl LanguageAdapter for JavaScriptLanguageAdapter {
    /// Returns the language name ("javascript").
    fn language_name(&self) -> &str {
        "javascript"
    }

    /// Parses JavaScript source code and returns a parse index.
    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

    /// Extracts AST patterns from parsed JavaScript code.
    fn extract_ast_patterns(
        &self,
        parse_index: &ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        // Extract entity-based patterns
        for (_id, entity) in &parse_index.entities {
            let node_type = format!("{:?}", entity.kind);
            let node_pattern = AstPattern {
                id: format!("node_type:{}", node_type),
                pattern_type: AstPatternType::NodeType,
                node_type: Some(node_type),
                subtree_signature: None,
                token_sequence: None,
                language: "javascript".to_string(),
                metadata: HashMap::new(),
            };
            patterns.push(node_pattern);
        }

        // JavaScript-specific token patterns
        let token_patterns = self.extract_js_token_sequences(source_code)?;
        patterns.extend(token_patterns);

        Ok(patterns)
    }
}

/// JavaScript-specific pattern extraction.
impl JavaScriptLanguageAdapter {
    /// Extracts common JavaScript token sequence patterns from source code.
    fn extract_js_token_sequences(&self, source_code: &str) -> Result<Vec<AstPattern>> {
        const COMMON_JS_SEQUENCES: &[&str] = &[
            "const ",
            "let ",
            "var ",
            "function(",
            "() => {",
            "require(",
            "module.exports",
            "console.log(",
            "JSON.stringify(",
            "JSON.parse(",
            ".then(",
            ".catch(",
            "async ",
            "await ",
        ];
        Ok(extract_token_sequence_patterns(
            source_code,
            COMMON_JS_SEQUENCES,
            "javascript",
            |s| s.replace(' ', "_").replace('(', "").replace(')', ""),
        ))
    }
}

/// TypeScript language adapter for AST pattern extraction.
///
/// Wraps the TypeScript tree-sitter parser and provides pattern extraction
/// for TypeScript-specific constructs including type annotations, interfaces,
/// enums, and access modifiers.
pub struct TypeScriptLanguageAdapter {
    adapter: crate::lang::typescript::TypeScriptAdapter,
}

/// TypeScript adapter constructor.
impl TypeScriptLanguageAdapter {
    /// Creates a new TypeScript language adapter.
    ///
    /// # Errors
    /// Returns an error if the tree-sitter TypeScript parser fails to initialize.
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::typescript::TypeScriptAdapter::new()?;
        Ok(Self { adapter })
    }
}

/// [`LanguageAdapter`] implementation for TypeScript.
impl LanguageAdapter for TypeScriptLanguageAdapter {
    /// Returns the language name ("typescript").
    fn language_name(&self) -> &str {
        "typescript"
    }

    /// Parses TypeScript source code and returns a parse index.
    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

    /// Extracts AST patterns from parsed TypeScript code.
    fn extract_ast_patterns(
        &self,
        parse_index: &ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        // Extract entity-based patterns
        for (_id, entity) in &parse_index.entities {
            let node_type = format!("{:?}", entity.kind);
            let node_pattern = AstPattern {
                id: format!("node_type:{}", node_type),
                pattern_type: AstPatternType::NodeType,
                node_type: Some(node_type),
                subtree_signature: None,
                token_sequence: None,
                language: "typescript".to_string(),
                metadata: HashMap::new(),
            };
            patterns.push(node_pattern);
        }

        // TypeScript-specific patterns
        let token_patterns = self.extract_ts_token_sequences(source_code)?;
        patterns.extend(token_patterns);

        Ok(patterns)
    }
}

/// TypeScript-specific pattern extraction.
impl TypeScriptLanguageAdapter {
    /// Extracts common TypeScript token sequence patterns from source code.
    fn extract_ts_token_sequences(&self, source_code: &str) -> Result<Vec<AstPattern>> {
        const COMMON_TS_SEQUENCES: &[&str] = &[
            ": string",
            ": number",
            ": boolean",
            ": void",
            "interface ",
            "type ",
            "enum ",
            "export ",
            "import ",
            "extends ",
            "implements ",
            "public ",
            "private ",
            "protected ",
            "readonly ",
            "as ",
        ];
        Ok(extract_token_sequence_patterns(
            source_code,
            COMMON_TS_SEQUENCES,
            "typescript",
            |s| s.replace(' ', "_"),
        ))
    }
}

/// Rust language adapter for AST pattern extraction.
///
/// Wraps the Rust tree-sitter parser and provides pattern extraction
/// for Rust-specific constructs including ownership patterns, Result/Option
/// handling, and common Rust idioms.
pub struct RustLanguageAdapter {
    adapter: crate::lang::rust_lang::RustAdapter,
}

/// Rust adapter constructor.
impl RustLanguageAdapter {
    /// Creates a new Rust language adapter.
    ///
    /// # Errors
    /// Returns an error if the tree-sitter Rust parser fails to initialize.
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::rust_lang::RustAdapter::new()?;
        Ok(Self { adapter })
    }
}

/// [`LanguageAdapter`] implementation for Rust.
impl LanguageAdapter for RustLanguageAdapter {
    /// Returns the language name ("rust").
    fn language_name(&self) -> &str {
        "rust"
    }

    /// Parses Rust source code and returns a parse index.
    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

    /// Extracts AST patterns from parsed Rust code.
    fn extract_ast_patterns(
        &self,
        parse_index: &ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        for (_id, entity) in &parse_index.entities {
            let node_type = format!("{:?}", entity.kind);
            let node_pattern = AstPattern {
                id: format!("node_type:{}", node_type),
                pattern_type: AstPatternType::NodeType,
                node_type: Some(node_type),
                subtree_signature: None,
                token_sequence: None,
                language: "rust".to_string(),
                metadata: HashMap::new(),
            };
            patterns.push(node_pattern);
        }

        let token_patterns = self.extract_rust_token_sequences(source_code)?;
        patterns.extend(token_patterns);

        Ok(patterns)
    }
}

/// Rust-specific pattern extraction.
impl RustLanguageAdapter {
    /// Extracts common Rust token sequence patterns from source code.
    fn extract_rust_token_sequences(&self, source_code: &str) -> Result<Vec<AstPattern>> {
        const COMMON_RUST_SEQUENCES: &[&str] = &[
            "use ", "pub ", "fn ", "struct ", "enum ", "impl ", "trait ", "let ", "mut ",
            "&self", "&mut self", "Result<", "Option<", "Vec<", "HashMap<", "println!",
            "eprintln!", "dbg!", ".unwrap()", ".expect(", "match ", "if let", "Some(",
            "None", "Ok(", "Err(",
        ];
        Ok(extract_token_sequence_patterns(
            source_code,
            COMMON_RUST_SEQUENCES,
            "rust",
            |s| s.replace(' ', "_").replace('<', "").replace('(', ""),
        ))
    }
}

/// Go language adapter for AST pattern extraction.
///
/// Wraps the Go tree-sitter parser and provides pattern extraction
/// for Go-specific constructs including goroutines, channels, defer
/// statements, and common Go error handling patterns.
pub struct GoLanguageAdapter {
    adapter: crate::lang::go::GoAdapter,
}

/// Go adapter constructor.
impl GoLanguageAdapter {
    /// Creates a new Go language adapter.
    ///
    /// # Errors
    /// Returns an error if the tree-sitter Go parser fails to initialize.
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::go::GoAdapter::new()?;
        Ok(Self { adapter })
    }
}

/// [`LanguageAdapter`] implementation for Go.
impl LanguageAdapter for GoLanguageAdapter {
    /// Returns the language name ("go").
    fn language_name(&self) -> &str {
        "go"
    }

    /// Parses Go source code and returns a parse index.
    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

    /// Extracts AST patterns from parsed Go code.
    fn extract_ast_patterns(
        &self,
        parse_index: &ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        for (_id, entity) in &parse_index.entities {
            let node_type = format!("{:?}", entity.kind);
            let node_pattern = AstPattern {
                id: format!("node_type:{}", node_type),
                pattern_type: AstPatternType::NodeType,
                node_type: Some(node_type),
                subtree_signature: None,
                token_sequence: None,
                language: "go".to_string(),
                metadata: HashMap::new(),
            };
            patterns.push(node_pattern);
        }

        let token_patterns = self.extract_go_token_sequences(source_code)?;
        patterns.extend(token_patterns);

        Ok(patterns)
    }
}

/// Go-specific pattern extraction.
impl GoLanguageAdapter {
    /// Extracts common Go token sequence patterns from source code.
    fn extract_go_token_sequences(&self, source_code: &str) -> Result<Vec<AstPattern>> {
        const COMMON_GO_SEQUENCES: &[&str] = &[
            "package ",
            "import ",
            "func ",
            "var ",
            "const ",
            "type ",
            "struct {",
            "interface {",
            "if err != nil",
            "return ",
            "fmt.Println(",
            "fmt.Printf(",
            "log.Fatal(",
            "make(",
            "append(",
            "len(",
            "cap(",
            ":= ",
            "go ",
            "defer ",
            "chan ",
            "select {",
            "for ",
            "range ",
        ];
        Ok(extract_token_sequence_patterns(
            source_code,
            COMMON_GO_SEQUENCES,
            "go",
            |s| s.replace(' ', "_").replace('{', "").replace('(', ""),
        ))
    }
}
