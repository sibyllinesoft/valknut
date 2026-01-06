//! Go language adapter with tree-sitter integration.

use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Tree};

use super::super::common::{
    create_base_metadata, extract_identifiers_by_kinds, extract_node_text, generate_entity_id,
    sort_and_dedup, EntityExtractor, EntityKind, LanguageAdapter, ParseIndex, ParsedEntity,
    SourceLocation,
};
use super::super::registry::{create_parser_for_language, get_tree_sitter_language};
use crate::core::ast_utils::{find_child_by_kind, node_text_normalized, walk_tree};
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

/// Parsing and entity extraction methods for [`GoAdapter`].
impl GoAdapter {
    /// Create a new Go adapter
    pub fn new() -> Result<Self> {
        let language = get_tree_sitter_language("go")?;
        let parser = create_parser_for_language("go")?;

        Ok(Self { parser, language })
    }

    /// Parse Go source code and extract entities
    pub fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("go", "Failed to parse Go source code"))?;

        let mut index = ParseIndex::new();
        let mut entity_id_counter = 0;

        // Walk the tree and extract entities (iterative to avoid stack overflow)
        self.extract_entities_iterative_go(
            tree.root_node(),
            source_code,
            file_path,
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
            let code_entity = entity.to_code_entity(source_code);
            code_entities.push(code_entity);
        }

        Ok(code_entities)
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
            let entity_id = generate_entity_id(file_path, entity_kind, *entity_id_counter);

            let location = SourceLocation::from_positions(
                file_path,
                node.start_position().row,
                node.start_position().column,
                node.end_position().row,
                node.end_position().column,
            );

            let metadata =
                create_base_metadata(node.kind(), node.start_byte(), node.end_byte());

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

    /// Iterative entity extraction for Go - avoids stack overflow on deeply nested code.
    /// Handles const/var declarations specially like the recursive version.
    fn extract_entities_iterative_go(
        &self,
        root: Node,
        source_code: &str,
        file_path: &str,
        index: &mut ParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
        // Stack entries: (node, parent_id)
        let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];

        while let Some((node, parent_id)) = stack.pop() {
            // Handle grouped const/var declarations specially
            if node.kind() == "const_declaration" || node.kind() == "var_declaration" {
                self.handle_grouped_declaration(
                    node,
                    source_code,
                    file_path,
                    parent_id.clone(),
                    index,
                    entity_id_counter,
                )?;
                // handle_grouped_declaration processes children via traverse_children,
                // but those children don't contain nested entities, so we continue
                continue;
            }

            // Process this node normally
            let new_parent_id = if let Some(entity) = self.node_to_entity(
                node,
                source_code,
                file_path,
                parent_id.clone(),
                entity_id_counter,
            )? {
                let entity_id = entity.id.clone();
                index.add_entity(entity);
                Some(entity_id)
            } else {
                parent_id
            };

            // Push children in reverse order for depth-first traversal
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            for child in children.into_iter().rev() {
                stack.push((child, new_parent_id.clone()));
            }
        }

        Ok(())
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

    /// Find the first type_spec child in a node.
    fn find_type_spec<'a>(node: &'a Node) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec" {
                return Some(child);
            }
        }
        None
    }

    /// Check if a type_spec child has a descendant of the given kind.
    fn type_spec_has_child(type_spec: &Node, target_kind: &str) -> bool {
        let mut cursor = type_spec.walk();
        for child in type_spec.children(&mut cursor) {
            if child.kind() == target_kind {
                return true;
            }
        }
        false
    }

    /// Check if a type_spec contains a child of the given kind.
    fn type_spec_contains(&self, node: &Node, target_kind: &str) -> bool {
        Self::find_type_spec(node)
            .map(|spec| Self::type_spec_has_child(&spec, target_kind))
            .unwrap_or(false)
    }

    /// Extract the name of an entity from its AST node
    fn extract_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        match node.kind() {
            "function_declaration" | "method_declaration" => {
                extract_node_text(node, source_code, "name", &["identifier"])
            }
            "type_declaration" => {
                match Self::find_type_spec(node) {
                    Some(spec) => extract_node_text(&spec, source_code, "name", &["type_identifier"]),
                    None => Ok(None),
                }
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

    /// Extract identifiers of a specific kind from a single node.
    fn collect_child_identifiers<'a>(
        node: &Node,
        source_code: &'a str,
        id_kind: &str,
    ) -> Vec<&'a str> {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .filter(|c| c.kind() == id_kind)
            .filter_map(|c| c.utf8_text(source_code.as_bytes()).ok())
            .collect()
    }

    /// Extract identifiers from nested declaration children.
    fn extract_nested_identifiers<'a>(
        parent: &Node,
        source_code: &'a str,
        decl_kind: &str,
        id_kind: &str,
    ) -> Result<Vec<&'a str>> {
        let mut cursor = parent.walk();
        let results: Vec<&str> = parent
            .children(&mut cursor)
            .filter(|c| c.kind() == decl_kind)
            .flat_map(|decl| Self::collect_child_identifiers(&decl, source_code, id_kind))
            .collect();
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

        let Some(field_list) = find_child_by_kind(struct_node, "field_declaration_list") else {
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
        let first = find_child_by_kind(node, path[0])?;
        if path.len() == 1 {
            return Some(first);
        }
        find_child_by_kind(&first, path[1])
    }

    /// Parse a Go import line and extract the import path
    /// Handles: "path/to/pkg", alias "path/to/pkg", . "path/to/pkg", _ "path/to/pkg"
    fn parse_go_import_line(line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        Self::extract_quoted_path(line, '"')
            .or_else(|| Self::extract_quoted_path(line, '`'))
    }

    /// Extract a path from between matching quote characters
    fn extract_quoted_path(line: &str, quote: char) -> Option<String> {
        let start = line.find(quote)?;
        let end = line[start + 1..].find(quote)?;
        let path = &line[start + 1..start + 1 + end];
        if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        }
    }

    /// Create an ImportStatement from a module path and line number
    fn create_import_statement(module: String, line_number: usize) -> ImportStatement {
        ImportStatement {
            module,
            imports: None,
            import_type: "import".to_string(),
            line_number,
        }
    }
}

