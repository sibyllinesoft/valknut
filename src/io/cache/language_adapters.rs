//! Language adapter implementations for AST analysis.

use std::collections::HashMap;

use crate::core::errors::Result;
use crate::lang::common::ParseIndex;

use super::types::{AstPattern, AstPatternType};

/// Language adapter trait for AST analysis
pub trait LanguageAdapter: Send + Sync {
    fn language_name(&self) -> &str;
    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex>;
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

/// Python language adapter implementation
pub struct PythonLanguageAdapter {
    adapter: crate::lang::python::PythonAdapter,
}

impl PythonLanguageAdapter {
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::python::PythonAdapter::new()?;
        Ok(Self { adapter })
    }
}

impl LanguageAdapter for PythonLanguageAdapter {
    fn language_name(&self) -> &str {
        "python"
    }

    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

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

impl PythonLanguageAdapter {
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

/// JavaScript language adapter implementation
pub struct JavaScriptLanguageAdapter {
    adapter: crate::lang::javascript::JavaScriptAdapter,
}

impl JavaScriptLanguageAdapter {
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::javascript::JavaScriptAdapter::new()?;
        Ok(Self { adapter })
    }
}

impl LanguageAdapter for JavaScriptLanguageAdapter {
    fn language_name(&self) -> &str {
        "javascript"
    }

    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

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

impl JavaScriptLanguageAdapter {
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

/// TypeScript language adapter implementation
pub struct TypeScriptLanguageAdapter {
    adapter: crate::lang::typescript::TypeScriptAdapter,
}

impl TypeScriptLanguageAdapter {
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::typescript::TypeScriptAdapter::new()?;
        Ok(Self { adapter })
    }
}

impl LanguageAdapter for TypeScriptLanguageAdapter {
    fn language_name(&self) -> &str {
        "typescript"
    }

    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

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

impl TypeScriptLanguageAdapter {
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

/// Rust language adapter implementation
pub struct RustLanguageAdapter {
    adapter: crate::lang::rust_lang::RustAdapter,
}

impl RustLanguageAdapter {
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::rust_lang::RustAdapter::new()?;
        Ok(Self { adapter })
    }
}

impl LanguageAdapter for RustLanguageAdapter {
    fn language_name(&self) -> &str {
        "rust"
    }

    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

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

impl RustLanguageAdapter {
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

/// Go language adapter implementation
pub struct GoLanguageAdapter {
    adapter: crate::lang::go::GoAdapter,
}

impl GoLanguageAdapter {
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::go::GoAdapter::new()?;
        Ok(Self { adapter })
    }
}

impl LanguageAdapter for GoLanguageAdapter {
    fn language_name(&self) -> &str {
        "go"
    }

    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

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

impl GoLanguageAdapter {
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
