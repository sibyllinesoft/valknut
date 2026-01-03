//! Health metrics and scoring.
//!
//! This module provides health-related analysis:
//! - Health tree computation
//! - Documentation health scoring
//! - Score conversions and normalization
//! - Suggestion generation

pub mod doc_health;
pub mod health_tree;
pub mod scoring_conversion;
pub mod suggestion_generator;

pub use doc_health::*;
pub use health_tree::*;
pub use scoring_conversion::*;
pub use suggestion_generator::*;
