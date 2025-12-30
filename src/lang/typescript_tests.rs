use super::*;
use serde_json::Value;

fn find_node_by_kind<'a>(
    node: tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_node_by_kind(child, kind) {
            return Some(found);
        }
    }
    None
}

#[test]
fn test_typescript_adapter_creation() {
    let adapter = TypeScriptAdapter::new();
    assert!(
        adapter.is_ok(),
        "Should create TypeScript adapter successfully"
    );
}

#[test]
fn test_parse_simple_function() {
    let mut adapter = TypeScriptAdapter::new().unwrap();
    let source = r#"
function greet(name: string): string {
    return `Hello, ${name}!`;
}
"#;
    let result = adapter.parse_source(source, "test.ts");
    assert!(result.is_ok(), "Should parse simple function");

    let index = result.unwrap();
    assert!(
        index.get_entities_in_file("test.ts").len() >= 1,
        "Should find at least one entity"
    );
}

#[test]
fn test_parse_interface_and_class() {
    let mut adapter = TypeScriptAdapter::new().unwrap();
    let source = r#"
interface User {
    name: string;
    age: number;
}

class UserService {
    private users: User[] = [];

    addUser(user: User): void {
        this.users.push(user);
    }

    getUser(name: string): User | undefined {
        return this.users.find(u => u.name === name);
    }
}
"#;
    let result = adapter.parse_source(source, "test.ts");
    assert!(result.is_ok(), "Should parse interface and class");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("test.ts");
    assert!(
        entities.len() >= 2,
        "Should find at least interface and class entities"
    );

    let has_interface = entities
        .iter()
        .any(|e| matches!(e.kind, EntityKind::Interface));
    let has_class = entities.iter().any(|e| matches!(e.kind, EntityKind::Class));
    assert!(
        has_interface || has_class,
        "Should find interface or class entity"
    );
}

#[test]
fn test_parse_generic_types() {
    let mut adapter = TypeScriptAdapter::new().unwrap();
    let source = r#"
interface Repository<T> {
    findById(id: number): Promise<T | null>;
    save(entity: T): Promise<T>;
}

class InMemoryRepository<T extends { id: number }> implements Repository<T> {
    private items: T[] = [];

    async findById(id: number): Promise<T | null> {
        return this.items.find(item => item.id === id) || null;
    }

    async save(entity: T): Promise<T> {
        this.items.push(entity);
        return entity;
    }
}
"#;
    let result = adapter.parse_source(source, "generics.ts");
    assert!(result.is_ok(), "Should parse generic TypeScript code");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("generics.ts");
    assert!(entities.len() >= 2, "Should find multiple entities");
}

#[test]
fn test_parse_modules_and_exports() {
    let mut adapter = TypeScriptAdapter::new().unwrap();
    let source = r#"
export interface Config {
    apiUrl: string;
    timeout: number;
}

export class HttpClient {
    constructor(private config: Config) {}

    async get<T>(url: string): Promise<T> {
        // Implementation would go here
        throw new Error("Not implemented");
    }
}

export default function createClient(config: Config): HttpClient {
    return new HttpClient(config);
}
"#;
    let result = adapter.parse_source(source, "http.ts");
    assert!(result.is_ok(), "Should parse modules and exports");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("http.ts");
    assert!(
        entities.len() >= 2,
        "Should find multiple exported entities"
    );
}

#[test]
fn test_empty_typescript_file() {
    let mut adapter = TypeScriptAdapter::new().unwrap();
    let source = "// TypeScript file with just comments\n/* Block comment */";
    let result = adapter.parse_source(source, "empty.ts");
    assert!(result.is_ok(), "Should handle empty TypeScript file");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("empty.ts");
    assert_eq!(
        entities.len(),
        0,
        "Should find no entities in comment-only file"
    );
}

