//! Comprehensive tests for the C++ language adapter.

use super::*;
use crate::lang::common::{EntityKind, LanguageAdapter};

// ============================================================================
// Basic Entity Parsing Tests
// ============================================================================

#[test]
fn test_parse_simple_function() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
int add(int a, int b) {
    return a + b;
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    assert_eq!(index.entities.len(), 1);

    let entity = index.entities.values().next().unwrap();
    assert_eq!(entity.name, "add");
    assert_eq!(entity.kind, EntityKind::Function);
}

#[test]
fn test_parse_void_function() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = "void doNothing() {}";

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let funcs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Function).collect();
    assert_eq!(funcs.len(), 1);
    assert_eq!(funcs[0].name, "doNothing");
}

#[test]
fn test_parse_multiple_functions() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
void first() {}
int second() { return 0; }
double third(int x) { return x * 1.5; }
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let funcs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Function).collect();
    assert_eq!(funcs.len(), 3);
}

#[test]
fn test_parse_class() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class MyClass {
public:
    void method();
private:
    int field;
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let classes: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "MyClass");
}

#[test]
fn test_parse_struct() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
struct Point {
    int x;
    int y;
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let structs: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Point");
}

#[test]
fn test_parse_enum() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
enum Color {
    Red,
    Green,
    Blue
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let enums: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Color");
}

#[test]
fn test_parse_scoped_enum() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
enum class Direction {
    North,
    South,
    East,
    West
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let enums: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Direction");

    // Check scoped enum metadata
    assert!(enums[0].metadata.get("is_scoped").is_some());
}

#[test]
fn test_parse_enum_with_underlying_type() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
enum class Status : uint8_t {
    OK = 0,
    Error = 1
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let enums: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Enum).collect();
    assert_eq!(enums.len(), 1);
}

// ============================================================================
// Namespace Tests
// ============================================================================

#[test]
fn test_parse_namespace() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
namespace foo {
    void bar() {}
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let namespaces: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Module)
        .collect();
    assert_eq!(namespaces.len(), 1);
    assert_eq!(namespaces[0].name, "foo");

    let functions: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Function)
        .collect();
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].name, "foo::bar");
}

#[test]
fn test_parse_nested_namespaces() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
namespace outer {
    namespace inner {
        void func() {}
    }
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let namespaces: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Module)
        .collect();
    assert_eq!(namespaces.len(), 2);

    let functions: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Function)
        .collect();
    assert_eq!(functions.len(), 1);
    assert!(functions[0].name.contains("inner"));
}

#[test]
fn test_parse_anonymous_namespace() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
namespace {
    void internal_func() {}
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let namespaces: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Module)
        .collect();
    assert_eq!(namespaces.len(), 1);
    assert!(namespaces[0].name.contains("anonymous") || namespaces[0].name == "<anonymous>");
}

#[test]
fn test_namespace_with_class() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
namespace mylib {
    class Widget {
    public:
        void draw();
    };
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let classes: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert!(classes[0].name.contains("Widget"));
}

// ============================================================================
// Template Tests
// ============================================================================

#[test]
fn test_parse_template_class() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
template<typename T>
class Container {
    T value;
public:
    T get() { return value; }
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let classes: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Container");
}

#[test]
fn test_parse_template_function() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
template<typename T>
T maximum(T a, T b) {
    return (a > b) ? a : b;
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let funcs: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Function)
        .collect();
    assert_eq!(funcs.len(), 1);
    assert_eq!(funcs[0].name, "maximum");
}

#[test]
fn test_parse_variadic_template() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
template<typename... Args>
void print(Args... args) {}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let funcs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Function).collect();
    assert_eq!(funcs.len(), 1);
}

#[test]
fn test_parse_template_with_multiple_params() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
template<typename K, typename V, int Size>
class Map {
    K keys[Size];
    V values[Size];
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert_eq!(classes.len(), 1);
}

// ============================================================================
// Inheritance Tests
// ============================================================================

#[test]
fn test_parse_class_with_inheritance() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class Base {
public:
    virtual void method() = 0;
};

class Derived : public Base {
public:
    void method() override {}
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let classes: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Class)
        .collect();
    assert_eq!(classes.len(), 2);

    let class_names: Vec<_> = classes.iter().map(|c| c.name.as_str()).collect();
    assert!(class_names.contains(&"Base"));
    assert!(class_names.contains(&"Derived"));
}

#[test]
fn test_parse_multiple_inheritance() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class A {};
class B {};
class C : public A, public B {};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert_eq!(classes.len(), 3);
}

#[test]
fn test_parse_struct_inheritance() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
struct Base {
    int x;
};

struct Derived : Base {
    int y;
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let structs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Struct).collect();
    assert_eq!(structs.len(), 2);
}

// ============================================================================
// Method and Member Tests
// ============================================================================

