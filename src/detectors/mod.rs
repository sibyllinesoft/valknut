//! Detection Algorithms and Feature Extractors
//!
//! This module provides specialized analysis algorithms that form the core of valknut's
//! code quality assessment capabilities. Each submodule implements specific detection
//! strategies targeting different aspects of code quality and maintainability.
//!
//! ## Available Detectors
//!
//! - **complexity**: Cyclomatic and cognitive complexity analysis
//! - **structure**: Directory organization and architectural pattern detection
//! - **lsh**: Locality Sensitive Hashing for code similarity and clone detection
//! - **coverage**: Code coverage analysis and gap identification
//! - **refactoring**: Refactoring opportunity detection and ranking
//! - **graph**: Dependency analysis and architectural metrics (v1.1)
//! - **cohesion**: Semantic cohesion and doc-code alignment analysis
//!
//! Experimental concepts that are not yet production-ready should live on
//! feature branches rather than in this crate to keep the public surface
//! honest about supported capabilities.
//!
//! ## Usage
//!
//! Detectors are typically used through the analysis pipeline, but can also be
//! invoked directly for targeted analysis:
//!
//! ```rust,no_run
//! use valknut::detectors::complexity::ComplexityDetector;
//! use valknut::core::featureset::FeatureExtractor;
//!
//! let detector = ComplexityDetector::new();
//! let features = detector.extract_features(&source_file)?;
//! ```

pub mod bundled;
pub mod cohesion;
pub mod complexity;
pub mod coverage;
pub mod graph;
pub mod lsh;
pub mod refactoring;
pub mod structure;
