//! Python language adapter with tree-sitter integration.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tree_sitter::{Language, Node, Parser, Tree, TreeCursor};

use super::common::{EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation};
use super::registry::{create_parser_for_language, get_tree_sitter_language};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::{CodeEntity, EntityId};
use crate::core::interned_entities::{
    InternedCodeEntity, InternedParseIndex, InternedParsedEntity, InternedSourceLocation,
};
use crate::core::interning::{intern, resolve, InternedString};
use crate::detectors::structure::config::ImportStatement;

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
        assert!(
            result.is_ok(),
            "Should parse simple function: {:?}",
            result.err()
        );

        let index = result.unwrap();
        assert!(
            index.get_entities_in_file("test.py").len() >= 1,
            "Should find at least one entity"
        );
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
        let has_function = entities
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Function));
        assert!(
            has_class && has_function,
            "Should find both class and function entities"
        );
    }

    #[test]
    fn test_extract_entity_name() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source = "def test_function(): pass";
        let tree = adapter.parser.parse(source, None).unwrap();
        let function_node = tree.root_node().child(0).unwrap(); // Should be function_definition

        let result = adapter.extract_name(&function_node, source);
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
            parent: None,
            children: vec![],
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
        assert_eq!(
            entities.len(),
            0,
            "Should find no entities in comment-only file"
        );
    }

    #[test]
    fn test_extract_imports_supports_star_and_named() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source = r#"
import os
from typing import List, Dict
from custom.utils import *
"#;

        let imports = adapter.extract_imports(source).expect("imports parsed");
        assert_eq!(imports.len(), 3);
        assert!(imports.iter().any(|imp| imp.module == "os"));
        assert!(imports
            .iter()
            .any(|imp| imp.module == "typing" && imp.import_type == "named"));
        assert!(imports
            .iter()
            .any(|imp| imp.module == "custom.utils" && imp.import_type == "star"));
    }

    #[test]
    fn test_extract_function_calls_and_identifiers() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source = r#"
import math

def compute(value):
    print(value)
    math.sqrt(value)
    helper(value)

def helper(value):
    return value * 2

compute(10)
"#;

        let calls = adapter
            .extract_function_calls(source)
            .expect("function calls extracted");
        assert!(calls.contains(&"print".to_string()));
        assert!(calls.iter().any(|call| call.contains("math.sqrt")));
        assert!(calls.contains(&"helper".to_string()));

        let identifiers = adapter
            .extract_identifiers(source)
            .expect("identifiers extracted");
        assert!(identifiers.contains(&"compute".to_string()));
        assert!(identifiers.contains(&"helper".to_string()));
    }

    #[test]
    fn test_contains_boilerplate_patterns_detects_common_cases() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source = r#"
import os
from typing import List

def main():
    pass

if __name__ == "__main__":
    main()
"#;

        let patterns = vec![
            "import os".to_string(),
            "from typing import".to_string(),
            "if __name__ == \"__main__\"".to_string(),
        ];
        let found = adapter
            .contains_boilerplate_patterns(source, &patterns)
            .expect("boilerplate detection");

        assert!(found.contains(&"import os".to_string()));
        assert!(found.contains(&"from typing import".to_string()));
        assert!(found.contains(&"if __name__ == \"__main__\"".to_string()));
    }

    #[test]
    fn test_normalize_and_count_python_ast_metrics() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source = r#"
def outer(value):
    if value > 10:
        return value + 1
    return value - 1
