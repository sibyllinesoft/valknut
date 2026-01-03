//! Clone verification and detection.
//!
//! This module provides clone-related verification:
//! - Clone detection algorithms
//! - APTED tree edit distance verification
//! - Coverage mapping for clone pairs

pub mod apted_verification;
pub mod clone_detection;
pub mod coverage_mapping;

pub use apted_verification::*;
pub use clone_detection::*;
pub use coverage_mapping::*;