#[test]
fn test_parse_method_qualifiers() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class Widget {
public:
    virtual void update() = 0;
    static Widget* instance();
    void process() const noexcept;
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    assert!(!index.entities.is_empty());
}

#[test]
fn test_parse_destructor() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class Resource {
public:
    Resource();
    ~Resource();
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    let classes: Vec<_> = index
        .entities
        .values()
        .filter(|e| e.kind == EntityKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
}

#[test]
fn test_parse_operator_overload() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class Number {
public:
    Number operator+(const Number& other) {
        return Number();
    }
    bool operator==(const Number& other) const {
        return true;
    }
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert_eq!(classes.len(), 1);
}

#[test]
fn test_parse_constexpr_function() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
constexpr int factorial(int n) {
    return n <= 1 ? 1 : n * factorial(n - 1);
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let funcs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Function).collect();
    assert_eq!(funcs.len(), 1);
}

#[test]
fn test_parse_inline_function() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
inline int square(int x) {
    return x * x;
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let funcs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Function).collect();
    assert_eq!(funcs.len(), 1);
}

#[test]
fn test_parse_static_method() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class Factory {
public:
    static Factory* create() {
        return new Factory();
    }
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    assert!(!index.entities.is_empty());
}

// ============================================================================
// Preprocessor and Include Tests
// ============================================================================

#[test]
fn test_extract_includes() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
#include <iostream>
#include <vector>
#include "myheader.h"

int main() {
    return 0;
}
"#;

    let imports = adapter.extract_imports(source).unwrap();
    assert_eq!(imports.len(), 3);

    let paths: Vec<_> = imports.iter().map(|i| i.module.as_str()).collect();
    assert!(paths.contains(&"iostream"));
    assert!(paths.contains(&"vector"));
    assert!(paths.contains(&"myheader.h"));
}

#[test]
fn test_parse_with_include_guard() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
#ifndef MY_HEADER_H
#define MY_HEADER_H

class GuardedClass {
public:
    void method();
};

#endif
"#;

    let index = adapter.parse_source(source, "test.h").unwrap();
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "GuardedClass");
}

#[test]
fn test_parse_with_pragma_once() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
#pragma once

struct PragmaStruct {
    int value;
};
"#;

    let index = adapter.parse_source(source, "test.h").unwrap();
    let structs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Struct).collect();
    assert_eq!(structs.len(), 1);
}

#[test]
fn test_parse_with_ifdef_blocks() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
#ifdef _WIN32
class WindowsImpl {};
#else
class UnixImpl {};
#endif

class CommonClass {};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    // Should find at least CommonClass and one of the platform classes
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert!(classes.len() >= 1);
}

// ============================================================================
// Function Call and Identifier Extraction Tests
// ============================================================================

#[test]
fn test_extract_function_calls() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
void foo() {
    bar();
    baz(1, 2);
    obj.method();
}
"#;

    let calls = adapter.extract_function_calls(source).unwrap();
    assert!(calls.contains(&"bar".to_string()));
    assert!(calls.contains(&"baz".to_string()));
}

#[test]
fn test_extract_function_calls_nested() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
void process() {
    outer(inner(value));
    a.b().c();
}
"#;

    let calls = adapter.extract_function_calls(source).unwrap();
    assert!(calls.len() >= 2);
}

#[test]
fn test_extract_identifiers() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
int globalVar = 10;
void func(int param) {
    int localVar = param + globalVar;
}
"#;

    let identifiers = adapter.extract_identifiers(source).unwrap();
    assert!(identifiers.contains(&"globalVar".to_string()));
    assert!(identifiers.contains(&"func".to_string()));
    assert!(identifiers.contains(&"param".to_string()));
    assert!(identifiers.contains(&"localVar".to_string()));
}

// ============================================================================
// Block Counting Tests
// ============================================================================

#[test]
fn test_count_distinct_blocks() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class Foo {
    void method() {
        if (true) {
            for (int i = 0; i < 10; i++) {
            }
        }
    }
};

void standalone() {
    while (true) {}
}
"#;

    let count = adapter.count_distinct_blocks(source).unwrap();
    assert!(count >= 5);
}

#[test]
fn test_count_blocks_with_switch() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
void process(int x) {
    switch (x) {
        case 1: break;
        case 2: break;
        default: break;
    }
}
"#;

    let count = adapter.count_distinct_blocks(source).unwrap();
    assert!(count >= 2); // function + switch
}

#[test]
fn test_count_blocks_with_try_catch() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
void risky() {
    try {
        throw 1;
    } catch (...) {
    }
}
"#;

    let count = adapter.count_distinct_blocks(source).unwrap();
    assert!(count >= 2); // function + try
}

// ============================================================================
// Edge Cases and Robustness Tests
// ============================================================================

