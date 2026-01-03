//! Report generation has been split into separate modules:
//! - `markdown_report.rs` - Markdown report generation
//! - `html_report.rs` - HTML report generation
//! - `report_helpers.rs` - Shared helper functions
//!
//! This module is kept for backwards compatibility but re-exports from the new modules.

pub use super::html_report::generate_html_report;
pub use super::markdown_report::generate_markdown_report;
