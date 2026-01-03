//! File discovery and pipeline services.
//!
//! This module provides:
//! - Git-aware file discovery
//! - Batched file reading
//! - Code dictionary management
//! - Stage orchestration services

pub mod code_dictionary;
pub mod file_discovery;
pub mod services;

pub use code_dictionary::*;
pub use file_discovery::*;
pub use services::*;
