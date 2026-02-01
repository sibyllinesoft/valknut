//! Language-specific adapter implementations.
//!
//! This module contains adapters for parsing and analyzing code in
//! various programming languages using tree-sitter.

pub mod cpp;
pub mod go;
pub mod javascript;
pub mod python;
pub mod rust_lang;
pub mod typescript;

pub use cpp::CppAdapter;
pub use go::GoAdapter;
pub use javascript::JavaScriptAdapter;
pub use python::PythonAdapter;
pub use rust_lang::RustAdapter;
pub use typescript::TypeScriptAdapter;
