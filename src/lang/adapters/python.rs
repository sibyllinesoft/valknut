//! Python language adapter with tree-sitter integration.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tree_sitter::{Language, Node, Parser, Tree, TreeCursor};

use super::super::common::{
    create_base_metadata, extract_identifiers_by_kinds, extract_node_text, find_boilerplate_patterns,
    generate_entity_id, sort_and_dedup, EntityExtractor, EntityKind, LanguageAdapter, ParseIndex,
    ParsedEntity, SourceLocation,
};
use super::super::registry::{create_parser_for_language, get_tree_sitter_language};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::{CodeEntity, EntityId};
use crate::core::interned_entities::{
    InternedCodeEntity, InternedParseIndex, InternedParsedEntity, InternedSourceLocation,
};
use crate::core::interning::{intern, resolve, InternedString};
use crate::detectors::structure::config::ImportStatement;

#[cfg(test)]
#[path = "python_tests.rs"]
mod tests;

/// Python-specific parsing and analysis
pub struct PythonAdapter {
    /// Tree-sitter parser for Python
    parser: Parser,

    /// Language instance
    language: Language,
}

/// Parsing and entity extraction methods for [`PythonAdapter`].
impl PythonAdapter {
    /// Create a new Python adapter
    pub fn new() -> Result<Self> {
        let language = get_tree_sitter_language("py")?;
        let parser = create_parser_for_language("py")?;

        Ok(Self { parser, language })
    }

