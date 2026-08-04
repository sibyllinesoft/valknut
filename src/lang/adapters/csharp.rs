//! C# language adapter backed by tree-sitter.

use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Tree};

use super::super::common::{
    create_base_metadata, extract_identifiers_by_kinds, generate_entity_id, sort_and_dedup,
    EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation,
};
use super::super::registry::{create_parser_for_language, get_tree_sitter_language};
use crate::core::ast_utils::{node_text_normalized, walk_tree};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::CodeEntity;
use crate::detectors::structure::config::ImportStatement;

/// C#-specific parsing and semantic extraction.
pub struct CSharpAdapter {
    parser: Parser,
    #[allow(dead_code)]
    language: Language,
}

impl CSharpAdapter {
    /// Create a C# adapter.
    pub fn new() -> Result<Self> {
        Ok(Self {
            parser: create_parser_for_language("cs")?,
            language: get_tree_sitter_language("cs")?,
        })
    }

    fn entity_kind(node_kind: &str) -> Option<EntityKind> {
        match node_kind {
            "namespace_declaration" | "file_scoped_namespace_declaration" => {
                Some(EntityKind::Module)
            }
            "class_declaration" | "record_declaration" => Some(EntityKind::Class),
            "struct_declaration" => Some(EntityKind::Struct),
            "interface_declaration" => Some(EntityKind::Interface),
            "enum_declaration" => Some(EntityKind::Enum),
            "delegate_declaration" => Some(EntityKind::Interface),
            "method_declaration"
            | "constructor_declaration"
            | "destructor_declaration"
            | "operator_declaration"
            | "conversion_operator_declaration"
            | "local_function_statement"
            | "accessor_declaration" => Some(EntityKind::Method),
            "lambda_expression" | "anonymous_method_expression" => Some(EntityKind::Function),
            "property_declaration" | "indexer_declaration" | "event_declaration" => {
                Some(EntityKind::Variable)
            }
            "variable_declarator" => Some(EntityKind::Variable),
            _ => None,
        }
    }

