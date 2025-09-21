//! Language-specific parsing and AST processing modules.

pub mod common;
// Tree-sitter adapters
pub mod go;
pub mod javascript;
pub mod python;
pub mod registry;
pub mod rust_lang;
pub mod typescript;

// Re-export common types and traits for easier access
pub use common::{EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation};
pub use registry::{
    adapter_for_file, adapter_for_language, language_key_for_path,
    get_tree_sitter_language, detect_language_from_path, create_parser_for_language
};
