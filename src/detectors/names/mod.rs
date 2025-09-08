//! Semantic naming analyzer using Qwen embeddings and behavior signature analysis.
//!
//! This module implements a sophisticated semantic naming analysis system that:
//! - Extracts behavior signatures from code (I/O patterns, mutations, async/sync, return types)
//! - Uses Qwen3-Embedding-0.6B model for semantic similarity analysis
//! - Applies deterministic naming rules based on observed effects
//! - Generates rename packs and contract mismatch packs
//! - Maintains project consistency through lexicon building

pub mod config;
pub mod analyzer;
pub mod generator;

// Re-export all public types and structs from config
pub use config::*;

// Re-export the main analyzer
pub use analyzer::SemanticNameAnalyzer;

// Re-export generator components for external use
pub use generator::{BehaviorExtractor, NameGenerator};