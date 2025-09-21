use crate::core::interning::{global_interner, intern, resolve, InternedString};
use crate::lang::common::{EntityKind, ParsedEntity, SourceLocation};
use crate::core::featureset::CodeEntity;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use std::collections::HashMap;
use std::fmt;

/// Interned version of SourceLocation with zero string allocations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InternedSourceLocation {
    /// Interned file path
    pub file_path: InternedString,
    /// Start line (1-based)
    pub start_line: usize,
    /// End line (1-based)  
    pub end_line: usize,
    /// Start column (1-based)
    pub start_column: usize,
    /// End column (1-based)
    pub end_column: usize,
}

impl InternedSourceLocation {
    /// Create a new interned source location
    pub fn new(file_path: &str, start_line: usize, end_line: usize, start_column: usize, end_column: usize) -> Self {
        Self {
            file_path: intern(file_path),
            start_line,
            end_line,
            start_column,
            end_column,
        }
    }

    /// Convert from regular SourceLocation
    pub fn from_source_location(location: &SourceLocation) -> Self {
        Self::new(
            &location.file_path,
            location.start_line,
            location.end_line,
            location.start_column,
            location.end_column,
        )
    }

    /// Convert to regular SourceLocation for compatibility
    pub fn to_source_location(&self) -> SourceLocation {
        SourceLocation {
            file_path: resolve(self.file_path).to_string(),
            start_line: self.start_line,
            end_line: self.end_line,
            start_column: self.start_column,
            end_column: self.end_column,
        }
    }

    /// Get file path as string (zero-cost lookup)
    pub fn file_path_str(&self) -> &str {
        resolve(self.file_path)
    }
}

/// Interned version of ParsedEntity with zero string allocations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternedParsedEntity {
    /// Interned unique identifier
    pub id: InternedString,
    /// Entity type
    pub kind: EntityKind,
    /// Interned entity name  
    pub name: InternedString,
    /// Interned parent entity (if any)
    pub parent: Option<InternedString>,
    /// Interned children entities
    pub children: Vec<InternedString>,
    /// Interned source location
    pub location: InternedSourceLocation,
    /// Additional metadata (keys are interned for performance)
    pub metadata: HashMap<InternedString, serde_json::Value>,
}

impl InternedParsedEntity {
    /// Create a new interned parsed entity
    pub fn new(id: &str, kind: EntityKind, name: &str, location: InternedSourceLocation) -> Self {
        Self {
            id: intern(id),
            kind,
            name: intern(name),
            parent: None,
            children: Vec::new(),
            location,
            metadata: HashMap::new(),
        }
    }

    /// Convert from regular ParsedEntity
    pub fn from_parsed_entity(entity: &ParsedEntity) -> Self {
        Self {
            id: intern(&entity.id),
            kind: entity.kind,
            name: intern(&entity.name),
            parent: entity.parent.as_ref().map(|p| intern(p)),
            children: entity.children.iter().map(|c| intern(c)).collect(),
            location: InternedSourceLocation::from_source_location(&entity.location),
            metadata: entity.metadata.iter()
                .map(|(k, v)| (intern(k), v.clone()))
                .collect(),
        }
    }

    /// Convert to regular ParsedEntity for compatibility
    pub fn to_parsed_entity(&self) -> ParsedEntity {
        ParsedEntity {
            id: resolve(self.id).to_string(),
            kind: self.kind,
            name: resolve(self.name).to_string(),
            parent: self.parent.map(|p| resolve(p).to_string()),
            children: self.children.iter().map(|&c| resolve(c).to_string()).collect(),
            location: self.location.to_source_location(),
            metadata: self.metadata.iter()
                .map(|(k, v)| (resolve(*k).to_string(), v.clone()))
                .collect(),
        }
    }

    /// Get entity name as string (zero-cost lookup)
    pub fn name_str(&self) -> &str {
        resolve(self.name)
    }

    /// Get entity id as string (zero-cost lookup)
    pub fn id_str(&self) -> &str {
        resolve(self.id)
    }

    /// Add a child entity (by interned id)
    pub fn add_child(&mut self, child_id: InternedString) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }

    /// Add a child entity (by string - will be interned)
    pub fn add_child_str(&mut self, child_id: &str) {
        self.add_child(intern(child_id));
    }

    /// Set parent entity (by interned id)
    pub fn set_parent(&mut self, parent_id: InternedString) {
        self.parent = Some(parent_id);
    }

    /// Set parent entity (by string - will be interned)
    pub fn set_parent_str(&mut self, parent_id: &str) {
        self.parent = Some(intern(parent_id));
    }
}

