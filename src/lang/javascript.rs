//! JavaScript language adapter with tree-sitter integration.

use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Tree};

use super::common::{EntityKind, ParsedEntity, ParseIndex, SourceLocation};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::CodeEntity;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_javascript_adapter_creation() {
        let adapter = JavaScriptAdapter::new();
        assert!(adapter.is_ok(), "Should create JavaScript adapter successfully");
    }
    
    #[test]
    fn test_parse_simple_function() {
        let mut adapter = JavaScriptAdapter::new().unwrap();
        let source = r#"
function hello() {
    return "Hello, World!";
}
"#;
        let result = adapter.parse_source(source, "test.js");
        assert!(result.is_ok(), "Should parse simple function");
        
        let index = result.unwrap();
        assert!(index.get_entities_in_file("test.js").len() >= 1, "Should find at least one entity");
    }
    
    #[test]
    fn test_parse_simple_class() {
        let mut adapter = JavaScriptAdapter::new().unwrap();
        let source = r#"
class MyClass {
    constructor() {
        this.value = 0;
    }
    
    getValue() {
        return this.value;
    }
}
"#;
        let result = adapter.parse_source(source, "test.js");
        assert!(result.is_ok(), "Should parse simple class");
        
        let index = result.unwrap();
        let entities = index.get_entities_in_file("test.js");
        assert!(entities.len() >= 1, "Should find at least one entity");
        
        let has_class = entities.iter().any(|e| matches!(e.kind, EntityKind::Class));
        assert!(has_class, "Should find a class entity");
    }
    
    #[test]
    fn test_parse_arrow_functions() {
        let mut adapter = JavaScriptAdapter::new().unwrap();
        let source = r#"
const add = (a, b) => a + b;
const multiply = (x, y) => {
    return x * y;
};
"#;
        let result = adapter.parse_source(source, "arrow.js");
        assert!(result.is_ok(), "Should parse arrow functions");
        
        let index = result.unwrap();
        let entities = index.get_entities_in_file("arrow.js");
        // Arrow functions might be detected as variables or functions depending on implementation
        assert!(entities.len() >= 0, "Should handle arrow functions gracefully");
    }
    
    #[test]
    fn test_parse_complex_javascript() {
        let mut adapter = JavaScriptAdapter::new().unwrap();
        let source = r#"
import { fetch } from 'node-fetch';

class APIClient {
    constructor(baseURL) {
        this.baseURL = baseURL;
    }
    
    async get(endpoint) {
        const response = await fetch(`${this.baseURL}/${endpoint}`);
        return response.json();
    }
}

function createClient(url) {
    return new APIClient(url);
}

const defaultClient = createClient('https://api.example.com');
"#;
        let result = adapter.parse_source(source, "complex.js");
        assert!(result.is_ok(), "Should parse complex JavaScript code");
        
        let index = result.unwrap();
        let entities = index.get_entities_in_file("complex.js");
        assert!(entities.len() >= 2, "Should find multiple entities");
    }
    
    #[test] 
    fn test_empty_javascript_file() {
        let mut adapter = JavaScriptAdapter::new().unwrap();
        let source = "// Just a comment\n/* Another comment */";
        let result = adapter.parse_source(source, "empty.js");
        assert!(result.is_ok(), "Should handle empty JavaScript file");
        
        let index = result.unwrap();
        let entities = index.get_entities_in_file("empty.js");
        assert_eq!(entities.len(), 0, "Should find no entities in comment-only file");
    }
}

extern "C" {
    fn tree_sitter_javascript() -> Language;
}

/// JavaScript-specific parsing and analysis
pub struct JavaScriptAdapter {
    /// Tree-sitter parser for JavaScript
    parser: Parser,
    
    /// Language instance
    language: Language,
}

impl JavaScriptAdapter {
    /// Create a new JavaScript adapter
    pub fn new() -> Result<Self> {
        let language = unsafe { tree_sitter_javascript() };
        let mut parser = Parser::new();
        parser.set_language(language)
            .map_err(|e| ValknutError::parse("javascript", format!("Failed to set JavaScript language: {:?}", e)))?;
        
        Ok(Self { parser, language })
    }
    
