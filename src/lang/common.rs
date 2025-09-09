//! Common AST and parsing abstractions.

use serde::{Deserialize, Serialize};

/// Common entity types across all languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    Function,
    Method,
    Class,
    Interface,
    Module,
    Variable,
    Constant,
    Enum,
    Struct,
}

/// Language-agnostic representation of a parsed entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedEntity {
    /// Unique identifier
    pub id: String,
    
    /// Entity type
    pub kind: EntityKind,
    
    /// Entity name
    pub name: String,
    
    /// Parent entity (if any)
    pub parent: Option<String>,
    
    /// Children entities
    pub children: Vec<String>,
    
    /// Source location
    pub location: SourceLocation,
    
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Source location information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    /// File path
    pub file_path: String,
    
    /// Start line (1-based)
    pub start_line: usize,
    
    /// End line (1-based)
    pub end_line: usize,
    
    /// Start column (1-based)
    pub start_column: usize,
    
    /// End column (1-based)
    pub end_column: usize,
}

/// Parse index containing all entities from a parsing session
#[derive(Debug, Default)]
pub struct ParseIndex {
    /// All parsed entities
    pub entities: std::collections::HashMap<String, ParsedEntity>,
    
    /// Entities by file
    pub entities_by_file: std::collections::HashMap<String, Vec<String>>,
    
    /// Dependency relationships
    pub dependencies: std::collections::HashMap<String, Vec<String>>,
}

impl ParseIndex {
    /// Create a new empty parse index
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add an entity to the index
    pub fn add_entity(&mut self, entity: ParsedEntity) {
        let file_path = entity.location.file_path.clone();
        let entity_id = entity.id.clone();
        
        // Add to entities by file
        self.entities_by_file
            .entry(file_path)
            .or_default()
            .push(entity_id.clone());
        
        // Add to main index
        self.entities.insert(entity_id, entity);
    }
    
    /// Get an entity by ID
    pub fn get_entity(&self, id: &str) -> Option<&ParsedEntity> {
        self.entities.get(id)
    }
    
