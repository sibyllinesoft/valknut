//! Go language adapter with tree-sitter integration.

use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Tree};

use super::common::{EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation};
use super::registry::{get_tree_sitter_language, create_parser_for_language};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::CodeEntity;

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
        // Special handling for grouped const/var declarations
        if node.kind() == "const_declaration" || node.kind() == "var_declaration" {
            let entity_kind = match node.kind() {
                "const_declaration" => EntityKind::Constant,
                "var_declaration" => EntityKind::Variable,
                _ => unreachable!(),
            };

            // Find all identifiers in this declaration (could be grouped)
            let identifiers = self.extract_all_identifiers_from_declaration(&node, source_code)?;

            for identifier in identifiers {
                *entity_id_counter += 1;
                let entity_id =
                    format!("{}:{}:{}", file_path, entity_kind as u8, *entity_id_counter);

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

            // Still process child nodes for nested entities
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
        } else if let Some(entity) = self.node_to_entity(
            node,
            source_code,
            file_path,
            parent_id.clone(),
            entity_id_counter,
        )? {
            let entity_id = entity.id.clone();
            index.add_entity(entity);

            // Process child nodes with this entity as parent
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.extract_entities_recursive(
                    child,
                    source_code,
                    file_path,
                    Some(entity_id.clone()),
                    index,
                    entity_id_counter,
                )?;
            }
        } else {
            // Process child nodes with current parent
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
        }

        Ok(())
    }

    /// Extract all identifiers from a const/var declaration (handles both single and grouped)
    fn extract_all_identifiers_from_declaration(
        &self,
        node: &Node,
        source_code: &str,
    ) -> Result<Vec<String>> {
        let mut identifiers = Vec::new();
        let mut cursor = node.walk();

        let (spec_kind, spec_list_kind) = match node.kind() {
            "const_declaration" => ("const_spec", "const_spec_list"),
            "var_declaration" => ("var_spec", "var_spec_list"),
            _ => return Ok(identifiers),
        };

        // Look for all const_spec/var_spec nodes or spec_list nodes
        for child in node.children(&mut cursor) {
            if child.kind() == spec_kind {
                // Single spec (e.g., const Pi = 3.14)
                let mut spec_cursor = child.walk();
                for spec_child in child.children(&mut spec_cursor) {
                    if spec_child.kind() == "identifier" {
                        let identifier = spec_child.utf8_text(source_code.as_bytes())?;
                        identifiers.push(identifier.to_string());
                    }
                }
            } else if child.kind() == spec_list_kind {
                // Grouped specs (e.g., var ( Name string; Version string = "1.0" ))
                let mut list_cursor = child.walk();
                for list_child in child.children(&mut list_cursor) {
                    if list_child.kind() == spec_kind {
                        let mut spec_cursor = list_child.walk();
                        for spec_child in list_child.children(&mut spec_cursor) {
                            if spec_child.kind() == "identifier" {
                                let identifier = spec_child.utf8_text(source_code.as_bytes())?;
                                identifiers.push(identifier.to_string());
                            }
                        }
                    }
                }
            }
        }
        Ok(identifiers)
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
        let entity_kind = match node.kind() {
            "function_declaration" => EntityKind::Function,
            "method_declaration" => EntityKind::Method,
            "type_declaration" => {
                // Check if this is a struct or interface
                if self.is_struct_declaration(&node, source_code)? {
                    EntityKind::Struct
                } else if self.is_interface_declaration(&node, source_code)? {
                    EntityKind::Interface
                } else {
                    // Generic type alias
                    EntityKind::Interface
                }
            }
            // const_declaration and var_declaration are handled separately in extract_entities_recursive
            _ => return Ok(None),
        };

        let name = self
            .extract_name(&node, source_code)?
            .unwrap_or_else(|| {
                // Provide fallback names for entities without extractable names
                match entity_kind {
                    EntityKind::Function => format!("anonymous_function_{}", *entity_id_counter),
                    EntityKind::Method => format!("anonymous_method_{}", *entity_id_counter),
                    EntityKind::Struct => format!("anonymous_struct_{}", *entity_id_counter),
                    EntityKind::Interface => format!("anonymous_interface_{}", *entity_id_counter),
                    EntityKind::Variable => format!("anonymous_variable_{}", *entity_id_counter),
                    EntityKind::Constant => format!("anonymous_constant_{}", *entity_id_counter),
                    _ => format!("anonymous_entity_{}", *entity_id_counter),
                }
            });

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

    /// Extract the name of an entity from its AST node
    fn extract_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        let mut cursor = node.walk();

        match node.kind() {
            "function_declaration" | "method_declaration" => {
                // Use field name if available
                if let Some(name_node) = node.child_by_field_name("name") {
                    return Ok(Some(
                        name_node.utf8_text(source_code.as_bytes())?.to_string(),
                    ));
                }

                // Fallback: Look for the identifier child
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
                    }
                }
            }
            "type_declaration" => {
                // Look for type_spec and then identifier
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_spec" {
                        // Use field name if available
                        if let Some(name_node) = child.child_by_field_name("name") {
                            return Ok(Some(
                                name_node.utf8_text(source_code.as_bytes())?.to_string(),
                            ));
                        }

                        let mut spec_cursor = child.walk();
                        for spec_child in child.children(&mut spec_cursor) {
                            if spec_child.kind() == "type_identifier" {
                                return Ok(Some(
                                    spec_child.utf8_text(source_code.as_bytes())?.to_string(),
                                ));
                            }
                        }
                    }
                }
            }
            // const_declaration and var_declaration are handled separately
            _ => {}
        }

        Ok(None)
    }

    /// Check if a type declaration is a struct
    fn is_struct_declaration(&self, node: &Node, source_code: &str) -> Result<bool> {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec" {
                let mut spec_cursor = child.walk();
                for spec_child in child.children(&mut spec_cursor) {
                    if spec_child.kind() == "struct_type" {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    /// Check if a type declaration is an interface
    fn is_interface_declaration(&self, node: &Node, source_code: &str) -> Result<bool> {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec" {
                let mut spec_cursor = child.walk();
                for spec_child in child.children(&mut spec_cursor) {
                    if spec_child.kind() == "interface_type" {
                        return Ok(true);
                    }
                }
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
        let mut return_types = Vec::new();
        let mut receiver_type = None;

        // Extract parameters using field name
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut param_cursor = params_node.walk();
            for param_child in params_node.children(&mut param_cursor) {
                if param_child.kind() == "parameter_declaration" {
                    let mut inner_cursor = param_child.walk();
                    for inner_child in param_child.children(&mut inner_cursor) {
                        if inner_child.kind() == "identifier" {
                            let param_name = inner_child.utf8_text(source_code.as_bytes())?;
                            parameters.push(param_name);
                        }
                    }
                }
            }
        }

        // Extract return types using field name
        if let Some(result_node) = node.child_by_field_name("result") {
            match result_node.kind() {
                "parameter_list" => {
                    // Multiple return types: (type1, type2)
                    let mut result_cursor = result_node.walk();
                    for result_child in result_node.children(&mut result_cursor) {
                        if result_child.kind() == "parameter_declaration" {
                            // Look for type information
                            let mut inner_cursor = result_child.walk();
                            for inner_child in result_child.children(&mut inner_cursor) {
                                if matches!(
                                    inner_child.kind(),
                                    "type_identifier" | "pointer_type" | "slice_type"
                                ) {
                                    let return_type =
                                        inner_child.utf8_text(source_code.as_bytes())?;
                                    return_types.push(return_type);
                                }
                            }
                        }
                    }
                }
                "type_identifier" | "pointer_type" | "slice_type" => {
                    // Single return type
                    let return_type = result_node.utf8_text(source_code.as_bytes())?;
                    return_types.push(return_type);
                }
                _ => {}
            }
        }

        // For methods, extract receiver using field name
        if node.kind() == "method_declaration" {
            if let Some(receiver_node) = node.child_by_field_name("receiver") {
                let receiver_text = receiver_node.utf8_text(source_code.as_bytes())?;
                receiver_type = Some(receiver_text.to_string());
            }
        }

        metadata.insert("parameters".to_string(), serde_json::json!(parameters));
        if !return_types.is_empty() {
            metadata.insert("return_types".to_string(), serde_json::json!(return_types));
        }
        if let Some(receiver) = receiver_type {
            metadata.insert(
                "receiver_type".to_string(),
                serde_json::Value::String(receiver),
            );
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
        let mut cursor = node.walk();
        let mut fields = Vec::new();
        let mut embedded_types = Vec::new();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec" {
                let mut spec_cursor = child.walk();
                for spec_child in child.children(&mut spec_cursor) {
                    if spec_child.kind() == "struct_type" {
                        let mut struct_cursor = spec_child.walk();
                        for struct_child in spec_child.children(&mut struct_cursor) {
                            if struct_child.kind() == "field_declaration_list" {
                                let mut field_cursor = struct_child.walk();
                                for field_child in struct_child.children(&mut field_cursor) {
                                    if field_child.kind() == "field_declaration" {
                                        let mut inner_cursor = field_child.walk();
                                        let mut field_name = None;
                                        let mut is_embedded = true;

                                        for inner_child in field_child.children(&mut inner_cursor) {
                                            if inner_child.kind() == "field_identifier" {
                                                field_name = Some(
                                                    inner_child
                                                        .utf8_text(source_code.as_bytes())?
                                                        .to_string(),
                                                );
                                                is_embedded = false;
                                            } else if inner_child.kind() == "type_identifier"
                                                && field_name.is_none()
                                            {
                                                // Embedded type
                                                let embedded_type = inner_child
                                                    .utf8_text(source_code.as_bytes())?;
                                                embedded_types.push(embedded_type);
                                            }
                                        }

                                        if let Some(name) = field_name {
                                            fields.push(name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        metadata.insert("fields".to_string(), serde_json::json!(fields));
        if !embedded_types.is_empty() {
            metadata.insert(
                "embedded_types".to_string(),
                serde_json::json!(embedded_types),
            );
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
        let mut cursor = node.walk();
        let mut methods = Vec::new();
        let mut embedded_interfaces = Vec::new();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec" {
                let mut spec_cursor = child.walk();
                for spec_child in child.children(&mut spec_cursor) {
                    if spec_child.kind() == "interface_type" {
                        let mut interface_cursor = spec_child.walk();
                        for interface_child in spec_child.children(&mut interface_cursor) {
                            if interface_child.kind() == "type_elem" {
                                // This is an embedded interface (type embedding in interface)
                                let embedded_interface =
                                    interface_child.utf8_text(source_code.as_bytes())?;
                                embedded_interfaces.push(embedded_interface.to_string());
                            } else if interface_child.kind() == "method_elem" {
                                // This is a method specification
                                let method_text =
                                    interface_child.utf8_text(source_code.as_bytes())?;
                                // Extract method name (everything before the first '(')
                                if let Some(method_name) = method_text.split('(').next() {
                                    let method_name = method_name.trim();
                                    if !method_name.is_empty() {
                                        methods.push(method_name.to_string());
                                    }
                                }
                            } else if interface_child.kind() == "constraint_elem" {
                                // Alternative for embedded interfaces (generics context)
                                let embedded_interface =
                                    interface_child.utf8_text(source_code.as_bytes())?;
                                embedded_interfaces.push(embedded_interface.to_string());
                            } else if interface_child.kind() == "method_spec" {
                                // Alternative method specification format
                                let mut inner_cursor = interface_child.walk();
                                for inner_child in interface_child.children(&mut inner_cursor) {
                                    if inner_child.kind() == "field_identifier" {
                                        let method_name =
                                            inner_child.utf8_text(source_code.as_bytes())?;
                                        methods.push(method_name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        metadata.insert("methods".to_string(), serde_json::json!(methods));
        if !embedded_interfaces.is_empty() {
            metadata.insert(
                "embedded_interfaces".to_string(),
                serde_json::json!(embedded_interfaces),
            );
        }

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

    fn extract_code_entities(&mut self, source: &str, file_path: &str) -> Result<Vec<crate::core::featureset::CodeEntity>> {
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
                language: get_tree_sitter_language("go").unwrap_or_else(|_| tree_sitter_go::LANGUAGE.into()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_adapter_creation() {
        let adapter = GoAdapter::new();
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_function_parsing() {
        let mut adapter = GoAdapter::new().unwrap();
        let source_code = r#"
package main

func add(x int, y int) int {
    return x + y
}

func multiply(a, b float64) (float64, error) {
    return a * b, nil
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.go")
            .unwrap();
        let function_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Function")
            .collect();
        assert_eq!(function_entities.len(), 2);

        let add_func = function_entities.iter().find(|e| e.name == "add").unwrap();
        assert_eq!(add_func.entity_type, "Function");

        let multiply_func = function_entities
            .iter()
            .find(|e| e.name == "multiply")
            .unwrap();
        let return_types = multiply_func.properties.get("return_types");
        assert!(return_types.is_some());
    }

    #[test]
    fn test_struct_parsing() {
        let mut adapter = GoAdapter::new().unwrap();
        let source_code = r#"
package main

type User struct {
    ID   int
    Name string
    Email *string
}

type Point struct {
    X, Y float64
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.go")
            .unwrap();

        let struct_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Struct")
            .collect();
        assert_eq!(struct_entities.len(), 2);

        let user_struct = struct_entities.iter().find(|e| e.name == "User").unwrap();
        assert_eq!(user_struct.entity_type, "Struct");

        let fields = user_struct.properties.get("fields");
        assert!(fields.is_some());
    }

    #[test]
    fn test_interface_parsing() {
        let mut adapter = GoAdapter::new().unwrap();
        let source_code = r#"
package main

type Reader interface {
    Read([]byte) (int, error)
}

type Writer interface {
    Write([]byte) (int, error)
}

type ReadWriter interface {
    Reader
    Writer
    Close() error
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.go")
            .unwrap();

        let interface_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Interface")
            .collect();

        assert_eq!(interface_entities.len(), 3);

        let reader_interface = interface_entities
            .iter()
            .find(|e| e.name == "Reader")
            .unwrap();
        let methods = reader_interface.properties.get("methods");
        assert!(methods.is_some());

        let readwriter_interface = interface_entities
            .iter()
            .find(|e| e.name == "ReadWriter")
            .unwrap();
        let embedded_interfaces = readwriter_interface.properties.get("embedded_interfaces");
        assert!(embedded_interfaces.is_some());
    }

    #[test]
    fn test_method_parsing() {
        let mut adapter = GoAdapter::new().unwrap();
        let source_code = r#"
package main

type Rectangle struct {
    Width, Height float64
}

func (r Rectangle) Area() float64 {
    return r.Width * r.Height
}

func (r *Rectangle) Scale(factor float64) {
    r.Width *= factor
    r.Height *= factor
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.go")
            .unwrap();

        let method_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Method")
            .collect();
        assert_eq!(method_entities.len(), 2);

        let area_method = method_entities.iter().find(|e| e.name == "Area").unwrap();
        assert_eq!(area_method.entity_type, "Method");

        let scale_method = method_entities.iter().find(|e| e.name == "Scale").unwrap();
        let receiver_type = scale_method.properties.get("receiver_type");
        assert!(receiver_type.is_some());
    }

    #[test]
    fn test_const_and_var() {
        let mut adapter = GoAdapter::new().unwrap();
        let source_code = r#"
package main

const Pi = 3.14159
const MaxInt = 1 << 63 - 1

var GlobalCount int
var (
    Name    string
    Version string = "1.0"
)
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.go")
            .unwrap();

        let const_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Constant")
            .collect();
        let var_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Variable")
            .collect();

        assert!(const_entities.len() >= 2); // Pi and MaxInt
        assert!(var_entities.len() >= 3); // GlobalCount, Name, Version

        let pi_const = const_entities.iter().find(|e| e.name == "Pi").unwrap();
        assert_eq!(pi_const.entity_type, "Constant");

        let global_var = var_entities
            .iter()
            .find(|e| e.name == "GlobalCount")
            .unwrap();
        assert_eq!(global_var.entity_type, "Variable");
    }
}
