//! Experimental and work-in-progress analysis components.
//!
//! Modules in this namespace are not yet production ready. They provide a place
//! to iterate on ambitious ideas without implying that they are part of the
//! stable analysis pipeline. Enable the `experimental` Cargo feature to access
//! them.

#[cfg(feature = "experimental")]
pub mod boilerplate_learning;

#[cfg(feature = "experimental")]
pub mod clone_detection;
