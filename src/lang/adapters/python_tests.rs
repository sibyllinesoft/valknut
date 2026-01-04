use super::*;

#[test]
fn test_python_adapter_creation() {
    let adapter = PythonAdapter::new();
    assert!(adapter.is_ok(), "Should create Python adapter successfully");
}

#[test]
fn test_parse_simple_function() {
    let mut adapter = PythonAdapter::new().unwrap();
    let source = r#"
def hello_world():
    return "Hello, World!"
"#;
    let result = adapter.parse_source(source, "test.py");
    assert!(
        result.is_ok(),
        "Should parse simple function: {:?}",
        result.err()
    );

    let index = result.unwrap();
    assert!(
        index.get_entities_in_file("test.py").len() >= 1,
        "Should find at least one entity"
    );
}

#[test]
fn test_parse_simple_class() {
    let mut adapter = PythonAdapter::new().unwrap();
    let source = r#"
class MyClass:
    def __init__(self):
        self.value = 0

    def get_value(self):
        return self.value
"#;
    let result = adapter.parse_source(source, "test.py");
    assert!(result.is_ok(), "Should parse simple class");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("test.py");
    assert!(entities.len() >= 1, "Should find at least one entity");

    let has_class = entities.iter().any(|e| matches!(e.kind, EntityKind::Class));
    assert!(has_class, "Should find a class entity");
}

#[test]
fn test_parse_complex_python() {
    let mut adapter = PythonAdapter::new().unwrap();
    let source = r#"
import os
import sys

class DataProcessor:
    """A sample data processor class."""

    def __init__(self, name: str):
        self.name = name
        self.data = []

    @property
    def size(self) -> int:
        return len(self.data)

    def add_data(self, item):
        self.data.append(item)

def process_file(filename: str) -> bool:
    """Process a file and return success status."""
    try:
        with open(filename, 'r') as f:
            content = f.read()
        return True
    except FileNotFoundError:
        return False

if __name__ == "__main__":
    processor = DataProcessor("test")
    success = process_file("data.txt")
"#;
    let result = adapter.parse_source(source, "complex.py");
    assert!(result.is_ok(), "Should parse complex Python code");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("complex.py");
    assert!(entities.len() >= 2, "Should find multiple entities");

    let has_class = entities.iter().any(|e| matches!(e.kind, EntityKind::Class));
    let has_function = entities
        .iter()
        .any(|e| matches!(e.kind, EntityKind::Function));
    assert!(
        has_class && has_function,
        "Should find both class and function entities"
    );
}

#[test]
fn test_extract_entity_name() {
    let mut adapter = PythonAdapter::new().unwrap();
    let source = "def test_function(): pass";
    let tree = adapter.parser.parse(source, None).unwrap();
    let function_node = tree.root_node().child(0).unwrap(); // Should be function_definition

    let result = adapter.extract_name(&function_node, source);
    assert!(result.is_ok());

    if let Ok(Some(name)) = result {
        assert_eq!(name, "test_function");
    }
}

#[test]
fn test_convert_to_code_entity() {
    let entity = ParsedEntity {
        id: "test-id".to_string(),
        name: "test_func".to_string(),
        kind: EntityKind::Function,
        location: SourceLocation {
            file_path: "test.py".to_string(),
            start_line: 1,
            end_line: 2,
            start_column: 0,
            end_column: 10,
        },
        parent: None,
        children: vec![],
        metadata: HashMap::new(),
    };

    let source = "def test_func(): pass";
    let code_entity = entity.to_code_entity(source);
    assert_eq!(code_entity.name, "test_func");
    assert_eq!(code_entity.file_path, "test.py");
}

#[test]
fn test_get_entities_empty_file() {
    let mut adapter = PythonAdapter::new().unwrap();
    let source = "# Just a comment\n";
    let result = adapter.parse_source(source, "empty.py");
    assert!(result.is_ok(), "Should handle empty Python file");

    let index = result.unwrap();
    let entities = index.get_entities_in_file("empty.py");
    assert_eq!(
        entities.len(),
        0,
        "Should find no entities in comment-only file"
    );
}

