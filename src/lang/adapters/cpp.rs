//! C++ language adapter with tree-sitter integration.

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

/// C++-specific parsing and analysis
pub struct CppAdapter {
    /// Tree-sitter parser for C++
    parser: Parser,

    /// Language instance
    language: Language,
}

/// Parsing and entity extraction methods for [`CppAdapter`].
impl CppAdapter {
    /// Create a new C++ adapter
    pub fn new() -> Result<Self> {
        let language = get_tree_sitter_language("cpp")?;
        let parser = create_parser_for_language("cpp")?;

        Ok(Self { parser, language })
    }

    /// Parse C++ source code and extract entities
    pub fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("cpp", "Failed to parse C++ source code"))?;

        let mut index = ParseIndex::new();
        let mut entity_id_counter = 0;

        // Walk the tree and extract entities (iterative to avoid stack overflow)
        self.extract_entities_iterative_cpp(
            tree.root_node(),
            source_code,
            file_path,
            &mut index,
            &mut entity_id_counter,
        )?;

        Ok(index)
    }

    /// Extract entities from C++ code and convert to CodeEntity format
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

    /// Iterative entity extraction for C++ - avoids stack overflow on deeply nested code.
    fn extract_entities_iterative_cpp(
        &self,
        root: Node,
        source_code: &str,
        file_path: &str,
        index: &mut ParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
        // Stack entries: (node, parent_id, namespace_context)
        let mut stack: Vec<(Node, Option<String>, Vec<String>)> = vec![(root, None, Vec::new())];

        while let Some((node, parent_id, namespace_ctx)) = stack.pop() {
            // For ERROR nodes, still try to process children - they may contain valid code
            // This handles partial parse failures gracefully
            if node.is_error() || node.kind() == "ERROR" {
                // Push children of error node - they might be valid
                let mut cursor = node.walk();
                let children: Vec<_> = node.children(&mut cursor).collect();
                for child in children.into_iter().rev() {
                    if !child.is_error() {
                        stack.push((child, parent_id.clone(), namespace_ctx.clone()));
                    }
                }
                continue;
            }

            // Skip preprocessor directives that don't contain code blocks
            let skip_kinds = [
                "preproc_include",
                "preproc_def",
                "preproc_function_def",
                "preproc_call",
                "preproc_directive",
            ];
            if skip_kinds.contains(&node.kind()) {
                continue;
            }

            // Process this node
            let (new_parent_id, new_namespace_ctx) = if let Some(entity) = self.node_to_entity_cpp(
                node,
                source_code,
                file_path,
                parent_id.clone(),
                entity_id_counter,
                &namespace_ctx,
            )? {
                let entity_id = entity.id.clone();
                let mut updated_ctx = namespace_ctx.clone();

                // If this is a namespace, add it to context for children
                if entity.kind == EntityKind::Module {
                    updated_ctx.push(entity.name.clone());
                }

                index.add_entity(entity);
                (Some(entity_id), updated_ctx)
            } else {
                (parent_id, namespace_ctx)
            };

            // Push children in reverse order for depth-first traversal
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            for child in children.into_iter().rev() {
                stack.push((child, new_parent_id.clone(), new_namespace_ctx.clone()));
            }
        }

        Ok(())
    }

    /// Convert a tree-sitter node to a ParsedEntity if it represents a code entity.
    fn node_to_entity_cpp(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<String>,
        entity_id_counter: &mut usize,
        namespace_ctx: &[String],
    ) -> Result<Option<ParsedEntity>> {
        let kind = match self.determine_entity_kind(&node, source_code)? {
            Some(k) => k,
            None => return Ok(None),
        };

        let name = match self.extract_name(&node, source_code, &kind)? {
            Some(n) => n,
            None => return Ok(None),
        };

        // Build fully qualified name if we're in a namespace
        let qualified_name = if namespace_ctx.is_empty() {
            name
        } else {
            format!("{}::{}", namespace_ctx.join("::"), name)
        };

        *entity_id_counter += 1;
        let entity_id = generate_entity_id(file_path, kind.clone(), *entity_id_counter);

        let location = SourceLocation::from_positions(
            file_path,
            node.start_position().row,
            node.start_position().column,
            node.end_position().row,
            node.end_position().column,
        );

        let mut metadata = create_base_metadata(node.kind(), node.start_byte(), node.end_byte());

        // Extract kind-specific metadata
        self.extract_entity_metadata(kind.clone(), &node, source_code, &mut metadata)?;

        Ok(Some(ParsedEntity {
            id: entity_id,
            name: qualified_name,
            kind,
            location,
            parent: parent_id,
            children: Vec::new(),
            metadata,
        }))
    }

    /// Determine entity kind from node kind.
    fn determine_entity_kind(&self, node: &Node, source_code: &str) -> Result<Option<EntityKind>> {
        Ok(match node.kind() {
            // Functions
            "function_definition" => {
                // Check if this is a method (inside a class) by looking at the declarator
                if self.is_method_definition(node) {
                    Some(EntityKind::Method)
                } else {
                    Some(EntityKind::Function)
                }
            }

            // Classes and structs - only if they have a body (not forward declarations)
            "class_specifier" => {
                // Forward declarations like `class Widget;` don't have field_declaration_list
                if self.has_body(node) {
                    Some(EntityKind::Class)
                } else {
                    None
                }
            }
            "struct_specifier" => {
                // Only create entity if it has a name and body (not anonymous or forward decl)
                if self.has_type_name(node, source_code) && self.has_body(node) {
                    Some(EntityKind::Struct)
                } else {
                    None
                }
            }

            // Namespaces
            "namespace_definition" => Some(EntityKind::Module),

            // Enums
            "enum_specifier" => {
                if self.has_type_name(node, source_code) {
                    Some(EntityKind::Enum)
                } else {
                    None
                }
            }

            // Template declarations - extract the underlying entity
            "template_declaration" => {
                // We'll process the inner declaration, not the template wrapper
                None
            }

            // Type aliases and typedefs
            "type_definition" | "alias_declaration" => Some(EntityKind::Interface),

            _ => None,
        })
    }

    /// Check if a function definition is actually a method (has :: in declarator or inside class body)
    fn is_method_definition(&self, node: &Node) -> bool {
        // Look for qualified_identifier in the declarator chain
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "function_declarator" {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "qualified_identifier"
                        || inner.kind() == "template_method"
                        || inner.kind() == "destructor_name"
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if a type specifier has a name.
    fn has_type_name(&self, node: &Node, source_code: &str) -> bool {
        // Look for name field or type_identifier child
        if node.child_by_field_name("name").is_some() {
            return true;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_identifier" {
                return true;
            }
        }
        false
    }

    /// Check if a class/struct specifier has a body (not a forward declaration).
    fn has_body(&self, node: &Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "field_declaration_list" {
                return true;
            }
        }
        false
    }

    /// Extract the name of an entity from its AST node.
    fn extract_name(
        &self,
        node: &Node,
        source_code: &str,
        kind: &EntityKind,
    ) -> Result<Option<String>> {
        match node.kind() {
            "function_definition" => self.extract_function_name(node, source_code),
            "class_specifier" | "struct_specifier" | "enum_specifier" => {
                self.extract_type_name(node, source_code)
            }
            "namespace_definition" => self.extract_namespace_name(node, source_code),
            "type_definition" => self.extract_typedef_name(node, source_code),
            "alias_declaration" => self.extract_alias_name(node, source_code),
            _ => Ok(None),
        }
    }

    /// Extract function name from a function_definition node.
    fn extract_function_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        // Navigate: function_definition -> function_declarator -> (identifier | qualified_identifier | destructor_name)
        let declarator = self.find_function_declarator(node)?;

        if let Some(decl) = declarator {
            // Check for destructor
            if let Some(dtor) = find_child_by_kind(&decl, "destructor_name") {
                if let Some(id) = find_child_by_kind(&dtor, "identifier") {
                    let name = id.utf8_text(source_code.as_bytes())?;
                    return Ok(Some(format!("~{}", name)));
                }
            }

            // Check for qualified identifier (Class::method)
            if let Some(qualified) = find_child_by_kind(&decl, "qualified_identifier") {
                // Get the last identifier (the actual function name)
                let mut last_name = None;
                let mut cursor = qualified.walk();
                for child in qualified.children(&mut cursor) {
                    if child.kind() == "identifier" || child.kind() == "destructor_name" {
                        last_name = Some(child);
                    }
                }
                if let Some(name_node) = last_name {
                    if name_node.kind() == "destructor_name" {
                        if let Some(id) = find_child_by_kind(&name_node, "identifier") {
                            let name = id.utf8_text(source_code.as_bytes())?;
                            return Ok(Some(format!("~{}", name)));
                        }
                    }
                    return Ok(Some(name_node.utf8_text(source_code.as_bytes())?.to_string()));
                }
            }

            // Check for operator overloading
            if let Some(op) = find_child_by_kind(&decl, "operator_name") {
                return Ok(Some(op.utf8_text(source_code.as_bytes())?.to_string()));
            }

            // Simple identifier
            if let Some(id) = find_child_by_kind(&decl, "identifier") {
                return Ok(Some(id.utf8_text(source_code.as_bytes())?.to_string()));
            }

            // Field identifier (for methods defined inside class)
            if let Some(id) = find_child_by_kind(&decl, "field_identifier") {
                return Ok(Some(id.utf8_text(source_code.as_bytes())?.to_string()));
            }
        }

        Ok(None)
    }

    /// Find the function_declarator node within a function_definition.
    fn find_function_declarator<'a>(&self, node: &'a Node) -> Result<Option<Node<'a>>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "function_declarator" {
                return Ok(Some(child));
            }
            // Handle pointer declarators wrapping function declarators
            if child.kind() == "pointer_declarator" || child.kind() == "reference_declarator" {
                if let Some(inner) = find_child_by_kind(&child, "function_declarator") {
                    return Ok(Some(inner));
                }
            }
        }
        Ok(None)
    }

    /// Extract type name from class/struct/enum specifier.
    fn extract_type_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        // Try name field first
        if let Some(name_node) = node.child_by_field_name("name") {
            return Ok(Some(name_node.utf8_text(source_code.as_bytes())?.to_string()));
        }

        // Look for type_identifier child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_identifier" {
                return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
            }
        }

        Ok(None)
    }

    /// Extract namespace name.
    fn extract_namespace_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        // Check for name field
        if let Some(name_node) = node.child_by_field_name("name") {
            // Handle nested namespace (C++17 namespace A::B::C)
            if name_node.kind() == "namespace_identifier" {
                return Ok(Some(name_node.utf8_text(source_code.as_bytes())?.to_string()));
            }
            return Ok(Some(name_node.utf8_text(source_code.as_bytes())?.to_string()));
        }

        // Look for identifier child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "namespace_identifier" {
                return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
            }
        }

        // Anonymous namespace
        Ok(Some("<anonymous>".to_string()))
    }

    /// Extract typedef name.
    fn extract_typedef_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        // Look for type_identifier in declarator
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_identifier" {
                return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
            }
            if child.kind() == "type_declarator" {
                if let Some(id) = find_child_by_kind(&child, "type_identifier") {
                    return Ok(Some(id.utf8_text(source_code.as_bytes())?.to_string()));
                }
            }
        }
        Ok(None)
    }

    /// Extract using alias name.
    fn extract_alias_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        if let Some(name_node) = node.child_by_field_name("name") {
            return Ok(Some(name_node.utf8_text(source_code.as_bytes())?.to_string()));
        }

        // Look for type_identifier
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_identifier" {
                return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
            }
        }
        Ok(None)
    }

    /// Extract metadata based on entity kind.
    fn extract_entity_metadata(
        &self,
        kind: EntityKind,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        match kind {
            EntityKind::Function | EntityKind::Method => {
                self.extract_function_metadata(node, source_code, metadata)
            }
            EntityKind::Class | EntityKind::Struct => {
                self.extract_class_metadata(node, source_code, metadata)
            }
            EntityKind::Enum => self.extract_enum_metadata(node, source_code, metadata),
            _ => Ok(()),
        }
    }

    /// Extract function-specific metadata.
    fn extract_function_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Check for various qualifiers by looking at the source text
        let source_text = node.utf8_text(source_code.as_bytes())?;

        // Virtual, static, inline qualifiers
        let mut cursor = node.walk();
        let mut is_virtual = false;
        let mut is_static = false;
        let mut is_inline = false;
        let mut is_constexpr = false;
        let mut is_explicit = false;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "virtual" => is_virtual = true,
                "static" => is_static = true,
                "inline" => is_inline = true,
                "constexpr" => is_constexpr = true,
                "explicit" => is_explicit = true,
                "storage_class_specifier" => {
                    let text = child.utf8_text(source_code.as_bytes())?;
                    if text == "static" {
                        is_static = true;
                    }
                }
                _ => {}
            }
        }

        // Check declarator for const, noexcept, override, final
        if let Ok(Some(decl)) = self.find_function_declarator(node) {
            let decl_text = decl.utf8_text(source_code.as_bytes())?;

            // Look at siblings/trailing elements for qualifiers
            let mut sibling_cursor = decl.walk();
            if let Some(parent) = decl.parent() {
                let mut parent_cursor = parent.walk();
                let mut found_decl = false;
                for sibling in parent.children(&mut parent_cursor) {
                    if found_decl {
                        match sibling.kind() {
                            "const" | "type_qualifier" => {
                                metadata.insert(
                                    "is_const".to_string(),
                                    serde_json::Value::Bool(true),
                                );
                            }
                            "noexcept" => {
                                metadata.insert(
                                    "is_noexcept".to_string(),
                                    serde_json::Value::Bool(true),
                                );
                            }
                            "virtual_specifier" => {
                                let text = sibling.utf8_text(source_code.as_bytes())?;
                                if text == "override" {
                                    metadata.insert(
                                        "is_override".to_string(),
                                        serde_json::Value::Bool(true),
                                    );
                                } else if text == "final" {
                                    metadata.insert(
                                        "is_final".to_string(),
                                        serde_json::Value::Bool(true),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    if sibling.id() == decl.id() {
                        found_decl = true;
                    }
                }
            }
        }

        // Check for pure virtual (= 0)
        let is_pure_virtual = source_text.contains("= 0");

        if is_virtual {
            metadata.insert("is_virtual".to_string(), serde_json::Value::Bool(true));
        }
        if is_static {
            metadata.insert("is_static".to_string(), serde_json::Value::Bool(true));
        }
        if is_inline {
            metadata.insert("is_inline".to_string(), serde_json::Value::Bool(true));
        }
        if is_constexpr {
            metadata.insert("is_constexpr".to_string(), serde_json::Value::Bool(true));
        }
        if is_explicit {
            metadata.insert("is_explicit".to_string(), serde_json::Value::Bool(true));
        }
        if is_pure_virtual {
            metadata.insert(
                "is_pure_virtual".to_string(),
                serde_json::Value::Bool(true),
            );
        }

        // Extract return type
        if let Some(return_type) = self.extract_return_type(node, source_code)? {
            metadata.insert(
                "return_type".to_string(),
                serde_json::Value::String(return_type),
            );
        }

        // Extract parameters
        let params = self.extract_parameters(node, source_code)?;
        if !params.is_empty() {
            metadata.insert("parameters".to_string(), serde_json::json!(params));
        }

        Ok(())
    }

    /// Extract return type from function definition.
    fn extract_return_type(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        // Look for type specifier at the beginning
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_identifier"
                | "primitive_type"
                | "sized_type_specifier"
                | "template_type"
                | "qualified_identifier" => {
                    return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
                }
                "placeholder_type_specifier" => {
                    return Ok(Some("auto".to_string()));
                }
                _ => {}
            }
        }
        Ok(None)
    }

    /// Extract parameter names from function definition.
    fn extract_parameters(&self, node: &Node, source_code: &str) -> Result<Vec<String>> {
        let mut params = Vec::new();

        if let Ok(Some(decl)) = self.find_function_declarator(node) {
            if let Some(param_list) = find_child_by_kind(&decl, "parameter_list") {
                let mut cursor = param_list.walk();
                for child in param_list.children(&mut cursor) {
                    if child.kind() == "parameter_declaration" {
                        // Look for identifier or declarator with identifier
                        if let Some(id) = find_child_by_kind(&child, "identifier") {
                            params.push(id.utf8_text(source_code.as_bytes())?.to_string());
                        } else {
                            // Check for pointer_declarator or reference_declarator containing identifier
                            let mut inner_cursor = child.walk();
                            for inner in child.children(&mut inner_cursor) {
                                if inner.kind() == "pointer_declarator"
                                    || inner.kind() == "reference_declarator"
                                {
                                    if let Some(id) = find_child_by_kind(&inner, "identifier") {
                                        params
                                            .push(id.utf8_text(source_code.as_bytes())?.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(params)
    }

    /// Extract class-specific metadata.
    fn extract_class_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Extract base classes
        let base_classes = self.extract_base_classes(node, source_code)?;
        if !base_classes.is_empty() {
            metadata.insert("base_classes".to_string(), serde_json::json!(base_classes));
        }

        // Check if it's a template
        if let Some(parent) = node.parent() {
            if parent.kind() == "template_declaration" {
                metadata.insert("is_template".to_string(), serde_json::Value::Bool(true));

                // Extract template parameters
                if let Some(params) = find_child_by_kind(&parent, "template_parameter_list") {
                    let template_params = self.extract_template_parameters(&params, source_code)?;
                    if !template_params.is_empty() {
                        metadata.insert(
                            "template_parameters".to_string(),
                            serde_json::json!(template_params),
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Extract base classes from class specifier.
    fn extract_base_classes(
        &self,
        node: &Node,
        source_code: &str,
    ) -> Result<Vec<HashMap<String, String>>> {
        let mut bases = Vec::new();

        if let Some(base_clause) = find_child_by_kind(node, "base_class_clause") {
            let mut cursor = base_clause.walk();
            for child in base_clause.children(&mut cursor) {
                if child.kind() == "base_class_specifier" {
                    let mut base_info = HashMap::new();

                    // Extract access specifier
                    let mut access = "private".to_string(); // default for class
                    if node.kind() == "struct_specifier" {
                        access = "public".to_string(); // default for struct
                    }

                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        match inner.kind() {
                            "access_specifier" => {
                                access = inner.utf8_text(source_code.as_bytes())?.to_string();
                            }
                            "type_identifier" | "qualified_identifier" | "template_type" => {
                                base_info.insert(
                                    "name".to_string(),
                                    inner.utf8_text(source_code.as_bytes())?.to_string(),
                                );
                            }
                            _ => {}
                        }
                    }

                    if base_info.contains_key("name") {
                        base_info.insert("access".to_string(), access);
                        bases.push(base_info);
                    }
                }
            }
        }

        Ok(bases)
    }

    /// Extract template parameters.
    fn extract_template_parameters(
        &self,
        params_node: &Node,
        source_code: &str,
    ) -> Result<Vec<String>> {
        let mut params = Vec::new();

        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "type_parameter_declaration" | "parameter_declaration" => {
                    // Look for identifier or type_identifier
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "type_identifier" || inner.kind() == "identifier" {
                            params.push(inner.utf8_text(source_code.as_bytes())?.to_string());
                            break;
                        }
                    }
                }
                "variadic_type_parameter_declaration" => {
                    if let Some(id) = find_child_by_kind(&child, "type_identifier") {
                        params.push(format!(
                            "{}...",
                            id.utf8_text(source_code.as_bytes())?
                        ));
                    }
                }
                _ => {}
            }
        }

        Ok(params)
    }

    /// Extract enum-specific metadata.
    fn extract_enum_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Check if scoped enum (enum class/struct)
        let source_text = node.utf8_text(source_code.as_bytes())?;
        let is_scoped = source_text.starts_with("enum class") || source_text.starts_with("enum struct");

        if is_scoped {
            metadata.insert("is_scoped".to_string(), serde_json::Value::Bool(true));
        }

        // Extract underlying type if specified
        if let Some(base) = node.child_by_field_name("base") {
            let base_type = base.utf8_text(source_code.as_bytes())?;
            metadata.insert(
                "underlying_type".to_string(),
                serde_json::Value::String(base_type.to_string()),
            );
        }

        // Extract enumerator names
        if let Some(body) = find_child_by_kind(node, "enumerator_list") {
            let mut enumerators = Vec::new();
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "enumerator" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        enumerators.push(name_node.utf8_text(source_code.as_bytes())?.to_string());
                    } else if let Some(id) = find_child_by_kind(&child, "identifier") {
                        enumerators.push(id.utf8_text(source_code.as_bytes())?.to_string());
                    }
                }
            }
            if !enumerators.is_empty() {
                metadata.insert("enumerators".to_string(), serde_json::json!(enumerators));
            }
        }

        Ok(())
    }
}

impl LanguageAdapter for CppAdapter {
    fn language_name(&self) -> &str {
        "cpp"
    }

    fn parse_tree(&mut self, source_code: &str) -> Result<Tree> {
        self.parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("cpp", "Failed to parse C++ source code"))
    }

    fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        CppAdapter::parse_source(self, source_code, file_path)
    }

    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        let mut calls = Vec::new();

        walk_tree(tree.root_node(), &mut |node: Node| {
            if node.kind() == "call_expression" {
                // Get the function name
                if let Some(func_node) = node.child_by_field_name("function") {
                    if let Ok(name) = func_node.utf8_text(source.as_bytes()) {
                        calls.push(name.to_string());
                    }
                } else {
                    // Try first child
                    if let Some(first_child) = node.child(0) {
                        if let Ok(name) = first_child.utf8_text(source.as_bytes()) {
                            calls.push(name.to_string());
                        }
                    }
                }
            }
        });

        sort_and_dedup(&mut calls);
        Ok(calls)
    }

    fn extract_identifiers(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        let identifier_kinds = &["identifier", "field_identifier", "type_identifier"];
        let mut identifiers = extract_identifiers_by_kinds(tree.root_node(), source, identifier_kinds);
        sort_and_dedup(&mut identifiers);
        Ok(identifiers)
    }

    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize> {
        let tree = self.parse_tree(source)?;
        let mut count = 0;

        walk_tree(tree.root_node(), &mut |node: Node| {
            match node.kind() {
                // Functions and methods
                "function_definition" => count += 1,
                // Classes and structs
                "class_specifier" | "struct_specifier" => count += 1,
                // Control flow
                "if_statement" | "for_statement" | "while_statement" | "do_statement"
                | "switch_statement" | "try_statement" => count += 1,
                // Namespaces
                "namespace_definition" => count += 1,
                _ => {}
            }
        });

        Ok(count)
    }

    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        let tree = self.parse_tree(source)?;
        let mut imports = Vec::new();

        walk_tree(tree.root_node(), &mut |node: Node| {
            if node.kind() == "preproc_include" {
                // Get the path child (string_literal or system_lib_string)
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "string_literal" | "system_lib_string" => {
                            if let Ok(path) = child.utf8_text(source.as_bytes()) {
                                // Remove quotes/brackets
                                let clean_path = path
                                    .trim_start_matches(|c| c == '"' || c == '<')
                                    .trim_end_matches(|c| c == '"' || c == '>');
                                imports.push(ImportStatement {
                                    module: clean_path.to_string(),
                                    imports: None,
                                    import_type: "include".to_string(),
                                    line_number: node.start_position().row,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        Ok(imports)
    }

    fn extract_code_entities(&mut self, source: &str, file_path: &str) -> Result<Vec<CodeEntity>> {
        CppAdapter::extract_code_entities(self, source, file_path)
    }
}

#[cfg(test)]
#[path = "cpp_tests.rs"]
mod tests;