/// Interned version of CodeEntity with zero string allocations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternedCodeEntity {
    /// Interned unique identifier
    pub id: InternedString,
    /// Interned entity type (function, class, module, etc.)
    pub entity_type: InternedString,
    /// Interned entity name
    pub name: InternedString,
    /// Interned source file path
    pub file_path: InternedString,
    /// Line number range
    pub line_range: Option<(usize, usize)>,
    /// Interned raw source code
    pub source_code: InternedString,
    /// Additional properties (keys are interned for performance)
    pub properties: HashMap<InternedString, serde_json::Value>,
}

impl InternedCodeEntity {
    /// Create a new interned code entity
    pub fn new(id: &str, entity_type: &str, name: &str, file_path: &str) -> Self {
        Self {
            id: intern(id),
            entity_type: intern(entity_type),
            name: intern(name),
            file_path: intern(file_path),
            line_range: None,
            source_code: intern(""), // Empty by default
            properties: HashMap::new(),
        }
    }

    /// Convert from regular CodeEntity
    pub fn from_code_entity(entity: &CodeEntity) -> Self {
        Self {
            id: intern(&entity.id),
            entity_type: intern(&entity.entity_type),
            name: intern(&entity.name),
            file_path: intern(&entity.file_path),
            line_range: entity.line_range,
            source_code: intern(&entity.source_code),
            properties: entity.properties.iter()
                .map(|(k, v)| (intern(k), v.clone()))
                .collect(),
        }
    }

    /// Convert to regular CodeEntity for compatibility
    pub fn to_code_entity(&self) -> CodeEntity {
        CodeEntity {
            id: resolve(self.id).to_string(),
            entity_type: resolve(self.entity_type).to_string(),
            name: resolve(self.name).to_string(),
            file_path: resolve(self.file_path).to_string(),
            line_range: self.line_range,
            source_code: resolve(self.source_code).to_string(),
            properties: self.properties.iter()
                .map(|(k, v)| (resolve(*k).to_string(), v.clone()))
                .collect(),
        }
    }

    /// Builder-style methods for easy construction
    pub fn with_source_code(mut self, source_code: &str) -> Self {
        self.source_code = intern(source_code);
        self
    }

    pub fn with_line_range(mut self, start: usize, end: usize) -> Self {
        self.line_range = Some((start, end));
        self
    }

    pub fn with_property(mut self, key: &str, value: serde_json::Value) -> Self {
        self.properties.insert(intern(key), value);
        self
    }

    /// Get entity name as string (zero-cost lookup)
    pub fn name_str(&self) -> &str {
        resolve(self.name)
    }

    /// Get entity id as string (zero-cost lookup)
    pub fn id_str(&self) -> &str {
        resolve(self.id)
    }

    /// Get entity type as string (zero-cost lookup)
    pub fn entity_type_str(&self) -> &str {
        resolve(self.entity_type)
    }

    /// Get file path as string (zero-cost lookup)
    pub fn file_path_str(&self) -> &str {
        resolve(self.file_path)
    }

    /// Get source code as string (zero-cost lookup)
    pub fn source_code_str(&self) -> &str {
        resolve(self.source_code)
    }

    /// Calculate line count from source code
    pub fn line_count(&self) -> usize {
        self.source_code_str().lines().count().max(1)
    }
}

/// Builder for creating InternedCodeEntity with fluent API
pub struct InternedCodeEntityBuilder {
    entity: InternedCodeEntity,
}

impl InternedCodeEntityBuilder {
    /// Create a new builder with required fields
    pub fn new(id: &str, entity_type: &str, name: &str, file_path: &str) -> Self {
        Self {
            entity: InternedCodeEntity::new(id, entity_type, name, file_path),
        }
    }

    /// Set source code
    pub fn with_source_code(mut self, source_code: &str) -> Self {
        self.entity.source_code = intern(source_code);
        self
    }

    /// Set line range
    pub fn with_line_range(mut self, start: usize, end: usize) -> Self {
        self.entity.line_range = Some((start, end));
        self
    }

    /// Add a property
    pub fn with_property(mut self, key: &str, value: serde_json::Value) -> Self {
        self.entity.properties.insert(intern(key), value);
        self
    }

    /// Build the final entity
    pub fn build(self) -> InternedCodeEntity {
        self.entity
    }
}

/// Parse index using interned entities for optimal performance
#[derive(Debug, Default)]
pub struct InternedParseIndex {
    /// All parsed entities (interned for performance)
    pub entities: HashMap<InternedString, InternedParsedEntity>,
    /// Entities by file (interned keys)
    pub entities_by_file: HashMap<InternedString, Vec<InternedString>>,
    /// Dependency relationships (interned keys)
    pub dependencies: HashMap<InternedString, Vec<InternedString>>,
}

impl InternedParseIndex {
    /// Create a new empty interned parse index
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entity to the index
    pub fn add_entity(&mut self, entity: InternedParsedEntity) {
        let file_path = entity.location.file_path;
        let entity_id = entity.id;

        // Add to entities by file
        self.entities_by_file
            .entry(file_path)
            .or_default()
            .push(entity_id);

        // Add to main index
        self.entities.insert(entity_id, entity);
    }

