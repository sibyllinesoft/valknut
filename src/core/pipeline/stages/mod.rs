//! Analysis stage implementations.
//!
//! Each stage performs a specific type of analysis on the codebase:
//! - Complexity analysis
//! - Coverage gap detection
//! - Impact/dependency analysis
//! - LSH clone detection
//! - Refactoring opportunity detection
//! - Structure analysis

pub mod complexity_stage;
pub mod coverage_stage;
pub mod impact_stage;
pub mod lsh_stage;
pub mod refactoring_stage;
pub mod structure_stage;

pub use complexity_stage::*;
pub use coverage_stage::*;
pub use impact_stage::*;
pub use lsh_stage::*;
pub use refactoring_stage::*;
pub use structure_stage::*;
