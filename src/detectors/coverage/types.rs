use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Coverage report format detection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CoverageFormat {
    CoveragePyXml,
    Lcov,
    Cobertura,
    JaCoCo,
    IstanbulJson,
    Unknown,
}

/// Represents a single line's coverage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineCoverage {
    pub line_number: usize,
    pub hits: usize,
    pub is_covered: bool,
}

/// Coverage information for an entire file
#[derive(Debug, Clone)]
pub struct FileCoverage {
    pub path: PathBuf,
    pub lines: Vec<LineCoverage>,
}

/// Represents an uncovered line span in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncoveredSpan {
    pub path: PathBuf,
    pub start: usize,
    pub end: usize,
    pub hits: Option<usize>,
}

/// Features computed for a coverage gap
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapFeatures {
    pub gap_loc: usize,
    pub cyclomatic_in_gap: f64,
    pub cognitive_in_gap: f64,
    pub fan_in_gap: usize,
    pub exports_touched: bool,
    pub dependency_centrality_file: f64,
    pub interface_surface: usize,
    pub docstring_or_comment_present: bool,
    pub exception_density_in_gap: f64,
}

/// Symbol information for gaps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapSymbol {
    pub kind: SymbolKind,
    pub name: String,
    pub signature: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Module,
}

/// Code snippet preview with context windows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetPreview {
    pub language: String,
    pub pre: Vec<String>,
    pub head: Vec<String>,
    pub tail: Vec<String>,
    pub post: Vec<String>,
    pub markers: GapMarkers,
    pub imports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapMarkers {
    pub start_line: usize,
    pub end_line: usize,
}

/// Value metrics for a coverage pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackValue {
    pub file_cov_gain: f64,
    pub repo_cov_gain_est: f64,
}

/// Effort estimation for a coverage pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackEffort {
    pub tests_to_write_est: usize,
    pub mocks_est: usize,
}

/// A collection of prioritized coverage gaps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoveragePack {
    pub kind: String,
    pub pack_id: String,
    pub path: PathBuf,
    pub file_info: FileInfo,
    pub gaps: Vec<CoverageGap>,
    pub value: PackValue,
    pub effort: PackEffort,
}

/// File-level coverage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub loc: usize,
    pub coverage_before: f64,
    pub coverage_after_if_filled: f64,
}

/// Represents a logical coverage gap with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageGap {
    pub path: PathBuf,
    pub span: UncoveredSpan,
    pub file_loc: usize,
    pub language: String,
    pub score: f64,
    pub features: GapFeatures,
    pub symbols: Vec<GapSymbol>,
    pub preview: SnippetPreview,
}

/// File-level metrics for scoring analysis
#[derive(Debug, Clone)]
pub struct FileMetrics {
    pub total_gap_loc: usize,
    pub avg_complexity: f64,
    pub centrality: f64,
    pub gap_count: usize,
}

/// Weights for gap scoring algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub size: f64,
    pub complexity: f64,
    pub fan_in: f64,
    pub exports: f64,
    pub centrality: f64,
    pub docs: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            size: 0.40,
            complexity: 0.20,
            fan_in: 0.15,
            exports: 0.10,
            centrality: 0.10,
            docs: 0.05,
        }
    }
}

// CoverageConfig is defined in `config.rs` and re-exported at the module level to avoid
// duplication. Keep feature-specific configuration there so detector types remain focused on
// analysis data structures.
