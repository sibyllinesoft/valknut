//! Common AST and parsing abstractions.

use crate::core::errors::Result;
use crate::detectors::structure::config::ImportStatement;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Common entity types across all languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Utility methods for [`EntityKind`].
impl EntityKind {
    /// Generate a fallback name for an anonymous entity of this kind.
    pub fn fallback_name(self, counter: usize) -> String {
        let kind_str = match self {
            EntityKind::Function => "function",
            EntityKind::Method => "method",
            EntityKind::Class => "class",
            EntityKind::Interface => "interface",
            EntityKind::Module => "module",
            EntityKind::Variable => "variable",
            EntityKind::Constant => "constant",
            EntityKind::Enum => "enum",
            EntityKind::Struct => "struct",
        };
        format!("anonymous_{}_{}", kind_str, counter)
    }
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

/// Factory, query, and mutation methods for [`ParseIndex`].
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

    /// Count AST nodes (approximate based on entities)
    pub fn count_ast_nodes(&self) -> usize {
        // Each entity represents multiple AST nodes
        // This is a heuristic approximation
        self.entities.len() * 8
    }

    /// Count distinct code blocks (functions, classes, control structures)
    pub fn count_distinct_blocks(&self) -> usize {
        let mut block_count = 0;

        for entity in self.entities.values() {
            match entity.kind {
                EntityKind::Function | EntityKind::Method => block_count += 1,
                EntityKind::Class
                | EntityKind::Interface
                | EntityKind::Struct
                | EntityKind::Enum => block_count += 1,
                EntityKind::Module => block_count += 1,
                _ => {}
            }
        }

        // Add heuristic for control structures based on function count
        let function_count = self
            .entities
            .values()
            .filter(|entity| matches!(entity.kind, EntityKind::Function | EntityKind::Method))
            .count();

        block_count += function_count * 2; // Heuristic: each function has ~2 control structures

        block_count.max(1) // At least 1 block
    }

    /// Get all function calls from the parsed entities
    pub fn get_function_calls(&self) -> Vec<String> {
        self.entities
            .values()
            .flat_map(|entity| {
                entity
                    .metadata
                    .get("function_calls")
                    .and_then(|m| m.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|call| call.as_str().map(String::from))
            })
            .collect()
    }

    /// Check if the parsed code contains boilerplate patterns
    pub fn contains_boilerplate_patterns(&self, patterns: &[String]) -> Vec<String> {
        let mut found_patterns: Vec<String> = self
            .entities
            .values()
            .flat_map(|entity| {
                patterns.iter().filter(move |pattern| {
                    entity.name.contains(pattern.as_str())
                        || entity
                            .metadata
                            .get("source_text")
                            .and_then(|v| v.as_str())
                            .map_or(false, |text| text.contains(pattern.as_str()))
                })
            })
            .cloned()
            .collect();

        found_patterns.sort();
        found_patterns.dedup();
        found_patterns
    }

    /// Extract identifiers from all entities
    pub fn extract_identifiers(&self) -> Vec<String> {
        let mut identifiers: Vec<String> = self
            .entities
            .values()
            .flat_map(|entity| {
                let metadata_ids = entity
                    .metadata
                    .get("identifiers")
                    .and_then(|m| m.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|id| id.as_str().map(String::from));

                std::iter::once(entity.name.clone()).chain(metadata_ids)
            })
            .collect();

        identifiers.sort();
        identifiers.dedup();
        identifiers
    }
}

/// Language adapter trait for AST parsing and analysis
#[async_trait]
pub trait LanguageAdapter: Send + Sync {
    /// Parse source code and return a parse index
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex>;

    /// Extract function calls from source code using tree-sitter
    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>>;

    /// Check if source contains boilerplate patterns using AST analysis
    fn contains_boilerplate_patterns(
        &mut self,
        source: &str,
        patterns: &[String],
    ) -> Result<Vec<String>>;

    /// Extract identifiers from source using tree-sitter
    fn extract_identifiers(&mut self, source: &str) -> Result<Vec<String>>;

    /// Count AST nodes in the source
    fn count_ast_nodes(&mut self, source: &str) -> Result<usize>;

    /// Count distinct code blocks (functions, classes, control structures)
    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize>;

    /// Normalize source code for comparison (AST-based)
    fn normalize_source(&mut self, source: &str) -> Result<String>;

    /// Get language name
    fn language_name(&self) -> &str;

    /// Extract import statements from source code
    fn extract_imports(&mut self, _source: &str) -> Result<Vec<ImportStatement>> {
        Ok(Vec::new())
    }

    /// Extract code entities (functions, classes, etc.) from source code
    fn extract_code_entities(
        &mut self,
        source: &str,
        file_path: &str,
    ) -> Result<Vec<crate::core::featureset::CodeEntity>>;

