//! Common AST and parsing abstractions.

use crate::core::ast_utils::{count_all_nodes, node_text_normalized, walk_tree};
use crate::core::errors::Result;
use crate::core::featureset::CodeEntity;
use crate::detectors::structure::config::ImportStatement;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Tree};

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

impl ParsedEntity {
    /// Convert this ParsedEntity to a CodeEntity.
    ///
    /// Extracts the source code for this entity from the provided source.
    pub fn to_code_entity(&self, source_code: &str) -> CodeEntity {
        let source_lines: Vec<&str> = source_code.lines().collect();
        let entity_source = if self.location.start_line <= source_lines.len()
            && self.location.end_line <= source_lines.len()
        {
            source_lines[(self.location.start_line - 1)..self.location.end_line].join("\n")
        } else {
            String::new()
        };

        let mut code_entity = CodeEntity::new(
            self.id.clone(),
            format!("{:?}", self.kind),
            self.name.clone(),
            self.location.file_path.clone(),
        )
        .with_line_range(self.location.start_line, self.location.end_line)
        .with_source_code(entity_source);

        for (key, value) in &self.metadata {
            code_entity.add_property(key.clone(), value.clone());
        }

        code_entity
    }
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

impl SourceLocation {
    /// Create a SourceLocation from 0-based row/column positions (tree-sitter format).
    ///
    /// Converts to 1-based line/column values.
    pub fn from_positions(
        file_path: &str,
        start_row: usize,
        start_col: usize,
        end_row: usize,
        end_col: usize,
    ) -> Self {
        Self {
            file_path: file_path.to_string(),
            start_line: start_row + 1,
            end_line: end_row + 1,
            start_column: start_col + 1,
            end_column: end_col + 1,
        }
    }
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

        sort_and_dedup(&mut found_patterns);
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

        sort_and_dedup(&mut identifiers);
        identifiers
    }
}

/// Language adapter trait for AST parsing and analysis
#[async_trait]
pub trait LanguageAdapter: Send + Sync {
    /// Parse source code into a tree-sitter AST.
    /// This is the foundation for other tree-based operations.
    fn parse_tree(&mut self, source: &str) -> Result<Tree>;

    /// Parse source code and return a parse index
    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex>;

    /// Extract function calls from source code using tree-sitter
    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>>;

    /// Check if source contains boilerplate patterns.
    /// Default implementation uses text-based pattern matching.
    /// Adapters can override this with AST-based detection.
    fn contains_boilerplate_patterns(
        &mut self,
        source: &str,
        patterns: &[String],
    ) -> Result<Vec<String>> {
        Ok(find_boilerplate_patterns(source, patterns))
    }

    /// Extract identifiers from source using tree-sitter
    fn extract_identifiers(&mut self, source: &str) -> Result<Vec<String>>;

    /// Count AST nodes in the source.
    /// Default implementation uses the parse_tree method.
    fn count_ast_nodes(&mut self, source: &str) -> Result<usize> {
        let tree = self.parse_tree(source)?;
        Ok(count_all_nodes(&tree.root_node()))
    }

    /// Count distinct code blocks (functions, classes, control structures)
    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize>;

