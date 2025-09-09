//! Python language adapter with tree-sitter integration.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tree_sitter::{Language, Node, Parser, Tree, TreeCursor};

use super::common::{EntityKind, ParsedEntity, ParseIndex, SourceLocation};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::{CodeEntity, EntityId};

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_python_adapter_creation() {
        let adapter = PythonAdapter::new();
        assert!(adapter.is_ok(), "Should create Python adapter successfully");
    }
    
    #[test]
    fn test_parse_simple_function() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source = r#"
def hello_world():
    return "Hello, World!"
"#;
        let result = adapter.parse_source(source, "test.py");
        assert!(result.is_ok(), "Should parse simple function");
        
        let index = result.unwrap();
        assert!(index.get_entities_in_file("test.py").len() >= 1, "Should find at least one entity");
    }
    
    #[test]
    fn test_parse_simple_class() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source = r#"
class MyClass:
    def __init__(self):
        self.value = 0
    
    def get_value(self):
        return self.value
"#;
        let result = adapter.parse_source(source, "test.py");
        assert!(result.is_ok(), "Should parse simple class");
        
        let index = result.unwrap();
        let entities = index.get_entities_in_file("test.py");
        assert!(entities.len() >= 1, "Should find at least one entity");
        
        let has_class = entities.iter().any(|e| matches!(e.kind, EntityKind::Class));
        assert!(has_class, "Should find a class entity");
    }
    
    #[test]
    fn test_parse_complex_python() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source = r#"
import os
import sys

class DataProcessor:
    """A sample data processor class."""
    
    def __init__(self, name: str):
        self.name = name
        self.data = []
    
    @property
    def size(self) -> int:
        return len(self.data)
    
    def add_data(self, item):
        self.data.append(item)

def process_file(filename: str) -> bool:
    """Process a file and return success status."""
    try:
        with open(filename, 'r') as f:
            content = f.read()
        return True
    except FileNotFoundError:
        return False

if __name__ == "__main__":
    processor = DataProcessor("test")
    success = process_file("data.txt")
"#;
        let result = adapter.parse_source(source, "complex.py");
        assert!(result.is_ok(), "Should parse complex Python code");
        
        let index = result.unwrap();
        let entities = index.get_entities_in_file("complex.py");
        assert!(entities.len() >= 2, "Should find multiple entities");
        
        let has_class = entities.iter().any(|e| matches!(e.kind, EntityKind::Class));
        let has_function = entities.iter().any(|e| matches!(e.kind, EntityKind::Function));
        assert!(has_class && has_function, "Should find both class and function entities");
    }
    
    #[test]
    fn test_extract_entity_name() {
        let adapter = PythonAdapter::new().unwrap();
        let source = "def test_function(): pass";
        let tree = adapter.parser.parse(source, None).unwrap();
        let function_node = tree.root_node().child(0).unwrap(); // Should be function_definition
        
        let result = adapter.extract_entity_name(&function_node, source);
        assert!(result.is_ok());
        
        if let Ok(Some(name)) = result {
            assert_eq!(name, "test_function");
        }
    }
    
    #[test]
    fn test_convert_to_code_entity() {
        let adapter = PythonAdapter::new().unwrap();
        let entity = ParsedEntity {
            id: "test-id".to_string(),
            name: "test_func".to_string(),
            kind: EntityKind::Function,
            location: SourceLocation {
                file_path: "test.py".to_string(),
                start_line: 1,
                end_line: 2,
                start_column: 0,
                end_column: 10,
            },
            parent_id: None,
            metadata: HashMap::new(),
        };
        
        let source = "def test_func(): pass";
        let result = adapter.convert_to_code_entity(&entity, source);
        assert!(result.is_ok(), "Should convert to CodeEntity successfully");
        
        let code_entity = result.unwrap();
        assert_eq!(code_entity.name, "test_func");
        assert_eq!(code_entity.file_path, "test.py");
    }
    
    #[test]
    fn test_get_entities_empty_file() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source = "# Just a comment\n";
        let result = adapter.parse_source(source, "empty.py");
        assert!(result.is_ok(), "Should handle empty Python file");
        
        let index = result.unwrap();
        let entities = index.get_entities_in_file("empty.py");
        assert_eq!(entities.len(), 0, "Should find no entities in comment-only file");
    }
}

extern "C" {
    fn tree_sitter_python() -> Language;
}

/// Python-specific parsing and analysis
pub struct PythonAdapter {
    /// Tree-sitter parser for Python
    parser: Parser,
    
    /// Language instance
    language: Language,
}

impl PythonAdapter {
    /// Create a new Python adapter
    pub fn new() -> Result<Self> {
        let language = unsafe { tree_sitter_python() };
        let mut parser = Parser::new();
        parser.set_language(language)
            .map_err(|e| ValknutError::parse("python", format!("Failed to set Python language: {:?}", e)))?;
        
        Ok(Self { parser, language })
    }
    
    /// Parse Python source code and extract entities
    pub fn parse_source(&mut self, source_code: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self.parser.parse(source_code, None)
            .ok_or_else(|| ValknutError::parse("python", "Failed to parse Python source code"))?;
        
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
    
    /// Extract entities from Python code and convert to CodeEntity format
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
            "function_definition" => EntityKind::Function,
            "class_definition" => EntityKind::Class,
            "module" => EntityKind::Module,
            "assignment" => {
                // Check if it's a constant assignment (all uppercase)
                if let Some(name) = self.extract_name(&node, source_code)? {
                    if name.chars().all(|c| c.is_uppercase() || c == '_') {
                        EntityKind::Constant
                    } else {
                        EntityKind::Variable
                    }
                } else {
                    return Ok(None);
                }
            }
            // Method definitions are handled as functions within classes
            _ => return Ok(None),
        };
        
