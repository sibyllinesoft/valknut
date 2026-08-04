//! Executable minimum capability contract for every registered language.

use std::path::Path;
use valknut_rs::core::config::ValknutConfig;
use valknut_rs::lang::{adapter_for_file, adapter_for_language, registered_languages};

struct LanguageFixture {
    key: &'static str,
    path: &'static str,
    source: &'static str,
}

const FIXTURES: &[LanguageFixture] = &[
    LanguageFixture {
        key: "py",
        path: "sample.py",
        source: "import pathlib\ndef run(value):\n    return pathlib.Path(value).exists()\n",
    },
    LanguageFixture {
        key: "js",
        path: "sample.js",
        source: "import path from 'node:path';\nfunction run(value) { return path.resolve(value); }\n",
    },
    LanguageFixture {
        key: "ts",
        path: "sample.ts",
        source: "import { resolve } from 'node:path';\ninterface Runner { run(value: string): string; }\nclass App { run(value: string) { return resolve(value); } }\n",
    },
    LanguageFixture {
        key: "rs",
        path: "sample.rs",
        source: "use std::path::Path;\nfn run(value: &str) -> bool { Path::new(value).exists() }\n",
    },
    LanguageFixture {
        key: "go",
        path: "sample.go",
        source: "package sample\nimport \"path/filepath\"\nfunc run(value string) string { return filepath.Clean(value) }\n",
    },
    LanguageFixture {
        key: "cpp",
        path: "sample.cpp",
        source: "#include <string>\nstd::string run(const std::string& value) { return value.substr(0); }\n",
    },
    LanguageFixture {
        key: "cs",
        path: "sample.cs",
        source: "using System.IO;\nclass App { string Run(string value) { return Path.GetFullPath(value); } }\n",
    },
];

#[test]
fn every_registered_language_meets_the_analysis_contract() {
    assert_eq!(FIXTURES.len(), registered_languages().len());

    for fixture in FIXTURES {
        assert!(
            registered_languages()
                .iter()
                .any(|language| language.key == fixture.key),
            "{} fixture must correspond to a registered language",
            fixture.key
        );
        let mut adapter = adapter_for_language(fixture.key).unwrap();
        let tree = adapter.parse_tree(fixture.source).unwrap();
        assert!(
            !tree.root_node().has_error(),
            "{} fixture must parse without errors",
            fixture.key
        );

        let entities = adapter
            .extract_code_entities(fixture.source, fixture.path)
            .unwrap();
        assert!(
            !entities.is_empty(),
            "{} must extract entities",
            fixture.key
        );
        assert!(entities.iter().all(|entity| {
            !entity.name.is_empty()
                && entity.line_range.is_some()
                && entity.properties.contains_key("node_kind")
                && entity.properties.contains_key("byte_range")
        }));

        assert!(
            !adapter
                .extract_function_calls(fixture.source)
                .unwrap()
                .is_empty(),
            "{} must extract calls",
            fixture.key
        );
        assert!(
            !adapter
                .extract_identifiers(fixture.source)
                .unwrap()
                .is_empty(),
            "{} must extract identifiers",
            fixture.key
        );
        assert!(
            !adapter.extract_imports(fixture.source).unwrap().is_empty(),
            "{} must extract dependencies",
            fixture.key
        );
        assert!(adapter.count_ast_nodes(fixture.source).unwrap() > 1);
        assert!(adapter.count_distinct_blocks(fixture.source).unwrap() > 0);
        assert!(!adapter.normalize_source(fixture.source).unwrap().is_empty());
    }
}

#[test]
fn hierarchy_and_multi_declarators_are_consistent() {
    let cases = [
        (
            "py",
            "class Box:\n    def run(self): pass\na, b = (1, 2)\n",
            "Box",
            "run",
            ["a", "b"],
        ),
        (
            "js",
            "class Box { run() {} }\nconst a = 1, b = 2;\n",
            "Box",
            "run",
            ["a", "b"],
        ),
        (
            "ts",
            "class Box { run(): void {} }\nconst a = 1, b = 2;\n",
            "Box",
            "run",
            ["a", "b"],
        ),
        (
            "cs",
            "class Box { const int a = 1, b = 2; void Run() {} }\n",
            "Box",
            "Run",
            ["a", "b"],
        ),
    ];

    for (key, source, parent_name, child_name, declarations) in cases {
        let mut adapter = adapter_for_language(key).unwrap();
        let index = adapter.parse_source(source, "sample").unwrap();
        let parent = index
            .entities
            .values()
            .find(|entity| entity.name == parent_name)
            .unwrap();
        let child = index
            .entities
            .values()
            .find(|entity| entity.name == child_name)
            .unwrap();
        assert_eq!(
            child.parent.as_deref(),
            Some(parent.id.as_str()),
            "{key} ownership"
        );
        assert!(
            parent.children.contains(&child.id),
            "{key} reverse hierarchy"
        );
        for declaration in declarations {
            assert!(
                index
                    .entities
                    .values()
                    .any(|entity| entity.name == declaration),
                "{key} missing {declaration}"
            );
        }
    }
}

