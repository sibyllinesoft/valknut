//! TypeScript language adapter with tree-sitter integration.

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
#[path = "typescript_tests.rs"]
mod tests;

/// TypeScript-specific parsing and analysis
pub struct TypeScriptAdapter {
    /// Tree-sitter parser for TypeScript
    parser: Parser,

    /// Language instance
    language: Language,
}

/// Parsing and entity extraction methods for [`TypeScriptAdapter`].
impl TypeScriptAdapter {
    /// Create a new TypeScript adapter
    pub fn new() -> Result<Self> {
        let language = get_tree_sitter_language("ts")?;
        let parser = create_parser_for_language("ts")?;

        Ok(Self { parser, language })
    }

    /// Parse TypeScript source code and extract entities
    pub fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self.parser.parse(source_code, None).ok_or_else(|| {
            ValknutError::parse("typescript", "Failed to parse TypeScript source code")
        })?;

        let mut index = ParseIndex::new();
        let mut entity_id_counter = 0;

        // Walk the tree and extract entities (iterative to avoid stack overflow)
        self.extract_entities_iterative(
            tree.root_node(),
            source_code,
            file_path,
            &mut index,
            &mut entity_id_counter,
        )?;

        Ok(index)
    }

    /// Extract entities from TypeScript code and convert to CodeEntity format
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
            "interface_declaration" | "type_alias_declaration" => Some(EntityKind::Interface),
            "enum_declaration" => Some(EntityKind::Enum),
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
            "function_declaration"
            | "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "type_alias_declaration" => {
                find_child_text(node, source_code, &["type_identifier", "identifier"])
            }
            "method_definition" => {
                find_child_text(node, source_code, &["property_identifier", "identifier"])
            }
            "function_expression" | "arrow_function" => Ok(Some("<anonymous>".to_string())),
            "variable_declaration" | "lexical_declaration" => {
                extract_variable_declarator_name(node, source_code)
            }
            _ => Ok(None),
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
        let mut return_type = None;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "formal_parameters" => {
                    parameters = extract_parameter_names(&child, source_code);
                }
                "async" => is_async = true,
                "*" => is_generator = true,
                "type_annotation" => {
                    return_type = child.utf8_text(source_code.as_bytes()).ok().map(String::from);
                }
                _ => {}
            }
        }

        metadata.insert("parameters".to_string(), serde_json::json!(parameters));
        metadata.insert("is_async".to_string(), serde_json::Value::Bool(is_async));
        metadata.insert(
            "is_generator".to_string(),
            serde_json::Value::Bool(is_generator),
        );
        if let Some(ret_type) = return_type {
            metadata.insert(
                "return_type".to_string(),
                serde_json::Value::String(ret_type),
            );
        }

        Ok(())
    }

    /// Extract class-specific metadata
    fn extract_class_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut extends_class = None;
        let mut implements = Vec::new();
        let mut is_abstract = false;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "class_heritage" => {
                    let (extends, impls) = self.parse_class_heritage(&child, source_code)?;
                    extends_class = extends;
                    implements = impls;
                }
                "abstract" => is_abstract = true,
                _ => {}
            }
        }

        if let Some(extends) = extends_class {
            metadata.insert("extends".to_string(), serde_json::Value::String(extends));
        }
        if !implements.is_empty() {
            metadata.insert("implements".to_string(), serde_json::json!(implements));
        }
        metadata.insert("is_abstract".to_string(), serde_json::Value::Bool(is_abstract));

        Ok(())
    }

    /// Parse class heritage to extract extends and implements
    fn parse_class_heritage(
        &self,
        heritage_node: &Node,
        source_code: &str,
    ) -> Result<(Option<String>, Vec<String>)> {
        let mut extends_class = None;
        let mut implements = Vec::new();
        let mut cursor = heritage_node.walk();

        for child in heritage_node.children(&mut cursor) {
            match child.kind() {
                "extends_clause" => {
                    extends_class = self.extract_first_type_identifier(&child, source_code)?;
                }
                "implements_clause" => {
                    implements = self.extract_type_identifiers(&child, source_code)?;
                }
                _ => {}
            }
        }

        Ok((extends_class, implements))
    }

    /// Extract the first type identifier from a clause node
    fn extract_first_type_identifier(&self, clause: &Node, source_code: &str) -> Result<Option<String>> {
        let mut cursor = clause.walk();
        for child in clause.children(&mut cursor) {
            if child.kind() == "type_identifier" || child.kind() == "identifier" {
                return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
            }
        }
        Ok(None)
    }

    /// Extract all type identifiers from a clause node
    fn extract_type_identifiers(&self, clause: &Node, source_code: &str) -> Result<Vec<String>> {
        let mut identifiers = Vec::new();
        let mut cursor = clause.walk();

        for child in clause.children(&mut cursor) {
            if child.kind() == "type_identifier" || child.kind() == "identifier" {
                identifiers.push(child.utf8_text(source_code.as_bytes())?.to_string());
            }
        }

        Ok(identifiers)
    }

    /// Extract interface-specific metadata
    fn extract_interface_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut extends_interfaces = Vec::new();

        for child in node.children(&mut cursor) {
            if child.kind() == "extends_clause" {
                extends_interfaces = self.extract_type_identifiers(&child, source_code)?;
            }
        }

        if !extends_interfaces.is_empty() {
            metadata.insert("extends".to_string(), serde_json::json!(extends_interfaces));
        }

        Ok(())
    }

    /// Extract enum-specific metadata
    fn extract_enum_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut enum_members = Vec::new();
        let mut is_const_enum = false;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "enum_body" => {
                    let mut body_cursor = child.walk();
                    for body_child in child.children(&mut body_cursor) {
                        if body_child.kind() == "property_identifier"
                            || body_child.kind() == "identifier"
                        {
                            enum_members
                                .push(body_child.utf8_text(source_code.as_bytes())?.to_string());
                        }
                    }
                }
                "const" => {
                    is_const_enum = true;
                }
                _ => {}
            }
        }

        metadata.insert("members".to_string(), serde_json::json!(enum_members));
        metadata.insert(
            "is_const".to_string(),
            serde_json::Value::Bool(is_const_enum),
        );

        Ok(())
    }
}

