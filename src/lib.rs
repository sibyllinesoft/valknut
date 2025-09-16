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
//! │  Core Engine  │  Detectors  │  Language  │  I/O & Storage  │
//! │              │             │  Adapters  │                 │
//! │ • Scoring    │ • Graph     │ • Python   │ • Cache         │
//! │ • Bayesian   │ • LSH/Hash  │ • JS/TS    │ • Persistence   │
//! │ • Pipeline   │ • Structure │ • Rust     │ • Reports       │
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
//!         .with_scoring_enabled()
//!         .with_graph_analysis();
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
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![cfg_attr(docsrs, feature(doc_cfg))]

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

    pub mod bayesian;
    pub mod config;
    pub mod errors;
    pub mod featureset;
    pub mod file_utils;
    pub mod pipeline;
    pub mod scoring;
}

// Specialized detection algorithms
pub mod detectors {
    //! Specialized code analysis detectors.

    pub mod clone_detection;
    pub mod complexity;
    pub mod coverage;
    pub mod graph;
    pub mod lsh;
    pub mod names_simple;
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
    pub mod rust_lang;
    pub mod typescript;
}

// I/O, persistence, and reporting
pub mod io {
    //! I/O operations, caching, and result persistence.

    pub mod cache;
    pub mod persistence;
    pub mod reports;
}

// AI refactoring oracle
pub mod oracle;

// Live reachability analysis
pub mod live {
    //! Live reachability analysis for production call graphs.

    pub mod cli;
    pub mod collectors;
    pub mod community;
    pub mod graph;
    pub mod reports;
    pub mod scoring;
    pub mod storage;
    pub mod types;
}

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
pub use api::results::AnalysisResults;
pub use core::errors::{Result, ValknutError, ValknutResultExt};

#[cfg(test)]
mod test_coverage_integration;

// Feature-gated exports
#[cfg(feature = "database")]
pub mod database {
    //! Database integration for large-scale analysis.
    pub use crate::io::persistence::DatabaseBackend;
}

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

    /// Check if database integration is available
    pub const fn has_database() -> bool {
        cfg!(feature = "database")
    }
}