#[test]
fn test_parse_empty_file() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = "";

    let index = adapter.parse_source(source, "empty.cpp").unwrap();
    assert_eq!(index.entities.len(), 0);
}

#[test]
fn test_parse_comments_only() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
// This is a comment
/* This is a
   multi-line comment */
"#;

    let index = adapter.parse_source(source, "comments.cpp").unwrap();
    assert_eq!(index.entities.len(), 0);
}

#[test]
fn test_parse_whitespace_only() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = "   \n\n\t\t\n   ";

    let index = adapter.parse_source(source, "whitespace.cpp").unwrap();
    assert_eq!(index.entities.len(), 0);
}

#[test]
fn test_parse_forward_declaration() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class ForwardDeclared;
struct AnotherForward;

class Defined {
    ForwardDeclared* ptr;
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    // Should at least find the Defined class
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert!(classes.len() >= 1);
}

#[test]
fn test_parse_typedef() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
typedef unsigned int uint;
typedef void (*FuncPtr)(int, int);
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    // Typedefs should be captured as Interface entities
    assert!(index.entities.len() >= 1);
}

#[test]
fn test_parse_using_alias() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
using StringVec = std::vector<std::string>;
using Callback = std::function<void(int)>;
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    // Using aliases should be captured
    assert!(index.entities.len() >= 1);
}

#[test]
fn test_parse_complex_return_type() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
std::vector<std::pair<int, std::string>> getData() {
    return {};
}

auto getAuto() -> std::unique_ptr<Widget> {
    return nullptr;
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let funcs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Function).collect();
    assert_eq!(funcs.len(), 2);
}

#[test]
fn test_parse_nested_class() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class Outer {
public:
    class Inner {
        void innerMethod();
    };

    void outerMethod();
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert_eq!(classes.len(), 2);
}

#[test]
fn test_parse_friend_declaration() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class Secret {
    friend class Accessor;
    friend void helper(Secret&);
private:
    int data;
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert_eq!(classes.len(), 1);
}

#[test]
fn test_parse_default_and_delete() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class NonCopyable {
public:
    NonCopyable() = default;
    NonCopyable(const NonCopyable&) = delete;
    NonCopyable& operator=(const NonCopyable&) = delete;
    ~NonCopyable() = default;
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert_eq!(classes.len(), 1);
}

#[test]
fn test_parse_noexcept_specifier() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
void safe() noexcept {}
void conditional() noexcept(true) {}
void throwing() noexcept(false) {}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let funcs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Function).collect();
    assert_eq!(funcs.len(), 3);
}

// ============================================================================
// Modern C++ Features Tests
// ============================================================================

#[test]
fn test_parse_auto_return_type() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
auto getValue() {
    return 42;
}

auto getRef() -> int& {
    static int x = 0;
    return x;
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let funcs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Function).collect();
    assert_eq!(funcs.len(), 2);
}

#[test]
fn test_parse_structured_binding() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
void useStructuredBinding() {
    auto [x, y] = std::make_pair(1, 2);
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let funcs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Function).collect();
    assert_eq!(funcs.len(), 1);
}

#[test]
fn test_parse_if_constexpr() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
template<typename T>
void process(T value) {
    if constexpr (std::is_integral_v<T>) {
        // integer handling
    } else {
        // other handling
    }
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    assert!(!index.entities.is_empty());
}

#[test]
fn test_parse_init_statement_in_if() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
void modern() {
    if (int x = getValue(); x > 0) {
        // use x
    }
}
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();
    let funcs: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Function).collect();
    assert_eq!(funcs.len(), 1);
}

// ============================================================================
// Location and Metadata Tests
// ============================================================================

#[test]
fn test_entity_location() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
void firstFunc() {}

class MyClass {};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    for entity in index.entities.values() {
        // All entities should have valid locations
        assert!(!entity.location.file_path.is_empty());
    }
}

#[test]
fn test_entity_metadata_exists() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class Test {
    void method() {}
};
"#;

    let index = adapter.parse_source(source, "test.cpp").unwrap();

    for entity in index.entities.values() {
        // All entities should have at least node_kind in metadata
        assert!(entity.metadata.contains_key("node_kind"));
    }
}

// ============================================================================
// LanguageAdapter Trait Tests
// ============================================================================

#[test]
fn test_language_name() {
    let adapter = CppAdapter::new().unwrap();
    assert_eq!(adapter.language_name(), "cpp");
}

#[test]
fn test_parse_tree() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = "int main() { return 0; }";

    let tree = adapter.parse_tree(source);
    assert!(tree.is_ok());
}

#[test]
fn test_adapter_creation() {
    let adapter = CppAdapter::new();
    assert!(adapter.is_ok());
}

// ============================================================================
// Large/Complex Code Tests
// ============================================================================

