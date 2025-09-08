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