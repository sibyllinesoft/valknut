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

/// Normalize a directory path to a consistent format.
/// - Replaces backslashes with forward slashes
/// - Removes leading "./"
/// - Removes trailing "/"
/// - Returns "." for empty paths (root directory)
pub fn normalize_dir_path(path: &str) -> String {
    let normalized = path
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string();
    if normalized.is_empty() {
        ".".to_string()
    } else {
        normalized
    }
}
