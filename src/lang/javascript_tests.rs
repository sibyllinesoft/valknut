use super::*;
use serde_json::Value;

#[test]
fn test_javascript_adapter_creation() {
    let adapter = JavaScriptAdapter::new();
    assert!(
        adapter.is_ok(),
        "Should create JavaScript adapter successfully"
    );
}

#[test]
fn test_parse_simple_function() {
    let mut adapter = JavaScriptAdapter::new().unwrap();
    let source = r#"
function hello() {
    return "Hello, World!";
}
"#;
    let result = adapter.parse_source(source, "test.js");
    assert!(result.is_ok(), "Should parse simple function");

    let index = result.unwrap();
    assert!(
        index.get_entities_in_file("test.js").len() >= 1,
        "Should find at least one entity"
    );
}

#[test]
fn test_parse_simple_class() {
    let mut adapter = JavaScriptAdapter::new().unwrap();
    let source = r#"
class MyClass {
    constructor() {
        this.value = 0;
    }

    getValue() {
        return this.value;
    }
}
"#;
    let result = adapter.parse_source(source, "test.js");
    assert!(result.is_ok(), "Should parse simple class");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("test.js");
    assert!(entities.len() >= 1, "Should find at least one entity");

    let has_class = entities.iter().any(|e| matches!(e.kind, EntityKind::Class));
    assert!(has_class, "Should find a class entity");
}

#[test]
fn test_parse_arrow_functions() {
    let mut adapter = JavaScriptAdapter::new().unwrap();
    let source = r#"
const add = (a, b) => a + b;
const multiply = (x, y) => {
    return x * y;
};
"#;
    let result = adapter.parse_source(source, "arrow.js");
    assert!(result.is_ok(), "Should parse arrow functions");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("arrow.js");
    // Arrow functions might be detected as variables or functions depending on implementation
    // entities.len() is unsigned, always >= 0 - should handle arrow functions gracefully
}

#[test]
fn test_parse_complex_javascript() {
    let mut adapter = JavaScriptAdapter::new().unwrap();
    let source = r#"
import { fetch } from 'node-fetch';

class APIClient {
    constructor(baseURL) {
        this.baseURL = baseURL;
    }

    async get(endpoint) {
        const response = await fetch(`${this.baseURL}/${endpoint}`);
        return response.json();
    }
}

function createClient(url) {
    return new APIClient(url);
}

const defaultClient = createClient('https://api.example.com');
"#;
    let result = adapter.parse_source(source, "complex.js");
    assert!(result.is_ok(), "Should parse complex JavaScript code");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("complex.js");
    assert!(entities.len() >= 2, "Should find multiple entities");
}

#[test]
fn test_empty_javascript_file() {
    let mut adapter = JavaScriptAdapter::new().unwrap();
    let source = "// Just a comment\n/* Another comment */";
    let result = adapter.parse_source(source, "empty.js");
    assert!(result.is_ok(), "Should handle empty JavaScript file");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("empty.js");
    assert_eq!(
        entities.len(),
        0,
        "Should find no entities in comment-only file"
    );
}

#[test]
fn test_extract_javascript_metadata_and_fallback_names() {
    let mut adapter = JavaScriptAdapter::new().expect("adapter");
    let source = r#"
async function fetchData(url, { retries = 0 } = {}) {
    return await doFetch(url, retries);
}

const iterate = function* iterateItems(items) {
    for (const item of items) {
        yield item;
    }
};

class Derived extends Base {
    *items() {
        yield* Base.items();
    }
}

const ANSWER = 42;
const alias = thisValue;
const worker = function () { return ANSWER; };
const arrow = () => alias;
"#;

    let code_entities = adapter
        .extract_code_entities(source, "metadata.js")
        .expect("code entities");

    let fetch_entity = code_entities
        .iter()
        .find(|entity| entity.name == "fetchData")
        .expect("fetchData entity missing");
    assert_eq!(
        fetch_entity
            .properties
            .get("is_async")
            .expect("async metadata"),
        &Value::Bool(true)
    );
    assert_eq!(
        fetch_entity
            .properties
            .get("is_generator")
            .expect("generator metadata"),
        &Value::Bool(false)
    );
    let params: Vec<_> = fetch_entity
        .properties
        .get("parameters")
        .and_then(Value::as_array)
        .expect("parameters metadata")
        .iter()
        .filter_map(|value| value.as_str())
        .collect();
    assert!(
        params.contains(&"url"),
        "parameters should include url, got {params:?}"
    );

    let generator_entity = code_entities
        .iter()
        .find(|entity| {
            entity
                .properties
                .get("is_generator")
                .map(|value| value == &Value::Bool(true))
                .unwrap_or(false)
        })
        .expect("generator entity missing");
    assert_eq!(generator_entity.name, "items");
    assert_eq!(
        generator_entity
            .properties
            .get("is_generator")
            .expect("iterate generator metadata"),
        &Value::Bool(true)
    );

    let class_entity = code_entities
        .iter()
        .find(|entity| entity.name == "Derived")
        .expect("class metadata missing");
    assert_eq!(
        class_entity.properties.get("extends"),
        Some(&Value::String("Base".to_string()))
    );

    assert!(
        code_entities
            .iter()
            .any(|entity| entity.name.starts_with("anonymous_function_")),
        "expected fallback name for anonymous function",
    );
    assert!(
        code_entities
            .iter()
            .any(|entity| entity.entity_type == "Constant" && entity.name == "ANSWER"),
        "expected constant entity for ANSWER"
    );
}