    fn entity_name(node: Node, source: &str, kind: EntityKind, counter: usize) -> String {
        for field in ["name", "operator"] {
            if let Some(name) = node.child_by_field_name(field) {
                if let Ok(text) = node_text_normalized(&name, source) {
                    if !text.trim().is_empty() {
                        return text.trim().to_string();
                    }
                }
            }
        }
        if node.kind() == "accessor_declaration" {
            let text = node_text_normalized(&node, source).unwrap_or_default();
            return text
                .split_whitespace()
                .find(|token| matches!(*token, "get" | "set" | "init" | "add" | "remove"))
                .unwrap_or("accessor")
                .to_string();
        }
        if node.kind() == "indexer_declaration" {
            return "this[]".to_string();
        }
        // Constructors and destructors expose their name as an identifier child in
        // some grammar versions rather than through a named field.
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if matches!(child.kind(), "identifier" | "name") {
                if let Ok(text) = node_text_normalized(&child, source) {
                    if !text.trim().is_empty() {
                        return text.trim().to_string();
                    }
                }
            }
        }
        kind.fallback_name(counter)
    }

    fn field_declaration_for(node: Node) -> Option<Node> {
        let mut parent = node.parent();
        while let Some(candidate) = parent {
            if matches!(
                candidate.kind(),
                "field_declaration" | "event_field_declaration"
            ) {
                return Some(candidate);
            }
            if candidate.kind() != "variable_declaration" {
                return None;
            }
            parent = candidate.parent();
        }
        None
    }

    fn child_text(node: Node, field: &str, source: &str) -> Option<String> {
        node.child_by_field_name(field)
            .and_then(|child| node_text_normalized(&child, source).ok())
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
    }

    fn direct_children_text(node: Node, kind: &str, source: &str) -> Vec<String> {
        let mut values = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == kind {
                if let Ok(text) = node_text_normalized(&child, source) {
                    values.push(text.trim().to_string());
                }
            }
        }
        values
    }

    fn parameter_names(node: Node, source: &str) -> Vec<String> {
        let Some(parameters) = node.child_by_field_name("parameters").or_else(|| {
            let mut cursor = node.walk();
            let found = node
                .children(&mut cursor)
                .find(|child| child.kind() == "parameter_list");
            found
        }) else {
            return Vec::new();
        };
        let mut names = Vec::new();
        walk_tree(parameters, &mut |candidate| {
            if candidate.kind() == "parameter" {
                if let Some(name) = candidate.child_by_field_name("name") {
                    if let Ok(text) = node_text_normalized(&name, source) {
                        names.push(text.trim().to_string());
                    }
                }
            }
        });
        names
    }

    fn add_semantic_metadata(
        node: Node,
        source: &str,
        metadata: &mut HashMap<String, serde_json::Value>,
    ) {
        let modifiers = Self::direct_children_text(node, "modifier", source);
        let visibility = modifiers
            .iter()
            .find(|value| {
                matches!(
                    value.as_str(),
                    "public" | "private" | "protected" | "internal"
                )
            })
            .cloned()
            .unwrap_or_else(|| "private".to_string());
        metadata.insert("visibility".to_string(), serde_json::json!(visibility));
        metadata.insert("modifiers".to_string(), serde_json::json!(modifiers));
        for modifier in [
            "async", "static", "abstract", "unsafe", "sealed", "readonly", "partial", "virtual",
            "override",
        ] {
            metadata.insert(
                format!("is_{modifier}"),
                serde_json::json!(modifiers.iter().any(|value| value == modifier)),
            );
        }

        let parameters = Self::parameter_names(node, source);
        if !parameters.is_empty()
            || matches!(
                node.kind(),
                "method_declaration" | "constructor_declaration" | "local_function_statement"
            )
        {
            metadata.insert("parameters".to_string(), serde_json::json!(parameters));
        }
        if let Some(return_type) = Self::child_text(node, "returns", source)
            .or_else(|| Self::child_text(node, "type", source))
        {
            metadata.insert("return_type".to_string(), serde_json::json!(return_type));
        }
        if let Some(type_parameters) = Self::child_text(node, "type_parameters", source) {
            metadata.insert(
                "generic_parameters".to_string(),
                serde_json::json!(type_parameters),
            );
        }

        let bases = Self::direct_children_text(node, "base_list", source);
        if let Some(base_list) = bases.first() {
            let values: Vec<_> = base_list
                .trim_start_matches(':')
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect();
            metadata.insert("base_types".to_string(), serde_json::json!(values));
        }

        if node.kind() == "enum_declaration" {
            let mut members = Vec::new();
            walk_tree(node, &mut |candidate| {
                if candidate.kind() == "enum_member_declaration" {
                    if let Some(name) = candidate.child_by_field_name("name") {
                        if let Ok(text) = node_text_normalized(&name, source) {
                            members.push(text.trim().to_string());
                        }
                    }
                }
            });
            metadata.insert("members".to_string(), serde_json::json!(members));
        }
    }

    fn calls_in_node(node: Node, source: &str) -> Vec<String> {
        let mut calls = Vec::new();
        walk_tree(node, &mut |candidate| {
            let target = match candidate.kind() {
                "invocation_expression" => candidate
                    .child_by_field_name("function")
                    .or_else(|| candidate.child(0)),
                "object_creation_expression" | "implicit_object_creation_expression" => {
                    candidate.child_by_field_name("type")
                }
                _ => None,
            };
            if let Some(target) = target {
                if let Ok(text) = node_text_normalized(&target, source) {
                    if !text.trim().is_empty() {
                        calls.push(text.trim().to_string());
                    }
                }
            }
        });
        sort_and_dedup(&mut calls);
        calls
    }

    fn parse_index(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        let tree = self.parse_tree(source)?;
        let mut index = ParseIndex::new();
        let mut counter = 0;
        let mut stack = vec![(tree.root_node(), None::<String>)];

        while let Some((node, parent_id)) = stack.pop() {
            let mut child_parent = parent_id.clone();
            if let Some(mut kind) = Self::entity_kind(node.kind()).filter(|_| {
                node.kind() != "variable_declarator" || Self::field_declaration_for(node).is_some()
            }) {
                let semantic_node = Self::field_declaration_for(node).unwrap_or(node);
                if node.kind() == "variable_declarator"
                    && Self::direct_children_text(semantic_node, "modifier", source)
                        .iter()
                        .any(|modifier| modifier == "const")
                {
                    kind = EntityKind::Constant;
                }
                counter += 1;
                let id = generate_entity_id(file_path, kind, counter);
                let name = Self::entity_name(node, source, kind, counter);
                let mut metadata =
                    create_base_metadata(node.kind(), node.start_byte(), node.end_byte());
                metadata.insert(
                    "function_calls".to_string(),
                    serde_json::json!(Self::calls_in_node(node, source)),
                );
                metadata.insert(
                    "identifiers".to_string(),
                    serde_json::json!(extract_identifiers_by_kinds(
                        node,
                        source,
                        &["identifier", "type_parameter", "qualified_name"]
                    )),
                );
                Self::add_semantic_metadata(semantic_node, source, &mut metadata);
                let entity = ParsedEntity {
                    id: id.clone(),
                    kind,
                    name,
                    parent: parent_id.clone(),
                    children: Vec::new(),
                    location: SourceLocation::from_positions(
                        file_path,
                        node.start_position().row,
                        node.start_position().column,
                        node.end_position().row,
                        node.end_position().column,
                    ),
                    metadata,
                };
                index.add_entity(entity);
                child_parent = Some(id);
            }

            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            for child in children.into_iter().rev() {
                stack.push((child, child_parent.clone()));
            }
        }
        Self::link_file_scoped_namespace(&mut index);
        Self::refresh_owner_paths(&mut index);
        Ok(index)
    }

    fn link_file_scoped_namespace(index: &mut ParseIndex) {
        let Some(namespace_id) = index
            .entities
            .values()
            .find(|entity| {
                entity
                    .metadata
                    .get("node_kind")
                    .and_then(|value| value.as_str())
                    == Some("file_scoped_namespace_declaration")
            })
            .map(|entity| entity.id.clone())
        else {
            return;
        };
        let children: Vec<_> = index
            .entities
            .values()
            .filter(|entity| entity.id != namespace_id && entity.parent.is_none())
            .map(|entity| entity.id.clone())
            .collect();
        for child_id in children {
            if let Some(child) = index.entities.get_mut(&child_id) {
                child.parent = Some(namespace_id.clone());
            }
            if let Some(namespace) = index.entities.get_mut(&namespace_id) {
                if !namespace.children.contains(&child_id) {
                    namespace.children.push(child_id);
                }
            }
        }
    }

    fn refresh_owner_paths(index: &mut ParseIndex) {
        let paths: Vec<_> = index
            .entities
            .values()
            .filter_map(|entity| {
                let mut names = Vec::new();
                let mut parent_id = entity.parent.as_ref();
                while let Some(id) = parent_id {
                    let parent = index.entities.get(id)?;
                    names.push(parent.name.clone());
                    parent_id = parent.parent.as_ref();
                }
                names.reverse();
                let parent = entity
                    .parent
                    .as_ref()
                    .and_then(|id| index.entities.get(id))
                    .map(|parent| (parent.name.clone(), format!("{:?}", parent.kind)));
                (!names.is_empty()).then(|| (entity.id.clone(), names.join("."), parent))
            })
            .collect();
        for (id, path, parent) in paths {
            if let Some(entity) = index.entities.get_mut(&id) {
                entity
                    .metadata
                    .insert("owner_path".to_string(), serde_json::json!(path));
                if let Some((parent_name, parent_kind)) = parent {
                    entity
                        .metadata
                        .insert("parent_name".to_string(), serde_json::json!(parent_name));
                    entity
                        .metadata
                        .insert("parent_kind".to_string(), serde_json::json!(parent_kind));
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl LanguageAdapter for CSharpAdapter {
    fn parse_tree(&mut self, source: &str) -> Result<Tree> {
        self.parser
            .parse(source, None)
            .ok_or_else(|| ValknutError::parse("cs", "Failed to parse C# source"))
    }

    fn parse_source(&mut self, source: &str, file_path: &str) -> Result<ParseIndex> {
        self.parse_index(source, file_path)
    }

    fn extract_function_calls(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        Ok(Self::calls_in_node(tree.root_node(), source))
    }

    fn extract_identifiers(&mut self, source: &str) -> Result<Vec<String>> {
        let tree = self.parse_tree(source)?;
        Ok(extract_identifiers_by_kinds(
            tree.root_node(),
            source,
            &["identifier", "type_parameter", "qualified_name"],
        ))
    }

    fn count_distinct_blocks(&mut self, source: &str) -> Result<usize> {
        Ok(self
            .parse_index(source, "<memory>")?
            .count_distinct_blocks())
    }

    fn language_name(&self) -> &str {
        "csharp"
    }

    fn extract_imports(&mut self, source: &str) -> Result<Vec<ImportStatement>> {
        let tree = self.parse_tree(source)?;
        let mut imports = Vec::new();
        walk_tree(tree.root_node(), &mut |node| {
            if node.kind() != "using_directive" {
                return;
            }
            let Ok(raw) = node_text_normalized(&node, source) else {
                return;
            };
            let body = raw
                .trim()
                .strip_prefix("global ")
                .unwrap_or(raw.trim())
                .strip_prefix("using ")
                .unwrap_or(raw.trim())
                .trim_end_matches(';')
                .trim();
            let (import_type, module) = if let Some(rest) = body.strip_prefix("static ") {
                ("using_static", rest.trim())
            } else if let Some((_, target)) = body.split_once('=') {
                ("using_alias", target.trim())
            } else {
                ("using", body)
            };
            if !module.is_empty() {
                imports.push(ImportStatement {
                    module: module.to_string(),
                    imports: None,
                    import_type: import_type.to_string(),
                    line_number: node.start_position().row + 1,
                });
            }
        });
        Ok(imports)
    }

    fn extract_code_entities(&mut self, source: &str, file_path: &str) -> Result<Vec<CodeEntity>> {
        Ok(self
            .parse_index(source, file_path)?
            .entities
            .into_values()
            .map(|entity| entity.to_code_entity(source))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
global using System;
using Collections = System.Collections.Generic;
namespace Demo;

public interface IGreeter { string Greet(string name); }
public record Person(string Name);
public class Greeter : IGreeter {
    public Greeter() { Console.WriteLine("ready"); }
    public string Greet(string name) {
        var person = new Person(name);
        return person.Name.ToUpperInvariant();
    }
}
"#;

    #[test]
    fn extracts_csharp_entities_calls_and_imports() {
        let mut adapter = CSharpAdapter::new().unwrap();
        let index = adapter.parse_source(SAMPLE, "Demo.cs").unwrap();
        let names: Vec<_> = index
            .entities
            .values()
            .map(|entity| entity.name.as_str())
            .collect();
        assert!(names.contains(&"Demo"));
        assert!(names.contains(&"IGreeter"));
        assert!(names.contains(&"Person"));
        assert!(names.contains(&"Greeter"));
        assert!(names.contains(&"Greet"));

        let calls = adapter.extract_function_calls(SAMPLE).unwrap();
        assert!(calls.iter().any(|call| call == "Console.WriteLine"));
        assert!(calls
            .iter()
            .any(|call| call == "person.Name.ToUpperInvariant"));
        assert!(calls.iter().any(|call| call == "Person"));

        let imports = adapter.extract_imports(SAMPLE).unwrap();
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].module, "System");
        assert_eq!(imports[1].module, "System.Collections.Generic");
        assert_eq!(imports[1].import_type, "using_alias");
    }

    #[test]
    fn parses_modern_csharp_without_errors() {
        let mut adapter = CSharpAdapter::new().unwrap();
        let tree = adapter.parse_tree(SAMPLE).unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn extracts_parity_metadata_and_modern_callable_forms() {
        let source = r#"
using static System.Math;
namespace Demo.Core {
    public enum Mode { Fast, Safe }
    public struct Point { public int X { get; init; } }
    public delegate string Formatter<T>(T value);
    public abstract class Base<T> { public abstract T Convert(string input); }
    public sealed class Service : Base<int> {
        public const int Limit = 10;
        public override int Convert(string input) {
            int ParseLocal(string value) => int.Parse(value);
            Func<int, int> clamp = value => value > Limit ? Limit : value;
            return clamp(ParseLocal(input));
        }
    }
}
"#;
        let mut adapter = CSharpAdapter::new().unwrap();
        let index = adapter.parse_source(source, "Parity.cs").unwrap();

        let service = index
            .entities
            .values()
            .find(|entity| entity.name == "Service")
            .unwrap();
        assert_eq!(service.metadata["visibility"], "public");
        assert_eq!(
            service.metadata["base_types"],
            serde_json::json!(["Base<int>"])
        );

        let convert = index
            .entities
            .values()
            .find(|entity| {
                entity.name == "Convert" && entity.parent.as_deref() == Some(service.id.as_str())
            })
            .unwrap();
        assert_eq!(convert.metadata["parameters"], serde_json::json!(["input"]));
        assert_eq!(convert.metadata["return_type"], "int");
        assert!(convert.metadata["modifiers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|modifier| modifier == "override"));

        let mode = index
            .entities
            .values()
            .find(|entity| entity.name == "Mode")
            .unwrap();
        assert_eq!(
            mode.metadata["members"],
            serde_json::json!(["Fast", "Safe"])
        );
        assert!(index
            .entities
            .values()
            .any(|entity| { entity.name == "Limit" && entity.kind == EntityKind::Constant }));
        assert!(index
            .entities
            .values()
            .any(|entity| entity.name == "ParseLocal"));
        assert!(index
            .entities
            .values()
            .any(|entity| entity.kind == EntityKind::Function));

        let normalized = adapter.normalize_source(source).unwrap();
        assert!(normalized.contains("class_declaration"));
        assert!(adapter.count_ast_nodes(source).unwrap() > 20);
        assert!(adapter.count_distinct_blocks(source).unwrap() >= 8);
    }
}