    /// Parse Python source code and extract entities
    pub fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("python", "Failed to parse Python source code"))?;

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

    /// Extract entities from Python code and convert to CodeEntity format
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

    /// OPTIMIZED: Parse source code and return interned entities for zero-allocation processing
    pub fn parse_source_interned(
        &mut self,
        source_code: &str,
        file_path: &str,
    ) -> Result<InternedParseIndex> {
        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("python", "Failed to parse Python source code"))?;

        let mut index = InternedParseIndex::new();
        let mut entity_id_counter = 0;

        // Walk the tree and extract entities using interned strings (iterative to avoid stack overflow)
        self.extract_entities_iterative_interned(
            tree.root_node(),
            source_code,
            file_path,
            &mut index,
            &mut entity_id_counter,
        )?;

        Ok(index)
    }

    /// OPTIMIZED: Extract entities and convert to interned CodeEntity format for maximum performance
    pub fn extract_code_entities_interned(
        &mut self,
        source_code: &str,
        file_path: &str,
    ) -> Result<Vec<InternedCodeEntity>> {
        let parse_index = self.parse_source_interned(source_code, file_path)?;
        let mut code_entities = Vec::with_capacity(parse_index.entity_count()); // Pre-allocate!

        for entity in parse_index.entities.values() {
            let code_entity = self.convert_to_interned_code_entity(entity, source_code)?;
            code_entities.push(code_entity);
        }

        Ok(code_entities)
    }

    /// Check if a name represents a constant (all uppercase with underscores).
    fn is_constant_name(name: &str) -> bool {
        name.chars().all(|c| c.is_uppercase() || c == '_')
    }

    /// Determine entity kind from node kind, returning None for non-entity nodes.
    fn determine_entity_kind(&self, node: &Node, source_code: &str) -> Result<Option<EntityKind>> {
        Ok(match node.kind() {
            "function_definition" => Some(EntityKind::Function),
            "class_definition" => Some(EntityKind::Class),
            "module" => None,
            "assignment" => {
                self.extract_name(node, source_code)?.map(|name| {
                    if Self::is_constant_name(&name) { EntityKind::Constant } else { EntityKind::Variable }
                })
            }
            _ => None,
        })
    }

    /// Extract the name of an entity from its AST node
    fn extract_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        match node.kind() {
            "function_definition" | "class_definition" => {
                extract_node_text(node, source_code, "name", &["identifier"])
            }
            "assignment" => {
                extract_node_text(node, source_code, "", &["identifier"])
            }
            _ => Ok(None),
        }
    }

    /// Extract parameter names from a parameters node.
    fn extract_parameters_from_node<'a>(node: &Node<'a>, source_code: &'a str) -> Result<Vec<&'a str>> {
        let mut parameters = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                parameters.push(child.utf8_text(source_code.as_bytes())?);
            }
        }
        Ok(parameters)
    }

    /// Scan function children for parameters, decorators, and return annotation.
    fn scan_function_children<'a>(
        node: &Node<'a>,
        source_code: &'a str,
    ) -> Result<(Vec<&'a str>, bool, Option<String>)> {
        let mut parameters = Vec::new();
        let mut has_decorators = false;
        let mut return_annotation = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "parameters" => {
                    parameters = Self::extract_parameters_from_node(&child, source_code)?;
                }
                "decorator" => has_decorators = true,
                "type" => {
                    return_annotation = Some(child.utf8_text(source_code.as_bytes())?.to_string());
                }
                _ => {}
            }
        }
        Ok((parameters, has_decorators, return_annotation))
    }

    /// Extract function-specific metadata
    fn extract_function_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let (parameters, has_decorators, return_annotation) =
            Self::scan_function_children(node, source_code)?;

        let mut function_calls = Vec::new();
        self.extract_function_calls_recursive(*node, source_code, &mut function_calls)?;

        metadata.insert("parameters".to_string(), serde_json::json!(parameters));
        metadata.insert("has_decorators".to_string(), serde_json::Value::Bool(has_decorators));
        if let Some(return_type) = return_annotation {
            metadata.insert("return_annotation".to_string(), serde_json::Value::String(return_type));
        }
        metadata.insert(
            "function_calls".to_string(),
            serde_json::Value::Array(function_calls.into_iter().map(serde_json::Value::String).collect()),
        );

        Ok(())
    }

    /// Extract base class names from an argument_list node.
    fn extract_base_classes<'a>(
        arg_list: &Node,
        source_code: &'a str,
    ) -> Vec<&'a str> {
        let mut arg_cursor = arg_list.walk();
        arg_list
            .children(&mut arg_cursor)
            .filter(|child| child.kind() == "identifier")
            .filter_map(|child| child.utf8_text(source_code.as_bytes()).ok())
            .collect()
    }

    /// Extract class-specific metadata
    fn extract_class_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut base_classes = Vec::new();
        let mut has_decorators = false;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "argument_list" => {
                    base_classes = Self::extract_base_classes(&child, source_code);
                }
                "decorator" => {
                    has_decorators = true;
                }
                _ => {}
            }
        }

        metadata.insert("base_classes".to_string(), serde_json::json!(base_classes));
        metadata.insert(
            "has_decorators".to_string(),
            serde_json::Value::Bool(has_decorators),
        );

        Ok(())
    }

    /// OPTIMIZED: Recursively extract entities using interned strings - ZERO STRING ALLOCATIONS!
    fn extract_entities_recursive_interned(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<InternedString>,
        index: &mut InternedParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
        // Check if this node represents an entity we care about
        if let Some(entity) = self.node_to_interned_entity(
            node,
            source_code,
            file_path,
            parent_id,
            entity_id_counter,
        )? {
            let entity_id = entity.id;
            index.add_entity(entity);

            // Process children with this entity as parent
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.extract_entities_recursive_interned(
                    child,
                    source_code,
                    file_path,
                    Some(entity_id),
                    index,
                    entity_id_counter,
                )?;
            }
        } else {
            // No entity for this node, but check its children
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.extract_entities_recursive_interned(
                    child,
                    source_code,
                    file_path,
                    parent_id,
                    index,
                    entity_id_counter,
                )?;
            }
        }

        Ok(())
    }

    /// OPTIMIZED: Extract entities iteratively using interned strings - avoids stack overflow.
    fn extract_entities_iterative_interned(
        &self,
        root: Node,
        source_code: &str,
        file_path: &str,
        index: &mut InternedParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
        // Stack entries: (node, parent_id)
        let mut stack: Vec<(Node, Option<InternedString>)> = vec![(root, None)];

        while let Some((node, parent_id)) = stack.pop() {
            // Process this node
            let new_parent_id = if let Some(entity) = self.node_to_interned_entity(
                node,
                source_code,
                file_path,
                parent_id,
                entity_id_counter,
            )? {
                let entity_id = entity.id;
                index.add_entity(entity);
                Some(entity_id)
            } else {
                parent_id
            };

            // Push children in reverse order for depth-first traversal
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            for child in children.into_iter().rev() {
                stack.push((child, new_parent_id));
            }
        }

        Ok(())
    }

    /// OPTIMIZED: Convert tree-sitter node to interned entity - MINIMAL ALLOCATIONS!
    fn node_to_interned_entity(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<InternedString>,
        entity_id_counter: &mut usize,
    ) -> Result<Option<InternedParsedEntity>> {
        let kind = node.kind();

        // Map node kinds to EntityKind (same logic as original)
        let entity_kind = match kind {
            "function_definition" => EntityKind::Function,
            "class_definition" => EntityKind::Class,
            "module" => EntityKind::Module,
            _ => return Ok(None), // Not an entity we track
        };

        // Extract name using interned strings - ZERO allocations for existing names!
        let name = match self.extract_name_interned(node, source_code)? {
            Some(name) => name,
            None => return Ok(None), // No name found
        };

        // Create entity ID with minimal allocation
        *entity_id_counter += 1;
        let entity_id_str = format!("python_{}_{}", kind, entity_id_counter);

        // Create location using interned file path
        let location = InternedSourceLocation::new(
            file_path,
            node.start_position().row + 1,
            node.end_position().row + 1,
            node.start_position().column + 1,
            node.end_position().column + 1,
        );

        // Create interned entity
        let mut entity = InternedParsedEntity::new(
            &entity_id_str,
            entity_kind,
            resolve(name), // Convert interned name back to &str for entity creation
            location,
        );

        // Set parent if provided
        if let Some(parent) = parent_id {
            entity.set_parent(parent);
        }

        Ok(Some(entity))
    }

    /// OPTIMIZED: Extract name from node using interned strings
    fn extract_name_interned(
        &self,
        node: Node,
        source_code: &str,
    ) -> Result<Option<InternedString>> {
        if !matches!(node.kind(), "function_definition" | "class_definition") {
            return Ok(None);
        }
        let mut cursor = node.walk();
        let identifier = node
            .children(&mut cursor)
            .find(|child| child.kind() == "identifier");

        match identifier {
            Some(id) => Ok(Some(intern(id.utf8_text(source_code.as_bytes())?))),
            None => Ok(None),
        }
    }

    /// OPTIMIZED: Convert interned ParsedEntity to interned CodeEntity - ZERO STRING ALLOCATIONS!
    fn convert_to_interned_code_entity(
        &self,
        entity: &InternedParsedEntity,
        source_code: &str,
    ) -> Result<InternedCodeEntity> {
        let source_lines: Vec<&str> = source_code.lines().collect();

        // Extract source code for entity (minimal allocations)
        let entity_source = if entity.location.start_line <= source_lines.len()
            && entity.location.end_line <= source_lines.len()
        {
            source_lines[(entity.location.start_line - 1)..entity.location.end_line].join("\n")
        } else {
            String::new()
        };

        // Create interned code entity
        let code_entity = InternedCodeEntity::new(
            entity.id_str(),                 // Zero-cost lookup
            &format!("{:?}", entity.kind),   // Only allocation is for kind formatting
            entity.name_str(),               // Zero-cost lookup
            entity.location.file_path_str(), // Zero-cost lookup
        )
        .with_line_range(entity.location.start_line, entity.location.end_line)
        .with_source_code(&entity_source); // This gets interned, so duplication is eliminated

        Ok(code_entity)
    }

    // Helper methods for LanguageAdapter trait implementation

    /// Extract function name from a call node.
    fn extract_call_function_name(&self, node: &Node, source: &str) -> Option<String> {
        node.child_by_field_name("function")
            .and_then(|func_node| func_node.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string())
    }

    /// Extract function calls recursively from AST
    fn extract_function_calls_recursive(
        &self,
        node: Node,
        source: &str,
        calls: &mut Vec<String>,
    ) -> Result<()> {
        match node.kind() {
            "call" => {
                if let Some(name) = self.extract_call_function_name(&node, source) {
                    calls.push(name);
                }
            }
            "attribute" => {
                if let Ok(attr_text) = node.utf8_text(source.as_bytes()) {
                    calls.push(attr_text.to_string());
                }
            }
            _ => {}
        }

        // Process children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_function_calls_recursive(child, source, calls)?;
        }

        Ok(())
    }

    /// Common modules considered boilerplate imports.
    const COMMON_MODULES: [&'static str; 5] = ["os", "sys", "json", "logging", "datetime"];

    /// Check for common import statement pattern.
    fn check_import_pattern(node: &Node, source: &str) -> Option<String> {
        let name_node = node.child_by_field_name("name")?;
        let module_name = name_node.utf8_text(source.as_bytes()).ok()?;
        Self::COMMON_MODULES.contains(&module_name).then(|| format!("import {}", module_name))
    }

    /// Check for typing import pattern.
    fn check_from_import_pattern(node: &Node, source: &str) -> Option<String> {
        let module_node = node.child_by_field_name("module_name")?;
        let module_name = module_node.utf8_text(source.as_bytes()).ok()?;
        (module_name == "typing").then(|| "from typing import".to_string())
    }

    /// Check for if __name__ == "__main__" pattern.
    fn check_main_guard_pattern(node: &Node, source: &str) -> Option<String> {
        let condition_node = node.child_by_field_name("condition")?;
        if condition_node.kind() != "comparison_operator" {
            return None;
        }
        let mut cursor = condition_node.walk();
        let children: Vec<_> = condition_node.children(&mut cursor).collect();
        if children.len() < 3 {
            return None;
        }
        let left_text = children[0].utf8_text(source.as_bytes()).ok()?;
        let right_text = children[2].utf8_text(source.as_bytes()).ok()?;
        (left_text == "__name__" && right_text.contains("__main__"))
            .then(|| "if __name__ == \"__main__\"".to_string())
    }

    /// Check for dunder method definition pattern.
    fn check_dunder_method_pattern(node: &Node, source: &str) -> Option<String> {
        let name_node = node.child_by_field_name("name")?;
        let func_name = name_node.utf8_text(source.as_bytes()).ok()?;
        (func_name.len() >= 4 && func_name.starts_with("__") && func_name.ends_with("__"))
            .then(|| func_name.to_string())
    }

    /// Check for boilerplate patterns in AST recursively
    fn check_boilerplate_patterns_recursive(
        &self,
        node: Node,
        source: &str,
        patterns: &[String],
        found_patterns: &mut Vec<String>,
    ) -> Result<()> {
        let pattern = match node.kind() {
            "import_statement" => Self::check_import_pattern(&node, source),
            "import_from_statement" => Self::check_from_import_pattern(&node, source),
            "if_statement" => Self::check_main_guard_pattern(&node, source),
            "function_definition" => Self::check_dunder_method_pattern(&node, source),
            _ => None,
        };
        if let Some(p) = pattern {
            found_patterns.push(p);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_boilerplate_patterns_recursive(child, source, patterns, found_patterns)?;
        }

        Ok(())
    }

    /// Count distinct code blocks recursively
    fn count_blocks_recursive(&self, node: Node, block_count: &mut usize) {
        match node.kind() {
            "function_definition" | "class_definition" => {
                *block_count += 1;
            }
            "if_statement" | "for_statement" | "while_statement" | "try_statement"
            | "with_statement" => {
                *block_count += 1;
            }
            _ => {}
        }

        // Process children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.count_blocks_recursive(child, block_count);
        }
    }

    /// Normalize AST recursively for comparison
    fn normalize_ast_recursive(
        &self,
        node: Node,
        source: &str,
        normalized_parts: &mut Vec<String>,
    ) -> Result<()> {
        match node.kind() {
            // Include semantic tokens, exclude syntactic noise
            "function_definition"
            | "class_definition"
            | "if_statement"
            | "for_statement"
            | "while_statement" => {
                normalized_parts.push(node.kind().to_string());
            }
            "identifier" => {
                if let Ok(identifier) = node.utf8_text(source.as_bytes()) {
                    // Normalize common identifier patterns
                    if identifier.len() > 1 && !identifier.starts_with("__") {
                        normalized_parts.push(identifier.to_string());
                    }
                }
            }
            "string" | "integer" | "float" => {
                // Normalize literals to generic types
                normalized_parts.push(format!("<{}>", node.kind()));
            }
            _ => {}
        }

        // Process children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.normalize_ast_recursive(child, source, normalized_parts)?;
        }

        Ok(())
    }
}