#[test]
fn test_extract_javascript_import_variants() {
    let mut adapter = JavaScriptAdapter::new().expect("adapter");
    let source = r#"
import defaultExport from "pkg-core";
import { alpha, beta as betaAlias } from './utils/helpers';
import { default as DefaultHelper } from "./alt.js";
import * as analytics from "@org/analytics";
const tools = require("./tools");
const dynamic = require(`../dynamic/index.js`);
"#;

    let imports = adapter.extract_imports(source).expect("imports");
    let modules: Vec<_> = imports.iter().map(|imp| imp.module.as_str()).collect();

    assert!(
        modules.contains(&"pkg-core")
            && modules.contains(&"./utils/helpers")
            && modules.contains(&"./alt.js")
            && modules.contains(&"@org/analytics")
            && modules.contains(&"./tools")
            && modules.contains(&"../dynamic/index.js"),
        "expected normalized modules in {modules:?}"
    );

    let named_values: Vec<_> = imports
        .iter()
        .filter(|imp| imp.import_type == "named")
        .filter_map(|imp| imp.imports.as_ref())
        .flat_map(|list| list.iter().map(|name| name.trim().to_string()))
        .collect();
    assert!(
        named_values.iter().any(|name| name == "alpha"),
        "expected alpha in named imports: {named_values:?}"
    );
    assert!(
        named_values.iter().any(|name| name.contains("beta")),
        "expected beta alias in named imports: {named_values:?}"
    );
    assert!(
        named_values.iter().any(|name| name == "DefaultHelper"),
        "expected default-as normalization in named imports: {named_values:?}"
    );

    assert!(
        imports.iter().any(|imp| imp.import_type == "default"),
        "expected default import variant"
    );
    assert!(
        imports.iter().any(|imp| imp.import_type == "star"),
        "expected namespace import variant"
    );
    assert!(
        imports.iter().any(|imp| imp.import_type == "require"),
        "expected require variant"
    );
}

#[test]
fn test_javascript_identifiers_and_calls() {
    let mut adapter = JavaScriptAdapter::new().expect("adapter");
    let source = r#"
export function outer(value) {
    function inner() { return value?.toString(); }
    return [Promise.resolve(value), inner()].map(item => item);
}

outer(42);
"#;

    let calls = adapter
        .extract_function_calls(source)
        .expect("function calls");
    assert!(calls.iter().any(|call| call.contains("outer")));
    assert!(calls.iter().any(|call| call.contains("Promise.resolve")));

    let identifiers = adapter.extract_identifiers(source).expect("identifiers");
    assert!(identifiers.contains(&"outer".to_string()));
    assert!(identifiers.contains(&"inner".to_string()));

    let normalized = adapter.normalize_source(source).expect("normalize");
    assert!(
        normalized.starts_with("(program"),
        "expected S-expression for normalized source"
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
    let distinct_blocks = adapter
        .count_distinct_blocks(source)
        .expect("distinct blocks");
    assert!(ast_nodes > 0);
    assert!(distinct_blocks > 0);
}

#[test]
fn test_detects_constants_and_variables() {
    let mut adapter = JavaScriptAdapter::new().expect("adapter");
    let source = r#"
const ANSWER = 42;
let counter = 0;
var legacy = counter + ANSWER;
"#;

    let entities = adapter
        .extract_code_entities(source, "vars.js")
        .expect("entities extracted");

    let answer = entities
        .iter()
        .find(|entity| entity.name == "ANSWER")
        .expect("missing ANSWER constant");
    assert_eq!(answer.entity_type, "Constant");

    let counter = entities
        .iter()
        .find(|entity| entity.name == "counter")
        .expect("missing counter variable");
    assert_eq!(counter.entity_type, "Variable");

    let legacy = entities
        .iter()
        .find(|entity| entity.name == "legacy")
        .expect("missing legacy var");
    assert_eq!(legacy.entity_type, "Variable");
}

mod import_tests {
    use super::*;

    #[test]
    fn test_javascript_import_extraction() {
        let mut adapter = JavaScriptAdapter::new().unwrap();
        let source = r#"
import express from 'express';
import { Router } from 'express';
import * as path from 'path';
import config from './config';
const fs = require('fs');
const { promisify } = require('util');
"#;
        let imports = adapter.extract_imports(source).unwrap();

        let modules: Vec<&str> = imports.iter().map(|i| i.module.as_str()).collect();

        assert!(
            modules.contains(&"express"),
            "Should find default import from 'express'"
        );
        assert!(
            modules.contains(&"path"),
            "Should find star import from 'path'"
        );
        assert!(
            modules.contains(&"./config"),
            "Should find default import from './config'"
        );
        assert!(modules.contains(&"fs"), "Should find require('fs')");
        assert!(modules.contains(&"util"), "Should find require('util')");

        // Check import types
        let express_default = imports
            .iter()
            .find(|i| i.module == "express" && i.import_type == "default");
        assert!(
            express_default.is_some(),
            "Should have default import for express"
        );

        let express_named = imports
            .iter()
            .find(|i| i.module == "express" && i.import_type == "named");
        assert!(
            express_named.is_some(),
            "Should have named import for express"
        );
        assert!(express_named
            .unwrap()
            .imports
            .as_ref()
            .unwrap()
            .contains(&"Router".to_string()));
    }
}