    /// Normalize source code for comparison (AST-based).
    /// Default implementation returns the S-expression of the AST.
    fn normalize_source(&mut self, source: &str) -> Result<String> {
        let tree = self.parse_tree(source)?;
        Ok(tree.root_node().to_sexp())
    }

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

/// Sort and deduplicate a vector in place.
///
/// Utility to reduce duplicate `vec.sort(); vec.dedup();` patterns across adapters.
pub fn sort_and_dedup<T: Ord>(vec: &mut Vec<T>) {
    vec.sort();
    vec.dedup();
}

/// Generate a unique entity ID from file path, entity kind, and counter.
///
/// Format: `{file_path}:{kind_as_u8}:{counter}`
pub fn generate_entity_id(file_path: &str, kind: EntityKind, counter: usize) -> String {
    format!("{}:{}:{}", file_path, kind as u8, counter)
}

/// Create base metadata for a parsed entity with common fields.
///
/// Initializes a HashMap with `node_kind` and `byte_range` fields.
pub fn create_base_metadata(
    node_kind: &str,
    start_byte: usize,
    end_byte: usize,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert(
        "node_kind".to_string(),
        serde_json::Value::String(node_kind.to_string()),
    );
    metadata.insert(
        "byte_range".to_string(),
        serde_json::json!([start_byte, end_byte]),
    );
    metadata
}

/// Find boilerplate patterns in source code using simple string matching.
///
/// Returns a sorted, deduplicated list of patterns found in the source.
/// This is a shared implementation used by multiple language adapters.
pub fn find_boilerplate_patterns(source: &str, patterns: &[String]) -> Vec<String> {
    let mut found: Vec<String> = patterns
        .iter()
        .filter(|pattern| !pattern.is_empty() && source.contains(pattern.as_str()))
        .cloned()
        .collect();

    sort_and_dedup(&mut found);
    found
}

/// Parse a CommonJS require() import statement.
///
/// This is used by JavaScript and TypeScript adapters to parse require() calls.
/// Returns None if the input doesn't match the expected format.
pub fn parse_require_import(require_part: &str, line_number: usize) -> Option<ImportStatement> {
    let eq_pos = require_part.find('=')?;
    let rhs = require_part[eq_pos + 1..].trim();
    let module_part = rhs
        .strip_prefix("require(")
        .and_then(|s| s.strip_suffix(");"))?;

    Some(ImportStatement {
        module: normalize_module_literal(module_part),
        imports: None,
        import_type: "require".to_string(),
        line_number,
    })
}

/// Extract ES6/CommonJS imports from source code.
///
/// Shared implementation for JavaScript and TypeScript adapters.
/// The `strip_prefix` parameter specifies a prefix to strip from named imports
/// (e.g., "default as " for JS, "type " for TS).
pub fn extract_imports_common(
    source: &str,
    strip_prefix: &str,
) -> Vec<ImportStatement> {
    let mut imports = Vec::new();

    for (line_number, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        if let Some(stmt) = parse_es_import_line(trimmed, line_number + 1, strip_prefix) {
            imports.push(stmt);
        }
    }

    imports
}

/// Parse an ES6 or CommonJS import line.
///
/// Shared implementation for JavaScript and TypeScript adapters.
pub fn parse_es_import_line(
    trimmed: &str,
    line_number: usize,
    strip_prefix: &str,
) -> Option<ImportStatement> {
    if let Some(import_part) = trimmed.strip_prefix("import ") {
        return parse_es_import(import_part, line_number, strip_prefix);
    }

    if let Some(require_part) = trimmed.strip_prefix("const ") {
        return parse_require_import(require_part, line_number);
    }

    None
}

/// Parse an ES6 import statement.
///
/// Shared implementation for JavaScript and TypeScript adapters.
pub fn parse_es_import(
    import_part: &str,
    line_number: usize,
    strip_prefix: &str,
) -> Option<ImportStatement> {
    let from_pos = import_part.find(" from ")?;
    let import_spec = import_part[..from_pos].trim();
    let module_part = normalize_module_literal(&import_part[from_pos + 6..]);

    let (imports_list, import_type) = parse_import_spec(import_spec, strip_prefix);

    Some(ImportStatement {
        module: module_part,
        imports: imports_list,
        import_type,
        line_number,
    })
}

/// Parse the import specifier (what's being imported).
///
/// Shared implementation for JavaScript and TypeScript adapters.
/// The `strip_prefix` parameter specifies a prefix to strip from named imports
/// (e.g., "default as " for JS, "type " for TS).
pub fn parse_import_spec(spec: &str, strip_prefix: &str) -> (Option<Vec<String>>, String) {
    if spec.starts_with('*') {
        return (None, "star".to_string());
    }

    if spec.starts_with('{') {
        let cleaned = spec.trim_matches(|c| c == '{' || c == '}');
        let items = cleaned
            .split(',')
            .map(|s| s.trim().trim_start_matches(strip_prefix).to_string())
            .collect();
        return (Some(items), "named".to_string());
    }

    (Some(vec![spec.to_string()]), "default".to_string())
}

/// Extract function and constructor call targets from a JavaScript/TypeScript AST.
///
/// This is used by JavaScript and TypeScript adapters to extract function calls
/// from call_expression and new_expression nodes.
pub fn extract_js_function_calls(root: Node, source: &str) -> Vec<String> {
    let mut calls = Vec::new();

    walk_tree(root, &mut |node| {
        let callee = match node.kind() {
            "call_expression" => node.child_by_field_name("function"),
            "new_expression" => node.child_by_field_name("constructor"),
            _ => return,
        };

        if let Some(target) = callee.or_else(|| node.child(0)) {
            if let Ok(text) = node_text_normalized(&target, source) {
                let cleaned = text.trim();
                if !cleaned.is_empty() {
                    calls.push(cleaned.to_string());
                }
            }
        }
    });

    sort_and_dedup(&mut calls);
    calls
}

/// Extract identifier tokens from an AST tree, matching specified node kinds.
///
/// This is a parameterized helper used by language adapters to extract identifiers.
/// Each language can specify which AST node kinds represent identifiers in their grammar.
pub fn extract_identifiers_by_kinds(root: Node, source: &str, kinds: &[&str]) -> Vec<String> {
    let mut identifiers = Vec::new();

    walk_tree(root, &mut |node| {
        if kinds.contains(&node.kind()) {
            if let Ok(text) = node_text_normalized(&node, source) {
                let cleaned = text.trim();
                if !cleaned.is_empty() {
                    identifiers.push(cleaned.to_string());
                }
            }
        }
    });

    sort_and_dedup(&mut identifiers);
    identifiers
}

/// Extract text from a node, trying field name first, then falling back to child search.
///
/// This is a common utility used by Go and Python adapters for extracting names
/// from AST nodes.
pub fn extract_node_text(node: &Node, source_code: &str, field: &str, fallback_kinds: &[&str]) -> Result<Option<String>> {
    if let Some(name_node) = node.child_by_field_name(field) {
        return Ok(Some(name_node.utf8_text(source_code.as_bytes())?.to_string()));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if fallback_kinds.contains(&child.kind()) {
            return Ok(Some(child.utf8_text(source_code.as_bytes())?.to_string()));
        }
    }
    Ok(None)
}

/// Trait for language adapters that extract entities from AST nodes.
///
/// Provides default implementations for recursive AST traversal.
/// Implementors only need to define the language-specific `node_to_entity` method.
pub trait EntityExtractor {
    /// Convert a tree-sitter node to a ParsedEntity if it represents an entity.
    ///
    /// This is the language-specific method that each adapter must implement.
    /// Returns Ok(None) for nodes that don't represent entities.
    fn node_to_entity(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<String>,
        entity_id_counter: &mut usize,
    ) -> Result<Option<ParsedEntity>>;