#[test]
fn test_extract_typescript_metadata_and_entities() {
    let mut adapter = TypeScriptAdapter::new().expect("adapter");
    let source = r#"
import type { Config } from "./types";

abstract class DataService extends BaseService implements Repository<User>, Auditable {
    constructor(private readonly repo: Repository<User>) {}

    async fetchAll(limit: number): Promise<User[]> {
        return this.repo.findAll(limit);
    }
}

interface Repository<T> extends Disposable {
    findAll(limit: number): T[];
}

const enum Flags {
    None,
    All = 1
}

type Identifier = string | number;

export default () => ({ status: Flags.None });
"#;

    let code_entities = adapter
        .extract_code_entities(source, "service.ts")
        .expect("code entities");

    let tree = adapter.parse_tree(source).expect("parse tree");
    let class_node = find_node_by_kind(tree.root_node(), "class_declaration")
        .or_else(|| find_node_by_kind(tree.root_node(), "abstract_class_declaration"))
        .expect("class node");
    let mut class_metadata = HashMap::new();
    adapter
        .extract_class_metadata(&class_node, source, &mut class_metadata)
        .expect("class metadata");
    assert_eq!(
        class_metadata.get("extends"),
        Some(&Value::String("BaseService".to_string()))
    );
    let implement_values: Vec<_> = class_metadata
        .get("implements")
        .and_then(Value::as_array)
        .expect("implements metadata")
        .iter()
        .filter_map(|value| value.as_str())
        .collect();
    assert!(
        implement_values
            .iter()
            .any(|value| value.contains("Auditable")),
        "expected Auditable in implements: {implement_values:?}"
    );
    assert_eq!(class_metadata.get("is_abstract"), Some(&Value::Bool(true)));

    let method_entity = code_entities
        .iter()
        .find(|entity| entity.name == "fetchAll")
        .expect("method entity missing");
    assert_eq!(method_entity.entity_type, "Method");
    assert_eq!(
        method_entity.properties.get("is_async"),
        Some(&Value::Bool(true))
    );
    let return_type = method_entity
        .properties
        .get("return_type")
        .and_then(Value::as_str)
        .expect("return type metadata");
    assert!(
        return_type.contains("Promise"),
        "expected Promise return type, got {return_type}"
    );
    let parameters = method_entity
        .properties
        .get("parameters")
        .and_then(Value::as_array)
        .expect("method parameters metadata");
    assert!(
        parameters
            .iter()
            .any(|value| value.as_str() == Some("limit"))
            || parameters.is_empty()
    );

    let interface_entity = code_entities
        .iter()
        .find(|entity| entity.name == "Repository")
        .expect("interface entity missing");
    assert_eq!(interface_entity.entity_type, "Interface");

    let enum_entity = code_entities
        .iter()
        .find(|entity| entity.name == "Flags")
        .expect("enum entity missing");
    assert_eq!(
        enum_entity.properties.get("is_const"),
        Some(&Value::Bool(true))
    );
    let members = enum_entity
        .properties
        .get("members")
        .and_then(Value::as_array)
        .expect("enum members metadata");
    assert!(members.iter().any(|value| value.as_str() == Some("None")));

    assert!(
        code_entities
            .iter()
            .any(|entity| entity.entity_type == "Interface" && entity.name == "Identifier"),
        "expected Identifier type alias"
    );
    assert!(
        code_entities
            .iter()
            .any(|entity| entity.name == "<anonymous>"),
        "expected anonymous default export entity"
    );
}

#[test]
fn test_extract_typescript_import_variants() {
    let mut adapter = TypeScriptAdapter::new().expect("adapter");
    let source = r#"
import defaultExport from "./core";
import { type Foo, Bar } from "./core";
import type { Baz } from "@types/baz";
const utils = require("../utils");
"#;

    let imports = adapter.extract_imports(source).expect("imports");
    let modules: Vec<_> = imports.iter().map(|imp| imp.module.as_str()).collect();
    assert!(
        modules.contains(&"./core")
            && modules.contains(&"@types/baz")
            && modules.contains(&"../utils"),
        "expected normalized modules in {modules:?}"
    );

    let named = imports
        .iter()
        .find(|imp| imp.import_type == "named")
        .expect("named import missing");
    let names = named
        .imports
        .as_ref()
        .expect("expected names in named import");
    assert!(names.iter().any(|name| name.trim() == "Foo"));
    assert!(names.iter().any(|name| name.trim() == "Bar"));

    assert!(
        imports.iter().any(|imp| imp.import_type == "default"),
        "expected default import variant"
    );
    assert!(
        imports.iter().any(|imp| imp.import_type == "require"),
        "expected require variant"
    );
}

#[test]
fn test_typescript_identifiers_calls_and_normalization() {
    let mut adapter = TypeScriptAdapter::new().expect("adapter");
    let source = r#"
async function outer<T>(items: T[]): Promise<T[]> {
    const result = items.map(item => transform(item));
    return await Promise.resolve(result);
}

function transform<T>(item: T): T {
    return item;
}

outer([1, 2, 3]);
"#;

    let calls = adapter
        .extract_function_calls(source)
        .expect("function calls");
    assert!(calls.iter().any(|call| call.contains("outer")));
    assert!(calls.iter().any(|call| call.contains("Promise.resolve")));

    let identifiers = adapter.extract_identifiers(source).expect("identifiers");
    assert!(identifiers.contains(&"outer".to_string()));
    assert!(identifiers.contains(&"transform".to_string()));

    let normalized = adapter.normalize_source(source).expect("normalize");
    assert!(
        normalized.starts_with("(program"),
        "expected normalized S-expression"
    );

    let patterns = adapter
        .contains_boilerplate_patterns(
            source,
            &[
                "Promise.resolve".to_string(),
                "nonexistent-pattern".to_string(),
            ],
        )
        .expect("patterns");
    assert_eq!(patterns, vec!["Promise.resolve".to_string()]);

    let ast_nodes = adapter.count_ast_nodes(source).expect("ast nodes");
    let distinct_blocks = adapter.count_distinct_blocks(source).expect("block count");
    assert!(ast_nodes > 0);
    assert!(distinct_blocks > 0);
}

#[test]
fn test_detects_enums_and_type_aliases() {
    let mut adapter = TypeScriptAdapter::new().expect("adapter");
    let source = r#"
export const enum Flags { None, All = 1 }
type Result<T> = Promise<T>;
let counter: number = 0;
"#;

    let entities = adapter
        .extract_code_entities(source, "types.ts")
        .expect("entities extracted");

    let enum_entity = entities
        .iter()
        .find(|entity| entity.name == "Flags")
        .expect("missing Flags enum");
    assert_eq!(enum_entity.entity_type, "Enum");

    let alias_entity = entities
        .iter()
        .find(|entity| entity.name == "Result")
        .expect("missing Result alias");
    assert_eq!(alias_entity.entity_type, "Interface");

    let variable_entity = entities
        .iter()
        .find(|entity| entity.name == "counter")
        .expect("missing counter variable");
    assert_eq!(variable_entity.entity_type, "Variable");
}
