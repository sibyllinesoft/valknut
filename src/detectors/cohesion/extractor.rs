//! Cohesion-specific entity and symbol extraction using tree-sitter.
//!
//! This module extracts the semantic information needed for cohesion analysis:
//! - Entity definitions (functions, classes, methods)
//! - Docstrings and comments
//! - Referenced symbols (calls, types, identifiers)

use std::collections::{HashMap, HashSet};
use std::path::Path;

use tree_sitter::{Node, Parser, Tree};

use crate::core::errors::{Result, ValknutError};
use crate::lang::registry::get_tree_sitter_language;

use super::symbols::{is_stop_token, tokenize_name, ExtractedSymbols};

/// Extracted entity with cohesion-relevant information.
#[derive(Debug, Clone)]
pub struct CohesionEntity {
    /// Entity name
    pub name: String,
    /// Entity kind (function, class, method, etc.)
    pub kind: String,
    /// Qualified name (parent::name format)
    pub qualified_name: String,
    /// Line range (start, end)
    pub line_range: (usize, usize),
    /// Docstring or leading comment (if any)
    pub docstring: Option<String>,
    /// Extracted symbols for embedding
    pub symbols: ExtractedSymbols,
}

/// Extract cohesion entities from a source file.
pub struct CohesionEntityExtractor {
    parser: Parser,
    language_key: String,
}

impl CohesionEntityExtractor {
    /// Create a new extractor for the given language.
    pub fn new(language_key: &str) -> Result<Self> {
        let mut parser = Parser::new();
        let language = get_tree_sitter_language(language_key)?;
        parser
            .set_language(&language)
            .map_err(|e| ValknutError::parse(language_key, format!("Failed to set language: {}", e)))?;

        Ok(Self {
            parser,
            language_key: language_key.to_string(),
        })
    }