    /// Get all entities in a file
    pub fn get_entities_in_file(&self, file_path: &str) -> Vec<&ParsedEntity> {
        self.entities_by_file
            .get(file_path)
            .map(|ids| ids.iter().filter_map(|id| self.entities.get(id)).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    #[test]
    fn test_entity_kind_variants() {
        // Test all variants can be created
        assert_eq!(EntityKind::Function, EntityKind::Function);
        assert_eq!(EntityKind::Method, EntityKind::Method);
        assert_eq!(EntityKind::Class, EntityKind::Class);
        assert_eq!(EntityKind::Interface, EntityKind::Interface);
        assert_eq!(EntityKind::Module, EntityKind::Module);
        assert_eq!(EntityKind::Variable, EntityKind::Variable);
        assert_eq!(EntityKind::Constant, EntityKind::Constant);
        assert_eq!(EntityKind::Enum, EntityKind::Enum);
        assert_eq!(EntityKind::Struct, EntityKind::Struct);
    }
    
    #[test]
    fn test_source_location() {
        let location = SourceLocation {
            file_path: "test.rs".to_string(),
            start_line: 1,
            end_line: 5,
            start_column: 0,
            end_column: 10,
        };
        
        assert_eq!(location.file_path, "test.rs");
        assert_eq!(location.start_line, 1);
        assert_eq!(location.end_line, 5);
        assert_eq!(location.start_column, 0);
        assert_eq!(location.end_column, 10);
    }
    
    #[test]
    fn test_parsed_entity() {
        let location = SourceLocation {
            file_path: "test.rs".to_string(),
            start_line: 1,
            end_line: 5,
            start_column: 0,
            end_column: 10,
        };
        
        let entity = ParsedEntity {
            id: "func1".to_string(),
            kind: EntityKind::Function,
            name: "test_function".to_string(),
            parent: None,
            children: vec!["var1".to_string()],
            location,
            metadata: HashMap::new(),
        };
        
        assert_eq!(entity.id, "func1");
        assert_eq!(entity.kind, EntityKind::Function);
        assert_eq!(entity.name, "test_function");
        assert_eq!(entity.parent, None);
        assert_eq!(entity.children.len(), 1);
        assert_eq!(entity.children[0], "var1");
        assert!(entity.metadata.is_empty());
    }
    
    #[test]
    fn test_parse_index_new() {
        let index = ParseIndex::new();
        assert!(index.entities.is_empty());
        assert!(index.entities_by_file.is_empty());
        assert!(index.dependencies.is_empty());
    }
    
    #[test]
    fn test_parse_index_default() {
        let index = ParseIndex::default();
        assert!(index.entities.is_empty());
        assert!(index.entities_by_file.is_empty());
        assert!(index.dependencies.is_empty());
    }
    
    #[test]
    fn test_parse_index_add_entity() {
        let mut index = ParseIndex::new();
        
        let location = SourceLocation {
            file_path: "test.rs".to_string(),
            start_line: 1,
            end_line: 5,
            start_column: 0,
            end_column: 10,
        };
        
        let entity = ParsedEntity {
            id: "func1".to_string(),
            kind: EntityKind::Function,
            name: "test_function".to_string(),
            parent: None,
            children: vec![],
            location,
            metadata: HashMap::new(),
        };
        
        index.add_entity(entity);
        
        assert_eq!(index.entities.len(), 1);
        assert_eq!(index.entities_by_file.len(), 1);
        assert!(index.entities_by_file.contains_key("test.rs"));
        assert_eq!(index.entities_by_file["test.rs"].len(), 1);
        assert_eq!(index.entities_by_file["test.rs"][0], "func1");
    }
    
    #[test]
    fn test_parse_index_get_entity() {
        let mut index = ParseIndex::new();
        
        let location = SourceLocation {
            file_path: "test.rs".to_string(),
            start_line: 1,
            end_line: 5,
            start_column: 0,
            end_column: 10,
        };
        
        let entity = ParsedEntity {
            id: "func1".to_string(),
            kind: EntityKind::Function,
            name: "test_function".to_string(),
            parent: None,
            children: vec![],
            location,
            metadata: HashMap::new(),
        };
        
        index.add_entity(entity);
        
        let retrieved = index.get_entity("func1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "func1");
        assert_eq!(retrieved.unwrap().name, "test_function");
        
        let not_found = index.get_entity("nonexistent");
        assert!(not_found.is_none());
    }
    
    #[test]
    fn test_parse_index_get_entities_in_file() {
        let mut index = ParseIndex::new();
        
        let location1 = SourceLocation {
            file_path: "test.rs".to_string(),
            start_line: 1,
            end_line: 5,
            start_column: 0,
            end_column: 10,
        };
        
        let location2 = SourceLocation {
            file_path: "test.rs".to_string(),
            start_line: 10,
            end_line: 15,
            start_column: 0,
            end_column: 20,
        };
        
        let entity1 = ParsedEntity {
            id: "func1".to_string(),
            kind: EntityKind::Function,
            name: "test_function1".to_string(),
            parent: None,
            children: vec![],
            location: location1,
            metadata: HashMap::new(),
        };
        
        let entity2 = ParsedEntity {
            id: "func2".to_string(),
            kind: EntityKind::Function,
            name: "test_function2".to_string(),
            parent: None,
            children: vec![],
            location: location2,
            metadata: HashMap::new(),
        };
        
        index.add_entity(entity1);
        index.add_entity(entity2);
        
        let entities_in_file = index.get_entities_in_file("test.rs");
        assert_eq!(entities_in_file.len(), 2);
        
        let entities_in_other = index.get_entities_in_file("other.rs");
        assert!(entities_in_other.is_empty());
    }
}