#[test]
fn test_parse_complex_class() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
template<typename T, typename Allocator = std::allocator<T>>
class ComplexContainer {
public:
    using value_type = T;
    using allocator_type = Allocator;
    using size_type = std::size_t;
    using iterator = T*;
    using const_iterator = const T*;

    ComplexContainer() = default;
    explicit ComplexContainer(size_type count) : data_(count) {}
    ComplexContainer(const ComplexContainer& other) = default;
    ComplexContainer(ComplexContainer&& other) noexcept = default;
    ~ComplexContainer() = default;

    ComplexContainer& operator=(const ComplexContainer&) = default;
    ComplexContainer& operator=(ComplexContainer&&) noexcept = default;

    T& operator[](size_type pos) { return data_[pos]; }
    const T& operator[](size_type pos) const { return data_[pos]; }

    iterator begin() noexcept { return data_.data(); }
    iterator end() noexcept { return data_.data() + data_.size(); }
    const_iterator begin() const noexcept { return data_.data(); }
    const_iterator end() const noexcept { return data_.data() + data_.size(); }

    bool empty() const noexcept { return data_.empty(); }
    size_type size() const noexcept { return data_.size(); }

    void push_back(const T& value) { data_.push_back(value); }
    void push_back(T&& value) { data_.push_back(std::move(value)); }

    template<typename... Args>
    void emplace_back(Args&&... args) {
        data_.emplace_back(std::forward<Args>(args)...);
    }

private:
    std::vector<T, Allocator> data_;
};
"#;

    let index = adapter.parse_source(source, "complex.hpp").unwrap();

    // Should find the class
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "ComplexContainer");

    // Should find multiple functions/methods
    let funcs: Vec<_> = index.entities.values()
        .filter(|e| e.kind == EntityKind::Function || e.kind == EntityKind::Method)
        .collect();
    assert!(funcs.len() > 5);
}

#[test]
fn test_parse_realistic_header() {
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
#ifndef MYLIB_WIDGET_H
#define MYLIB_WIDGET_H

#include <string>
#include <memory>
#include <vector>

namespace mylib {

class Widget;
using WidgetPtr = std::shared_ptr<Widget>;
using WidgetList = std::vector<WidgetPtr>;

enum class WidgetState {
    Idle,
    Active,
    Disabled
};

class Widget {
public:
    Widget();
    explicit Widget(const std::string& name);
    Widget(const Widget& other);
    Widget(Widget&& other) noexcept;
    virtual ~Widget();

    Widget& operator=(const Widget& other);
    Widget& operator=(Widget&& other) noexcept;

    const std::string& name() const noexcept { return name_; }
    void setName(const std::string& name) { name_ = name; }

    WidgetState state() const noexcept { return state_; }
    void setState(WidgetState state) { state_ = state; }

    virtual void update() {}
    virtual void draw() const {}

    void addChild(WidgetPtr child);
    void removeChild(const Widget* child);
    const WidgetList& children() const { return children_; }

protected:
    virtual void onStateChanged(WidgetState oldState, WidgetState newState) {}

private:
    std::string name_;
    WidgetState state_ = WidgetState::Idle;
    WidgetList children_;
    Widget* parent_ = nullptr;
};

} // namespace mylib

#endif // MYLIB_WIDGET_H
"#;

    let index = adapter.parse_source(source, "widget.h").unwrap();

    // Should find namespace, class, enum
    let namespaces: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Module).collect();
    assert_eq!(namespaces.len(), 1);

    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert_eq!(classes.len(), 1);

    let enums: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Enum).collect();
    assert_eq!(enums.len(), 1);

    // Should find many methods
    let total_entities = index.entities.len();
    assert!(total_entities > 10);
}

// ============================================================================
// Regression Tests
// ============================================================================

#[test]
fn test_regression_preprocessor_blocks() {
    // Ensure we extract entities from inside preprocessor blocks
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
#ifndef GUARD_H
#define GUARD_H

namespace detail {
    class Hidden {
        void secret();
    };
}

#endif
"#;

    let index = adapter.parse_source(source, "guarded.h").unwrap();

    // Must find the class inside the guard
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert_eq!(classes.len(), 1);
    assert!(classes[0].name.contains("Hidden"));
}

#[test]
fn test_regression_partial_parse_recovery() {
    // Ensure we recover entities from files with parse errors
    let mut adapter = CppAdapter::new().unwrap();
    let source = r#"
class ValidClass {
    void validMethod();
};

// Some invalid syntax that might cause parse errors
extern template class SomeTemplate<int>;

class AnotherValid {
    void anotherMethod();
};
"#;

    let index = adapter.parse_source(source, "partial.cpp").unwrap();

    // Should find at least some classes despite potential parse errors
    let classes: Vec<_> = index.entities.values().filter(|e| e.kind == EntityKind::Class).collect();
    assert!(classes.len() >= 1);
}
