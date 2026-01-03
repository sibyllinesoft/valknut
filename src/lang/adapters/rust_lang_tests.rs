use super::*;

#[test]
fn test_rust_adapter_creation() {
    let adapter = RustAdapter::new();
    assert!(adapter.is_ok(), "Should create Rust adapter successfully");
}

#[test]
fn test_parse_simple_function() {
    let mut adapter = RustAdapter::new().unwrap();
    let source = r#"
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#;
    let result = adapter.parse_source(source, "test.rs");
    assert!(result.is_ok(), "Should parse simple function");

    let index = result.unwrap();
    assert!(
        index.get_entities_in_file("test.rs").len() >= 1,
        "Should find at least one entity"
    );
}

#[test]
fn test_parse_struct_and_impl() {
    let mut adapter = RustAdapter::new().unwrap();
    let source = r#"
struct User {
    name: String,
    age: u32,
}

impl User {
    fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}
"#;
    let result = adapter.parse_source(source, "test.rs");
    assert!(result.is_ok(), "Should parse struct and impl");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("test.rs");
    assert!(
        entities.len() >= 2,
        "Should find at least struct and impl entities"
    );

    let has_struct = entities
        .iter()
        .any(|e| matches!(e.kind, EntityKind::Struct));
    assert!(has_struct, "Should find a struct entity");
}

#[test]
fn test_parse_traits_and_enums() {
    let mut adapter = RustAdapter::new().unwrap();
    let source = r#"
trait Display {
    fn display(&self) -> String;
}

enum Color {
    Red,
    Green,
    Blue,
}

impl Display for Color {
    fn display(&self) -> String {
        match self {
            Color::Red => "Red".to_string(),
            Color::Green => "Green".to_string(),
            Color::Blue => "Blue".to_string(),
        }
    }
}
"#;
    let result = adapter.parse_source(source, "traits.rs");
    assert!(result.is_ok(), "Should parse traits and enums");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("traits.rs");
    assert!(entities.len() >= 2, "Should find multiple entities");

    let has_enum = entities.iter().any(|e| matches!(e.kind, EntityKind::Enum));
    assert!(has_enum, "Should find an enum entity");
}

#[test]
fn test_parse_modules() {
    let mut adapter = RustAdapter::new().unwrap();
    let source = r#"
mod network {
    use std::net::TcpStream;

    pub fn connect(addr: &str) -> Result<TcpStream, std::io::Error> {
        TcpStream::connect(addr)
    }
}

pub mod utils {
    pub fn format_string(s: &str) -> String {
        s.to_uppercase()
    }
}
"#;
    let result = adapter.parse_source(source, "modules.rs");
    assert!(result.is_ok(), "Should parse modules");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("modules.rs");
    assert!(
        entities.len() >= 2,
        "Should find multiple entities including modules"
    );

    let has_module = entities
        .iter()
        .any(|e| matches!(e.kind, EntityKind::Module));
    assert!(has_module, "Should find module entities");
}

#[test]
fn test_empty_rust_file() {
    let mut adapter = RustAdapter::new().unwrap();
    let source = "// Rust file with just comments\n/* Block comment */";
    let result = adapter.parse_source(source, "empty.rs");
    assert!(result.is_ok(), "Should handle empty Rust file");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("empty.rs");
    assert_eq!(
        entities.len(),
        0,
        "Should find no entities in comment-only file"
    );
}

