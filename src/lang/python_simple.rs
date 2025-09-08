//! Simple Python language adapter for testing tree-sitter integration.

use std::collections::HashMap;
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::CodeEntity;

/// Simple Python adapter for testing
pub struct SimplePythonAdapter;

impl SimplePythonAdapter {
    /// Create a new simple Python adapter
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
    
    /// Extract code entities from Python source (simple implementation)
    pub fn extract_code_entities(&mut self, source_code: &str, file_path: &str) -> Result<Vec<CodeEntity>> {
        let mut entities = Vec::new();
        let lines: Vec<&str> = source_code.lines().collect();
        
        for (line_no, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            
            // Simple function detection
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                if let Some(name_start) = trimmed.find("def ").map(|i| i + 4) {
                    if let Some(name_end) = trimmed[name_start..].find('(') {
                        let function_name = trimmed[name_start..name_start + name_end].trim();
                        
                        let mut entity = CodeEntity::new(
                            format!("{}:function:{}", file_path, line_no + 1),
                            "Function".to_string(),
                            function_name.to_string(),
                            file_path.to_string(),
                        )
                        .with_line_range(line_no + 1, line_no + 1)
                        .with_source_code(line.to_string());
                        
                        entity.add_property("simple_parser".to_string(), serde_json::Value::Bool(true));
                        entities.push(entity);
                    }
                }
            }
            
            // Simple class detection
            if trimmed.starts_with("class ") && trimmed.ends_with(':') {
                if let Some(name_start) = trimmed.find("class ").map(|i| i + 6) {
                    let class_part = if let Some(paren_pos) = trimmed[name_start..].find('(') {
                        &trimmed[name_start..name_start + paren_pos]
                    } else {
                        &trimmed[name_start..trimmed.len() - 1]
                    };
                    
                    let class_name = class_part.trim();
                    
                    let mut entity = CodeEntity::new(
                        format!("{}:class:{}", file_path, line_no + 1),
                        "Class".to_string(),
                        class_name.to_string(),
                        file_path.to_string(),
                    )
                    .with_line_range(line_no + 1, line_no + 1)
                    .with_source_code(line.to_string());
                    
                    entity.add_property("simple_parser".to_string(), serde_json::Value::Bool(true));
                    entities.push(entity);
                }
            }
        }
        
        Ok(entities)
    }
}

impl Default for SimplePythonAdapter {
    fn default() -> Self {
        Self::new().expect("Failed to create simple Python adapter")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_python_adapter_creation() {
        let adapter = SimplePythonAdapter::new();
        assert!(adapter.is_ok());
    }
    
    #[test]
    fn test_simple_function_parsing() {
        let mut adapter = SimplePythonAdapter::new().unwrap();
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
        
        // Check our simple parser property
        let simple_parser = function_entity.properties.get("simple_parser");
        assert_eq!(simple_parser, Some(&serde_json::Value::Bool(true)));
    }
    
    #[test]
    fn test_simple_class_parsing() {
        let mut adapter = SimplePythonAdapter::new().unwrap();
        let source_code = r#"
class TestClass:
    def __init__(self, value):
        self.value = value
    
    def get_value(self):
        return self.value

class AnotherClass(BaseClass):
    pass
"#;
        
        let entities = adapter.extract_code_entities(source_code, "test.py").unwrap();
        
        // Should find classes and methods
        let class_entities: Vec<_> = entities.iter().filter(|e| e.entity_type == "Class").collect();
        let function_entities: Vec<_> = entities.iter().filter(|e| e.entity_type == "Function").collect();
        
        assert_eq!(class_entities.len(), 2); // TestClass and AnotherClass
        assert_eq!(function_entities.len(), 2); // __init__ and get_value
        
        assert_eq!(class_entities[0].name, "TestClass");
        assert_eq!(class_entities[1].name, "AnotherClass");
    }
}