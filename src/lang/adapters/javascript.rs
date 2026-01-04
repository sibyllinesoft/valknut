//! JavaScript language adapter with tree-sitter integration.

use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Tree};

use super::super::common::{
    create_base_metadata, extract_identifiers_by_kinds, extract_js_function_calls,
    generate_entity_id, normalize_module_literal, parse_require_import, sort_and_dedup, EntityKind,
    EntityExtractor, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation,
};
use super::super::registry::{create_parser_for_language, get_tree_sitter_language};
use crate::core::ast_utils::{
    extract_parameter_names, extract_variable_declarator_name, find_child_text, is_const_declaration,
};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::CodeEntity;
use crate::detectors::structure::config::ImportStatement;

#[cfg(test)]
#[path = "javascript_tests.rs"]
mod tests;

/// JavaScript-specific parsing and analysis
pub struct JavaScriptAdapter {
    /// Tree-sitter parser for JavaScript
    parser: Parser,

    /// Language instance
    language: Language,
}

/// Parsing and entity extraction methods for [`JavaScriptAdapter`].
impl JavaScriptAdapter {
    /// Create a new JavaScript adapter
    pub fn new() -> Result<Self> {
        let language = get_tree_sitter_language("js")?;
        let parser = create_parser_for_language("js")?;

        Ok(Self { parser, language })
    }

    /// Parse JavaScript source code and extract entities
    pub fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self.parser.parse(source_code, None).ok_or_else(|| {
            ValknutError::parse("javascript", "Failed to parse JavaScript source code")
        })?;

        let mut index = ParseIndex::new();
        let mut entity_id_counter = 0;

        // Walk the tree and extract entities
        self.extract_entities_recursive(
            tree.root_node(),
            source_code,
            file_path,
            None,
            &mut index,
            &mut entity_id_counter,
        )?;

        Ok(index)
    }

    /// Extract entities from JavaScript code and convert to CodeEntity format
    pub fn extract_code_entities(
        &mut self,
        source_code: &str,
        file_path: &str,
    ) -> Result<Vec<CodeEntity>> {
        let parse_index = self.parse_source(source_code, file_path)?;
        let mut code_entities = Vec::new();

        for entity in parse_index.entities.values() {
            let code_entity = entity.to_code_entity(source_code);
            code_entities.push(code_entity);
        }

        Ok(code_entities)
    }

    /// Determine entity kind from node kind, returning None for non-entity nodes.
    fn determine_entity_kind(&self, node: &Node, source_code: &str) -> Result<Option<EntityKind>> {
        Ok(match node.kind() {
            "function_declaration" | "function_expression" | "arrow_function" => Some(EntityKind::Function),
            "method_definition" => Some(EntityKind::Method),
            "class_declaration" => Some(EntityKind::Class),
            "variable_declaration" | "lexical_declaration" => {
                Some(if is_const_declaration(node, source_code)? {
                    EntityKind::Constant
                } else {
                    EntityKind::Variable
                })
            }
            _ => None,
        })
    }

    /// Extract the name of an entity from its AST node
    fn extract_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        match node.kind() {
            "function_declaration" | "class_declaration" | "function_expression" | "arrow_function" => {
                find_child_text(node, source_code, &["identifier"])
            }
            "method_definition" => {
                find_child_text(node, source_code, &["property_identifier", "identifier"])
            }
            "variable_declaration" | "lexical_declaration" => {
                extract_variable_declarator_name(node, source_code)
            }
            _ => find_child_text(node, source_code, &["identifier", "property_identifier"]),
        }
    }

    /// Extract function-specific metadata
    fn extract_function_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut parameters = Vec::new();
        let mut is_async = false;
        let mut is_generator = false;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "formal_parameters" => {
                    parameters = extract_parameter_names(&child, source_code);
                }
                "async" => is_async = true,
                "*" => is_generator = true,
                _ => {}
            }
        }

        metadata.insert("parameters".to_string(), serde_json::json!(parameters));
        metadata.insert("is_async".to_string(), serde_json::Value::Bool(is_async));
        metadata.insert(
            "is_generator".to_string(),
            serde_json::Value::Bool(is_generator),
        );

        Ok(())
    }

    /// Extract the extends class name from a class_heritage node.
    fn extract_extends_class(heritage_node: &Node, source_code: &str) -> Option<String> {
        let mut cursor = heritage_node.walk();
        for child in heritage_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return child.utf8_text(source_code.as_bytes()).ok().map(String::from);
            }
        }
        None
    }

    /// Extract class-specific metadata
    fn extract_class_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();

        let extends_class = node
            .children(&mut cursor)
            .find(|child| child.kind() == "class_heritage")
            .and_then(|heritage| Self::extract_extends_class(&heritage, source_code));

        if let Some(extends) = extends_class {
            metadata.insert("extends".to_string(), serde_json::Value::String(extends));
        }

        Ok(())
    }
}

