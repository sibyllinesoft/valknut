//! Detection algorithms and feature extractors.

pub mod complexity;
pub mod graph;
pub mod lsh;
pub mod structure;
pub mod coverage;
pub mod refactoring;
// pub mod names; // Temporarily disabled for build - embedding-based version
pub mod names_simple; // Simplified rule-based version
pub mod embedding;
pub mod clone_detection;
pub mod boilerplate_learning;