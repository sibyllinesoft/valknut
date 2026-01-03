//! TypeScript language adapter with tree-sitter integration.

use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Tree};

use super::super::common::{
    normalize_module_literal, EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation,
};
use super::super::registry::{create_parser_for_language, get_tree_sitter_language};
use crate::core::ast_utils::{node_text_normalized, walk_tree};
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

    /// Parses source code into a tree-sitter AST.
    fn parse_tree(&mut self, source_code: &str) -> Result<Tree> {
        self.parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("typescript", "Failed to parse TypeScript source"))
    }

    /// Walks the AST nodes, invoking the callback on each node.
    fn walk_tree<F>(node: Node, callback: &mut F)
    where
        F: FnMut(Node),
    {
        walk_tree(node, callback);
    }

    /// Extracts and normalizes text from an AST node.
    fn node_text(node: &Node, source_code: &str) -> Result<String> {
        node_text_normalized(node, source_code)
    }

    /// Parse TypeScript source code and extract entities
    pub fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self.parser.parse(source_code, None).ok_or_else(|| {
            ValknutError::parse("typescript", "Failed to parse TypeScript source code")
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

    /// Extract entities from TypeScript code and convert to CodeEntity format
    pub fn extract_code_entities(
        &mut self,
        source_code: &str,
        file_path: &str,
    ) -> Result<Vec<CodeEntity>> {
        let parse_index = self.parse_source(source_code, file_path)?;
        let mut code_entities = Vec::new();

        for entity in parse_index.entities.values() {
            let code_entity = self.convert_to_code_entity(entity, source_code)?;
            code_entities.push(code_entity);
        }

        Ok(code_entities)
    }

    /// Recursively extract entities from the AST
    fn extract_entities_recursive(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<String>,
        index: &mut ParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
        // Check if this node represents an entity we care about
        if let Some(entity) = self.node_to_entity(
            node,
            source_code,
            file_path,
            parent_id.clone(),
            entity_id_counter,
        )? {
            let entity_id = entity.id.clone();
            index.add_entity(entity);
            self.traverse_children(node, source_code, file_path, Some(entity_id), index, entity_id_counter)?;
        } else {
            self.traverse_children(node, source_code, file_path, parent_id, index, entity_id_counter)?;
        }
        Ok(())
    }

    /// Traverse and process all child nodes recursively.
    fn traverse_children(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<String>,
        index: &mut ParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_entities_recursive(
                child,
                source_code,
                file_path,
                parent_id.clone(),
                index,
                entity_id_counter,
            )?;
        }
        Ok(())
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
                Some(if self.is_const_declaration(node, source_code)? {
                    EntityKind::Constant
                } else {
                    EntityKind::Variable
                })
            }
            _ => None,
        })
    }

    /// Convert a tree-sitter node to a ParsedEntity if it represents an entity
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
        let entity_id = format!("{}:{}:{}", file_path, entity_kind as u8, *entity_id_counter);

        let location = SourceLocation {
            file_path: file_path.to_string(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            start_column: node.start_position().column + 1,
            end_column: node.end_position().column + 1,
        };

        let mut metadata = HashMap::new();

        // Add TypeScript-specific metadata
        metadata.insert(
            "node_kind".to_string(),
            serde_json::Value::String(node.kind().to_string()),
        );
        metadata.insert(
            "byte_range".to_string(),
            serde_json::json!([node.start_byte(), node.end_byte()]),
        );

        // Extract additional metadata based on entity type
        match entity_kind {
            EntityKind::Function | EntityKind::Method => {
                self.extract_function_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Class => {
                self.extract_class_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Interface => {
                self.extract_interface_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Enum => {
                self.extract_enum_metadata(&node, source_code, &mut metadata)?;
            }
            _ => {}
        }

        let entity = ParsedEntity {
            id: entity_id,
            kind: entity_kind,
            name,
            parent: parent_id,
            children: Vec::new(), // Will be populated later
            location,
            metadata,
        };

        Ok(Some(entity))
    }

    /// Find the first child matching any of the given kinds and return its text.
    fn find_child_text(node: &Node, source_code: &str, kinds: &[&str]) -> Result<Option<String>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if kinds.contains(&child.kind()) {
                return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
            }
        }
        Ok(None)
    }

    /// Extract name from a variable_declarator child.
    fn extract_variable_declarator_name(node: &Node, source_code: &str) -> Result<Option<String>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                return Self::find_child_text(&child, source_code, &["identifier"]);
            }
        }
        Ok(None)
    }

    /// Extract parameter names from a formal_parameters node.
    fn extract_parameter_names<'a>(params_node: &Node, source_code: &'a str) -> Vec<&'a str> {
        let mut cursor = params_node.walk();
        params_node
            .children(&mut cursor)
            .filter(|child| child.kind() == "identifier")
            .filter_map(|child| child.utf8_text(source_code.as_bytes()).ok())
            .collect()
    }

    /// Extract the name of an entity from its AST node
    fn extract_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        match node.kind() {
            "function_declaration"
            | "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "type_alias_declaration" => {
                Self::find_child_text(node, source_code, &["type_identifier", "identifier"])
            }
            "method_definition" => {
                Self::find_child_text(node, source_code, &["property_identifier", "identifier"])
            }
            "function_expression" | "arrow_function" => Ok(Some("<anonymous>".to_string())),
            "variable_declaration" | "lexical_declaration" => {
                Self::extract_variable_declarator_name(node, source_code)
            }
            _ => Ok(None),
        }
    }

    /// Check if a declaration is a const declaration
    fn is_const_declaration(&self, node: &Node, source_code: &str) -> Result<bool> {
        let mut cursor = node.walk();

        // Look for 'const' keyword
        for child in node.children(&mut cursor) {
            if child.kind() == "const"
                || (child.kind() == "identifier"
                    && child.utf8_text(source_code.as_bytes())? == "const")
            {
                return Ok(true);
            }
        }

        Ok(false)
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
                    parameters = Self::extract_parameter_names(&child, source_code);
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

    /// Convert ParsedEntity to CodeEntity format
    fn convert_to_code_entity(
        &self,
        entity: &ParsedEntity,
        source_code: &str,
    ) -> Result<CodeEntity> {
        let source_lines: Vec<&str> = source_code.lines().collect();
        let entity_source = if entity.location.start_line <= source_lines.len()
            && entity.location.end_line <= source_lines.len()
        {
            source_lines[(entity.location.start_line - 1)..entity.location.end_line].join("\n")
        } else {
            String::new()
        };

        let mut code_entity = CodeEntity::new(
            entity.id.clone(),
            format!("{:?}", entity.kind),
            entity.name.clone(),
            entity.location.file_path.clone(),
        )
        .with_line_range(entity.location.start_line, entity.location.end_line)
        .with_source_code(entity_source);

        // Add metadata from parsed entity
        for (key, value) in &entity.metadata {
            code_entity.add_property(key.clone(), value.clone());
        }

        Ok(code_entity)
    }
}

