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
#[path = "rust_lang_tests.rs"]
mod tests;

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
    fn determine_entity_kind(&self, node: Node) -> Option<EntityKind> {
        match node.kind() {
            "function_item" | "function_signature_item" => {
                if self.is_inside_trait(node) { None } else { Some(EntityKind::Function) }
            }
            "impl_item" => None,
            "struct_item" => Some(EntityKind::Struct),
            "enum_item" => Some(EntityKind::Enum),
            "trait_item" => Some(EntityKind::Interface),
            "mod_item" => Some(EntityKind::Module),
            "const_item" | "static_item" => Some(EntityKind::Constant),
            _ => None,
        }
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
        let entity_kind = match self.determine_entity_kind(node) {
            Some(kind) => kind,
            None => return Ok(None),
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
        self.extract_entity_metadata(&entity_kind, &node, source_code, &mut metadata)?;

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

    /// Check if a node or its function_modifiers children contain a specific modifier kind.
    fn has_modifier(node: &Node, modifier_kind: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == modifier_kind {
                return true;
            }
            if child.kind() == "function_modifiers" {
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() == modifier_kind {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Extract parameter names from a parameters node.
    fn extract_parameters<'a>(params_node: &Node, source_code: &'a str) -> Result<Vec<&'a str>> {
        let mut parameters = Vec::new();
        let mut cursor = params_node.walk();
        for param in params_node.children(&mut cursor) {
            if param.kind() == "parameter" {
                let mut inner = param.walk();
                for child in param.children(&mut inner) {
                    if child.kind() == "identifier" {
                        parameters.push(child.utf8_text(source_code.as_bytes())?);
                        break;
                    }
                }
            }
        }
        Ok(parameters)
    }

    /// Extract metadata based on entity kind, dispatching to the appropriate extractor.
    fn extract_entity_metadata(
        &self,
        entity_kind: &EntityKind,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, Value>,
    ) -> Result<()> {
        match entity_kind {
            EntityKind::Function => self.extract_function_metadata(node, source_code, metadata),
            EntityKind::Struct => self.extract_struct_metadata(node, source_code, metadata),
            EntityKind::Enum => self.extract_enum_metadata(node, source_code, metadata),
            EntityKind::Interface => self.extract_trait_metadata(node, source_code, metadata),
            EntityKind::Module => self.extract_module_metadata(node, source_code, metadata),
            _ => Ok(()),
        }
    }

    /// Extract function-specific metadata
    fn extract_function_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, Value>,
    ) -> Result<()> {
        let is_async = Self::has_modifier(node, "async");
        let is_unsafe = Self::has_modifier(node, "unsafe");
        let is_const = Self::has_modifier(node, "const");

        let mut parameters = Vec::new();
        let mut return_type = None;
        let mut visibility = "private".to_string();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "parameters" => parameters = Self::extract_parameters(&child, source_code)?,
                "visibility_modifier" => visibility = child.utf8_text(source_code.as_bytes())?.to_string(),
                "type_identifier" | "reference_type" | "tuple_type" | "array_type" | "generic_type" => {
                    return_type = Some(child.utf8_text(source_code.as_bytes())?.to_string());
                }
                _ => {}
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

    /// Extract identifiers from nested children matching outer/inner kind patterns.
    fn extract_nested_identifiers<'a>(
        parent: &Node,
        source_code: &'a str,
        outer_kind: &str,
        inner_kind: &str,
    ) -> Result<Vec<&'a str>> {
        let mut results = Vec::new();
        let mut cursor = parent.walk();
        for child in parent.children(&mut cursor) {
            if child.kind() == outer_kind {
                let mut inner = child.walk();
                for inner_child in child.children(&mut inner) {
                    if inner_child.kind() == inner_kind {
                        results.push(inner_child.utf8_text(source_code.as_bytes())?);
                    }
                }
            }
        }
        Ok(results)
    }

    /// Extract struct-specific metadata
    fn extract_struct_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, Value>,
    ) -> Result<()> {
        let mut fields = Vec::new();
        let mut visibility = "private".to_string();
        let mut generic_params = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "field_declaration_list" => {
                    fields = Self::extract_nested_identifiers(&child, source_code, "field_declaration", "field_identifier")?;
                }
                "visibility_modifier" => {
                    visibility = child.utf8_text(source_code.as_bytes())?.to_string();
                }
                "type_parameters" => {
                    generic_params = Self::extract_nested_identifiers(&child, source_code, "type_parameter", "type_identifier")?;
                }
                _ => {}
            }
        }

        metadata.insert("fields".to_string(), serde_json::json!(fields));
        metadata.insert("visibility".to_string(), Value::String(visibility));
        if !generic_params.is_empty() {
            metadata.insert("generic_parameters".to_string(), serde_json::json!(generic_params));
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
                    variants = self.collect_enum_variants(&child, source_code)?;
                }
                "visibility_modifier" => {
                    visibility = self.extract_visibility(&child, source_code)?;
                }
                _ => {}
            }
        }

        metadata.insert("variants".to_string(), serde_json::json!(variants));
        metadata.insert("visibility".to_string(), Value::String(visibility));

        Ok(())
    }

    /// Collect enum variant names from an enum_variant_list node
    fn collect_enum_variants<'a>(
        &self,
        list_node: &Node<'a>,
        source_code: &'a str,
    ) -> Result<Vec<&'a str>> {
        let mut variants = Vec::new();
        let mut cursor = list_node.walk();

        for variant_child in list_node.children(&mut cursor) {
            if variant_child.kind() != "enum_variant" {
                continue;
            }
            if let Some(name) = self.find_identifier_in_node(&variant_child, source_code)? {
                variants.push(name);
            }
        }

        Ok(variants)
    }

    /// Find the first identifier child in a node
    fn find_identifier_in_node<'a>(
        &self,
        node: &Node<'a>,
        source_code: &'a str,
    ) -> Result<Option<&'a str>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Ok(Some(child.utf8_text(source_code.as_bytes())?));
            }
        }
        Ok(None)
    }

    /// Extract visibility modifier text from a node
    fn extract_visibility(&self, node: &Node, source_code: &str) -> Result<String> {
        Ok(node.utf8_text(source_code.as_bytes())?.to_string())
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
                    methods = self.collect_trait_methods(&child, source_code)?;
                }
                "visibility_modifier" => {
                    visibility = self.extract_visibility(&child, source_code)?;
                }
                "trait_bounds" => {
                    supertrait_bounds = self.collect_trait_bounds(&child, source_code)?;
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

    /// Collect method names from a trait's declaration_list node
    fn collect_trait_methods(&self, list_node: &Node, source_code: &str) -> Result<Vec<String>> {
        let mut methods = Vec::new();
        let mut cursor = list_node.walk();

        for child in list_node.children(&mut cursor) {
            if child.kind() != "function_signature_item" {
                continue;
            }
            if let Some(name) = self.extract_name(&child, source_code)? {
                methods.push(name);
            }
        }

        Ok(methods)
    }

    /// Collect supertrait bounds from a trait_bounds node
    fn collect_trait_bounds<'a>(
        &self,
        bounds_node: &Node<'a>,
        source_code: &'a str,
    ) -> Result<Vec<&'a str>> {
        let mut bounds = Vec::new();
        let mut cursor = bounds_node.walk();

        for child in bounds_node.children(&mut cursor) {
            if child.kind() == "type_identifier" {
                bounds.push(child.utf8_text(source_code.as_bytes())?);
            }
        }

        Ok(bounds)
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

            // Handle mod declarations (e.g., `mod foo;` or `pub mod bar;`)
            let mod_part = trimmed
                .strip_prefix("pub mod ")
                .or_else(|| trimmed.strip_prefix("pub(crate) mod "))
                .or_else(|| trimmed.strip_prefix("pub(super) mod "))
                .or_else(|| trimmed.strip_prefix("mod "));

            if let Some(mod_decl) = mod_part {
                // Only handle external mod declarations (ending with ;), not inline modules
                if let Some(mod_name) = mod_decl.strip_suffix(';') {
                    let mod_name = mod_name.trim();
                    if !mod_name.is_empty() && !mod_name.contains('{') {
                        imports.push(ImportStatement {
                            module: mod_name.to_string(),
                            imports: None,
                            import_type: "mod".to_string(),
                            line_number: line_number + 1,
                        });
                    }
                }
            }

            // Handle use statements
            if let Some(use_part) = trimmed.strip_prefix("use ") {
                let use_part = use_part.trim_end_matches(';');
                Self::parse_use_statement(use_part, line_number + 1, &mut imports);
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

impl RustAdapter {
    /// Parse a use statement and extract all module references
    fn parse_use_statement(use_part: &str, line_number: usize, imports: &mut Vec<ImportStatement>) {
        // Handle grouped imports: use foo::{bar, baz};
        if let Some(brace_pos) = use_part.find('{') {
            let module = use_part[..brace_pos].trim().trim_end_matches(':').to_string();
            let items_part = &use_part[brace_pos + 1..];

            if let Some(close_brace) = items_part.find('}') {
                let items = &items_part[..close_brace];
                let specific_imports: Vec<String> = items
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                // Add the grouped import with all named items
                if !module.is_empty() {
                    imports.push(ImportStatement {
                        module: format!("{}::", module),
                        imports: Some(specific_imports),
                        import_type: "named".to_string(),
                        line_number,
                    });
                }
            }
        } else {
            // Simple use: use foo::bar;
            imports.push(ImportStatement {
                module: use_part.to_string(),
                imports: None,
                import_type: "module".to_string(),
                line_number,
            });
        }
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
