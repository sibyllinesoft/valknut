//! Semantic naming analyzer using Qwen embeddings and behavior signature analysis.
//!
//! This module implements a sophisticated semantic naming analysis system that:
//! - Extracts behavior signatures from code (I/O patterns, mutations, async/sync, return types)
//! - Uses Qwen3-Embedding-0.6B model for semantic similarity analysis
//! - Applies deterministic naming rules based on observed effects
//! - Generates rename packs and contract mismatch packs
//! - Maintains project consistency through lexicon building

mod names;

// Re-export all public items from the names module
pub use names::*;