//! Language-specific parsing and AST processing modules.

pub mod common;
// Tree-sitter adapters
pub mod python;
pub mod javascript;
pub mod typescript;
pub mod go;
pub mod rust_lang;

// Re-export common types and traits for easier access
pub use common::{
    EntityKind, ParsedEntity, ParseIndex, SourceLocation, LanguageAdapter
};