    /// Recursively extract entities from the AST.
    ///
    /// Default implementation that handles the common traversal pattern.
    fn extract_entities_recursive(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<String>,
        index: &mut ParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
        if let Some(entity) = self.node_to_entity(
            node,
            source_code,
            file_path,
            parent_id.clone(),
            entity_id_counter,
        )? {
            let entity_id = entity.id.clone();
            index.add_entity(entity);
            self.traverse_children(node, source_code, file_path, Some(entity_id), index, entity_id_counter)?;
        } else {
            self.traverse_children(node, source_code, file_path, parent_id, index, entity_id_counter)?;
        }
        Ok(())
    }

    /// Traverse and process all child nodes recursively.
    ///
    /// Default implementation that iterates over children and calls extract_entities_recursive.
    fn traverse_children(
        &self,
        node: Node,
        source_code: &str,
        file_path: &str,
        parent_id: Option<String>,
        index: &mut ParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
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
        Ok(())
    }

    /// Extract entities from the AST using an iterative stack-based approach.
    ///
    /// This avoids stack overflow on deeply nested code by using an explicit stack
    /// instead of the call stack. Preferred over `extract_entities_recursive` for
    /// production use.
    fn extract_entities_iterative(
        &self,
        root: Node,
        source_code: &str,
        file_path: &str,
        index: &mut ParseIndex,
        entity_id_counter: &mut usize,
    ) -> Result<()> {
        // Stack entries: (node, parent_id)
        let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];

        while let Some((node, parent_id)) = stack.pop() {
            // Process this node
            let new_parent_id = if let Some(entity) = self.node_to_entity(
                node,
                source_code,
                file_path,
                parent_id.clone(),
                entity_id_counter,
            )? {
                let entity_id = entity.id.clone();
                index.add_entity(entity);
                Some(entity_id)
            } else {
                parent_id
            };

            // Push children in reverse order for depth-first traversal
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            for child in children.into_iter().rev() {
                stack.push((child, new_parent_id.clone()));
            }
        }

        Ok(())
    }
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
        fn parse_tree(&mut self, _source: &str) -> Result<Tree> {
            Err(crate::core::errors::ValknutError::parse(
                "dummy",
                "DummyAdapter does not support parse_tree",
            ))
        }

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