mod additional_tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_rust_adapter_creation_additional() {
        let adapter = RustAdapter::new();
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_function_parsing() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
pub fn calculate(x: i32, y: i32) -> i32 {
    x + y
}

async unsafe fn complex_function() -> Result<(), Error> {
    Ok(())
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();
        assert_eq!(entities.len(), 2);

        let calc_func = entities.iter().find(|e| e.name == "calculate").unwrap();
        assert_eq!(calc_func.entity_type, "Function");
        assert_eq!(
            calc_func.properties.get("visibility"),
            Some(&Value::String("pub".to_string()))
        );

        let complex_func = entities
            .iter()
            .find(|e| e.name == "complex_function")
            .unwrap();
        assert_eq!(
            complex_func.properties.get("is_async"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            complex_func.properties.get("is_unsafe"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn test_struct_parsing() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
pub struct User {
    id: u64,
    name: String,
    email: Option<String>,
}

struct Point<T> {
    x: T,
    y: T,
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();

        let struct_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Struct")
            .collect();
        assert_eq!(struct_entities.len(), 2);

        let user_struct = struct_entities.iter().find(|e| e.name == "User").unwrap();
        assert_eq!(
            user_struct.properties.get("visibility"),
            Some(&Value::String("pub".to_string()))
        );

        let point_struct = struct_entities.iter().find(|e| e.name == "Point").unwrap();
        let generic_params = point_struct.properties.get("generic_parameters");
        assert!(generic_params.is_some());
    }

    #[test]
    fn test_enum_parsing() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
#[derive(Debug, Clone)]
pub enum Status {
    Active,
    Inactive,
    Pending(String),
    Expired { reason: String },
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();
        assert_eq!(entities.len(), 1);

        let enum_entity = &entities[0];
        assert_eq!(enum_entity.entity_type, "Enum");
        assert_eq!(enum_entity.name, "Status");
        assert_eq!(
            enum_entity.properties.get("visibility"),
            Some(&Value::String("pub".to_string()))
        );

        let variants = enum_entity.properties.get("variants");
        assert!(variants.is_some());
    }

    #[test]
    fn test_trait_parsing() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
pub trait Display: Debug + Clone {
    fn fmt(&self) -> String;
    fn print(&self) {
        println!("{}", self.fmt());
    }
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();
        assert_eq!(entities.len(), 1);

        let trait_entity = &entities[0];
        assert_eq!(trait_entity.entity_type, "Interface");
        assert_eq!(trait_entity.name, "Display");
        assert_eq!(
            trait_entity.properties.get("visibility"),
            Some(&Value::String("pub".to_string()))
        );

        let methods = trait_entity.properties.get("methods");
        assert!(methods.is_some());
    }

    #[test]
    fn test_module_parsing() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
pub mod utils;

mod internal {
    pub fn helper() -> i32 {
        42
    }
}
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();

        let module_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Module")
            .collect();
        assert!(module_entities.len() >= 2); // utils and internal modules

        let internal_mod = module_entities
            .iter()
            .find(|e| e.name == "internal")
            .unwrap();
        assert_eq!(
            internal_mod.properties.get("is_inline"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn test_const_and_static() {
        let mut adapter = RustAdapter::new().unwrap();
        let source_code = r#"
const PI: f64 = 3.14159;
static GLOBAL_COUNT: AtomicUsize = AtomicUsize::new(0);
"#;

        let entities = adapter
            .extract_code_entities(source_code, "test.rs")
            .unwrap();

        let const_entities: Vec<_> = entities
            .iter()
            .filter(|e| e.entity_type == "Constant")
            .collect();
        assert_eq!(const_entities.len(), 2);

        let pi_const = const_entities.iter().find(|e| e.name == "PI").unwrap();
        assert_eq!(pi_const.entity_type, "Constant");

        let global_static = const_entities
            .iter()
            .find(|e| e.name == "GLOBAL_COUNT")
            .unwrap();
        assert_eq!(global_static.entity_type, "Constant");
    }
}

mod import_tests {
    use super::*;

    #[test]
    fn test_rust_import_extraction() {
        let mut adapter = RustAdapter::new().unwrap();
        let source = r#"
mod config;
pub mod utils;
pub(crate) mod helpers;

use std::collections::HashMap;
use crate::core::{Config, Error};
use super::parent_module;
use self::local_module;
use anyhow::{Result, Context};
"#;
        let imports = adapter.extract_imports(source).unwrap();

        let modules: Vec<&str> = imports.iter().map(|i| i.module.as_str()).collect();

        // Check mod declarations
        assert!(modules.contains(&"config"), "Should find 'mod config'");
        assert!(modules.contains(&"utils"), "Should find 'pub mod utils'");
        assert!(modules.contains(&"helpers"), "Should find 'pub(crate) mod helpers'");

        // Check use statements
        assert!(modules.contains(&"std::collections::HashMap"), "Should find HashMap use");
        assert!(modules.contains(&"crate::core::"), "Should find crate::core use");
        assert!(modules.contains(&"super::parent_module"), "Should find super:: use");
        assert!(modules.contains(&"self::local_module"), "Should find self:: use");
        assert!(modules.contains(&"anyhow::"), "Should find anyhow use");

        // Check named imports are extracted
        let core_import = imports.iter().find(|i| i.module == "crate::core::").unwrap();
        assert!(core_import.imports.as_ref().unwrap().contains(&"Config".to_string()));
        assert!(core_import.imports.as_ref().unwrap().contains(&"Error".to_string()));
    }
}
