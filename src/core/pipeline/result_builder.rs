//! Legacy shim retained for coverage datasets.
//!
//! The real result conversion logic now lives in
//! `crate::core::pipeline::result_conversions`. This file remains so that
//! historical coverage reports referencing `result_builder.rs` still resolve
//! a concrete source file during integration tests.

// Re-export the public conversion helpers for discoverability when this
// module is imported directly.
pub use crate::core::pipeline::result_conversions::*;