    /// Extract code entities using interned strings for optimal performance
    /// Default implementation converts from regular extraction - language adapters should override
    fn extract_code_entities_interned(
        &mut self,
        source: &str,
        file_path: &str,
    ) -> Result<Vec<crate::core::interned_entities::InternedCodeEntity>> {
        let regular_entities = self.extract_code_entities(source, file_path)?;
        Ok(regular_entities
            .into_iter()
            .map(|entity| {
                crate::core::interned_entities::InternedCodeEntity::from_code_entity(&entity)
            })
            .collect())
    }
}

/// Normalize a raw module literal by removing trailing semicolons and surrounding quotes.
///
/// Shared utility for JavaScript and TypeScript import extraction.
pub fn normalize_module_literal(raw: &str) -> String {
    raw.trim()
        .trim_end_matches(';')
        .trim_matches(['"', '\'', '`'])
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    #[test]
    fn test_parse_index_metadata_helpers() {
        let mut index = ParseIndex::new();

        let mut metadata = HashMap::new();
        metadata.insert(
            "function_calls".to_string(),
            json!(["helper()", "utility_call()"]),
        );
        metadata.insert(
            "source_text".to_string(),
            json!("fn helper() { /* boilerplate */ }"),
        );
        metadata.insert("identifiers".to_string(), json!(["helper", "value"]));

        let function = ParsedEntity {
            id: "func1".to_string(),
            kind: EntityKind::Function,
            name: "helper".to_string(),
            parent: None,
            children: vec![],
            location: SourceLocation {
                file_path: "test.rs".to_string(),
                start_line: 10,
                end_line: 20,
                start_column: 0,
                end_column: 5,
            },
            metadata,
        };

        let class = ParsedEntity {
            id: "class1".to_string(),
            kind: EntityKind::Class,
            name: "Utility".to_string(),
            parent: None,
            children: vec!["func1".to_string()],
            location: SourceLocation {
                file_path: "test.rs".to_string(),
                start_line: 1,
                end_line: 40,
                start_column: 0,
                end_column: 1,
            },
            metadata: HashMap::new(),
        };

        index.add_entity(function);
        index.add_entity(class);

        assert_eq!(index.count_ast_nodes(), index.entities.len() * 8);
        assert!(index.count_distinct_blocks() >= 3);

        let calls = index.get_function_calls();
        assert!(calls.contains(&"helper()".to_string()));
        assert!(calls.contains(&"utility_call()".to_string()));

        let patterns = index.contains_boilerplate_patterns(&[
            "helper".to_string(),
            "boilerplate".to_string(),
            "absent".to_string(),
        ]);
        assert!(patterns.contains(&"helper".to_string()));
        assert!(patterns.contains(&"boilerplate".to_string()));
        assert!(!patterns.contains(&"absent".to_string()));

        let identifiers = index.extract_identifiers();
        assert!(identifiers.contains(&"helper".to_string()));
        assert!(identifiers.contains(&"value".to_string()));
        assert!(identifiers.contains(&"Utility".to_string()));
        assert_eq!(
            identifiers.iter().filter(|id| *id == "helper").count(),
            1,
            "identifiers should be deduplicated"
        );
    }

    struct DummyAdapter {
        call_count: AtomicUsize,
    }

    #[async_trait]
    impl LanguageAdapter for DummyAdapter {
        fn parse_source(&mut self, _source: &str, _file_path: &str) -> Result<ParseIndex> {
            Ok(ParseIndex::new())
        }

        fn extract_function_calls(&mut self, _source: &str) -> Result<Vec<String>> {
            Ok(vec!["call()".to_string()])
        }

        fn contains_boilerplate_patterns(
            &mut self,
            _source: &str,
            patterns: &[String],
        ) -> Result<Vec<String>> {
            Ok(patterns.iter().cloned().collect())
        }

        fn extract_identifiers(&mut self, _source: &str) -> Result<Vec<String>> {
            Ok(vec!["identifier".to_string()])
        }

        fn count_ast_nodes(&mut self, _source: &str) -> Result<usize> {
            Ok(1)
        }

        fn count_distinct_blocks(&mut self, _source: &str) -> Result<usize> {
            Ok(1)
        }

        fn normalize_source(&mut self, source: &str) -> Result<String> {
            Ok(source.to_string())
        }

        fn language_name(&self) -> &str {
            "dummy"
        }

        fn extract_code_entities(
            &mut self,
            _source: &str,
            file_path: &str,
        ) -> Result<Vec<crate::core::featureset::CodeEntity>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let entity = crate::core::featureset::CodeEntity::new(
                "entity-1", "Function", "Dummy", file_path,
            )
            .with_line_range(1, 5)
            .with_source_code("fn dummy() {}");
            Ok(vec![entity])
        }
    }

    #[test]
    fn language_adapter_default_methods_cover_interning() {
        let mut adapter = DummyAdapter {
            call_count: AtomicUsize::new(0),
        };
        let imports = adapter.extract_imports("package main").unwrap();
        assert!(imports.is_empty());

        let interned = adapter
            .extract_code_entities_interned("fn main() {}", "main.rs")
            .expect("interned conversion");
        assert_eq!(interned.len(), 1);
        assert_eq!(interned[0].name_str(), "Dummy");
        assert_eq!(adapter.call_count.load(Ordering::SeqCst), 1);
    }
}