    /// Get an entity by interned ID
    pub fn get_entity(&self, id: InternedString) -> Option<&InternedParsedEntity> {
        self.entities.get(&id)
    }

    /// Get an entity by string ID
    pub fn get_entity_by_str(&self, id: &str) -> Option<&InternedParsedEntity> {
        let interned_id = global_interner().get(id)?;
        self.entities.get(&interned_id)
    }

    /// Get all entities in a file
    pub fn get_entities_in_file(&self, file_path: &str) -> Vec<&InternedParsedEntity> {
        let file_path_interned = match global_interner().get(file_path) {
            Some(interned) => interned,
            None => return Vec::new(),
        };

        self.entities_by_file
            .get(&file_path_interned)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.entities.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Convert to regular ParseIndex for compatibility
    pub fn to_parse_index(&self) -> crate::lang::common::ParseIndex {
        let mut parse_index = crate::lang::common::ParseIndex::new();
        
        for entity in self.entities.values() {
            parse_index.add_entity(entity.to_parsed_entity());
        }
        
        parse_index
    }

    /// Get entity count
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Get file count
    pub fn file_count(&self) -> usize {
        self.entities_by_file.len()
    }
}

// Implement Display for debugging
impl fmt::Display for InternedCodeEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InternedCodeEntity(id: {}, type: {}, name: {}, file: {})",
            self.id_str(),
            self.entity_type_str(),
            self.name_str(),
            self.file_path_str()
        )
    }
}

impl fmt::Display for InternedParsedEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InternedParsedEntity(id: {}, kind: {:?}, name: {})",
            self.id_str(),
            self.kind,
            self.name_str()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::common::EntityKind;

    #[test]
    fn test_interned_code_entity_creation() {
        let entity = InternedCodeEntity::new("test-id", "function", "test_func", "/test/file.rs")
            .with_source_code("fn test_func() { }")
            .with_line_range(1, 3);

        assert_eq!(entity.name_str(), "test_func");
        assert_eq!(entity.entity_type_str(), "function");
        assert_eq!(entity.file_path_str(), "/test/file.rs");
        assert_eq!(entity.source_code_str(), "fn test_func() { }");
        assert_eq!(entity.line_range, Some((1, 3)));
    }

    #[test]
    fn test_interned_parsed_entity_creation() {
        let location = InternedSourceLocation::new("/test/file.rs", 1, 3, 0, 10);
        let entity = InternedParsedEntity::new("test-id", EntityKind::Function, "test_func", location);

        assert_eq!(entity.name_str(), "test_func");
        assert_eq!(entity.id_str(), "test-id");
        assert_eq!(entity.kind, EntityKind::Function);
        assert_eq!(entity.location.file_path_str(), "/test/file.rs");
    }

    #[test]
    fn test_conversion_compatibility() {
        // Create original entities
        let location = SourceLocation {
            file_path: "/test/file.rs".to_string(),
            start_line: 1,
            end_line: 3,
            start_column: 0,
            end_column: 10,
        };

        let parsed_entity = ParsedEntity {
            id: "test-id".to_string(),
            kind: EntityKind::Function,
            name: "test_func".to_string(),
            parent: None,
            children: vec![],
            location,
            metadata: HashMap::new(),
        };

        // Convert to interned and back
        let interned = InternedParsedEntity::from_parsed_entity(&parsed_entity);
        let converted_back = interned.to_parsed_entity();

        assert_eq!(parsed_entity.id, converted_back.id);
        assert_eq!(parsed_entity.name, converted_back.name);
        assert_eq!(parsed_entity.kind, converted_back.kind);
    }

    #[test]
    fn test_interned_parse_index() {
        let mut index = InternedParseIndex::new();
        
        let location = InternedSourceLocation::new("/test/file.rs", 1, 3, 0, 10);
        let entity = InternedParsedEntity::new("test-id", EntityKind::Function, "test_func", location);
        
        index.add_entity(entity);
        
        assert_eq!(index.entity_count(), 1);
        assert_eq!(index.file_count(), 1);
        
        let entities = index.get_entities_in_file("/test/file.rs");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name_str(), "test_func");
    }

    #[test]
    fn test_string_deduplication() {
        let entity1 = InternedCodeEntity::new("id1", "function", "same_name", "/file.rs");
        let entity2 = InternedCodeEntity::new("id2", "function", "same_name", "/file.rs");
        
        // Names should have the same interned key (deduplication)
        assert_eq!(entity1.name, entity2.name);
        assert_eq!(entity1.entity_type, entity2.entity_type);
        assert_eq!(entity1.file_path, entity2.file_path);
    }
}