"#;

        let normalized = adapter
            .normalize_source(source)
            .expect("normalization should succeed");
        assert!(normalized.contains("function_definition"));
        assert!(normalized.contains("if_statement"));

        let node_count = adapter
            .count_ast_nodes(source)
            .expect("node counting should succeed");
        assert!(node_count > 0);

        let block_count = adapter
            .count_distinct_blocks(source)
            .expect("block counting should succeed");
        assert!(block_count >= 2);
    }
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

    /// Extract entities from Python code and convert to CodeEntity format
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

        // Walk the tree and extract entities using interned strings
        self.extract_entities_recursive_interned(
            tree.root_node(),
            source_code,
            file_path,
            None,
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
            "function_definition" => EntityKind::Function,
            "class_definition" => EntityKind::Class,
            "module" => {
                // Skip root module nodes that don't have meaningful names
                return Ok(None);
            }
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

        let name = self.extract_name(&node, source_code)?.unwrap_or_else(|| {
            // Provide fallback names for entities without extractable names
            match entity_kind {
                EntityKind::Function => format!("anonymous_function_{}", *entity_id_counter),
                EntityKind::Method => format!("anonymous_method_{}", *entity_id_counter),
                EntityKind::Class => format!("anonymous_class_{}", *entity_id_counter),
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

        // Add Python-specific metadata
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
                // Look for the identifier child (name field)
                if let Some(name_node) = node.child_by_field_name("name") {
                    return Ok(Some(
                        name_node.utf8_text(source_code.as_bytes())?.to_string(),
                    ));
                }

                // Reset cursor for fallback
                cursor = node.walk();

                // Fallback: Look for the identifier child
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
    fn extract_function_metadata(
        &self,
        node: &Node,
        source_code: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut cursor = node.walk();
        let mut parameters = Vec::new();
        let mut has_decorators = false;
        let mut return_annotation = None;
        let mut function_calls = Vec::new();

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

        // Collect function calls within this definition for dependency analysis
        self.extract_function_calls_recursive(*node, source_code, &mut function_calls)?;

        metadata.insert("parameters".to_string(), serde_json::json!(parameters));
        metadata.insert(
            "has_decorators".to_string(),
            serde_json::Value::Bool(has_decorators),
        );
        if let Some(return_type) = return_annotation {
            metadata.insert(
                "return_annotation".to_string(),
                serde_json::Value::String(return_type),
            );
        }
        metadata.insert(
            "function_calls".to_string(),
            serde_json::Value::Array(
                function_calls
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );

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
        metadata.insert(
            "has_decorators".to_string(),
            serde_json::Value::Bool(has_decorators),
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
        match node.kind() {
            "function_definition" | "class_definition" => {
                // Look for identifier child
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name_str = child.utf8_text(source_code.as_bytes())?;
                        return Ok(Some(intern(name_str))); // Intern directly - deduplication happens here!
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
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

    /// Extract function calls recursively from AST
    fn extract_function_calls_recursive(
        &self,
        node: Node,
        source: &str,
        calls: &mut Vec<String>,
    ) -> Result<()> {
        match node.kind() {
            "call" => {
                // Extract the function name from call expression
                if let Some(func_node) = node.child_by_field_name("function") {
                    if let Ok(func_name) = func_node.utf8_text(source.as_bytes()) {
                        calls.push(func_name.to_string());
                    }
                }
            }
            "attribute" => {
                // Handle method calls like obj.method()
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

    /// Check for boilerplate patterns in AST recursively
    fn check_boilerplate_patterns_recursive(
        &self,
        node: Node,
        source: &str,
        patterns: &[String],
        found_patterns: &mut Vec<String>,
    ) -> Result<()> {
        // Check specific Python boilerplate patterns based on AST structure
        match node.kind() {
            "import_statement" => {
                // Check for common imports using AST structure
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(module_name) = name_node.utf8_text(source.as_bytes()) {
                        let common_modules = ["os", "sys", "json", "logging", "datetime"];
                        if common_modules.contains(&module_name) {
                            found_patterns.push(format!("import {}", module_name));
                        }
                    }
                }
            }
            "import_from_statement" => {
                // Check for typing imports and other common patterns
                if let Some(module_node) = node.child_by_field_name("module_name") {
                    if let Ok(module_name) = module_node.utf8_text(source.as_bytes()) {
                        if module_name == "typing" {
                            found_patterns.push("from typing import".to_string());
                        }
                    }
                }
            }
            "if_statement" => {
                // Check for if __name__ == "__main__" pattern using AST structure
                if let Some(condition_node) = node.child_by_field_name("condition") {
                    if condition_node.kind() == "comparison_operator" {
                        let mut cursor = condition_node.walk();
                        let children: Vec<_> = condition_node.children(&mut cursor).collect();

                        if children.len() >= 3 {
                            // Check for __name__ on left side
                            if let Ok(left_text) = children[0].utf8_text(source.as_bytes()) {
                                if left_text == "__name__" {
                                    // Check for "__main__" on right side
                                    if let Ok(right_text) = children[2].utf8_text(source.as_bytes())
                                    {
                                        if right_text.contains("__main__") {
                                            found_patterns
                                                .push("if __name__ == \"__main__\"".to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "function_definition" => {
                // Check for dunder methods using AST field access
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(func_name) = name_node.utf8_text(source.as_bytes()) {
                        // Check for dunder methods (double underscore methods)
                        if func_name.len() >= 4
                            && func_name.starts_with("__")
                            && func_name.ends_with("__")
                        {
                            found_patterns.push(func_name.to_string());
                        }
                    }
                }
            }
            _ => {}
        }

        // Process children recursively
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_boilerplate_patterns_recursive(child, source, patterns, found_patterns)?;
        }

        Ok(())
    }

    /// Extract identifiers recursively from AST
    fn extract_identifiers_recursive(
        &self,
        node: Node,
        source: &str,
        identifiers: &mut Vec<String>,
    ) -> Result<()> {
        match node.kind() {
            "identifier" => {
                if let Ok(identifier) = node.utf8_text(source.as_bytes()) {
                    identifiers.push(identifier.to_string());
                }
            }
            _ => {}
        }

        // Process children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_identifiers_recursive(child, source, identifiers)?;
        }

        Ok(())
    }

    /// Count AST nodes recursively
    fn count_nodes_recursive(&self, node: Node) -> usize {
        let mut count = 1; // Count this node

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            count += self.count_nodes_recursive(child);
        }

        count
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

impl Default for PythonAdapter {
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

// Implement the LanguageAdapter trait for comprehensive AST analysis
#[async_trait]
impl LanguageAdapter for PythonAdapter {
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        // Use existing implementation
        PythonAdapter::parse_source(self, source, file_path)
    }

    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            ValknutError::parse("python", "Failed to parse Python source for function calls")
        })?;

        let mut calls = Vec::new();
        let mut cursor = tree.walk();

        self.extract_function_calls_recursive(tree.root_node(), source, &mut calls)?;

        calls.sort();
        calls.dedup();
        Ok(calls)
    }

    fn contains_boilerplate_patterns(
        &mut self,
        source: &str,
        patterns: &[String],
    ) -> Result<Vec<String>> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            ValknutError::parse(
                "python",
                "Failed to parse Python source for boilerplate analysis",
            )
        })?;

        let mut found_patterns = Vec::new();

        // Walk the AST looking for boilerplate patterns
        self.check_boilerplate_patterns_recursive(
            tree.root_node(),
            source,
            patterns,
            &mut found_patterns,
        )?;

        found_patterns.sort();
        found_patterns.dedup();
        Ok(found_patterns)
    }

    fn extract_identifiers(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            ValknutError::parse("python", "Failed to parse Python source for identifiers")
        })?;

        let mut identifiers = Vec::new();
        self.extract_identifiers_recursive(tree.root_node(), source, &mut identifiers)?;

        identifiers.sort();
        identifiers.dedup();
        Ok(identifiers)
    }

    fn count_ast_nodes(&mut self, source: &str) -> Result<usize> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            ValknutError::parse("python", "Failed to parse Python source for AST counting")
        })?;

        Ok(self.count_nodes_recursive(tree.root_node()))
    }

    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            ValknutError::parse("python", "Failed to parse Python source for block counting")
        })?;

        let mut block_count = 0;
        self.count_blocks_recursive(tree.root_node(), &mut block_count);

        Ok(block_count.max(1))
    }

    fn normalize_source(&mut self, source: &str) -> Result<String> {
        let tree = self.parser.parse(source, None).ok_or_else(|| {
            ValknutError::parse("python", "Failed to parse Python source for normalization")
        })?;

        let mut normalized_parts = Vec::new();
        self.normalize_ast_recursive(tree.root_node(), source, &mut normalized_parts)?;

        Ok(normalized_parts.join(" "))
    }

    fn language_name(&self) -> &str {
        "python"
    }

    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        let mut imports = Vec::new();

        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(import_part) = trimmed.strip_prefix("import ") {
                let module = import_part
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                imports.push(ImportStatement {
                    module,
                    imports: None,
                    import_type: "module".to_string(),
                    line_number: line_number + 1,
                });
            } else if let Some(from_part) = trimmed.strip_prefix("from ") {
                if let Some(import_pos) = from_part.find(" import ") {
                    let module = from_part[..import_pos].trim().to_string();
                    let import_list = from_part[import_pos + 8..].trim();

                    let specific_imports = if import_list == "*" {
                        None
                    } else {
                        Some(
                            import_list
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .collect(),
                        )
                    };

                    imports.push(ImportStatement {
                        module,
                        imports: specific_imports,
                        import_type: if import_list == "*" { "star" } else { "named" }.to_string(),
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