/// [`LanguageAdapter`] implementation for TypeScript source code.
impl LanguageAdapter for TypeScriptAdapter {
    /// Parses source code into a tree-sitter AST.
    fn parse_tree(&mut self, source: &str) -> Result<Tree> {
        self.parser
            .parse(source, None)
            .ok_or_else(|| ValknutError::parse("typescript", "Failed to parse TypeScript source"))
    }

    /// Parses TypeScript source code and returns a parse index.
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        TypeScriptAdapter::parse_source(self, source, file_path)
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
            &[
                "identifier",
                "type_identifier",
                "shorthand_property_identifier",
                "property_identifier",
                "namespace_identifier",
            ],
        ))
    }

    /// Counts distinct code blocks in the source.
    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize> {
        let index = TypeScriptAdapter::parse_source(self, source, "<memory>")?;
        Ok(index.count_distinct_blocks())
    }

    /// Returns the language name ("typescript").
    fn language_name(&self) -> &str {
        "typescript"
    }

    /// Extracts import and require statements from TypeScript source.
    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        Ok(crate::lang::common::extract_imports_common(source, "type "))
    }

    /// Extracts code entities from TypeScript source code.
    fn extract_code_entities(
        &mut self,
        source: &str,
        file_path: &str,
    ) -> Result<Vec<crate::core::featureset::CodeEntity>> {
        TypeScriptAdapter::extract_code_entities(self, source, file_path)
    }
}

/// [`EntityExtractor`] implementation providing the language-specific node conversion.
impl EntityExtractor for TypeScriptAdapter {
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
        let location = create_source_location(file_path, &node);
        let mut metadata = create_base_metadata(node.kind(), node.start_byte(), node.end_byte());

        self.extract_entity_metadata(entity_kind, &node, source_code, &mut metadata)?;

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

/// Create source location from a tree-sitter node.
fn create_source_location(file_path: &str, node: &Node) -> SourceLocation {
    SourceLocation::from_positions(
        file_path,
        node.start_position().row,
        node.start_position().column,
        node.end_position().row,
        node.end_position().column,
    )
}

/// Entity metadata extraction dispatch for TypeScriptAdapter.
impl TypeScriptAdapter {
    fn extract_entity_metadata(
        &self,
        kind: EntityKind,
        node: &Node,
        source_code: &str,
        metadata: &mut std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        match kind {
            EntityKind::Function | EntityKind::Method => {
                self.extract_function_metadata(node, source_code, metadata)?;
            }
            EntityKind::Class => {
                self.extract_class_metadata(node, source_code, metadata)?;
            }
            EntityKind::Interface => {
                self.extract_interface_metadata(node, source_code, metadata)?;
            }
            EntityKind::Enum => {
                self.extract_enum_metadata(node, source_code, metadata)?;
            }
            _ => {}
        }
        Ok(())
    }
}

/// Default implementation for [`TypeScriptAdapter`].
impl Default for TypeScriptAdapter {
    /// Returns a new TypeScript adapter, or a minimal fallback on failure.
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!(
                "Warning: Failed to create TypeScript adapter, using minimal fallback: {}",
                e
            );
            TypeScriptAdapter {
                parser: tree_sitter::Parser::new(),
                language: get_tree_sitter_language("ts")
                    .unwrap_or_else(|_| tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            }
        })
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;

    #[test]
    fn test_typescript_import_extraction() {
        let mut adapter = TypeScriptAdapter::new().unwrap();
        let source = r#"
import React from 'react';
import { useState, useEffect } from 'react';
import * as utils from './utils';
import type { Config } from '../types';
import { foo, bar, baz } from '@/components';
const legacy = require('./legacy');
"#;
        let imports = adapter.extract_imports(source).unwrap();
        
        let modules: Vec<&str> = imports.iter().map(|i| i.module.as_str()).collect();
        
        assert!(modules.contains(&"react"), "Should find default import from 'react'");
        assert!(modules.contains(&"./utils"), "Should find star import from './utils'");
        assert!(modules.contains(&"../types"), "Should find type import from '../types'");
        assert!(modules.contains(&"@/components"), "Should find named import from '@/components'");
        assert!(modules.contains(&"./legacy"), "Should find require('./legacy')");
        
        // Check named imports
        let react_named = imports.iter().find(|i| i.module == "react" && i.import_type == "named").unwrap();
        assert!(react_named.imports.as_ref().unwrap().contains(&"useState".to_string()));
        assert!(react_named.imports.as_ref().unwrap().contains(&"useEffect".to_string()));
    }
}