#[test]
fn rust_impl_methods_are_owned_by_their_type() {
    let source = "struct Box;\nimpl Box { fn run(&self) {} }\ntrait Runner { fn stop(&self); }\n";
    let mut adapter = adapter_for_language("rs").unwrap();
    let index = adapter.parse_source(source, "sample.rs").unwrap();
    let owner = index
        .entities
        .values()
        .find(|entity| entity.name == "Box")
        .unwrap();
    let method = index
        .entities
        .values()
        .find(|entity| entity.name == "run")
        .unwrap();
    assert_eq!(method.kind, valknut_rs::lang::EntityKind::Method);
    assert_eq!(method.parent.as_deref(), Some(owner.id.as_str()));
    assert!(owner.children.contains(&method.id));
    assert!(index.entities.values().any(|entity| entity.name == "stop"));
}

#[test]
fn dependency_variants_preserve_semantics() {
    let mut go = adapter_for_language("go").unwrap();
    let imports = go
        .extract_imports(
            "package p\nimport alias \"example.com/pkg\"\nimport _ \"example.com/driver\"\n",
        )
        .unwrap();
    assert_eq!(imports[0].import_type, "aliased_import");
    assert_eq!(
        imports[0].imports.as_deref(),
        Some(&["alias".to_string()][..])
    );
    assert_eq!(imports[1].import_type, "blank_import");

    let mut cpp = adapter_for_language("cpp").unwrap();
    let imports = cpp
        .extract_imports("import widgets.core;\nexport import widgets.api;\n")
        .unwrap();
    assert!(imports
        .iter()
        .any(|item| item.module == "widgets.core" && item.import_type == "module_import"));
    assert!(imports
        .iter()
        .any(|item| item.module == "widgets.api" && item.import_type == "export_import"));
}

#[test]
fn extension_specific_grammar_variants_parse() {
    let source = "type Props = { name: string };\nexport const View = ({ name }: Props) => <section>{name}</section>;\n";
    let mut adapter = adapter_for_file(Path::new("View.tsx")).unwrap();
    let tree = adapter.parse_tree(source).unwrap();
    assert!(!tree.root_node().has_error());
    assert!(adapter
        .extract_code_entities(source, "View.tsx")
        .unwrap()
        .iter()
        .any(|entity| entity.name == "View"));
}

#[test]
fn callable_variants_are_not_silently_dropped() {
    let cases = [
        ("py", "factory = lambda value: value\n", "lambda"),
        ("js", "function* values() { yield 1; }\nconst map = value => value;\n", "generator_function_declaration"),
        ("ts", "function* values(): Generator<number> { yield 1; }\nconst map = (value: number) => value;\n", "generator_function_declaration"),
        ("rs", "fn main() { let map = |value: i32| value + 1; }\n", "closure_expression"),
        ("go", "package p\nvar mapValue = func(value int) int { return value + 1 }\n", "func_literal"),
        ("cpp", "auto map_value = [](int value) { return value + 1; };\n", "lambda_expression"),
        ("cs", "class C { Func<int, int> Map = value => value + 1; }\n", "lambda_expression"),
    ];
    for (key, source, node_kind) in cases {
        let mut adapter = adapter_for_language(key).unwrap();
        let entities = adapter.extract_code_entities(source, "sample").unwrap();
        assert!(
            entities.iter().any(|entity| {
                entity
                    .properties
                    .get("node_kind")
                    .and_then(|value| value.as_str())
                    == Some(node_kind)
            }),
            "{key} dropped {node_kind}"
        );
    }
}

