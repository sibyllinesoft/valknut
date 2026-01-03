//! Language-specific documentation scanners.
//!
//! Each module provides scanning logic for detecting missing or
//! incomplete documentation in a specific programming language.

pub mod python;
pub mod rust;
pub mod typescript;

pub use python::scan_python;
pub use rust::scan_rust;
pub use typescript::scan_typescript;