/// [`LanguageAdapter`] implementation for TypeScript source code.
impl LanguageAdapter for TypeScriptAdapter {
    /// Parses TypeScript source code and returns a parse index.
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        TypeScriptAdapter::parse_source(self, source, file_path)
    }

    /// Extracts all function and constructor call targets.
    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        let mut calls = Vec::new();

        Self::walk_tree(tree.root_node(), &mut |node| {
            let callee = match node.kind() {
                "call_expression" => node.child_by_field_name("function"),
                "new_expression" => node.child_by_field_name("constructor"),
                _ => None,
            };

            if let Some(target) = callee.or_else(|| node.child(0)) {
                if let Ok(text) = Self::node_text(&target, source) {
                    let cleaned = text.trim();
                    if !cleaned.is_empty() {
                        calls.push(cleaned.to_string());
                    }
                }
            }
        });

        calls.sort();
        calls.dedup();
        Ok(calls)
    }

    /// Checks for boilerplate patterns in the source code.
    fn contains_boilerplate_patterns(
        &mut self,
        source: &str,
        patterns: &[String],
    ) -> Result<Vec<String>> {
        let mut found: Vec<String> = patterns
            .iter()
            .filter(|pattern| !pattern.is_empty() && source.contains(pattern.as_str()))
            .cloned()
            .collect();

        found.sort();
        found.dedup();
        Ok(found)
    }

    /// Extracts all identifier tokens from the source.
    fn extract_identifiers(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        let mut identifiers = Vec::new();

        Self::walk_tree(tree.root_node(), &mut |node| match node.kind() {
            "identifier"
            | "type_identifier"
            | "shorthand_property_identifier"
            | "property_identifier"
            | "namespace_identifier" => {
                if let Ok(text) = Self::node_text(&node, source) {
                    let cleaned = text.trim();
                    if !cleaned.is_empty() {
                        identifiers.push(cleaned.to_string());
                    }
                }
            }
            _ => {}
        });

        identifiers.sort();
        identifiers.dedup();
        Ok(identifiers)
    }

    /// Counts the total number of AST nodes.
    fn count_ast_nodes(&mut self, source: &str) -> Result<usize> {
        let tree = self.parse_tree(source)?;
        let mut count = 0usize;
        Self::walk_tree(tree.root_node(), &mut |_| count += 1);
        Ok(count)
    }

    /// Counts distinct code blocks in the source.
    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize> {
        let index = TypeScriptAdapter::parse_source(self, source, "<memory>")?;
        Ok(index.count_distinct_blocks())
    }

    /// Normalizes source to an S-expression representation.
    fn normalize_source(&mut self, source: &str) -> Result<String> {
        let tree = self.parse_tree(source)?;
        Ok(tree.root_node().to_sexp())
    }

    /// Returns the language name ("typescript").
    fn language_name(&self) -> &str {
        "typescript"
    }

    /// Extracts import and require statements from TypeScript source.
    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        let mut imports = Vec::new();

        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            if let Some(stmt) = Self::parse_ts_import_line(trimmed, line_number + 1) {
                imports.push(stmt);
            }
        }

        Ok(imports)
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