        let name = self.extract_name(&node, source_code)?
            .ok_or_else(|| ValknutError::parse("python", "Could not extract entity name"))?;
        
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
        
        // Add Python-specific metadata
        metadata.insert("node_kind".to_string(), serde_json::Value::String(node.kind().to_string()));
        metadata.insert("byte_range".to_string(), serde_json::json!([node.start_byte(), node.end_byte()]));
        
        // Extract additional metadata based on entity type
        match entity_kind {
            EntityKind::Function => {
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
            "function_definition" | "class_definition" => {
                // Look for the identifier child
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
                    }
                }
            }
            "assignment" => {
                // Look for the left-hand side identifier
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
                    }
                }
            }
            _ => {}
        }
        
        Ok(None)
    }
    
    /// Extract function-specific metadata
    fn extract_function_metadata(&self, node: &Node, source_code: &str, metadata: &mut HashMap<String, serde_json::Value>) -> Result<()> {
        let mut cursor = node.walk();
        let mut parameters = Vec::new();
        let mut has_decorators = false;
        let mut return_annotation = None;
        
        for child in node.children(&mut cursor) {
            match child.kind() {
                "parameters" => {
                    // Extract parameter information
                    let mut param_cursor = child.walk();
                    for param_child in child.children(&mut param_cursor) {
                        if param_child.kind() == "identifier" {
                            let param_name = param_child.utf8_text(source_code.as_bytes())?;
                            parameters.push(param_name);
                        }
                    }
                }
                "decorator" => {
                    has_decorators = true;
                }
                "type" => {
                    // Return type annotation
                    return_annotation = Some(child.utf8_text(source_code.as_bytes())?.to_string());
                }
                _ => {}
            }
        }
        
        metadata.insert("parameters".to_string(), serde_json::json!(parameters));
        metadata.insert("has_decorators".to_string(), serde_json::Value::Bool(has_decorators));
        if let Some(return_type) = return_annotation {
            metadata.insert("return_annotation".to_string(), serde_json::Value::String(return_type));
        }
        
        Ok(())
    }
    
    /// Extract class-specific metadata
    fn extract_class_metadata(&self, node: &Node, source_code: &str, metadata: &mut HashMap<String, serde_json::Value>) -> Result<()> {
        let mut cursor = node.walk();
        let mut base_classes = Vec::new();
        let mut has_decorators = false;
        
        for child in node.children(&mut cursor) {
            match child.kind() {
                "argument_list" => {
                    // Base classes
                    let mut arg_cursor = child.walk();
                    for arg_child in child.children(&mut arg_cursor) {
                        if arg_child.kind() == "identifier" {
                            let base_name = arg_child.utf8_text(source_code.as_bytes())?;
                            base_classes.push(base_name);
                        }
                    }
                }
                "decorator" => {
                    has_decorators = true;
                }
                _ => {}
            }
        }
        
        metadata.insert("base_classes".to_string(), serde_json::json!(base_classes));
        metadata.insert("has_decorators".to_string(), serde_json::Value::Bool(has_decorators));
        
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

impl Default for PythonAdapter {
    fn default() -> Self {
        Self::new().expect("Failed to create Python adapter")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_python_adapter_creation() {
        let adapter = PythonAdapter::new();
        assert!(adapter.is_ok());
    }
    
    #[test]
    fn test_simple_function_parsing() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source_code = r#"
def hello_world():
    print("Hello, world!")
    return 42
"#;
        
        let entities = adapter.extract_code_entities(source_code, "test.py").unwrap();
        assert_eq!(entities.len(), 1);
        
        let function_entity = &entities[0];
        assert_eq!(function_entity.entity_type, "Function");
        assert_eq!(function_entity.name, "hello_world");
        assert!(function_entity.source_code.contains("def hello_world"));
    }
    
    #[test]
    fn test_class_parsing() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source_code = r#"
class TestClass:
    def __init__(self, value):
        self.value = value
    
    def get_value(self):
        return self.value
"#;
        
        let entities = adapter.extract_code_entities(source_code, "test.py").unwrap();
        
        // Should find the class and the methods
        let class_entities: Vec<_> = entities.iter().filter(|e| e.entity_type == "Class").collect();
        let function_entities: Vec<_> = entities.iter().filter(|e| e.entity_type == "Function").collect();
        
        assert_eq!(class_entities.len(), 1);
        assert!(function_entities.len() >= 2); // __init__ and get_value
        
        assert_eq!(class_entities[0].name, "TestClass");
    }
    
    #[test]
    fn test_function_with_parameters() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source_code = r#"
def calculate(x, y, z=10):
    return x + y + z
"#;
        
        let entities = adapter.extract_code_entities(source_code, "test.py").unwrap();
        assert_eq!(entities.len(), 1);
        
        let function_entity = &entities[0];
        assert_eq!(function_entity.name, "calculate");
        
        // Check if parameters metadata exists
        let parameters = function_entity.properties.get("parameters");
        assert!(parameters.is_some());
    }
}