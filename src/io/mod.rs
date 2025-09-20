//! I/O, Caching, and Reporting Infrastructure
//!
//! This module provides comprehensive I/O capabilities for valknut, including
//! result caching and multi-format report generation.
//!
//! ## Key Components
//!
//! - **cache**: High-performance result caching to avoid redundant analysis
//! - **reports**: Multi-format report generation (HTML, JSON, Markdown, CSV)
//!
//! ## Report Formats
//!
//! The reporting system supports multiple output formats optimized for different use cases:
//! - **HTML**: Interactive reports with charts and drill-down capabilities
//! - **JSON/JSONL**: Machine-readable data for CI/CD integration
//! - **Markdown**: Human-readable reports for documentation
//! - **CSV**: Spreadsheet-compatible data for analysis
//! - **SonarQube**: Integration format for quality gates
//!
//! ## Usage
//!
//! ```rust,no_run
//! use valknut::io::reports::ReportGenerator;
//! use valknut::io::cache::ResultCache;
//!
//! // Generate interactive HTML report
//! let report = ReportGenerator::html().generate(&analysis_results)?;
//!
//! // Use result caching for performance
//! let cache = ResultCache::new("./cache");
//! let cached_result = cache.get_or_compute(file_hash, || analyze_file(path))?;
//! ```

pub mod cache;
pub mod reports;
