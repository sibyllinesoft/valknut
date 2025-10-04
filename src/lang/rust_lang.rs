//! Rust language adapter with tree-sitter integration.

use serde_json::{self, Value};
use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Tree};

use super::common::{EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation};
use super::registry::{create_parser_for_language, get_tree_sitter_language};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::CodeEntity;
use crate::detectors::structure::config::ImportStatement;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_adapter_creation() {
        let adapter = RustAdapter::new();
        assert!(adapter.is_ok(), "Should create Rust adapter successfully");
    }

    #[test]
    fn test_parse_simple_function() {
        let mut adapter = RustAdapter::new().unwrap();
        let source = r#"
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#;
        let result = adapter.parse_source(source, "test.rs");
        assert!(result.is_ok(), "Should parse simple function");

        let index = result.unwrap();
        assert!(
            index.get_entities_in_file("test.rs").len() >= 1,
            "Should find at least one entity"
        );
    }

    #[test]
    fn test_parse_struct_and_impl() {
        let mut adapter = RustAdapter::new().unwrap();
        let source = r#"
struct User {
    name: String,
    age: u32,
}

impl User {
    fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }
    
    fn get_name(&self) -> &str {
        &self.name
    }
}
"#;
        let result = adapter.parse_source(source, "test.rs");
        assert!(result.is_ok(), "Should parse struct and impl");

        let index = result.unwrap();
        let entities = index.get_entities_in_file("test.rs");
        assert!(
            entities.len() >= 2,
            "Should find at least struct and impl entities"
        );

        let has_struct = entities
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Struct));
        assert!(has_struct, "Should find a struct entity");
    }

    #[test]
    fn test_parse_traits_and_enums() {
        let mut adapter = RustAdapter::new().unwrap();
        let source = r#"
trait Display {
    fn display(&self) -> String;
}

enum Color {
    Red,
    Green,
    Blue,
}

impl Display for Color {
    fn display(&self) -> String {
        match self {
            Color::Red => "Red".to_string(),
            Color::Green => "Green".to_string(),
            Color::Blue => "Blue".to_string(),
        }
    }
}
"#;
        let result = adapter.parse_source(source, "traits.rs");
        assert!(result.is_ok(), "Should parse traits and enums");

        let index = result.unwrap();
        let entities = index.get_entities_in_file("traits.rs");
        assert!(entities.len() >= 2, "Should find multiple entities");

        let has_enum = entities.iter().any(|e| matches!(e.kind, EntityKind::Enum));
        assert!(has_enum, "Should find an enum entity");
    }

    #[test]
    fn test_parse_modules() {
        let mut adapter = RustAdapter::new().unwrap();
        let source = r#"
mod network {
    use std::net::TcpStream;
    
    pub fn connect(addr: &str) -> Result<TcpStream, std::io::Error> {
        TcpStream::connect(addr)
    }
}

pub mod utils {
    pub fn format_string(s: &str) -> String {
        s.to_uppercase()
    }
}
"#;
        let result = adapter.parse_source(source, "modules.rs");
        assert!(result.is_ok(), "Should parse modules");

        let index = result.unwrap();
        let entities = index.get_entities_in_file("modules.rs");
        assert!(
            entities.len() >= 2,
            "Should find multiple entities including modules"
        );

        let has_module = entities
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Module));
        assert!(has_module, "Should find module entities");
    }

    #[test]
    fn test_empty_rust_file() {
        let mut adapter = RustAdapter::new().unwrap();
        let source = "// Rust file with just comments\n/* Block comment */";
        let result = adapter.parse_source(source, "empty.rs");
        assert!(result.is_ok(), "Should handle empty Rust file");

        let index = result.unwrap();
        let entities = index.get_entities_in_file("empty.rs");
        assert_eq!(
            entities.len(),
            0,
            "Should find no entities in comment-only file"
        );
    }
}

/// Rust-specific parsing and analysis
pub struct RustAdapter {
    /// Tree-sitter parser for Rust
    parser: Parser,

    /// Language instance
    language: Language,
}

