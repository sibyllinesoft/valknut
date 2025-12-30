use super::*;

#[test]
fn test_go_adapter_creation() {
    let adapter = GoAdapter::new();
    assert!(adapter.is_ok());
}

#[test]
fn test_function_parsing() {
    let mut adapter = GoAdapter::new().unwrap();
    let source_code = r#"
package main

func add(x int, y int) int {
    return x + y
}

func multiply(a, b float64) (float64, error) {
    return a * b, nil
}
"#;

    let entities = adapter
        .extract_code_entities(source_code, "test.go")
        .unwrap();
    let function_entities: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "Function")
        .collect();
    assert_eq!(function_entities.len(), 2);

    let add_func = function_entities.iter().find(|e| e.name == "add").unwrap();
    assert_eq!(add_func.entity_type, "Function");

    let multiply_func = function_entities
        .iter()
        .find(|e| e.name == "multiply")
        .unwrap();
    let return_types = multiply_func.properties.get("return_types");
    assert!(return_types.is_some());
}

#[test]
fn test_struct_parsing() {
    let mut adapter = GoAdapter::new().unwrap();
    let source_code = r#"
package main

type User struct {
    ID   int
    Name string
    Email *string
}

type Point struct {
    X, Y float64
}
"#;

    let entities = adapter
        .extract_code_entities(source_code, "test.go")
        .unwrap();

    let struct_entities: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "Struct")
        .collect();
    assert_eq!(struct_entities.len(), 2);

    let user_struct = struct_entities.iter().find(|e| e.name == "User").unwrap();
    assert_eq!(user_struct.entity_type, "Struct");

    let fields = user_struct.properties.get("fields");
    assert!(fields.is_some());
}

#[test]
fn test_interface_parsing() {
    let mut adapter = GoAdapter::new().unwrap();
    let source_code = r#"
package main

type Reader interface {
    Read([]byte) (int, error)
}

type Writer interface {
    Write([]byte) (int, error)
}

type ReadWriter interface {
    Reader
    Writer
    Close() error
}
"#;

    let entities = adapter
        .extract_code_entities(source_code, "test.go")
        .unwrap();

    let interface_entities: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "Interface")
        .collect();

    assert_eq!(interface_entities.len(), 3);

    let reader_interface = interface_entities
        .iter()
        .find(|e| e.name == "Reader")
        .unwrap();
    let methods = reader_interface.properties.get("methods");
    assert!(methods.is_some());

    let readwriter_interface = interface_entities
        .iter()
        .find(|e| e.name == "ReadWriter")
        .unwrap();
    let embedded_interfaces = readwriter_interface.properties.get("embedded_interfaces");
    assert!(embedded_interfaces.is_some());
}

#[test]
fn test_method_parsing() {
    let mut adapter = GoAdapter::new().unwrap();
    let source_code = r#"
package main

type Rectangle struct {
    Width, Height float64
}

func (r Rectangle) Area() float64 {
    return r.Width * r.Height
}

func (r *Rectangle) Scale(factor float64) {
    r.Width *= factor
    r.Height *= factor
}
"#;

    let entities = adapter
        .extract_code_entities(source_code, "test.go")
        .unwrap();

    let method_entities: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "Method")
        .collect();
    assert_eq!(method_entities.len(), 2);

    let area_method = method_entities.iter().find(|e| e.name == "Area").unwrap();
    assert_eq!(area_method.entity_type, "Method");

    let scale_method = method_entities.iter().find(|e| e.name == "Scale").unwrap();
    let receiver_type = scale_method.properties.get("receiver_type");
    assert!(receiver_type.is_some());
}

#[test]
fn test_const_and_var() {
    let mut adapter = GoAdapter::new().unwrap();
    let source_code = r#"
package main

const Pi = 3.14159
const MaxInt = 1 << 63 - 1

var GlobalCount int
var (
    Name    string
    Version string = "1.0"
)
"#;

    let entities = adapter
        .extract_code_entities(source_code, "test.go")
        .unwrap();

    let const_entities: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "Constant")
        .collect();
    let var_entities: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_type == "Variable")
        .collect();

    assert!(const_entities.len() >= 2); // Pi and MaxInt
    assert!(var_entities.len() >= 3); // GlobalCount, Name, Version

    let pi_const = const_entities.iter().find(|e| e.name == "Pi").unwrap();
    assert_eq!(pi_const.entity_type, "Constant");

    let global_var = var_entities
        .iter()
        .find(|e| e.name == "GlobalCount")
        .unwrap();
    assert_eq!(global_var.entity_type, "Variable");
}

#[test]
fn test_go_adapter_analysis_helpers() {
    let mut adapter = GoAdapter::new().expect("adapter");
    let source = r#"
package main

import (
    "fmt"
)

func main() {
    fmt.Println("hi")
    helper()
}

func helper() int {
    return 42
}
"#;

    let calls = adapter
        .extract_function_calls(source)
        .expect("function calls");
    assert!(
        calls.iter().any(|call| call.contains("fmt.Println")),
        "expected fmt.Println in {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call.contains("helper")),
        "expected helper call in {calls:?}"
    );

    let boilerplate = adapter
        .contains_boilerplate_patterns(
            source,
            &["fmt.Println".to_string(), "nonexistent".to_string()],
        )
        .expect("boilerplate detection");
    assert_eq!(boilerplate, vec!["fmt.Println".to_string()]);

    let identifiers = adapter.extract_identifiers(source).expect("identifiers");
    assert!(identifiers.contains(&"main".to_string()));
    assert!(identifiers.contains(&"helper".to_string()));

    let normalized = adapter.normalize_source(source).expect("normalize");
    assert!(
        normalized.starts_with("(source_file"),
        "expected normalized S-expression"
    );

    let ast_nodes = adapter.count_ast_nodes(source).expect("ast node count");
    assert!(ast_nodes > 0);

    let blocks = adapter
        .count_distinct_blocks(source)
        .expect("distinct blocks");
    assert!(blocks > 0);

    assert_eq!(adapter.language_name(), "go");
}

mod import_tests {
    use super::*;

    #[test]
    fn test_go_import_extraction() {
        let mut adapter = GoAdapter::new().unwrap();
        let source = r#"
package main

import "fmt"
import alias "path/to/pkg"

import (
    "os"
    "strings"
    _ "github.com/lib/pq"
    . "github.com/onsi/ginkgo"
    custom "my/custom/package"
)
"#;
        let imports = adapter.extract_imports(source).unwrap();

        let modules: Vec<&str> = imports.iter().map(|i| i.module.as_str()).collect();

        // Check single-line imports
        assert!(modules.contains(&"fmt"), "Should find 'import \"fmt\"'");
        assert!(modules.contains(&"path/to/pkg"), "Should find aliased import");

        // Check block imports
        assert!(modules.contains(&"os"), "Should find 'os' in import block");
        assert!(modules.contains(&"strings"), "Should find 'strings' in import block");
        assert!(modules.contains(&"github.com/lib/pq"), "Should find blank import");
        assert!(modules.contains(&"github.com/onsi/ginkgo"), "Should find dot import");
        assert!(modules.contains(&"my/custom/package"), "Should find custom alias in block");

        // Verify count
        assert_eq!(imports.len(), 7, "Should have 7 imports total");
    }
}