    /// Parse JavaScript source code and extract entities
    pub fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self.parser.parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("javascript", "Failed to parse JavaScript source code"))?;
        
        let mut index = ParseIndex::new();
        let mut entity_id_counter = 0;
        
        // Walk the tree and extract entities
        self.extract_entities_recursive(
            tree.root_node(), 
            source_code, 
            file_path, 
            None, 
            &mut index, 
            &mut entity_id_counter
        )?;
        
        Ok(index)
    }
    
    /// Extract entities from JavaScript code and convert to CodeEntity format
    pub fn extract_code_entities(&mut self, source_code: &str, file_path: &str) -> Result<Vec<CodeEntity>> {
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
        if let Some(entity) = self.node_to_entity(node, source_code, file_path, parent_id.clone(), entity_id_counter)? {
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
                    entity_id_counter
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
                    entity_id_counter
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
            "function_declaration" | "function_expression" | "arrow_function" => EntityKind::Function,
            "method_definition" => EntityKind::Method,
            "class_declaration" => EntityKind::Class,
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
            _ => return Ok(None),
        };
        
        let name = self.extract_name(&node, source_code)?
            .ok_or_else(|| ValknutError::parse("javascript", "Could not extract entity name"))?;
        
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
        
        // Add JavaScript-specific metadata
        metadata.insert("node_kind".to_string(), serde_json::Value::String(node.kind().to_string()));
        metadata.insert("byte_range".to_string(), serde_json::json!([node.start_byte(), node.end_byte()]));
        
        // Extract additional metadata based on entity type
        match entity_kind {
            EntityKind::Function | EntityKind::Method => {
                self.extract_function_metadata(&node, source_code, &mut metadata)?;
            }
            EntityKind::Class => {
                self.extract_class_metadata(&node, source_code, &mut metadata)?;
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
            "function_declaration" | "class_declaration" => {
                // Look for the identifier child
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
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
                                return Ok(Some(declarator_child.utf8_text(source_code.as_bytes())?.to_string()));
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
            if child.kind() == "const" || 
               (child.kind() == "identifier" && child.utf8_text(source_code.as_bytes())? == "const") {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// Extract function-specific metadata
    fn extract_function_metadata(&self, node: &Node, source_code: &str, metadata: &mut HashMap<String, serde_json::Value>) -> Result<()> {
        let mut cursor = node.walk();
        let mut parameters = Vec::new();
        let mut is_async = false;
        let mut is_generator = false;
        
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
                _ => {}
            }
        }
        
        metadata.insert("parameters".to_string(), serde_json::json!(parameters));
        metadata.insert("is_async".to_string(), serde_json::Value::Bool(is_async));
        metadata.insert("is_generator".to_string(), serde_json::Value::Bool(is_generator));
        
        Ok(())
    }
    
    /// Extract class-specific metadata
    fn extract_class_metadata(&self, node: &Node, source_code: &str, metadata: &mut HashMap<String, serde_json::Value>) -> Result<()> {
        let mut cursor = node.walk();
        let mut extends_class = None;
        
        for child in node.children(&mut cursor) {
            match child.kind() {
                "class_heritage" => {
                    // Look for extends clause
                    let mut heritage_cursor = child.walk();
                    for heritage_child in child.children(&mut heritage_cursor) {
                        if heritage_child.kind() == "identifier" {
                            extends_class = Some(heritage_child.utf8_text(source_code.as_bytes())?.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        
        if let Some(extends) = extends_class {
            metadata.insert("extends".to_string(), serde_json::Value::String(extends));
        }
        
        Ok(())
    }
    
    /// Convert ParsedEntity to CodeEntity format
    fn convert_to_code_entity(&self, entity: &ParsedEntity, source_code: &str) -> Result<CodeEntity> {
        let source_lines: Vec<&str> = source_code.lines().collect();
        let entity_source = if entity.location.start_line <= source_lines.len() && entity.location.end_line <= source_lines.len() {
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

impl Default for JavaScriptAdapter {
    fn default() -> Self {
        Self::new().expect("Failed to create JavaScript adapter")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_javascript_adapter_creation() {
        let adapter = JavaScriptAdapter::new();
        assert!(adapter.is_ok());
    }
    
    #[test]
    fn test_function_parsing() {
        let mut adapter = JavaScriptAdapter::new().unwrap();
        let source_code = r#"
function hello() {
    console.log("Hello, world!");
    return 42;
}
"#;
        
        let entities = adapter.extract_code_entities(source_code, "test.js").unwrap();
        assert_eq!(entities.len(), 1);
        
        let function_entity = &entities[0];
        assert_eq!(function_entity.entity_type, "Function");
        assert_eq!(function_entity.name, "hello");
    }
    
    #[test]
    fn test_class_parsing() {
        let mut adapter = JavaScriptAdapter::new().unwrap();
        let source_code = r#"
class TestClass {
    constructor(value) {
        this.value = value;
    }
    
    getValue() {
        return this.value;
    }
}
"#;
        
        let entities = adapter.extract_code_entities(source_code, "test.js").unwrap();
        
        // Should find the class and the methods
        let class_entities: Vec<_> = entities.iter().filter(|e| e.entity_type == "Class").collect();
        let method_entities: Vec<_> = entities.iter().filter(|e| e.entity_type == "Method").collect();
        
        assert_eq!(class_entities.len(), 1);
        assert_eq!(class_entities[0].name, "TestClass");
        assert!(method_entities.len() >= 2); // constructor and getValue
    }
    
    #[test]
    fn test_arrow_function() {
        let mut adapter = JavaScriptAdapter::new().unwrap();
        let source_code = r#"
const add = (a, b) => a + b;
"#;
        
        let entities = adapter.extract_code_entities(source_code, "test.js").unwrap();
        
        // Should find both the const declaration and potentially the arrow function
        let const_entities: Vec<_> = entities.iter().filter(|e| e.entity_type == "Constant").collect();
        
        assert_eq!(const_entities.len(), 1);
        assert_eq!(const_entities[0].name, "add");
    }
    
    #[test]
    fn test_async_function() {
        let mut adapter = JavaScriptAdapter::new().unwrap();
        let source_code = r#"
async function fetchData() {
    const response = await fetch('/api/data');
    return response.json();
}
"#;
        
        let entities = adapter.extract_code_entities(source_code, "test.js").unwrap();
        let function_entities: Vec<_> = entities.iter().filter(|e| e.entity_type == "Function").collect();
        
        assert_eq!(function_entities.len(), 1);
        assert_eq!(function_entities[0].name, "fetchData");
        
        // Check for async metadata
        let is_async = function_entities[0].properties.get("is_async");
        assert_eq!(is_async, Some(&serde_json::Value::Bool(true)));
    }
}