/// Import parsing helper methods for TypeScriptAdapter.
impl TypeScriptAdapter {
    /// Parse a TypeScript/JavaScript import line.
    fn parse_ts_import_line(trimmed: &str, line_number: usize) -> Option<ImportStatement> {
        if let Some(import_part) = trimmed.strip_prefix("import ") {
            return Self::parse_es_import(import_part, line_number);
        }

        if let Some(require_part) = trimmed.strip_prefix("const ") {
            return Self::parse_require_import(require_part, line_number);
        }

        None
    }

    /// Parse an ES6 import statement.
    fn parse_es_import(import_part: &str, line_number: usize) -> Option<ImportStatement> {
        let from_pos = import_part.find(" from ")?;
        let import_spec = import_part[..from_pos].trim();
        let module_part = normalize_module_literal(&import_part[from_pos + 6..]);

        let (imports_list, import_type) = Self::parse_import_spec(import_spec);

        Some(ImportStatement {
            module: module_part,
            imports: imports_list,
            import_type,
            line_number,
        })
    }

    /// Parse the import specifier (what's being imported).
    fn parse_import_spec(spec: &str) -> (Option<Vec<String>>, String) {
        if spec.starts_with('*') {
            return (None, "star".to_string());
        }

        if spec.starts_with('{') {
            let cleaned = spec.trim_matches(|c| c == '{' || c == '}');
            let items = cleaned
                .split(',')
                .map(|s| s.trim().trim_start_matches("type ").to_string())
                .collect();
            return (Some(items), "named".to_string());
        }

        (Some(vec![spec.to_string()]), "default".to_string())
    }

    /// Parse a CommonJS require statement.
    fn parse_require_import(require_part: &str, line_number: usize) -> Option<ImportStatement> {
        let eq_pos = require_part.find('=')?;
        let rhs = require_part[eq_pos + 1..].trim();
        let module_part = rhs
            .strip_prefix("require(")
            .and_then(|s| s.strip_suffix(");"))?;

        Some(ImportStatement {
            module: normalize_module_literal(module_part),
            imports: None,
            import_type: "require".to_string(),
            line_number,
        })
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