/// Default implementation for [`PythonAdapter`].
impl Default for PythonAdapter {
    /// Returns a new Python adapter, or a minimal fallback on failure.
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!(
                "Warning: Failed to create Python adapter, using minimal fallback: {}",
                e
            );
            PythonAdapter {
                parser: tree_sitter::Parser::new(),
                language: get_tree_sitter_language("py")
                    .unwrap_or_else(|_| tree_sitter_python::LANGUAGE.into()),
            }
        })
    }
}

/// [`LanguageAdapter`] implementation for Python source code.
#[async_trait]
impl LanguageAdapter for PythonAdapter {
    /// Parses source code into a tree-sitter AST.
    fn parse_tree(&mut self, source: &str) -> Result<Tree> {
        self.parser
            .parse(source, None)
            .ok_or_else(|| ValknutError::parse("python", "Failed to parse Python source"))
    }

    /// Parses Python source code and returns a parse index.
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        // Use existing implementation
        PythonAdapter::parse_source(self, source, file_path)
    }

    /// Extracts all function call targets from the source.
    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;

        let mut calls = Vec::new();
        self.extract_function_calls_recursive(tree.root_node(), source, &mut calls)?;

        sort_and_dedup(&mut calls);
        Ok(calls)
    }

    /// Checks for boilerplate patterns in the source code.
    fn contains_boilerplate_patterns(
        &mut self,
        source: &str,
        patterns: &[String],
    ) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;

        let mut found_patterns = Vec::new();

        // Walk the AST looking for boilerplate patterns
        self.check_boilerplate_patterns_recursive(
            tree.root_node(),
            source,
            patterns,
            &mut found_patterns,
        )?;

        sort_and_dedup(&mut found_patterns);
        Ok(found_patterns)
    }

    /// Extracts all identifier tokens from the source.
    fn extract_identifiers(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;

        Ok(extract_identifiers_by_kinds(
            tree.root_node(),
            source,
            &["identifier"],
        ))
    }

    /// Counts distinct code blocks in the source.
    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize> {
        let tree = self.parse_tree(source)?;

        let mut block_count = 0;
        self.count_blocks_recursive(tree.root_node(), &mut block_count);

        Ok(block_count.max(1))
    }

    /// Normalizes source to a structured representation.
    /// Overrides default to use Python-specific AST normalization.
    fn normalize_source(&mut self, source: &str) -> Result<String> {
        let tree = self.parse_tree(source)?;

        let mut normalized_parts = Vec::new();
        self.normalize_ast_recursive(tree.root_node(), source, &mut normalized_parts)?;

        Ok(normalized_parts.join(" "))
    }

    /// Returns the language name ("python").
    fn language_name(&self) -> &str {
        "python"
    }

    /// Extracts import statements from Python source code.
    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        let mut imports = Vec::new();

        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(stmt) = Self::parse_import_line(trimmed, line_number + 1) {
                imports.push(stmt);
            }
        }

        Ok(imports)
    }

    /// Extracts code entities from Python source code.
    fn extract_code_entities(
        &mut self,
        source: &str,
        file_path: &str,
    ) -> Result<Vec<crate::core::featureset::CodeEntity>> {
        PythonAdapter::extract_code_entities(self, source, file_path)
    }

    /// Optimized interned extraction - bypasses string allocations entirely
    fn extract_code_entities_interned(
        &mut self,
        source: &str,
        file_path: &str,
    ) -> Result<Vec<crate::core::interned_entities::InternedCodeEntity>> {
        PythonAdapter::extract_code_entities_interned(self, source, file_path)
    }
}

