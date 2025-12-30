//! Go language adapter with tree-sitter integration.

use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Tree};

use super::common::{EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation};
use super::registry::{create_parser_for_language, get_tree_sitter_language};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::CodeEntity;
use crate::detectors::structure::config::ImportStatement;

/// Go-specific parsing and analysis
pub struct GoAdapter {
    /// Tree-sitter parser for Go
    parser: Parser,

    /// Language instance
    language: Language,
}

impl GoAdapter {
    /// Create a new Go adapter
    pub fn new() -> Result<Self> {
        let language = get_tree_sitter_language("go")?;
        let parser = create_parser_for_language("go")?;

        Ok(Self { parser, language })
    }

    fn parse_tree(&mut self, source_code: &str) -> Result<Tree> {
        self.parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("go", "Failed to parse Go source"))
    }

    fn walk_tree<F>(node: Node, callback: &mut F)
    where
        F: FnMut(Node),
    {
        callback(node);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_tree(child, callback);
        }
    }

    fn node_text(node: &Node, source_code: &str) -> Result<String> {
        Ok(node
            .utf8_text(source_code.as_bytes())?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "))
    }

    /// Parse Go source code and extract entities
    pub fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("go", "Failed to parse Go source code"))?;

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

    /// Extract entities from Go code and convert to CodeEntity format
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
        // Handle grouped const/var declarations specially
        if node.kind() == "const_declaration" || node.kind() == "var_declaration" {
            return self.handle_grouped_declaration(node, source_code, file_path, parent_id, index, entity_id_counter);
        }

        // Check if this node represents an entity we care about
        if let Some(entity) = self.node_to_entity(node, source_code, file_path, parent_id.clone(), entity_id_counter)? {
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

    /// Handle grouped const/var declarations by creating entities for each identifier.
    fn handle_grouped_declaration(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<String>,
        index: &mut ParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
        let entity_kind = match node.kind() {
            "const_declaration" => EntityKind::Constant,
            "var_declaration" => EntityKind::Variable,
            _ => return Ok(()),
        };

        let identifiers = self.extract_all_identifiers_from_declaration(&node, source_code)?;

        for identifier in identifiers {
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
            metadata.insert(
                "node_kind".to_string(),
                serde_json::Value::String(node.kind().to_string()),
            );
            metadata.insert(
                "byte_range".to_string(),
                serde_json::json!([node.start_byte(), node.end_byte()]),
            );

            let entity = ParsedEntity {
                id: entity_id,
                name: identifier,
                kind: entity_kind.clone(),
                location,
                parent: parent_id.clone(),
                children: Vec::new(),
                metadata,
            };

            index.add_entity(entity);
        }

        self.traverse_children(node, source_code, file_path, parent_id, index, entity_id_counter)
    }

    /// Extract all identifiers from a const/var declaration (handles both single and grouped)
    fn extract_all_identifiers_from_declaration(
        &self,
        node: &Node,
        source_code: &str,
    ) -> Result<Vec<String>> {
        let (spec_kind, spec_list_kind) = match node.kind() {
            "const_declaration" => ("const_spec", "const_spec_list"),
            "var_declaration" => ("var_spec", "var_spec_list"),
            _ => return Ok(Vec::new()),
        };

        let mut identifiers = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == spec_kind {
                self.collect_identifiers_from_spec(&child, source_code, &mut identifiers)?;
            } else if child.kind() == spec_list_kind {
                self.collect_identifiers_from_spec_list(&child, source_code, spec_kind, &mut identifiers)?;
            }
        }

        Ok(identifiers)
    }

    /// Collect identifiers from a single spec node (const_spec or var_spec)
    fn collect_identifiers_from_spec(
        &self,
        spec_node: &Node,
        source_code: &str,
        identifiers: &mut Vec<String>,
    ) -> Result<()> {
        let mut cursor = spec_node.walk();
        for child in spec_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                identifiers.push(child.utf8_text(source_code.as_bytes())?.to_string());
            }
        }
        Ok(())
    }

    /// Collect identifiers from a spec list node (const_spec_list or var_spec_list)
    fn collect_identifiers_from_spec_list(
        &self,
        list_node: &Node,
        source_code: &str,
        spec_kind: &str,
        identifiers: &mut Vec<String>,
    ) -> Result<()> {
        let mut cursor = list_node.walk();
        for child in list_node.children(&mut cursor) {
            if child.kind() == spec_kind {
                self.collect_identifiers_from_spec(&child, source_code, identifiers)?;
            }
        }
        Ok(())
    }

    /// Determine entity kind from node kind, returning None for non-entity nodes.
    fn determine_entity_kind(&self, node: &Node, source_code: &str) -> Result<Option<EntityKind>> {
        Ok(match node.kind() {
            "function_declaration" => Some(EntityKind::Function),
            "method_declaration" => Some(EntityKind::Method),
            "type_declaration" => Some(
                if self.is_struct_declaration(node, source_code)? {
                    EntityKind::Struct
                } else if self.is_interface_declaration(node, source_code)? {
                    EntityKind::Interface
                } else {
                    EntityKind::Interface // Generic type alias
                }
            ),
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

        // Add Go-specific metadata
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
            EntityKind::Struct => {
                self.extract_struct_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Interface => {
                self.extract_interface_metadata(&node, source_code, &mut metadata)?;
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

    /// Extract text from a node, trying field name first, then falling back to child search.
    fn extract_node_text(node: &Node, source_code: &str, field: &str, fallback_kinds: &[&str]) -> Result<Option<String>> {
        if let Some(name_node) = node.child_by_field_name(field) {
            return Ok(Some(name_node.utf8_text(source_code.as_bytes())?.to_string()));
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if fallback_kinds.contains(&child.kind()) {
                return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
            }
        }
        Ok(None)
    }

    /// Check if a type_spec contains a child of the given kind.
    fn type_spec_contains(&self, node: &Node, target_kind: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec" {
                let mut spec_cursor = child.walk();
                for spec_child in child.children(&mut spec_cursor) {
                    if spec_child.kind() == target_kind {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Extract the name of an entity from its AST node
    fn extract_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        match node.kind() {
            "function_declaration" | "method_declaration" => {
                Self::extract_node_text(node, source_code, "name", &["identifier"])
            }
            "type_declaration" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_spec" {
                        return Self::extract_node_text(&child, source_code, "name", &["type_identifier"]);
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Check if a type declaration is a struct
    fn is_struct_declaration(&self, node: &Node, _source_code: &str) -> Result<bool> {
        Ok(self.type_spec_contains(node, "struct_type"))
    }

    /// Check if a type declaration is an interface
    fn is_interface_declaration(&self, node: &Node, _source_code: &str) -> Result<bool> {
        Ok(self.type_spec_contains(node, "interface_type"))
    }

    /// Extract identifiers from nested declaration children.
    fn extract_nested_identifiers<'a>(
        parent: &Node,
        source_code: &'a str,
        decl_kind: &str,
        id_kind: &str,
    ) -> Result<Vec<&'a str>> {
        let mut results = Vec::new();
        let mut cursor = parent.walk();
        for child in parent.children(&mut cursor) {
            if child.kind() == decl_kind {
                let mut inner = child.walk();
                for inner_child in child.children(&mut inner) {
                    if inner_child.kind() == id_kind {
                        results.push(inner_child.utf8_text(source_code.as_bytes())?);
                    }
                }
            }
        }
        Ok(results)
    }

    /// Extract return types from a result node.
    fn extract_return_types<'a>(result_node: &Node, source_code: &'a str) -> Result<Vec<&'a str>> {
        const TYPE_KINDS: &[&str] = &["type_identifier", "pointer_type", "slice_type"];
        match result_node.kind() {
            "parameter_list" => Self::extract_nested_identifiers(result_node, source_code, "parameter_declaration", "type_identifier"),
            kind if TYPE_KINDS.contains(&kind) => Ok(vec![result_node.utf8_text(source_code.as_bytes())?]),
            _ => Ok(vec![]),
        }
    }

    /// Extract function-specific metadata
    fn extract_function_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let parameters = node.child_by_field_name("parameters")
            .map(|p| Self::extract_nested_identifiers(&p, source_code, "parameter_declaration", "identifier"))
            .transpose()?
            .unwrap_or_default();

        let return_types = node.child_by_field_name("result")
            .map(|r| Self::extract_return_types(&r, source_code))
            .transpose()?
            .unwrap_or_default();

        let receiver_type = if node.kind() == "method_declaration" {
            node.child_by_field_name("receiver")
                .map(|r| r.utf8_text(source_code.as_bytes()).map(|s| s.to_string()))
                .transpose()?
        } else {
            None
        };

        metadata.insert("parameters".to_string(), serde_json::json!(parameters));
        if !return_types.is_empty() {
            metadata.insert("return_types".to_string(), serde_json::json!(return_types));
        }
        if let Some(receiver) = receiver_type {
            metadata.insert("receiver_type".to_string(), serde_json::Value::String(receiver));
        }

        Ok(())
    }

    /// Extract struct-specific metadata
    fn extract_struct_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let Some(struct_type) = self.find_nested_child(node, &["type_spec", "struct_type"]) else {
            return Ok(());
        };

        let (fields, embedded_types) = self.parse_struct_fields(&struct_type, source_code)?;

        metadata.insert("fields".to_string(), serde_json::json!(fields));
        if !embedded_types.is_empty() {
            metadata.insert("embedded_types".to_string(), serde_json::json!(embedded_types));
        }

        Ok(())
    }

    /// Parse struct fields and embedded types from a struct_type node
    fn parse_struct_fields<'a>(
        &self,
        struct_node: &Node<'a>,
        source_code: &'a str,
    ) -> Result<(Vec<String>, Vec<&'a str>)> {
        let mut fields = Vec::new();
        let mut embedded_types = Vec::new();

        let Some(field_list) = self.find_child_by_kind(struct_node, "field_declaration_list") else {
            return Ok((fields, embedded_types));
        };

        let mut cursor = field_list.walk();
        for field_child in field_list.children(&mut cursor) {
            if field_child.kind() != "field_declaration" {
                continue;
            }
            self.parse_field_declaration(&field_child, source_code, &mut fields, &mut embedded_types)?;
        }

        Ok((fields, embedded_types))
    }

    /// Parse a single field declaration
    fn parse_field_declaration<'a>(
        &self,
        field_node: &Node<'a>,
        source_code: &'a str,
        fields: &mut Vec<String>,
        embedded_types: &mut Vec<&'a str>,
    ) -> Result<()> {
        let mut cursor = field_node.walk();
        let mut field_name = None;

        for child in field_node.children(&mut cursor) {
            if child.kind() == "field_identifier" {
                field_name = Some(child.utf8_text(source_code.as_bytes())?.to_string());
            } else if child.kind() == "type_identifier" && field_name.is_none() {
                embedded_types.push(child.utf8_text(source_code.as_bytes())?);
            }
        }

        if let Some(name) = field_name {
            fields.push(name);
        }

        Ok(())
    }

    /// Extract interface-specific metadata
    fn extract_interface_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let Some(interface_type) = self.find_nested_child(node, &["type_spec", "interface_type"]) else {
            return Ok(());
        };

        let (methods, embedded_interfaces) = self.parse_interface_members(&interface_type, source_code)?;

        metadata.insert("methods".to_string(), serde_json::json!(methods));
        if !embedded_interfaces.is_empty() {
            metadata.insert("embedded_interfaces".to_string(), serde_json::json!(embedded_interfaces));
        }

        Ok(())
    }

    /// Parse interface methods and embedded interfaces
    fn parse_interface_members(
        &self,
        interface_node: &Node,
        source_code: &str,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut methods = Vec::new();
        let mut embedded = Vec::new();
        let mut cursor = interface_node.walk();

        for child in interface_node.children(&mut cursor) {
            match child.kind() {
                "type_elem" | "constraint_elem" => {
                    embedded.push(child.utf8_text(source_code.as_bytes())?.to_string());
                }
                "method_elem" => {
                    if let Some(name) = self.extract_method_name_from_text(&child, source_code)? {
                        methods.push(name);
                    }
                }
                "method_spec" => {
                    if let Some(name) = self.extract_method_name_from_spec(&child, source_code)? {
                        methods.push(name);
                    }
                }
                _ => {}
            }
        }

        Ok((methods, embedded))
    }

    /// Extract method name from method_elem text
    fn extract_method_name_from_text(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        let text = node.utf8_text(source_code.as_bytes())?;
        if let Some(name) = text.split('(').next() {
            let name = name.trim();
            if !name.is_empty() {
                return Ok(Some(name.to_string()));
            }
        }
        Ok(None)
    }

    /// Extract method name from method_spec node
    fn extract_method_name_from_spec(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "field_identifier" {
                return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
            }
        }
        Ok(None)
    }

    /// Find a child node by traversing a path of kinds (2 levels deep)
    fn find_nested_child<'a>(&self, node: &Node<'a>, path: &[&str]) -> Option<Node<'a>> {
        if path.is_empty() {
            return Some(*node);
        }
        let first = self.find_child_by_kind(node, path[0])?;
        if path.len() == 1 {
            return Some(first);
        }
        self.find_child_by_kind(&first, path[1])
    }

    /// Find immediate child by kind
    fn find_child_by_kind<'a>(&self, node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == kind {
                return Some(child);
            }
        }
        None
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

    /// Parse a Go import line and extract the import path
    /// Handles: "path/to/pkg", alias "path/to/pkg", . "path/to/pkg", _ "path/to/pkg"
    fn parse_go_import_line(line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        // Find the quoted path
        if let Some(start_quote) = line.find('"') {
            if let Some(end_quote) = line[start_quote + 1..].find('"') {
                let path = &line[start_quote + 1..start_quote + 1 + end_quote];
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
        }

        // Handle backtick quoted imports (raw strings)
        if let Some(start_quote) = line.find('`') {
            if let Some(end_quote) = line[start_quote + 1..].find('`') {
                let path = &line[start_quote + 1..start_quote + 1 + end_quote];
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
        }

        None
    }
}

impl LanguageAdapter for GoAdapter {
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        GoAdapter::parse_source(self, source, file_path)
    }

    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        let mut calls = Vec::new();

        Self::walk_tree(tree.root_node(), &mut |node| {
            if node.kind() == "call_expression" {
                let callee = node
                    .child_by_field_name("function")
                    .or_else(|| node.child(0));

                if let Some(target) = callee {
                    if let Ok(text) = Self::node_text(&target, source) {
                        let cleaned = text.trim();
                        if !cleaned.is_empty() {
                            calls.push(cleaned.to_string());
                        }
                    }
                }
            }
        });

        calls.sort();
        calls.dedup();
        Ok(calls)
    }

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

    fn extract_identifiers(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        let mut identifiers = Vec::new();

        Self::walk_tree(tree.root_node(), &mut |node| match node.kind() {
            "identifier" | "field_identifier" | "type_identifier" | "package_identifier" => {
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

    fn count_ast_nodes(&mut self, source: &str) -> Result<usize> {
        let tree = self.parse_tree(source)?;
        let mut count = 0usize;
        Self::walk_tree(tree.root_node(), &mut |_| count += 1);
        Ok(count)
    }

    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize> {
        let index = GoAdapter::parse_source(self, source, "<memory>")?;
        Ok(index.count_distinct_blocks())
    }

    fn normalize_source(&mut self, source: &str) -> Result<String> {
        let tree = self.parse_tree(source)?;
        Ok(tree.root_node().to_sexp())
    }

    fn language_name(&self) -> &str {
        "go"
    }

    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        let mut imports = Vec::new();
        let mut in_import_block = false;

        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            // Handle import block start: import (
            if trimmed == "import (" {
                in_import_block = true;
                continue;
            }

            // Handle import block end
            if in_import_block && trimmed == ")" {
                in_import_block = false;
                continue;
            }

            // Handle imports inside a block
            if in_import_block {
                let import_path = Self::parse_go_import_line(trimmed);
                if let Some(path) = import_path {
                    imports.push(ImportStatement {
                        module: path,
                        imports: None,
                        import_type: "import".to_string(),
                        line_number: line_number + 1,
                    });
                }
                continue;
            }

            // Handle single-line import: import "fmt" or import alias "path/to/pkg"
            if let Some(import_part) = trimmed.strip_prefix("import ") {
                let import_path = Self::parse_go_import_line(import_part);
                if let Some(path) = import_path {
                    imports.push(ImportStatement {
                        module: path,
                        imports: None,
                        import_type: "import".to_string(),
                        line_number: line_number + 1,
                    });
                }
            }
        }

        Ok(imports)
    }

    fn extract_code_entities(
        &mut self,
        source: &str,
        file_path: &str,
    ) -> Result<Vec<crate::core::featureset::CodeEntity>> {
        GoAdapter::extract_code_entities(self, source, file_path)
    }
}

impl Default for GoAdapter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!(
                "Warning: Failed to create Go adapter, using minimal fallback: {}",
                e
            );
            GoAdapter {
                parser: tree_sitter::Parser::new(),
                language: get_tree_sitter_language("go")
                    .unwrap_or_else(|_| tree_sitter_go::LANGUAGE.into()),
            }
        })
    }
}

#[cfg(test)]
#[path = "go_tests.rs"]
mod tests;
