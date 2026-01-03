//! Signature generation and representation for LSH.
//!
//! This module contains:
//! - MinHash signature types and operations
//! - Signature generation from shingles
//! - Shingle extraction from code
//! - Weighted signature analysis

pub mod generator;
pub mod shingles;
pub mod types;
pub mod weighted;

pub use generator::SignatureGenerator;
pub use shingles::{count_tokens, ShingleGenerator};
pub use types::MinHashSignature;
pub use weighted::{WeightedMinHashSignature, WeightedShingleAnalyzer, WeightedShingleStats};