/// [`LanguageAdapter`] implementation for Go source code.
impl LanguageAdapter for GoAdapter {
    /// Parses source code into a tree-sitter AST.
    fn parse_tree(&mut self, source: &str) -> Result<Tree> {
        self.parser
            .parse(source, None)
            .ok_or_else(|| ValknutError::parse("go", "Failed to parse Go source"))
    }

    /// Parses Go source code and returns a parse index.
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        GoAdapter::parse_source(self, source, file_path)
    }

    /// Extracts all function call targets from the source.
    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        let mut calls = Vec::new();

        walk_tree(tree.root_node(), &mut |node| {
            if node.kind() == "call_expression" {
                let callee = node
                    .child_by_field_name("function")
                    .or_else(|| node.child(0));

                if let Some(target) = callee {
                    if let Ok(text) = node_text_normalized(&target, source) {
                        let cleaned = text.trim();
                        if !cleaned.is_empty() {
                            calls.push(cleaned.to_string());
                        }
                    }
                }
            }
        });

        sort_and_dedup(&mut calls);
        Ok(calls)
    }

    /// Extracts all identifier tokens from the source.
    fn extract_identifiers(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        Ok(extract_identifiers_by_kinds(
            tree.root_node(),
            source,
            &["identifier", "field_identifier", "type_identifier", "package_identifier"],
        ))
    }

    /// Counts distinct code blocks in the source.
    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize> {
        let index = GoAdapter::parse_source(self, source, "<memory>")?;
        Ok(index.count_distinct_blocks())
    }

    /// Returns the language name ("go").
    fn language_name(&self) -> &str {
        "go"
    }

    /// Extracts import statements from Go source code.
    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        let mut imports = Vec::new();
        let mut in_import_block = false;

        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            // Handle import block start/end
            if trimmed == "import (" {
                in_import_block = true;
                continue;
            }
            if in_import_block && trimmed == ")" {
                in_import_block = false;
                continue;
            }

            // Parse import line (either in block or single-line)
            let import_text = if in_import_block {
                Some(trimmed)
            } else {
                trimmed.strip_prefix("import ")
            };

            if let Some(text) = import_text {
                if let Some(path) = Self::parse_go_import_line(text) {
                    imports.push(Self::create_import_statement(path, line_number + 1));
                }
            }
        }

        Ok(imports)
    }

    /// Extracts code entities from Go source code.
    fn extract_code_entities(
        &mut self,
        source: &str,
        file_path: &str,
    ) -> Result<Vec<crate::core::featureset::CodeEntity>> {
        GoAdapter::extract_code_entities(self, source, file_path)
    }
}

/// [`EntityExtractor`] implementation providing the language-specific node conversion.
/// Go overrides `extract_entities_recursive` to handle grouped const/var declarations.
impl EntityExtractor for GoAdapter {
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
            EntityKind::Struct => {
                self.extract_struct_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Interface => {
                self.extract_interface_metadata(&node, source_code, &mut metadata)?;
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

    /// Override: Handle grouped const/var declarations specially.
    fn extract_entities_recursive(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<String>,
        index: &mut ParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
        if node.kind() == "const_declaration" || node.kind() == "var_declaration" {
            return self.handle_grouped_declaration(node, source_code, file_path, parent_id, index, entity_id_counter);
        }

        if let Some(entity) = self.node_to_entity(node, source_code, file_path, parent_id.clone(), entity_id_counter)? {
            let entity_id = entity.id.clone();
            index.add_entity(entity);
            self.traverse_children(node, source_code, file_path, Some(entity_id), index, entity_id_counter)?;
        } else {
            self.traverse_children(node, source_code, file_path, parent_id, index, entity_id_counter)?;
        }

        Ok(())
    }
}

/// Default implementation for [`GoAdapter`].
impl Default for GoAdapter {
    /// Returns a new Go adapter, or a minimal fallback on failure.
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