#[test]
fn test_extract_imports_supports_star_and_named() {
    let mut adapter = PythonAdapter::new().unwrap();
    let source = r#"
import os
from typing import List, Dict
from custom.utils import *
"#;

    let imports = adapter.extract_imports(source).expect("imports parsed");
    assert_eq!(imports.len(), 3);
    assert!(imports.iter().any(|imp| imp.module == "os"));
    assert!(imports
        .iter()
        .any(|imp| imp.module == "typing" && imp.import_type == "named"));
    assert!(imports
        .iter()
        .any(|imp| imp.module == "custom.utils" && imp.import_type == "star"));
}

#[test]
fn test_extract_function_calls_and_identifiers() {
    let mut adapter = PythonAdapter::new().unwrap();
    let source = r#"
import math

def compute(value):
    print(value)
    math.sqrt(value)
    helper(value)

def helper(value):
    return value * 2

compute(10)
"#;

    let calls = adapter
        .extract_function_calls(source)
        .expect("function calls extracted");
    assert!(calls.contains(&"print".to_string()));
    assert!(calls.iter().any(|call| call.contains("math.sqrt")));
    assert!(calls.contains(&"helper".to_string()));

    let identifiers = adapter
        .extract_identifiers(source)
        .expect("identifiers extracted");
    assert!(identifiers.contains(&"compute".to_string()));
    assert!(identifiers.contains(&"helper".to_string()));
}

#[test]
fn test_contains_boilerplate_patterns_detects_common_cases() {
    let mut adapter = PythonAdapter::new().unwrap();
    let source = r#"
import os
from typing import List

def main():
    pass

if __name__ == "__main__":
    main()
"#;

    let patterns = vec![
        "import os".to_string(),
        "from typing import".to_string(),
        "if __name__ == \"__main__\"".to_string(),
    ];
    let found = adapter
        .contains_boilerplate_patterns(source, &patterns)
        .expect("boilerplate detection");

    assert!(found.contains(&"import os".to_string()));
    assert!(found.contains(&"from typing import".to_string()));
    assert!(found.contains(&"if __name__ == \"__main__\"".to_string()));
}

#[test]
fn test_normalize_and_count_python_ast_metrics() {
    let mut adapter = PythonAdapter::new().unwrap();
    let source = r#"
def outer(value):
    if value > 10:
        return value + 1
    return value - 1
"#;

    let normalized = adapter
        .normalize_source(source)
        .expect("normalization should succeed");
    assert!(normalized.contains("function_definition"));
    assert!(normalized.contains("if_statement"));

    let node_count = adapter
        .count_ast_nodes(source)
        .expect("node counting should succeed");
    assert!(node_count > 0);

    let block_count = adapter
        .count_distinct_blocks(source)
        .expect("block counting should succeed");
    assert!(block_count >= 2);
}

mod import_tests {
    use super::*;

    #[test]
    fn test_python_import_extraction() {
        let mut adapter = PythonAdapter::new().unwrap();
        let source = r#"
import os
import sys
from pathlib import Path
from collections import defaultdict, Counter
from typing import List, Optional, Dict
from . import sibling_module
from .. import parent_module
from ..utils import helper_function
import numpy as np
"#;
        let imports = adapter.extract_imports(source).unwrap();

        // Verify we get all imports
        let modules: Vec<&str> = imports.iter().map(|i| i.module.as_str()).collect();

        assert!(modules.contains(&"os"), "Should find 'import os'");
        assert!(modules.contains(&"sys"), "Should find 'import sys'");
        assert!(modules.contains(&"pathlib"), "Should find 'from pathlib'");
        assert!(modules.contains(&"collections"), "Should find 'from collections'");
        assert!(modules.contains(&"typing"), "Should find 'from typing'");
        assert!(modules.contains(&"."), "Should find 'from .'");
        assert!(modules.contains(&".."), "Should find 'from ..'");
        assert!(modules.contains(&"..utils"), "Should find 'from ..utils'");
        assert!(modules.contains(&"numpy"), "Should find 'import numpy as np'");

        // Check specific imports are captured
        let collections_import = imports.iter().find(|i| i.module == "collections").unwrap();
        assert!(collections_import.imports.as_ref().unwrap().contains(&"defaultdict".to_string()));
        assert!(collections_import.imports.as_ref().unwrap().contains(&"Counter".to_string()));
    }
}
