//! Clone Denoising Integration Tests
//! 
//! Comprehensive test suite for the complete clone denoising system.
//! This integration test file includes all phase-specific tests and
//! end-to-end integration tests.

mod clone_denoising;
mod fixtures;

// Re-export all test modules for easier access
pub use clone_denoising::*;
pub use fixtures::clone_denoising_test_data::*;