/// [`EntityExtractor`] implementation providing the language-specific node conversion.
impl EntityExtractor for PythonAdapter {
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
            EntityKind::Function => {
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

/// Import parsing helper methods for PythonAdapter.
impl PythonAdapter {
    /// Parse a single import line into an ImportStatement if valid.
    fn parse_import_line(trimmed: &str, line_number: usize) -> Option<ImportStatement> {
        if let Some(import_part) = trimmed.strip_prefix("import ") {
            return Some(Self::parse_simple_import(import_part, line_number));
        }

        if let Some(from_part) = trimmed.strip_prefix("from ") {
            return Self::parse_from_import(from_part, line_number);
        }

        None
    }

    /// Parse a simple "import module" statement.
    fn parse_simple_import(import_part: &str, line_number: usize) -> ImportStatement {
        let module = import_part
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        ImportStatement {
            module,
            imports: None,
            import_type: "module".to_string(),
            line_number,
        }
    }

    /// Parse a "from module import ..." statement.
    fn parse_from_import(from_part: &str, line_number: usize) -> Option<ImportStatement> {
        let import_pos = from_part.find(" import ")?;
        let module = from_part[..import_pos].trim().to_string();
        let import_list = from_part[import_pos + 8..].trim();

        let (specific_imports, import_type) = if import_list == "*" {
            (None, "star")
        } else {
            let imports: Vec<String> = import_list
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            (Some(imports), "named")
        };

        Some(ImportStatement {
            module,
            imports: specific_imports,
            import_type: import_type.to_string(),
            line_number,
        })
    }
}