impl RustAdapter {
    /// Create a new Rust adapter
    pub fn new() -> Result<Self> {
        let language = get_tree_sitter_language("rs")?;
        let parser = create_parser_for_language("rs")?;

        Ok(Self { parser, language })
    }

    fn parse_tree(&mut self, source_code: &str) -> Result<Tree> {
        self.parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("rust", "Failed to parse Rust source"))
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

    /// Parse Rust source code and extract entities
    pub fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("rust", "Failed to parse Rust source code"))?;

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

    /// Extract entities from Rust code and convert to CodeEntity format
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
            "function_item" => {
                // Skip function items that are inside traits
                // They should be included as metadata of the trait, not separate entities
                if self.is_inside_trait(node) {
                    return Ok(None);
                }
                EntityKind::Function
            }
            "impl_item" => return Ok(None), // Skip impl blocks themselves
            "struct_item" => EntityKind::Struct,
            "enum_item" => EntityKind::Enum,
            "trait_item" => EntityKind::Interface, // Treat traits as interfaces
            "mod_item" => EntityKind::Module,
            "const_item" => EntityKind::Constant,
            "static_item" => EntityKind::Constant,
            "function_signature_item" => {
                // Skip function signatures that are inside traits
                // They should be included as metadata of the trait, not separate entities
                if self.is_inside_trait(node) {
                    return Ok(None);
                }
                EntityKind::Function
            }
            _ => return Ok(None),
        };

        let name = self
            .extract_name(&node, source_code)?
            .ok_or_else(|| ValknutError::parse("rust", "Could not extract entity name"))?;

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

        // Add Rust-specific metadata
        metadata.insert(
            "node_kind".to_string(),
            Value::String(node.kind().to_string()),
        );
        metadata.insert(
            "byte_range".to_string(),
            serde_json::json!([node.start_byte(), node.end_byte()]),
        );

        // Extract additional metadata based on entity type
        match entity_kind {
            EntityKind::Function => {
                self.extract_function_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Struct => {
                self.extract_struct_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Enum => {
                self.extract_enum_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Interface => {
                // trait
                self.extract_trait_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Module => {
                self.extract_module_metadata(&node, source_code, &mut metadata)?;
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
            "function_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "mod_item"
            | "const_item"
            | "static_item"
            | "function_signature_item" => {
                // Look for the identifier child
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
                    } else if child.kind() == "type_identifier" {
                        return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
                    }
                }
            }
            _ => {}
        }

        Ok(None)
    }

    /// Extract function-specific metadata
    fn extract_function_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut parameters = Vec::new();
        let mut is_async = false;
        let mut is_unsafe = false;
        let mut is_const = false;
        let mut return_type = None;
        let mut visibility = "private".to_string();

        // Check for modifiers in the function signature using AST structure
        // Look for modifier nodes before the function keyword
        let mut signature_cursor = node.walk();
        for sig_child in node.children(&mut signature_cursor) {
            match sig_child.kind() {
                "async" => is_async = true,
                "unsafe" => is_unsafe = true,
                "const" => is_const = true,
                "function_modifiers" => {
                    // Check inside function_modifiers for async/unsafe
                    let mut mod_cursor = sig_child.walk();
                    for mod_child in sig_child.children(&mut mod_cursor) {
                        match mod_child.kind() {
                            "async" => is_async = true,
                            "unsafe" => is_unsafe = true,
                            "const" => is_const = true,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        for child in node.children(&mut cursor) {
            match child.kind() {
                "parameters" => {
                    // Extract parameter information
                    let mut param_cursor = child.walk();
                    for param_child in child.children(&mut param_cursor) {
                        if param_child.kind() == "parameter" {
                            let mut inner_cursor = param_child.walk();
                            for inner_child in param_child.children(&mut inner_cursor) {
                                if inner_child.kind() == "identifier" {
                                    let param_name =
                                        inner_child.utf8_text(source_code.as_bytes())?;
                                    parameters.push(param_name);
                                    break;
                                }
                            }
                        }
                    }
                }
                "visibility_modifier" => {
                    let vis_text = child.utf8_text(source_code.as_bytes())?;
                    visibility = vis_text.to_string();
                }
                _ => {
                    // Check for specific return type nodes in function signature
                    if matches!(
                        child.kind(),
                        "type_identifier"
                            | "reference_type"
                            | "tuple_type"
                            | "array_type"
                            | "generic_type"
                    ) {
                        return_type = Some(child.utf8_text(source_code.as_bytes())?.to_string());
                    }
                }
            }
        }

        metadata.insert("parameters".to_string(), serde_json::json!(parameters));
        metadata.insert("is_async".to_string(), Value::Bool(is_async));
        metadata.insert("is_unsafe".to_string(), Value::Bool(is_unsafe));
        metadata.insert("is_const".to_string(), Value::Bool(is_const));
        metadata.insert("visibility".to_string(), Value::String(visibility));
        if let Some(ret_type) = return_type {
            metadata.insert("return_type".to_string(), Value::String(ret_type));
        }

        Ok(())
    }

    /// Extract struct-specific metadata
    fn extract_struct_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut fields = Vec::new();
        let mut visibility = "private".to_string();
        let mut generic_params = Vec::new();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "field_declaration_list" => {
                    let mut field_cursor = child.walk();
                    for field_child in child.children(&mut field_cursor) {
                        if field_child.kind() == "field_declaration" {
                            let mut inner_cursor = field_child.walk();
                            for inner_child in field_child.children(&mut inner_cursor) {
                                if inner_child.kind() == "field_identifier" {
                                    let field_name =
                                        inner_child.utf8_text(source_code.as_bytes())?;
                                    fields.push(field_name);
                                }
                            }
                        }
                    }
                }
                "visibility_modifier" => {
                    let vis_text = child.utf8_text(source_code.as_bytes())?;
                    visibility = vis_text.to_string();
                }
                "type_parameters" => {
                    let mut param_cursor = child.walk();
                    for param_child in child.children(&mut param_cursor) {
                        if param_child.kind() == "type_parameter" {
                            // Look for the name field within the type_parameter
                            let mut inner_cursor = param_child.walk();
                            for inner_child in param_child.children(&mut inner_cursor) {
                                if inner_child.kind() == "type_identifier" {
                                    let param_name =
                                        inner_child.utf8_text(source_code.as_bytes())?;
                                    generic_params.push(param_name);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        metadata.insert("fields".to_string(), serde_json::json!(fields));
        metadata.insert("visibility".to_string(), Value::String(visibility));
        if !generic_params.is_empty() {
            metadata.insert(
                "generic_parameters".to_string(),
                serde_json::json!(generic_params),
            );
        }

        Ok(())
    }

    /// Extract enum-specific metadata
    fn extract_enum_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut variants = Vec::new();
        let mut visibility = "private".to_string();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "enum_variant_list" => {
                    let mut variant_cursor = child.walk();
                    for variant_child in child.children(&mut variant_cursor) {
                        if variant_child.kind() == "enum_variant" {
                            let mut inner_cursor = variant_child.walk();
                            for inner_child in variant_child.children(&mut inner_cursor) {
                                if inner_child.kind() == "identifier" {
                                    let variant_name =
                                        inner_child.utf8_text(source_code.as_bytes())?;
                                    variants.push(variant_name);
                                    break;
                                }
                            }
                        }
                    }
                }
                "visibility_modifier" => {
                    let vis_text = child.utf8_text(source_code.as_bytes())?;
                    visibility = vis_text.to_string();
                }
                _ => {}
            }
        }

        metadata.insert("variants".to_string(), serde_json::json!(variants));
        metadata.insert("visibility".to_string(), Value::String(visibility));

        Ok(())
    }

    /// Extract trait-specific metadata
    fn extract_trait_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut methods = Vec::new();
        let mut visibility = "private".to_string();
        let mut supertrait_bounds = Vec::new();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "declaration_list" => {
                    let mut method_cursor = child.walk();
                    for method_child in child.children(&mut method_cursor) {
                        if method_child.kind() == "function_signature_item" {
                            let method_name = self.extract_name(&method_child, source_code)?;
                            if let Some(name) = method_name {
                                methods.push(name);
                            }
                        }
                    }
                }
                "visibility_modifier" => {
                    let vis_text = child.utf8_text(source_code.as_bytes())?;
                    visibility = vis_text.to_string();
                }
                "trait_bounds" => {
                    let mut bounds_cursor = child.walk();
                    for bounds_child in child.children(&mut bounds_cursor) {
                        if bounds_child.kind() == "type_identifier" {
                            let bound_name = bounds_child.utf8_text(source_code.as_bytes())?;
                            supertrait_bounds.push(bound_name);
                        }
                    }
                }
                _ => {}
            }
        }

        metadata.insert("methods".to_string(), serde_json::json!(methods));
        metadata.insert("visibility".to_string(), Value::String(visibility));
        if !supertrait_bounds.is_empty() {
            metadata.insert(
                "supertrait_bounds".to_string(),
                serde_json::json!(supertrait_bounds),
            );
        }

        Ok(())
    }

    /// Extract module-specific metadata
    fn extract_module_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut visibility = "private".to_string();
        let mut is_inline = false;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "visibility_modifier" => {
                    let vis_text = child.utf8_text(source_code.as_bytes())?;
                    visibility = vis_text.to_string();
                }
                "declaration_list" => {
                    is_inline = true; // Has a body, so it's an inline module
                }
                _ => {}
            }
        }

        metadata.insert("visibility".to_string(), Value::String(visibility));
        metadata.insert("is_inline".to_string(), Value::Bool(is_inline));

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

    /// Check if a node is inside a trait definition
    fn is_inside_trait(&self, node: Node) -> bool {
        let mut current = node.parent();
        while let Some(parent) = current {
            if parent.kind() == "trait_item" {
                return true;
            }
            current = parent.parent();
        }
        false
    }
}

impl LanguageAdapter for RustAdapter {
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        RustAdapter::parse_source(self, source, file_path)
    }

    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        let mut calls = Vec::new();

        Self::walk_tree(tree.root_node(), &mut |node| {
            let target = match node.kind() {
                "call_expression" => node.child_by_field_name("function"),
                "macro_invocation" => node.child_by_field_name("macro"),
                _ => None,
            };

            if let Some(candidate) = target.or_else(|| node.child(0)) {
                if let Ok(text) = Self::node_text(&candidate, source) {
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
            "identifier" | "field_identifier" | "type_identifier" | "scoped_identifier"
            | "lifetime" => {
                if let Ok(text) = Self::node_text(&node, source) {
                    let cleaned = text.trim();
                    if !cleaned.is_empty() {
                        identifiers.push(cleaned.trim_matches('"').to_string());
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
        let index = RustAdapter::parse_source(self, source, "<memory>")?;
        Ok(index.count_distinct_blocks())
    }

    fn normalize_source(&mut self, source: &str) -> Result<String> {
        let tree = self.parse_tree(source)?;
        Ok(tree.root_node().to_sexp())
    }

    fn language_name(&self) -> &str {
        "rust"
    }

    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        let mut imports = Vec::new();

        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }

            if let Some(use_part) = trimmed.strip_prefix("use ") {
                let use_part = use_part.trim_end_matches(';');

                if let Some(brace_pos) = use_part.find('{') {
                    let module = use_part[..brace_pos].trim().to_string();
                    let items_part = &use_part[brace_pos + 1..];

                    if let Some(close_brace) = items_part.find('}') {
                        let items = &items_part[..close_brace];
                        let specific_imports =
                            Some(items.split(',').map(|s| s.trim().to_string()).collect());

                        imports.push(ImportStatement {
                            module,
                            imports: specific_imports,
                            import_type: "named".to_string(),
                            line_number: line_number + 1,
                        });
                    }
                } else {
                    imports.push(ImportStatement {
                        module: use_part.to_string(),
                        imports: None,
                        import_type: "module".to_string(),
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
        RustAdapter::extract_code_entities(self, source, file_path)
    }
}

impl Default for RustAdapter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!(
                "Warning: Failed to create Rust adapter, using minimal fallback: {}",
                e
            );
            RustAdapter {
                parser: tree_sitter::Parser::new(),
                language: get_tree_sitter_language("rs")
                    .unwrap_or_else(|_| tree_sitter_rust::LANGUAGE.into()),
            }
        })
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn test_rust_adapter_creation_additional() {
        let adapter = RustAdapter::new();
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_function_parsing() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
pub fn calculate(x: i32, y: i32) -> i32 {
    x + y
}

async unsafe fn complex_function() -> Result<(), Error> {
    Ok(())
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();
        assert_eq!(entities.len(), 2);

        let calc_func = entities.iter().find(|e| e.name == "calculate").unwrap();
        assert_eq!(calc_func.entity_type, "Function");
        assert_eq!(
            calc_func.properties.get("visibility"),
            Some(&Value::String("pub".to_string()))
        );

        let complex_func = entities
            .iter()
            .find(|e| e.name == "complex_function")
            .unwrap();
        assert_eq!(
            complex_func.properties.get("is_async"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            complex_func.properties.get("is_unsafe"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn test_struct_parsing() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
pub struct User {
    id: u64,
    name: String,
    email: Option<String>,
}

struct Point<T> {
    x: T,
    y: T,
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();

        let struct_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Struct")
            .collect();
        assert_eq!(struct_entities.len(), 2);

        let user_struct = struct_entities.iter().find(|e| e.name == "User").unwrap();
        assert_eq!(
            user_struct.properties.get("visibility"),
            Some(&Value::String("pub".to_string()))
        );

        let point_struct = struct_entities.iter().find(|e| e.name == "Point").unwrap();
        let generic_params = point_struct.properties.get("generic_parameters");
        assert!(generic_params.is_some());
    }

    #[test]
    fn test_enum_parsing() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
#[derive(Debug, Clone)]
pub enum Status {
    Active,
    Inactive,
    Pending(String),
    Expired { reason: String },
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();
        assert_eq!(entities.len(), 1);

        let enum_entity = &entities[0];
        assert_eq!(enum_entity.entity_type, "Enum");
        assert_eq!(enum_entity.name, "Status");
        assert_eq!(
            enum_entity.properties.get("visibility"),
            Some(&Value::String("pub".to_string()))
        );

        let variants = enum_entity.properties.get("variants");
        assert!(variants.is_some());
    }

    #[test]
    fn test_trait_parsing() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
pub trait Display: Debug + Clone {
    fn fmt(&self) -> String;
    fn print(&self) {
        println!("{}", self.fmt());
    }
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();
        assert_eq!(entities.len(), 1);

        let trait_entity = &entities[0];
        assert_eq!(trait_entity.entity_type, "Interface");
        assert_eq!(trait_entity.name, "Display");
        assert_eq!(
            trait_entity.properties.get("visibility"),
            Some(&Value::String("pub".to_string()))
        );

        let methods = trait_entity.properties.get("methods");
        assert!(methods.is_some());
    }

    #[test]
    fn test_module_parsing() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
pub mod utils;

mod internal {
    pub fn helper() -> i32 {
        42
    }
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();

        let module_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Module")
            .collect();
        assert!(module_entities.len() >= 2); // utils and internal modules

        let internal_mod = module_entities
            .iter()
            .find(|e| e.name == "internal")
            .unwrap();
        assert_eq!(
            internal_mod.properties.get("is_inline"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn test_const_and_static() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
const PI: f64 = 3.14159;
static GLOBAL_COUNT: AtomicUsize = AtomicUsize::new(0);
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();

        let const_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Constant")
            .collect();
        assert_eq!(const_entities.len(), 2);

        let pi_const = const_entities.iter().find(|e| e.name == "PI").unwrap();
        assert_eq!(pi_const.entity_type, "Constant");

        let global_static = const_entities
            .iter()
            .find(|e| e.name == "GLOBAL_COUNT")
            .unwrap();
        assert_eq!(global_static.entity_type, "Constant");
    }
}