#[test]
fn mature_dependency_variants_are_complete() {
    let mut python = adapter_for_language("py").unwrap();
    let imports = python
        .extract_imports("import os, sys as system\n")
        .unwrap();
    assert_eq!(imports.len(), 2);
    assert_eq!(
        imports[1].imports.as_deref(),
        Some(&["system".to_string()][..])
    );

    for key in ["js", "ts"] {
        let mut adapter = adapter_for_language(key).unwrap();
        let imports = adapter
            .extract_imports(
                "import './setup.js';\nimport {\n  readFile,\n  writeFile\n} from 'node:fs';\n",
            )
            .unwrap();
        assert!(imports.iter().any(|item| item.import_type == "side_effect"));
        assert!(imports.iter().any(|item| item.module == "node:fs"));
    }

    let mut rust = adapter_for_language("rs").unwrap();
    let imports = rust
        .extract_imports("use std::{\n    fs,\n    path::Path,\n};\n")
        .unwrap();
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0].module, "std");
}

#[test]
fn mature_adapters_preserve_parameter_signatures_and_calls() {
    let cases = [
        (
            "py",
            "def run(value: str = 'x', *args):\n    return helper(value)\n",
            "run",
            "value: str = 'x'",
            "helper",
        ),
        (
            "js",
            "function run({ value }, fallback = 'x') { return helper(value); }\n",
            "run",
            "{ value }",
            "helper",
        ),
        (
            "ts",
            "function run(value: string = 'x'): string { return helper(value); }\n",
            "run",
            "value: string = 'x'",
            "helper",
        ),
    ];
    for (key, source, name, parameter, call) in cases {
        let mut adapter = adapter_for_language(key).unwrap();
        let entities = adapter.extract_code_entities(source, "sample").unwrap();
        let entity = entities.iter().find(|entity| entity.name == name).unwrap();
        assert!(entity.properties["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == parameter));
        assert!(entity.properties["function_calls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == call));
    }
}

#[test]
fn configuration_defaults_cover_every_registered_extension() {
    let config = ValknutConfig::default();
    for language in registered_languages() {
        for extension in language.extensions {
            let expected = format!(".{extension}");
            assert!(
                config.languages.values().any(|settings| {
                    settings.enabled
                        && settings
                            .file_extensions
                            .iter()
                            .any(|value| value == &expected)
                }),
                "default config omitted {expected}"
            );
        }
    }
}

#[test]
fn overloads_and_partial_owners_have_stable_linkage() {
    let mut typescript = adapter_for_language("ts").unwrap();
    let source = "function parse(value: string): string;\nfunction parse(value: number): number;\nfunction parse(value: string | number) { return value; }\n";
    let index = typescript.parse_source(source, "parse.ts").unwrap();
    let implementation = index
        .entities
        .values()
        .find(|entity| {
            entity.name == "parse"
                && entity
                    .metadata
                    .get("declaration_only")
                    .and_then(|value| value.as_bool())
                    != Some(true)
        })
        .unwrap();
    assert!(implementation.metadata["overload_group"].is_string());
    assert_eq!(
        implementation.metadata["overloads"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let mut csharp = adapter_for_language("cs").unwrap();
    let source = "namespace Demo; public partial class Service { public void Run() {} }";
    let index = csharp.parse_source(source, "Service.cs").unwrap();
    let method = index
        .entities
        .values()
        .find(|entity| entity.name == "Run")
        .unwrap();
    assert_eq!(method.metadata["owner_path"], "Demo.Service");
    assert!(method.metadata["parent_modifiers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "partial"));
}

#[test]
fn package_and_external_type_ownership_remain_queryable() {
    let mut go = adapter_for_language("go").unwrap();
    let index = go
        .parse_source(
            "package tools\nfunc Map[T any](value T) T { return value }\n",
            "map.go",
        )
        .unwrap();
    let function = index
        .entities
        .values()
        .find(|entity| entity.name == "Map")
        .unwrap();
    assert_eq!(function.metadata["parent_name"], "tools");
    assert!(function.metadata["generic_parameters"].is_string());

    let mut rust = adapter_for_language("rs").unwrap();
    let index = rust
        .parse_source(
            "impl external::Service { fn run(&self) {} }\n",
            "service.rs",
        )
        .unwrap();
    let method = index
        .entities
        .values()
        .find(|entity| entity.name == "run")
        .unwrap();
    assert_eq!(method.metadata["owner_path"], "external::Service");
}