    /// Extract all cohesion entities from source code.
    pub fn extract_entities(&mut self, source: &str, file_path: &Path) -> Result<Vec<CohesionEntity>> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| ValknutError::parse(&self.language_key, "Failed to parse source"))?;

        let mut entities = Vec::new();
        self.extract_recursive(
            tree.root_node(),
            source,
            file_path,
            None,
            &mut entities,
        );

        Ok(entities)
    }

    /// Extract module-level docstring (for file cohesion).
    pub fn extract_module_docstring(&mut self, source: &str) -> Option<String> {
        let tree = self.parser.parse(source, None)?;
        self.find_module_docstring(tree.root_node(), source)
    }

    fn extract_recursive(
        &self,
        node: Node,
        source: &str,
        file_path: &Path,
        parent_name: Option<&str>,
        entities: &mut Vec<CohesionEntity>,
    ) {
        let kind = node.kind();

        // Check if this is an entity we care about
        if let Some((entity_kind, name)) = self.classify_entity(node, source) {
            let qualified_name = match parent_name {
                Some(parent) => format!("{}::{}", parent, name),
                None => name.clone(),
            };

            let line_range = (node.start_position().row + 1, node.end_position().row + 1);
            let docstring = self.extract_docstring(node, source);
            let symbols = self.extract_entity_symbols(node, source, &name, &entity_kind);

            entities.push(CohesionEntity {
                name: name.clone(),
                kind: entity_kind,
                qualified_name: qualified_name.clone(),
                line_range,
                docstring,
                symbols,
            });

            // Recurse with this entity as parent
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.extract_recursive(child, source, file_path, Some(&qualified_name), entities);
            }
        } else {
            // Not an entity - continue recursion with same parent
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.extract_recursive(child, source, file_path, parent_name, entities);
            }
        }
    }

    /// Classify a node as an entity type, returning (kind, name) if it is one.
    fn classify_entity(&self, node: Node, source: &str) -> Option<(String, String)> {
        match self.language_key.as_str() {
            "python" => self.classify_python_entity(node, source),
            "javascript" | "typescript" => self.classify_js_entity(node, source),
            "rust" => self.classify_rust_entity(node, source),
            "go" => self.classify_go_entity(node, source),
            _ => None,
        }
    }

    fn classify_python_entity(&self, node: Node, source: &str) -> Option<(String, String)> {
        match node.kind() {
            "function_definition" => {
                let name = self.get_child_text(node, "name", source)?;
                let kind = if name.starts_with('_') && !name.starts_with("__") {
                    "private_function"
                } else if name.starts_with("__") && name.ends_with("__") {
                    "dunder_method"
                } else {
                    "function"
                };
                Some((kind.to_string(), name))
            }
            "class_definition" => {
                let name = self.get_child_text(node, "name", source)?;
                Some(("class".to_string(), name))
            }
            _ => None,
        }
    }

    fn classify_js_entity(&self, node: Node, source: &str) -> Option<(String, String)> {
        match node.kind() {
            "function_declaration" => {
                let name = self.get_child_text(node, "name", source)?;
                Some(("function".to_string(), name))
            }
            "class_declaration" => {
                let name = self.get_child_text(node, "name", source)?;
                Some(("class".to_string(), name))
            }
            "method_definition" => {
                let name = self.get_child_text(node, "name", source)?;
                Some(("method".to_string(), name))
            }
            "arrow_function" => {
                // Try to get name from parent variable declarator
                if let Some(parent) = node.parent() {
                    if parent.kind() == "variable_declarator" {
                        if let Some(name) = self.get_child_text(parent, "name", source) {
                            return Some(("function".to_string(), name));
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn classify_rust_entity(&self, node: Node, source: &str) -> Option<(String, String)> {
        match node.kind() {
            "function_item" => {
                let name = self.get_child_text(node, "name", source)?;
                Some(("function".to_string(), name))
            }
            "impl_item" => {
                // Get the type being implemented
                let type_node = node.child_by_field_name("type")?;
                let name = self.node_text(type_node, source);
                Some(("impl".to_string(), name))
            }
            "struct_item" => {
                let name = self.get_child_text(node, "name", source)?;
                Some(("struct".to_string(), name))
            }
            "enum_item" => {
                let name = self.get_child_text(node, "name", source)?;
                Some(("enum".to_string(), name))
            }
            "trait_item" => {
                let name = self.get_child_text(node, "name", source)?;
                Some(("trait".to_string(), name))
            }
            _ => None,
        }
    }

    fn classify_go_entity(&self, node: Node, source: &str) -> Option<(String, String)> {
        match node.kind() {
            "function_declaration" => {
                let name = self.get_child_text(node, "name", source)?;
                Some(("function".to_string(), name))
            }
            "method_declaration" => {
                let name = self.get_child_text(node, "name", source)?;
                Some(("method".to_string(), name))
            }
            "type_declaration" => {
                // Get the type spec
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_spec" {
                        if let Some(name) = self.get_child_text(child, "name", source) {
                            return Some(("type".to_string(), name));
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Extract docstring for an entity.
    fn extract_docstring(&self, node: Node, source: &str) -> Option<String> {
        match self.language_key.as_str() {
            "python" => self.extract_python_docstring(node, source),
            "javascript" | "typescript" => self.extract_js_docstring(node, source),
            "rust" => self.extract_rust_docstring(node, source),
            "go" => self.extract_go_docstring(node, source),
            _ => None,
        }
    }

    fn extract_python_docstring(&self, node: Node, source: &str) -> Option<String> {
        // Python docstrings are the first statement in a function/class body
        let body = node.child_by_field_name("body")?;
        let mut cursor = body.walk();

        for child in body.children(&mut cursor) {
            if child.kind() == "expression_statement" {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "string" {
                        let text = self.node_text(inner, source);
                        // Strip quotes
                        return Some(self.strip_string_quotes(&text));
                    }
                }
            }
            // Only check the first statement
            break;
        }
        None
    }

    fn extract_js_docstring(&self, node: Node, source: &str) -> Option<String> {
        // Look for JSDoc comment before the node
        if let Some(prev) = node.prev_sibling() {
            if prev.kind() == "comment" {
                let text = self.node_text(prev, source);
                if text.starts_with("/**") {
                    return Some(self.clean_jsdoc(&text));
                }
            }
        }
        None
    }

    fn extract_rust_docstring(&self, node: Node, source: &str) -> Option<String> {
        // Rust doc comments are siblings before the item
        let mut docs = Vec::new();
        let mut current = node.prev_sibling();

        while let Some(prev) = current {
            match prev.kind() {
                "line_comment" => {
                    let text = self.node_text(prev, source);
                    if text.starts_with("///") || text.starts_with("//!") {
                        docs.push(text[3..].trim().to_string());
                    } else {
                        break;
                    }
                }
                "block_comment" => {
                    let text = self.node_text(prev, source);
                    if text.starts_with("/**") || text.starts_with("/*!") {
                        docs.push(self.clean_block_comment(&text));
                    }
                    break;
                }
                _ => break,
            }
            current = prev.prev_sibling();
        }

        if docs.is_empty() {
            None
        } else {
            docs.reverse();
            Some(docs.join(" "))
        }
    }

    fn extract_go_docstring(&self, node: Node, source: &str) -> Option<String> {
        // Go doc comments are line comments before the declaration
        let mut docs = Vec::new();
        let mut current = node.prev_sibling();

        while let Some(prev) = current {
            if prev.kind() == "comment" {
                let text = self.node_text(prev, source);
                if text.starts_with("//") {
                    docs.push(text[2..].trim().to_string());
                }
            } else {
                break;
            }
            current = prev.prev_sibling();
        }

        if docs.is_empty() {
            None
        } else {
            docs.reverse();
            Some(docs.join(" "))
        }
    }

    fn find_module_docstring(&self, root: Node, source: &str) -> Option<String> {
        match self.language_key.as_str() {
            "python" => {
                // First string in module
                let mut cursor = root.walk();
                for child in root.children(&mut cursor) {
                    if child.kind() == "expression_statement" {
                        let mut inner_cursor = child.walk();
                        for inner in child.children(&mut inner_cursor) {
                            if inner.kind() == "string" {
                                return Some(self.strip_string_quotes(&self.node_text(inner, source)));
                            }
                        }
                    }
                    // Only check first statement
                    break;
                }
            }
            "rust" => {
                // //! comments at start
                let mut cursor = root.walk();
                let mut docs = Vec::new();
                for child in root.children(&mut cursor) {
                    if child.kind() == "line_comment" {
                        let text = self.node_text(child, source);
                        if text.starts_with("//!") {
                            docs.push(text[3..].trim().to_string());
                        } else {
                            break;
                        }
                    } else if child.kind() != "attribute_item" {
                        break;
                    }
                }
                if !docs.is_empty() {
                    return Some(docs.join(" "));
                }
            }
            _ => {}
        }
        None
    }

    /// Extract symbols from an entity for embedding.
    fn extract_entity_symbols(
        &self,
        node: Node,
        source: &str,
        name: &str,
        kind: &str,
    ) -> ExtractedSymbols {
        let name_tokens = tokenize_name(name);
        let signature_tokens = self.extract_signature_tokens(node, source);
        let referenced_symbols = self.extract_referenced_symbols(node, source);

        ExtractedSymbols {
            kind: kind.to_string(),
            qualified_name: name.to_string(),
            name_tokens,
            signature_tokens,
            referenced_symbols,
            doc_summary: None,
        }
    }

    /// Extract signature tokens (parameters, return type).
    fn extract_signature_tokens(&self, node: Node, source: &str) -> Vec<String> {
        let mut tokens = Vec::new();

        match self.language_key.as_str() {
            "python" => {
                if let Some(params) = node.child_by_field_name("parameters") {
                    self.collect_identifiers(params, source, &mut tokens);
                }
                if let Some(ret) = node.child_by_field_name("return_type") {
                    self.collect_identifiers(ret, source, &mut tokens);
                }
            }
            "rust" => {
                if let Some(params) = node.child_by_field_name("parameters") {
                    self.collect_identifiers(params, source, &mut tokens);
                }
                if let Some(ret) = node.child_by_field_name("return_type") {
                    self.collect_identifiers(ret, source, &mut tokens);
                }
            }
            "javascript" | "typescript" => {
                if let Some(params) = node.child_by_field_name("parameters") {
                    self.collect_identifiers(params, source, &mut tokens);
                }
                // TypeScript return type
                if let Some(ret) = node.child_by_field_name("return_type") {
                    self.collect_identifiers(ret, source, &mut tokens);
                }
            }
            "go" => {
                if let Some(params) = node.child_by_field_name("parameters") {
                    self.collect_identifiers(params, source, &mut tokens);
                }
                if let Some(result) = node.child_by_field_name("result") {
                    self.collect_identifiers(result, source, &mut tokens);
                }
            }
            _ => {}
        }

        // Filter and tokenize
        tokens
            .into_iter()
            .flat_map(|t| tokenize_name(&t))
            .filter(|t| !is_stop_token(t))
            .collect()
    }

    /// Extract referenced symbols (calls, types, identifiers).
    fn extract_referenced_symbols(&self, node: Node, source: &str) -> Vec<String> {
        let mut symbols = HashSet::new();
        self.collect_referenced_symbols_recursive(node, source, &mut symbols);

        symbols
            .into_iter()
            .flat_map(|s| tokenize_name(&s))
            .filter(|t| !is_stop_token(t))
            .collect()
    }

    fn collect_referenced_symbols_recursive(
        &self,
        node: Node,
        source: &str,
        symbols: &mut HashSet<String>,
    ) {
        let kind = node.kind();

        // Collect based on node type
        match kind {
            // Function/method calls
            "call" | "call_expression" => {
                if let Some(func) = node.child_by_field_name("function") {
                    let text = self.node_text(func, source);
                    // Handle method calls like obj.method
                    if let Some(method) = text.split('.').last() {
                        symbols.insert(method.to_string());
                    } else {
                        symbols.insert(text);
                    }
                }
            }
            // Type references
            "type_identifier" | "type" | "primitive_type" | "generic_type" => {
                let text = self.node_text(node, source);
                symbols.insert(text);
            }
            // Identifiers (but filter out definitions)
            "identifier" => {
                // Check if this is a reference, not a definition
                if let Some(parent) = node.parent() {
                    let parent_kind = parent.kind();
                    // Skip if it's a definition
                    if !matches!(
                        parent_kind,
                        "function_definition"
                            | "function_declaration"
                            | "function_item"
                            | "class_definition"
                            | "class_declaration"
                            | "struct_item"
                            | "enum_item"
                            | "parameter"
                            | "formal_parameters"
                    ) {
                        let text = self.node_text(node, source);
                        if text.len() > 1 {
                            // Skip single-char identifiers
                            symbols.insert(text);
                        }
                    }
                }
            }
            // Attribute access
            "attribute" | "member_expression" | "field_expression" => {
                if let Some(attr) = node.child_by_field_name("attribute")
                    .or_else(|| node.child_by_field_name("property"))
                    .or_else(|| node.child_by_field_name("field"))
                {
                    let text = self.node_text(attr, source);
                    symbols.insert(text);
                }
            }
            _ => {}
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_referenced_symbols_recursive(child, source, symbols);
        }
    }

    fn collect_identifiers(&self, node: Node, source: &str, tokens: &mut Vec<String>) {
        if node.kind() == "identifier" || node.kind() == "type_identifier" {
            tokens.push(self.node_text(node, source));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_identifiers(child, source, tokens);
        }
    }

    fn get_child_text(&self, node: Node, field: &str, source: &str) -> Option<String> {
        node.child_by_field_name(field)
            .map(|n| self.node_text(n, source))
    }

    fn node_text(&self, node: Node, source: &str) -> String {
        source[node.byte_range()].to_string()
    }

    fn strip_string_quotes(&self, s: &str) -> String {
        let s = s.trim();
        if s.starts_with("\"\"\"") || s.starts_with("'''") {
            s.trim_start_matches("\"\"\"")
                .trim_start_matches("'''")
                .trim_end_matches("\"\"\"")
                .trim_end_matches("'''")
                .trim()
                .to_string()
        } else if s.starts_with('"') || s.starts_with('\'') {
            s.trim_matches(|c| c == '"' || c == '\'').to_string()
        } else {
            s.to_string()
        }
    }

    fn clean_jsdoc(&self, s: &str) -> String {
        s.trim_start_matches("/**")
            .trim_end_matches("*/")
            .lines()
            .map(|line| line.trim().trim_start_matches('*').trim())
            .filter(|line| !line.starts_with('@'))
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }

    fn clean_block_comment(&self, s: &str) -> String {
        s.trim_start_matches("/**")
            .trim_start_matches("/*!")
            .trim_end_matches("*/")
            .lines()
            .map(|line| line.trim().trim_start_matches('*').trim())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_python_function() {
        let mut extractor = CohesionEntityExtractor::new("python").unwrap();
        let source = r#"
def calculate_total(items, tax_rate):
    """Calculate the total price with tax."""
    subtotal = sum(item.price for item in items)
    return subtotal * (1 + tax_rate)
"#;
        let entities = extractor.extract_entities(source, Path::new("test.py")).unwrap();

        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "calculate_total");
        assert_eq!(entities[0].kind, "function");
        assert!(entities[0].docstring.as_ref().unwrap().contains("Calculate the total"));
        assert!(!entities[0].symbols.name_tokens.is_empty());
    }

    #[test]
    fn extract_python_class() {
        let mut extractor = CohesionEntityExtractor::new("python").unwrap();
        let source = r#"
class UserManager:
    """Manages user operations."""

    def create_user(self, name):
        return User(name)
"#;
        let entities = extractor.extract_entities(source, Path::new("test.py")).unwrap();

        assert!(entities.iter().any(|e| e.name == "UserManager"));
        assert!(entities.iter().any(|e| e.name == "create_user"));
    }

    #[test]
    fn extract_rust_function() {
        let mut extractor = CohesionEntityExtractor::new("rust").unwrap();
        let source = r#"
/// Calculates the sum of values.
fn calculate_sum(values: &[i32]) -> i32 {
    values.iter().sum()
}
"#;
        let entities = extractor.extract_entities(source, Path::new("test.rs")).unwrap();

        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "calculate_sum");
        assert!(entities[0].docstring.as_ref().unwrap().contains("sum"));
    }

    #[test]
    fn tokenize_referenced_symbols() {
        let mut extractor = CohesionEntityExtractor::new("python").unwrap();
        let source = r#"
def process_data(data):
    result = DataProcessor.transform(data)
    validator.validate(result)
    return result
"#;
        let entities = extractor.extract_entities(source, Path::new("test.py")).unwrap();

        assert_eq!(entities.len(), 1);
        let symbols = &entities[0].symbols.referenced_symbols;
        // Should contain tokenized versions of DataProcessor, transform, validator, validate
        assert!(symbols.iter().any(|s| s.contains("data") || s.contains("processor")));
    }
}
