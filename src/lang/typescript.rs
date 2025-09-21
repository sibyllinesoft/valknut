//! TypeScript language adapter with tree-sitter integration.

use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Tree};

use super::common::{EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation};
use super::registry::{get_tree_sitter_language, create_parser_for_language};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::CodeEntity;
use crate::detectors::structure::config::ImportStatement;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typescript_adapter_creation() {
        let adapter = TypeScriptAdapter::new();
        assert!(
            adapter.is_ok(),
            "Should create TypeScript adapter successfully"
        );
    }

    #[test]
    fn test_parse_simple_function() {
        let mut adapter = TypeScriptAdapter::new().unwrap();
        let source = r#"
function greet(name: string): string {
    return `Hello, ${name}!`;
}
"#;
        let result = adapter.parse_source(source, "test.ts");
        assert!(result.is_ok(), "Should parse simple function");

        let index = result.unwrap();
        assert!(
            index.get_entities_in_file("test.ts").len() >= 1,
            "Should find at least one entity"
        );
    }

    #[test]
    fn test_parse_interface_and_class() {
        let mut adapter = TypeScriptAdapter::new().unwrap();
        let source = r#"
interface User {
    name: string;
    age: number;
}

class UserService {
    private users: User[] = [];
    
    addUser(user: User): void {
        this.users.push(user);
    }
    
    getUser(name: string): User | undefined {
        return this.users.find(u => u.name === name);
    }
}
"#;
        let result = adapter.parse_source(source, "test.ts");
        assert!(result.is_ok(), "Should parse interface and class");

        let index = result.unwrap();
        let entities = index.get_entities_in_file("test.ts");
        assert!(
            entities.len() >= 2,
            "Should find at least interface and class entities"
        );

        let has_interface = entities
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Interface));
        let has_class = entities.iter().any(|e| matches!(e.kind, EntityKind::Class));
        assert!(
            has_interface || has_class,
            "Should find interface or class entity"
        );
    }

    #[test]
    fn test_parse_generic_types() {
        let mut adapter = TypeScriptAdapter::new().unwrap();
        let source = r#"
interface Repository<T> {
    findById(id: number): Promise<T | null>;
    save(entity: T): Promise<T>;
}

class InMemoryRepository<T extends { id: number }> implements Repository<T> {
    private items: T[] = [];
    
    async findById(id: number): Promise<T | null> {
        return this.items.find(item => item.id === id) || null;
    }
    
    async save(entity: T): Promise<T> {
        this.items.push(entity);
        return entity;
    }
}
"#;
        let result = adapter.parse_source(source, "generics.ts");
        assert!(result.is_ok(), "Should parse generic TypeScript code");

        let index = result.unwrap();
        let entities = index.get_entities_in_file("generics.ts");
        assert!(entities.len() >= 2, "Should find multiple entities");
    }

    #[test]
    fn test_parse_modules_and_exports() {
        let mut adapter = TypeScriptAdapter::new().unwrap();
        let source = r#"
export interface Config {
    apiUrl: string;
    timeout: number;
}

export class HttpClient {
    constructor(private config: Config) {}
    
    async get<T>(url: string): Promise<T> {
        // Implementation would go here
        throw new Error("Not implemented");
    }
}

export default function createClient(config: Config): HttpClient {
    return new HttpClient(config);
}
"#;
        let result = adapter.parse_source(source, "http.ts");
        assert!(result.is_ok(), "Should parse modules and exports");

        let index = result.unwrap();
        let entities = index.get_entities_in_file("http.ts");
        assert!(
            entities.len() >= 2,
            "Should find multiple exported entities"
        );
    }

    #[test]
    fn test_empty_typescript_file() {
        let mut adapter = TypeScriptAdapter::new().unwrap();
        let source = "// TypeScript file with just comments\n/* Block comment */";
        let result = adapter.parse_source(source, "empty.ts");
        assert!(result.is_ok(), "Should handle empty TypeScript file");

        let index = result.unwrap();
        let entities = index.get_entities_in_file("empty.ts");
        assert_eq!(
            entities.len(),
            0,
            "Should find no entities in comment-only file"
        );
    }
}

/// TypeScript-specific parsing and analysis
pub struct TypeScriptAdapter {
    /// Tree-sitter parser for TypeScript
    parser: Parser,

    /// Language instance
    language: Language,
}