/// [`LanguageAdapter`] implementation for JavaScript source code.
impl LanguageAdapter for JavaScriptAdapter {
    /// Parses source code into a tree-sitter AST.
    fn parse_tree(&mut self, source: &str) -> Result<Tree> {
        self.parser
            .parse(source, None)
            .ok_or_else(|| ValknutError::parse("javascript", "Failed to parse JavaScript source"))
    }

    /// Parses JavaScript source code and returns a parse index.
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        JavaScriptAdapter::parse_source(self, source, file_path)
    }

    /// Extracts all function and constructor call targets.
    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        Ok(extract_js_function_calls(tree.root_node(), source))
    }

    /// Extracts all identifier tokens from the source.
    fn extract_identifiers(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        Ok(extract_identifiers_by_kinds(
            tree.root_node(),
            source,
            &["identifier", "shorthand_property_identifier", "property_identifier"],
        ))
    }

    /// Counts distinct code blocks in the source.
    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize> {
        let index = JavaScriptAdapter::parse_source(self, source, "<memory>")?;
        Ok(index.count_distinct_blocks())
    }

    /// Returns the language name ("javascript").
    fn language_name(&self) -> &str {
        "javascript"
    }

    /// Extracts import and require statements from JavaScript source.
    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        Ok(crate::lang::common::extract_imports_common(source, "default as "))
    }

    /// Extracts code entities from JavaScript source code.
    fn extract_code_entities(
        &mut self,
        source: &str,
        file_path: &str,
    ) -> Result<Vec<crate::core::featureset::CodeEntity>> {
        JavaScriptAdapter::extract_code_entities(self, source, file_path)
    }
}

/// [`EntityExtractor`] implementation providing the language-specific node conversion.
impl EntityExtractor for JavaScriptAdapter {
    fn node_to_entity(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<String>,
        entity_id_counter: &mut usize,
    ) -> Result<Option<ParsedEntity>> {
        let entity_kind = match self.determine_entity_kind(&node, source_code)? {
            Some(kind) => kind,
            None => return Ok(None),
        };

        let name = self.extract_name(&node, source_code)?
            .unwrap_or_else(|| entity_kind.fallback_name(*entity_id_counter));

        *entity_id_counter += 1;
        let entity_id = generate_entity_id(file_path, entity_kind, *entity_id_counter);

        let location = SourceLocation::from_positions(
            file_path,
            node.start_position().row,
            node.start_position().column,
            node.end_position().row,
            node.end_position().column,
        );

        let mut metadata = create_base_metadata(node.kind(), node.start_byte(), node.end_byte());

        match entity_kind {
            EntityKind::Function | EntityKind::Method => {
                self.extract_function_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Class => {
                self.extract_class_metadata(&node, source_code, &mut metadata)?;
            }
            _ => {}
        }

        Ok(Some(ParsedEntity {
            id: entity_id,
            kind: entity_kind,
            name,
            parent: parent_id,
            children: Vec::new(),
            location,
            metadata,
        }))
    }
}

/// Default implementation for [`JavaScriptAdapter`].
impl Default for JavaScriptAdapter {
    /// Returns a new JavaScript adapter, or a minimal fallback on failure.
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!(
                "Warning: Failed to create JavaScript adapter, using minimal fallback: {}",
                e
            );
            JavaScriptAdapter {
                parser: tree_sitter::Parser::new(),
                language: get_tree_sitter_language("js")
                    .unwrap_or_else(|_| tree_sitter_javascript::LANGUAGE.into()),
            }
        })
    }
}
