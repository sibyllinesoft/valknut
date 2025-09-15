//! Language-specific parsing and AST processing modules.

pub mod common;
// Tree-sitter adapters
pub mod go;
pub mod javascript;
pub mod python;
pub mod rust_lang;
pub mod typescript;

// Re-export common types and traits for easier access
pub use common::{EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation};