impl TypeScriptAdapter {
    /// Create a new TypeScript adapter
    pub fn new() -> Result<Self> {
        let language = get_tree_sitter_language("ts")?;
        let parser = create_parser_for_language("ts")?;

        Ok(Self { parser, language })
    }

    fn parse_tree(&mut self, source_code: &str) -> Result<Tree> {
        self.parser
            .parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("typescript", "Failed to parse TypeScript source"))
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
            "function_declaration" | "function_expression" | "arrow_function" => {
                EntityKind::Function
            }
            "method_definition" => EntityKind::Method,
            "class_declaration" => EntityKind::Class,
            "interface_declaration" => EntityKind::Interface,
            "enum_declaration" => EntityKind::Enum,
            "variable_declaration" => {
                // Check if it's a const declaration (constant)
                if self.is_const_declaration(&node, source_code)? {
                    EntityKind::Constant
                } else {
                    EntityKind::Variable
                }
            }
            "lexical_declaration" => {
                // let/const declarations
                if self.is_const_declaration(&node, source_code)? {
                    EntityKind::Constant
                } else {
                    EntityKind::Variable
                }
            }
            "type_alias_declaration" => {
                // TypeScript type aliases - treat as interfaces for now
                EntityKind::Interface
            }
            _ => return Ok(None),
        };

        let name = self
            .extract_name(&node, source_code)?
            .unwrap_or_else(|| {
                // Provide fallback names for entities without extractable names
                match entity_kind {
                    EntityKind::Function => format!("anonymous_function_{}", *entity_id_counter),
                    EntityKind::Method => format!("anonymous_method_{}", *entity_id_counter),
                    EntityKind::Class => format!("anonymous_class_{}", *entity_id_counter),
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

    /// Extract the name of an entity from its AST node
    fn extract_name(&self, node: &Node, source_code: &str) -> Result<Option<String>> {
        let mut cursor = node.walk();

        match node.kind() {
            "function_declaration"
            | "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "type_alias_declaration" => {
                // Look for the identifier child
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_identifier" || child.kind() == "identifier" {
                        return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
                    }
                }
            }
            "method_definition" => {
                // Look for property_identifier or identifier
                for child in node.children(&mut cursor) {
                    if child.kind() == "property_identifier" || child.kind() == "identifier" {
                        return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
                    }
                }
            }
            "function_expression" | "arrow_function" => {
                // For anonymous functions, check if they're assigned to a variable
                return Ok(Some("<anonymous>".to_string()));
            }
            "variable_declaration" | "lexical_declaration" => {
                // Look for variable_declarator and then identifier
                for child in node.children(&mut cursor) {
                    if child.kind() == "variable_declarator" {
                        let mut declarator_cursor = child.walk();
                        for declarator_child in child.children(&mut declarator_cursor) {
                            if declarator_child.kind() == "identifier" {
                                return Ok(Some(
                                    declarator_child
                                        .utf8_text(source_code.as_bytes())?
                                        .to_string(),
                                ));
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(None)
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
                    // Extract parameter information
                    let mut param_cursor = child.walk();
                    for param_child in child.children(&mut param_cursor) {
                        if param_child.kind() == "identifier" {
                            let param_name = param_child.utf8_text(source_code.as_bytes())?;
                            parameters.push(param_name);
                        }
                    }
                }
                "async" => {
                    is_async = true;
                }
                "*" => {
                    is_generator = true;
                }
                "type_annotation" => {
                    // TypeScript return type annotation
                    return_type = Some(child.utf8_text(source_code.as_bytes())?.to_string());
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
                    // Look for extends clause
                    let mut heritage_cursor = child.walk();
                    for heritage_child in child.children(&mut heritage_cursor) {
                        if heritage_child.kind() == "extends_clause" {
                            let mut extends_cursor = heritage_child.walk();
                            for extends_child in heritage_child.children(&mut extends_cursor) {
                                if extends_child.kind() == "type_identifier"
                                    || extends_child.kind() == "identifier"
                                {
                                    extends_class = Some(
                                        extends_child
                                            .utf8_text(source_code.as_bytes())?
                                            .to_string(),
                                    );
                                }
                            }
                        } else if heritage_child.kind() == "implements_clause" {
                            let mut implements_cursor = heritage_child.walk();
                            for implements_child in heritage_child.children(&mut implements_cursor)
                            {
                                if implements_child.kind() == "type_identifier"
                                    || implements_child.kind() == "identifier"
                                {
                                    implements.push(
                                        implements_child
                                            .utf8_text(source_code.as_bytes())?
                                            .to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
                "abstract" => {
                    is_abstract = true;
                }
                _ => {}
            }
        }

        if let Some(extends) = extends_class {
            metadata.insert("extends".to_string(), serde_json::Value::String(extends));
        }
        if !implements.is_empty() {
            metadata.insert("implements".to_string(), serde_json::json!(implements));
        }
        metadata.insert(
            "is_abstract".to_string(),
            serde_json::Value::Bool(is_abstract),
        );

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
        let mut extends_interfaces = Vec::new();

        for child in node.children(&mut cursor) {
            if child.kind() == "extends_clause" {
                let mut extends_cursor = child.walk();
                for extends_child in child.children(&mut extends_cursor) {
                    if extends_child.kind() == "type_identifier"
                        || extends_child.kind() == "identifier"
                    {
                        extends_interfaces
                            .push(extends_child.utf8_text(source_code.as_bytes())?.to_string());
                    }
                }
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

fn normalize_module_literal(raw: &str) -> String {
    raw.trim()
        .trim_end_matches(';')
        .trim_matches(['"', '\'', '`'])
        .trim()
        .to_string()
}

impl LanguageAdapter for TypeScriptAdapter {
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        TypeScriptAdapter::parse_source(self, source, file_path)
    }

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

    fn count_ast_nodes(&mut self, source: &str) -> Result<usize> {
        let tree = self.parse_tree(source)?;
        let mut count = 0usize;
        Self::walk_tree(tree.root_node(), &mut |_| count += 1);
        Ok(count)
    }

    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize> {
        let index = TypeScriptAdapter::parse_source(self, source, "<memory>")?;
        Ok(index.count_distinct_blocks())
    }

    fn normalize_source(&mut self, source: &str) -> Result<String> {
        let tree = self.parse_tree(source)?;
        Ok(tree.root_node().to_sexp())
    }

    fn language_name(&self) -> &str {
        "typescript"
    }

    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        let mut imports = Vec::new();

        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            if let Some(import_part) = trimmed.strip_prefix("import ") {
                if let Some(from_pos) = import_part.find(" from ") {
                    let import_spec = import_part[..from_pos].trim();
                    let module_part = normalize_module_literal(&import_part[from_pos + 6..]);

                    let (imports_list, import_type) = if import_spec.starts_with("*") {
                        (None, "star".to_string())
                    } else if import_spec.starts_with('{') {
                        let cleaned = import_spec.trim_matches(|c| c == '{' || c == '}');
                        let items = cleaned
                            .split(',')
                            .map(|s| s.trim().trim_start_matches("type ").to_string())
                            .collect();
                        (Some(items), "named".to_string())
                    } else {
                        (Some(vec![import_spec.to_string()]), "default".to_string())
                    };

                    imports.push(ImportStatement {
                        module: module_part,
                        imports: imports_list,
                        import_type,
                        line_number: line_number + 1,
                    });
                }
            } else if let Some(require_part) = trimmed.strip_prefix("const ") {
                if let Some(eq_pos) = require_part.find('=') {
                    let rhs = require_part[eq_pos + 1..].trim();
                    if let Some(module_part) = rhs
                        .strip_prefix("require(")
                        .and_then(|s| s.strip_suffix(");"))
                    {
                        let module = normalize_module_literal(module_part);
                        imports.push(ImportStatement {
                            module,
                            imports: None,
                            import_type: "require".to_string(),
                            line_number: line_number + 1,
                        });
                    }
                }
            }
        }

        Ok(imports)
    }

    fn extract_code_entities(&mut self, source: &str, file_path: &str) -> Result<Vec<crate::core::featureset::CodeEntity>> {
        TypeScriptAdapter::extract_code_entities(self, source, file_path)
    }
}

impl Default for TypeScriptAdapter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!(
                "Warning: Failed to create TypeScript adapter, using minimal fallback: {}",
                e
            );
            TypeScriptAdapter {
                parser: tree_sitter::Parser::new(),
                language: get_tree_sitter_language("ts").unwrap_or_else(|_| tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            }
        })
    }
}
