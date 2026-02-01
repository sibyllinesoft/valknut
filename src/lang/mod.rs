//! Language-specific parsing and AST processing modules.

pub mod adapters;
pub mod common;
pub mod registry;

// Re-export adapters for backward compatibility
pub use adapters::cpp;
pub use adapters::go;
pub use adapters::javascript;
pub use adapters::python;
pub use adapters::rust_lang;
pub use adapters::typescript;

// Re-export common types and traits for easier access
pub use common::{EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation};
pub use registry::{
    adapter_for_file, adapter_for_language, create_parser_for_language,
    detect_language_from_path, extension_is_supported, get_tree_sitter_language,
    language_key_for_path, registered_languages, LanguageInfo, LanguageStability,
};

// Re-export individual adapters
pub use adapters::{
    CppAdapter, GoAdapter, JavaScriptAdapter, PythonAdapter, RustAdapter, TypeScriptAdapter,
};
