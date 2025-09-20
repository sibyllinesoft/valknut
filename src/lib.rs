//! # Valknut-RS: High-Performance Code Analysis Engine
//!
//! A Rust implementation of the valknut code analysis platform, designed for superior
//! performance and memory safety. This library provides comprehensive code analysis
//! capabilities including:
//!
//! - **Statistical Analysis**: Bayesian normalization and feature scoring
//! - **Graph Analysis**: Dependency graphs, centrality metrics, and cycle detection  
//! - **Similarity Detection**: LSH-based duplicate detection and MinHash signatures
//! - **Refactoring Analysis**: Code smell detection and refactoring opportunities
//! - **Multi-language Support**: Python, JavaScript, TypeScript, Rust, Go
//!
//! ## Performance Features
//!
//! - Zero-cost abstractions with compile-time optimizations
//! - SIMD-accelerated mathematical computations  
//! - Lock-free concurrent data structures
//! - Memory-efficient probabilistic algorithms
//! - Async-first design for I/O operations
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        API Layer                            │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Core Engine  │  Detectors  │  Language  │  I/O & Reports  │
//! │              │             │  Adapters  │                 │
//! │ • Scoring    │ • Graph     │ • Python   │ • Cache         │
//! │ • Bayesian   │ • LSH/Hash  │ • JS/TS    │ • Reports       │
//! │ • Pipeline   │ • Structure │ • Rust     │                 │
//! │ • Config     │ • Coverage  │ • Go       │                 │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use valknut_rs::{ValknutEngine, AnalysisConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = AnalysisConfig::default()
//!         .with_language("python")
//!         .enable_all_modules();
//!
//!     let mut engine = ValknutEngine::new(config).await?;
//!     let results = engine.analyze_directory("./src").await?;
//!     
//!     println!("Analysis completed: {} files processed", results.files_analyzed());
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(unsafe_code)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::suspicious)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// Additional allows for tests and examples
#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(test, allow(clippy::expect_used))]

// Memory allocator selection (mutually exclusive)
#[cfg(all(feature = "mimalloc", not(feature = "jemalloc")))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(feature = "jemalloc", not(feature = "mimalloc")))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

// Core analysis engine modules
pub mod core {
    //! Core analysis algorithms and data structures.

    pub mod ast_service;
    pub mod ast_utils;
    pub mod bayesian;
    pub mod config;
    pub mod dependency;
    pub mod errors;
    pub mod featureset;
    pub mod file_utils;
    pub mod pipeline;
    pub mod scoring;
}

// Specialized detection algorithms
pub mod detectors {
    //! Specialized code analysis detectors.

    pub mod complexity;
    pub mod coverage;
    pub mod graph;
    pub mod lsh;
    pub mod refactoring;
    pub mod structure;
}

// Language-specific AST adapters
pub mod lang {
    //! Language-specific parsing and AST processing.

    pub mod common;
    // Tree-sitter adapters
    pub mod go;
    pub mod javascript;
    pub mod python;
    pub mod registry;
    pub mod rust_lang;
    pub mod typescript;

    pub use common::{EntityKind, LanguageAdapter, ParseIndex, ParsedEntity, SourceLocation};
    pub use registry::{adapter_for_file, adapter_for_language, language_key_for_path};
}

// I/O, caching, and reporting
pub mod io {
    //! I/O operations, caching, and report generation.

    pub mod cache;
    pub mod reports;
}

// AI refactoring oracle
pub mod oracle;

// Public API and engine interface
pub mod api {
    //! High-level API and engine interface.

    pub mod config_types;
    pub mod engine;
    pub mod results;
}

// Re-export primary types for convenience
pub use api::config_types::AnalysisConfig;
pub use api::engine::ValknutEngine;
pub use crate::core::pipeline::AnalysisResults;
pub use core::errors::{Result, ValknutError, ValknutResultExt};

#[cfg(test)]
mod test_coverage_integration;

/// Library version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Build-time feature detection
pub mod features {
    //! Runtime feature detection.

    /// Check if SIMD acceleration is available
    pub const fn has_simd() -> bool {
        cfg!(feature = "simd")
    }

    /// Check if parallel processing is enabled
    pub const fn has_parallel() -> bool {
        cfg!(feature = "parallel")
    }
}
