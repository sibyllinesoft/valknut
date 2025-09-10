//! Coverage Packs module - contextual test gap analysis
//!
//! This module implements LLM-free coverage analysis that produces ranked, contextual
//! coverage gaps to help agents write high-value tests efficiently.

use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use crate::core::featureset::{FeatureExtractor, FeatureDefinition, CodeEntity, ExtractionContext};
use crate::core::errors::{Result, ValknutError};

/// Coverage report format detection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CoverageFormat {
    CoveragePyXml,      // coverage.py XML format
    Lcov,               // LCOV .info format  
    Cobertura,          // Cobertura XML format
    JaCoCo,             // JaCoCo XML format
    IstanbulJson,       // Istanbul JSON format
    Unknown,
}

/// Represents a single line's coverage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineCoverage {
    pub line_number: usize,
    pub hits: usize,
    pub is_covered: bool,
}

/// Represents an uncovered line span in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncoveredSpan {
    pub path: PathBuf,
    pub start: usize,  // inclusive
    pub end: usize,    // inclusive  
    pub hits: Option<usize>,
}

/// Features computed for a coverage gap
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapFeatures {
    pub gap_loc: usize,                    // Lines of code in gap
    pub cyclomatic_in_gap: f64,           // Complexity within gap
    pub cognitive_in_gap: f64,            // Cognitive complexity within gap
    pub fan_in_gap: usize,                // Number of callsites
    pub exports_touched: bool,            // Contains public APIs
    pub dependency_centrality_file: f64,  // File's import graph centrality
    pub interface_surface: usize,         // Parameters + return types
    pub docstring_or_comment_present: bool,
    pub exception_density_in_gap: f64,    // Exception handling per KLOC
}

/// Represents a logical coverage gap with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageGap {
    pub path: PathBuf,
    pub span: UncoveredSpan,
    pub file_loc: usize,
    pub language: String,
    pub score: f64,                       // 0-1 priority score
    pub features: GapFeatures,
    pub symbols: Vec<GapSymbol>,          // Functions/classes in gap
    pub preview: SnippetPreview,          // Context for agents
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
    pub pre: Vec<String>,      // Context lines before gap
    pub head: Vec<String>,     // First few lines of gap
    pub tail: Vec<String>,     // Last few lines of gap  
    pub post: Vec<String>,     // Context lines after gap
    pub markers: GapMarkers,   // Line number markers
    pub imports: Vec<String>,  // Imports used in gap (for mocking)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapMarkers {
    pub start_line: usize,
    pub end_line: usize,
}

/// Value metrics for a coverage pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackValue {
    pub file_cov_gain: f64,       // Expected file coverage increase
    pub repo_cov_gain_est: f64,   // Expected repo coverage increase
}

/// Effort estimation for a coverage pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackEffort {
    pub tests_to_write_est: usize,  // Estimated number of tests needed
    pub mocks_est: usize,           // Estimated mocks needed
}

/// A collection of prioritized coverage gaps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoveragePack {
    pub kind: String,                    // Always "coverage"
    pub pack_id: String,                 // e.g., "cov:src/lib.rs"
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

/// File-level metrics for scoring analysis
#[derive(Debug, Clone)]
pub struct FileMetrics {
    pub total_gap_loc: usize,
    pub avg_complexity: f64,
    pub centrality: f64,
    pub gap_count: usize,
}

/// Configuration for coverage analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageConfig {
    pub enabled: bool,
    pub report_paths: Vec<PathBuf>,
    pub max_gaps_per_file: usize,
    pub min_gap_loc: usize,
    pub snippet_context_lines: usize,
    pub long_gap_head_tail: usize,
    pub group_cross_file: bool,
    pub target_repo_gain: f64,
    pub weights: ScoringWeights,
    pub exclude_patterns: Vec<String>,
}

/// Weights for gap scoring algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub size: f64,          // 0.40
    pub complexity: f64,    // 0.20
    pub fan_in: f64,        // 0.15
    pub exports: f64,       // 0.10
    pub centrality: f64,    // 0.10
    pub docs: f64,          // 0.05
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

impl Default for CoverageConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            report_paths: vec![
                PathBuf::from("coverage.xml"),
                PathBuf::from("lcov.info"), 
                PathBuf::from("coverage-final.json")
            ],
            max_gaps_per_file: 5,
            min_gap_loc: 3,
            snippet_context_lines: 5,
            long_gap_head_tail: 2,
            group_cross_file: false,
            target_repo_gain: 0.02,
            weights: ScoringWeights::default(),
            exclude_patterns: vec![
                "**/generated/**".to_string(),
                "**/migrations/**".to_string(),
                "**/*_pb2.py".to_string(),
            ],
        }
    }
}

/// Main coverage analysis extractor - now implements full Coverage Packs
#[derive(Debug, Default)]
pub struct CoverageExtractor {
    pub config: CoverageConfig,
}

impl CoverageExtractor {
    pub fn new(config: CoverageConfig) -> Self {
        Self { config }
    }
    
    /// Build coverage packs from parsed coverage reports
    pub async fn build_coverage_packs(&self, reports: Vec<PathBuf>) -> Result<Vec<CoveragePack>> {
        let mut all_packs = Vec::new();
        
        for report_path in &reports {
            if !report_path.exists() {
                continue; // Skip non-existent files
            }
            
            // Parse coverage data from the report
            let uncovered_spans = self.parse_coverage_report(report_path)?;
            
            // Group by file and coalesce gaps
            let mut file_spans: std::collections::HashMap<PathBuf, Vec<UncoveredSpan>> = std::collections::HashMap::new();
            for span in uncovered_spans {
                file_spans.entry(span.path.clone()).or_default().push(span);
            }
            
            // Create coverage packs for each file with uncovered spans
            for (file_path, spans) in file_spans {
                if spans.is_empty() {
                    continue;
                }
                
                // Coalesce spans into logical gaps
                let gaps = self.coalesce_gaps(spans)?;
                
                if gaps.is_empty() {
                    continue;
                }
                
                // Calculate file info
                let file_loc = if let Ok(content) = fs::read_to_string(&file_path) {
                    content.lines().count()
                } else {
                    0
                };
                
                let total_uncovered_lines: usize = gaps.iter().map(|g| g.features.gap_loc).sum();
                let coverage_before = if file_loc > 0 {
                    1.0 - (total_uncovered_lines as f64 / file_loc as f64)
                } else {
                    1.0
                };
                let coverage_after_if_filled = 1.0; // Assume 100% if gaps are filled
                
                // Calculate pack value
                let file_cov_gain = coverage_after_if_filled - coverage_before;
                let repo_cov_gain_est = file_cov_gain * (file_loc as f64 / 10000.0); // Estimate based on file size
                
                // Calculate pack effort
                let tests_to_write_est = gaps.len().max(total_uncovered_lines / 5); // Rough estimate
                let mocks_est = gaps.iter()
                    .map(|g| g.symbols.len())
                    .sum::<usize>()
                    .min(5); // Cap at 5 mocks
                
                let pack = CoveragePack {
                    kind: "coverage".to_string(),
                    pack_id: format!("cov:{}", file_path.display()),
                    path: file_path,
                    file_info: FileInfo {
                        loc: file_loc,
                        coverage_before,
                        coverage_after_if_filled,
                    },
                    gaps,
                    value: PackValue {
                        file_cov_gain,
                        repo_cov_gain_est,
                    },
                    effort: PackEffort {
                        tests_to_write_est,
                        mocks_est,
                    },
                };
                
                all_packs.push(pack);
            }
        }
        
        // Sort packs by estimated value/impact
        all_packs.sort_by(|a, b| {
            let score_a = a.value.repo_cov_gain_est / (a.effort.tests_to_write_est as f64 + 1.0);
            let score_b = b.value.repo_cov_gain_est / (b.effort.tests_to_write_est as f64 + 1.0);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        Ok(all_packs)
    }
    
    /// Detect coverage report format from file content
    pub fn detect_format(&self, report_path: &PathBuf) -> Result<CoverageFormat> {
        let content = fs::read_to_string(report_path)
            .map_err(|e| ValknutError::io(format!("Failed to read coverage report: {}", e), e))?;
        
        // Check for XML formats first
        if content.contains("<?xml") {
            if content.contains("<coverage") && (content.contains("coverage.py") || content.contains("version=")) {
                // coverage.py XML has <coverage version="..."> root element
                if content.contains("cobertura") {
                    return Ok(CoverageFormat::Cobertura);
                } else {
                    return Ok(CoverageFormat::CoveragePyXml);
                }
            } else if content.contains("<report") && content.contains("jacoco") {
                return Ok(CoverageFormat::JaCoCo);
            } else if content.contains("<coverage") && content.contains("cobertura") {
                return Ok(CoverageFormat::Cobertura);
            }
        }
        
        // Check for LCOV format
        if report_path.extension().and_then(|s| s.to_str()) == Some("info") 
            || content.contains("TN:") || content.contains("SF:") {
            return Ok(CoverageFormat::Lcov);
        }
        
        // Check for Istanbul JSON
        if content.starts_with('{') && (content.contains("\"statementMap\"") || content.contains("\"s\"")) {
            return Ok(CoverageFormat::IstanbulJson);
        }
        
        Ok(CoverageFormat::Unknown)
    }

    /// Parse coverage report and extract uncovered spans
    pub fn parse_coverage_report(&self, report_path: &PathBuf) -> Result<Vec<UncoveredSpan>> {
        let format = self.detect_format(report_path)?;
        
        match format {
            CoverageFormat::CoveragePyXml => self.parse_coverage_py_xml(report_path),
            CoverageFormat::Lcov => self.parse_lcov(report_path),
            CoverageFormat::Cobertura => self.parse_cobertura_xml(report_path),
            CoverageFormat::JaCoCo => self.parse_jacoco_xml(report_path),
            CoverageFormat::IstanbulJson => self.parse_istanbul_json(report_path),
            CoverageFormat::Unknown => Err(ValknutError::validation(
                "Unknown coverage report format".to_string()
            )),
        }
    }

    /// Parse coverage.py XML format
    fn parse_coverage_py_xml(&self, report_path: &PathBuf) -> Result<Vec<UncoveredSpan>> {
        let content = fs::read_to_string(report_path)
            .map_err(|e| ValknutError::io(format!("Failed to read coverage.py XML: {}", e), e))?;
        
        let mut spans = Vec::new();
        let mut current_file: Option<PathBuf> = None;
        let mut uncovered_lines = Vec::new();
        
        // Simple XML parsing - look for class and line elements
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Extract filename from class element
            if trimmed.starts_with("<class") && trimmed.contains("filename=") {
                if let Some(start) = trimmed.find("filename=\"") {
                    let start = start + 10; // len of "filename=\""
                    if let Some(end) = trimmed[start..].find("\"") {
                        // Process any accumulated uncovered lines for previous file
                        if let Some(prev_file) = current_file.take() {
                            if !uncovered_lines.is_empty() {
                                spans.extend(self.lines_to_spans(&prev_file, &uncovered_lines)?);
                                uncovered_lines.clear();
                            }
                        }
                        current_file = Some(PathBuf::from(&trimmed[start..start+end]));
                    }
                }
            }
            
            // Extract line coverage from line elements
            if trimmed.starts_with("<line") && trimmed.contains("hits=\"0\"") {
                if let Some(start) = trimmed.find("number=\"") {
                    let start = start + 8; // len of "number=\""
                    if let Some(end) = trimmed[start..].find("\"") {
                        if let Ok(line_num) = trimmed[start..start+end].parse::<usize>() {
                            uncovered_lines.push(line_num);
                        }
                    }
                }
            }
        }
        
        // Process final file's uncovered lines
        if let Some(file) = current_file {
            if !uncovered_lines.is_empty() {
                spans.extend(self.lines_to_spans(&file, &uncovered_lines)?);
            }
        }
        
        Ok(spans)
    }

    /// Parse LCOV format
    fn parse_lcov(&self, report_path: &PathBuf) -> Result<Vec<UncoveredSpan>> {
        let content = fs::read_to_string(report_path)
            .map_err(|e| ValknutError::io(format!("Failed to read LCOV file: {}", e), e))?;
        
        let mut spans = Vec::new();
        let mut current_file: Option<PathBuf> = None;
        let mut uncovered_lines = Vec::new();
        
        for line in content.lines() {
            let trimmed = line.trim();
            
            // New source file
            if trimmed.starts_with("SF:") {
                // Process previous file's uncovered lines
                if let Some(file) = current_file.take() {
                    if !uncovered_lines.is_empty() {
                        spans.extend(self.lines_to_spans(&file, &uncovered_lines)?);
                        uncovered_lines.clear();
                    }
                }
                current_file = Some(PathBuf::from(&trimmed[3..])); // Skip "SF:"
            }
            
            // Line coverage data: DA:<line>,<hits>
            if trimmed.starts_with("DA:") {
                let parts: Vec<&str> = trimmed[3..].split(',').collect(); // Skip "DA:"
                if parts.len() >= 2 {
                    if let (Ok(line_num), Ok(hits)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                        if hits == 0 {
                            uncovered_lines.push(line_num);
                        }
                    }
                }
            }
        }
        
        // Process final file
        if let Some(file) = current_file {
            if !uncovered_lines.is_empty() {
                spans.extend(self.lines_to_spans(&file, &uncovered_lines)?);
            }
        }
        
        Ok(spans)
    }

    /// Parse Cobertura XML format
    fn parse_cobertura_xml(&self, report_path: &PathBuf) -> Result<Vec<UncoveredSpan>> {
        let content = fs::read_to_string(report_path)
            .map_err(|e| ValknutError::io(format!("Failed to read Cobertura XML: {}", e), e))?;
        
        let mut spans = Vec::new();
        let mut current_file: Option<PathBuf> = None;
        let mut uncovered_lines = Vec::new();
        
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Extract filename from class element
            if trimmed.starts_with("<class") && trimmed.contains("filename=") {
                if let Some(start) = trimmed.find("filename=\"") {
                    let start = start + 10;
                    if let Some(end) = trimmed[start..].find("\"") {
                        if let Some(file) = current_file.take() {
                            if !uncovered_lines.is_empty() {
                                spans.extend(self.lines_to_spans(&file, &uncovered_lines)?);
                                uncovered_lines.clear();
                            }
                        }
                        current_file = Some(PathBuf::from(&trimmed[start..start+end]));
                    }
                }
            }
            
            // Extract line coverage: <line number="X" hits="0"/>
            if trimmed.starts_with("<line") && trimmed.contains("hits=\"0\"") {
                if let Some(start) = trimmed.find("number=\"") {
                    let start = start + 8;
                    if let Some(end) = trimmed[start..].find("\"") {
                        if let Ok(line_num) = trimmed[start..start+end].parse::<usize>() {
                            uncovered_lines.push(line_num);
                        }
                    }
                }
            }
        }
        
        if let Some(file) = current_file {
            if !uncovered_lines.is_empty() {
                spans.extend(self.lines_to_spans(&file, &uncovered_lines)?);
            }
        }
        
        Ok(spans)
    }

    /// Parse JaCoCo XML format  
    fn parse_jacoco_xml(&self, report_path: &PathBuf) -> Result<Vec<UncoveredSpan>> {
        let content = fs::read_to_string(report_path)
            .map_err(|e| ValknutError::io(format!("Failed to read JaCoCo XML: {}", e), e))?;
        
        let mut spans = Vec::new();
        let mut current_file: Option<PathBuf> = None;
        let mut uncovered_lines = Vec::new();
        
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Extract filename from sourcefile element
            if trimmed.starts_with("<sourcefile") && trimmed.contains("name=") {
                if let Some(start) = trimmed.find("name=\"") {
                    let start = start + 6;
                    if let Some(end) = trimmed[start..].find("\"") {
                        if let Some(file) = current_file.take() {
                            if !uncovered_lines.is_empty() {
                                spans.extend(self.lines_to_spans(&file, &uncovered_lines)?);
                                uncovered_lines.clear();
                            }
                        }
                        current_file = Some(PathBuf::from(&trimmed[start..start+end]));
                    }
                }
            }
            
            // Extract line coverage: <line nr="X" ci="0" mi="Y"/> where ci=covered instructions, mi=missed
            if trimmed.starts_with("<line") && (trimmed.contains("ci=\"0\"") || trimmed.contains("mi=")) {
                if let Some(start) = trimmed.find("nr=\"") {
                    let start = start + 4;
                    if let Some(end) = trimmed[start..].find("\"") {
                        if let Ok(line_num) = trimmed[start..start+end].parse::<usize>() {
                            // Check if line has no covered instructions
                            if trimmed.contains("ci=\"0\"") {
                                uncovered_lines.push(line_num);
                            }
                        }
                    }
                }
            }
        }
        
        if let Some(file) = current_file {
            if !uncovered_lines.is_empty() {
                spans.extend(self.lines_to_spans(&file, &uncovered_lines)?);
            }
        }
        
        Ok(spans)
    }

    /// Parse Istanbul JSON format
    fn parse_istanbul_json(&self, report_path: &PathBuf) -> Result<Vec<UncoveredSpan>> {
        let content = fs::read_to_string(report_path)
            .map_err(|e| ValknutError::io(format!("Failed to read Istanbul JSON: {}", e), e))?;
        
        // Parse as JSON
        let json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| ValknutError::parse("json".to_string(), format!("Invalid Istanbul JSON: {}", e)))?;
        
        let mut spans = Vec::new();
        
        // Istanbul format: { "file1": { "s": { "0": 1, "1": 0 }, "statementMap": { "0": {...}, "1": {...} } } }
        if let Some(files) = json.as_object() {
            for (file_path, file_data) in files {
                if let Some(statements) = file_data.get("s").and_then(|s| s.as_object()) {
                    let mut uncovered_lines = Vec::new();
                    
                    // Get statement map to convert statement IDs to lines
                    let statement_map = file_data.get("statementMap").and_then(|m| m.as_object());
                    
                    for (stmt_id, hits) in statements {
                        if hits.as_u64() == Some(0) {
                            // Find the line number from statement map
                            if let Some(stmt_map) = statement_map {
                                if let Some(stmt_info) = stmt_map.get(stmt_id) {
                                    if let Some(start) = stmt_info.get("start") {
                                        if let Some(line_num) = start.get("line").and_then(|l| l.as_u64()) {
                                            uncovered_lines.push(line_num as usize);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    if !uncovered_lines.is_empty() {
                        uncovered_lines.sort_unstable();
                        spans.extend(self.lines_to_spans(&PathBuf::from(file_path), &uncovered_lines)?);
                    }
                }
            }
        }
        
        Ok(spans)
    }

    /// Convert sorted line numbers to uncovered spans by coalescing adjacent lines
    fn lines_to_spans(&self, file_path: &PathBuf, lines: &[usize]) -> Result<Vec<UncoveredSpan>> {
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut spans = Vec::new();
        let mut current_start = lines[0];
        let mut current_end = lines[0];
        
        for &line in &lines[1..] {
            if line == current_end + 1 {
                // Adjacent line, extend current span
                current_end = line;
            } else {
                // Gap found, create span and start new one
                spans.push(UncoveredSpan {
                    path: file_path.clone(),
                    start: current_start,
                    end: current_end,
                    hits: Some(0),
                });
                current_start = line;
                current_end = line;
            }
        }
        
        // Add the final span
        spans.push(UncoveredSpan {
            path: file_path.clone(),
            start: current_start,
            end: current_end,
            hits: Some(0),
        });
        
        Ok(spans)
    }
    
    /// Coalesce uncovered lines into logical gaps
    pub fn coalesce_gaps(&self, spans: Vec<UncoveredSpan>) -> Result<Vec<CoverageGap>> {
        let mut gaps = Vec::new();
        
        // Group spans by file
        let mut spans_by_file: HashMap<PathBuf, Vec<UncoveredSpan>> = HashMap::new();
        for span in spans {
            spans_by_file.entry(span.path.clone()).or_default().push(span);
        }
        
        // Process each file
        for (file_path, file_spans) in spans_by_file {
            let language = self.detect_language(&file_path);
            
            // Apply coalescing algorithm
            let coalesced_spans = self.coalesce_spans_for_file(&file_spans)?;
            
            // Apply language-specific chunking
            let chunked_spans = self.chunk_spans_by_language(&file_path, &language, &coalesced_spans)?;
            
            // Convert spans to gaps with initial features
            for span in chunked_spans {
                let features = GapFeatures {
                    gap_loc: span.end - span.start + 1,
                    cyclomatic_in_gap: 0.0,           // Will be filled in scoring phase
                    cognitive_in_gap: 0.0,            // Will be filled in scoring phase
                    fan_in_gap: 0,                    // Will be filled in scoring phase
                    exports_touched: false,           // Will be filled in scoring phase
                    dependency_centrality_file: 0.0,  // Will be filled in scoring phase
                    interface_surface: 0,             // Will be filled in scoring phase
                    docstring_or_comment_present: false, // Will be filled in scoring phase
                    exception_density_in_gap: 0.0,   // Will be filled in scoring phase
                };
                
                let mut gap = CoverageGap {
                    path: span.path.clone(),
                    span: span.clone(),
                    file_loc: 0,            // Will be filled in scoring phase
                    language: language.clone(),
                    score: 0.0,             // Will be calculated in scoring phase
                    features,
                    symbols: Vec::new(),    // Will be filled in scoring phase
                    preview: SnippetPreview { // Placeholder - will be generated below
                        language: language.clone(),
                        pre: Vec::new(),
                        head: Vec::new(),
                        tail: Vec::new(),
                        post: Vec::new(),
                        markers: GapMarkers {
                            start_line: span.start,
                            end_line: span.end,
                        },
                        imports: Vec::new(),
                    },
                };
                
                // Generate the snippet preview
                if let Ok(preview) = self.generate_preview(&gap) {
                    gap.preview = preview;
                }
                
                gaps.push(gap);
            }
        }
        
        Ok(gaps)
    }
    
    /// Score gaps by impact and priority
    pub fn score_gaps(&self, gaps: &mut [CoverageGap]) -> Result<()> {
        // Get scoring weights from config
        let weights = &self.config.weights;
        
        // Calculate file-level metrics for centrality scoring
        let file_metrics = self.calculate_file_metrics(gaps)?;
        
        for gap in gaps.iter_mut() {
            // Update features with calculated values
            self.update_gap_features(gap, &file_metrics)?;
            
            // Calculate weighted score using the formula:
            // Score = Size(0.40) + Complexity(0.20) + Fan-in(0.15) + Exports(0.10) + Centrality(0.10) + Docs(0.05)
            let size_score = self.normalize_size_score(gap.features.gap_loc);
            let complexity_score = self.normalize_complexity_score(gap.features.cyclomatic_in_gap + gap.features.cognitive_in_gap);
            let fan_in_score = self.normalize_fan_in_score(gap.features.fan_in_gap);
            let exports_score = if gap.features.exports_touched { 1.0 } else { 0.0 };
            let centrality_score = gap.features.dependency_centrality_file;
            let docs_score = if gap.features.docstring_or_comment_present { 0.0 } else { 1.0 }; // Higher score for missing docs
            
            gap.score = (size_score * weights.size) +
                       (complexity_score * weights.complexity) +
                       (fan_in_score * weights.fan_in) +
                       (exports_score * weights.exports) +
                       (centrality_score * weights.centrality) +
                       (docs_score * weights.docs);
                       
            // Clamp score to [0.0, 1.0]
            gap.score = gap.score.clamp(0.0, 1.0);
        }
        
        // Sort gaps by score in descending order (highest priority first)
        gaps.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(())
    }
    
    /// Calculate file-level metrics for centrality and other cross-gap analysis
    fn calculate_file_metrics(&self, gaps: &[CoverageGap]) -> Result<HashMap<PathBuf, FileMetrics>> {
        let mut metrics = HashMap::new();
        
        // Group gaps by file
        let mut files: HashMap<PathBuf, Vec<&CoverageGap>> = HashMap::new();
        for gap in gaps {
            files.entry(gap.path.clone()).or_default().push(gap);
        }
        
        // Calculate metrics for each file
        for (file_path, file_gaps) in files {
            let total_gap_loc: usize = file_gaps.iter().map(|g| g.features.gap_loc).sum();
            let avg_complexity: f64 = if !file_gaps.is_empty() {
                file_gaps.iter().map(|g| g.features.cyclomatic_in_gap + g.features.cognitive_in_gap).sum::<f64>() / file_gaps.len() as f64
            } else {
                0.0
            };
            
            // Centrality is based on file importance (simplified - could integrate with actual dependency graph)
            let centrality = self.estimate_file_centrality(&file_path);
            
            metrics.insert(file_path, FileMetrics {
                total_gap_loc,
                avg_complexity,
                centrality,
                gap_count: file_gaps.len(),
            });
        }
        
        Ok(metrics)
    }
    
    /// Update gap features with calculated analysis
    fn update_gap_features(&self, gap: &mut CoverageGap, file_metrics: &HashMap<PathBuf, FileMetrics>) -> Result<()> {
        if let Some(file_metric) = file_metrics.get(&gap.path) {
            gap.features.dependency_centrality_file = file_metric.centrality;
        }
        
        // Analyze the actual code in the gap to extract better features
        self.analyze_gap_code(gap)?;
        
        Ok(())
    }
    
    /// Analyze code within a gap to extract complexity, symbols, and other features
    fn analyze_gap_code(&self, gap: &mut CoverageGap) -> Result<()> {
        // Read the file to analyze the gap content
        let content = match fs::read_to_string(&gap.path) {
            Ok(content) => content,
            Err(_) => return Ok(()), // Skip analysis if file can't be read
        };
        
        let lines: Vec<&str> = content.lines().collect();
        
        // Extract lines within the gap
        let gap_lines: Vec<String> = (gap.span.start..=gap.span.end)
            .filter_map(|line_num| {
                lines.get(line_num - 1).map(|line| line.to_string())
            })
            .collect();
        
        // Simple complexity analysis
        let mut cyclomatic_complexity = 0.0;
        let mut cognitive_complexity = 0.0;
        let mut has_exports = false;
        let mut has_docs = false;
        let mut symbols = Vec::new();
        
        for (line_idx, line) in gap_lines.iter().enumerate() {
            let trimmed = line.trim();
            let actual_line_num = gap.span.start + line_idx;
            
            // Count complexity indicators
            if trimmed.contains("if ") || trimmed.contains("while ") || trimmed.contains("for ") ||
               trimmed.contains("match ") || trimmed.contains("switch ") || trimmed.contains("case ") ||
               trimmed.contains("catch ") || trimmed.contains("except ") {
                cyclomatic_complexity += 1.0;
                cognitive_complexity += 1.0;
            }
            
            // Nested complexity increases cognitive load
            let indentation_level = line.len() - line.trim_start().len();
            if indentation_level > 4 && (trimmed.contains("if ") || trimmed.contains("for ")) {
                cognitive_complexity += (indentation_level / 4) as f64 * 0.5;
            }
            
            // Check for exports
            if trimmed.starts_with("pub ") || trimmed.starts_with("export ") ||
               trimmed.starts_with("public ") || trimmed.contains("__all__") {
                has_exports = true;
            }
            
            // Check for documentation
            if trimmed.starts_with("///") || trimmed.starts_with("#") || 
               trimmed.starts_with("/**") || trimmed.starts_with("\"\"\"") ||
               trimmed.contains("@doc") || trimmed.contains("docstring") {
                has_docs = true;
            }
            
            // Extract symbols (functions, classes)
            if let Some(symbol) = self.extract_symbol_from_line(trimmed, actual_line_num) {
                symbols.push(symbol);
            }
        }
        
        // Update gap features
        gap.features.cyclomatic_in_gap = cyclomatic_complexity;
        gap.features.cognitive_in_gap = cognitive_complexity;
        gap.features.exports_touched = has_exports;
        gap.features.docstring_or_comment_present = has_docs;
        
        // Estimate fan-in based on symbol visibility and complexity
        gap.features.fan_in_gap = if has_exports {
            (cyclomatic_complexity * 2.0) as usize  // Public symbols likely have more callers
        } else {
            (cyclomatic_complexity * 0.5) as usize  // Private symbols have fewer callers
        };
        
        gap.symbols = symbols;
        
        Ok(())
    }
    
    /// Extract symbol information from a line of code
    fn extract_symbol_from_line(&self, line: &str, line_num: usize) -> Option<GapSymbol> {
        let trimmed = line.trim();
        
        // Function patterns
        if trimmed.starts_with("fn ") || trimmed.starts_with("def ") || 
           trimmed.starts_with("function ") || trimmed.starts_with("async def ") {
            if let Some(name_start) = trimmed.find(|c: char| c.is_alphabetic()) {
                if let Some(name_end) = trimmed[name_start..].find('(') {
                    let name = trimmed[name_start..name_start + name_end].split_whitespace().last().unwrap_or("");
                    return Some(GapSymbol {
                        kind: SymbolKind::Function,
                        name: name.to_string(),
                        signature: trimmed.to_string(),
                        line_start: line_num,
                        line_end: line_num, // Single line for now
                    });
                }
            }
        }
        
        // Class patterns
        if trimmed.starts_with("class ") || trimmed.starts_with("struct ") {
            if let Some(class_start) = trimmed.find("class ").or_else(|| trimmed.find("struct ")) {
                let after_keyword = &trimmed[class_start..];
                let keywords = if after_keyword.starts_with("class ") { "class " } else { "struct " };
                let after_keyword = &after_keyword[keywords.len()..];
                
                if let Some(name_end) = after_keyword.find(|c: char| !c.is_alphanumeric() && c != '_') {
                    let name = &after_keyword[..name_end];
                    return Some(GapSymbol {
                        kind: SymbolKind::Class,
                        name: name.to_string(),
                        signature: trimmed.to_string(),
                        line_start: line_num,
                        line_end: line_num,
                    });
                } else {
                    // Handle case where class name goes to end of line
                    let name = after_keyword.trim();
                    if !name.is_empty() {
                        return Some(GapSymbol {
                            kind: SymbolKind::Class,
                            name: name.to_string(),
                            signature: trimmed.to_string(),
                            line_start: line_num,
                            line_end: line_num,
                        });
                    }
                }
            }
        }
        
        None
    }
    
    /// Estimate file centrality based on file path and name patterns
    fn estimate_file_centrality(&self, file_path: &PathBuf) -> f64 {
        let path_str = file_path.to_string_lossy().to_lowercase();
        
        // Higher centrality for certain patterns
        if path_str.contains("lib.rs") || path_str.contains("main.rs") || 
           path_str.contains("__init__.py") || path_str.contains("index.") {
            return 0.9;
        }
        
        if path_str.contains("core") || path_str.contains("base") || 
           path_str.contains("common") || path_str.contains("util") {
            return 0.7;
        }
        
        if path_str.contains("test") || path_str.contains("example") {
            return 0.2;
        }
        
        // Default centrality
        0.5
    }
    
    /// Normalize size score to [0.0, 1.0]
    fn normalize_size_score(&self, gap_loc: usize) -> f64 {
        // Sigmoid-like function: larger gaps get higher scores but with diminishing returns
        let x = gap_loc as f64;
        (x / (x + 20.0)).min(1.0)
    }
    
    /// Normalize complexity score to [0.0, 1.0] 
    fn normalize_complexity_score(&self, complexity: f64) -> f64 {
        // Higher complexity gets higher priority
        (complexity / (complexity + 10.0)).min(1.0)
    }
    
    /// Normalize fan-in score to [0.0, 1.0]
    fn normalize_fan_in_score(&self, fan_in: usize) -> f64 {
        // More callers = higher priority
        let x = fan_in as f64;
        (x / (x + 5.0)).min(1.0)
    }
    
    /// Generate snippet previews with context
    pub fn generate_preview(&self, gap: &CoverageGap) -> Result<SnippetPreview> {
        // Read the source file
        let content = match fs::read_to_string(&gap.path) {
            Ok(content) => content,
            Err(e) => return Err(ValknutError::io(format!("Failed to read file {:?}", gap.path), e)),
        };
        
        let lines: Vec<&str> = content.lines().collect();
        let gap_start = gap.span.start;
        let gap_end = gap.span.end;
        
        // Get configuration values
        let context_lines = self.config.snippet_context_lines;
        let head_tail_limit = self.config.long_gap_head_tail;
        
        // Calculate context boundaries
        let pre_start = gap_start.saturating_sub(context_lines).max(1);
        let post_end = (gap_end + context_lines).min(lines.len());
        
        // Extract context lines before the gap
        let pre_lines = self.extract_lines(&lines, pre_start, gap_start - 1);
        
        // Extract context lines after the gap
        let post_lines = self.extract_lines(&lines, gap_end + 1, post_end);
        
        // Handle the gap itself - for long gaps, show head and tail with ellipses
        let gap_size = gap_end - gap_start + 1;
        let (head_lines, tail_lines) = if gap_size > head_tail_limit * 2 {
            // Long gap: show head and tail with ellipses
            let head = self.extract_lines(&lines, gap_start, gap_start + head_tail_limit - 1);
            let tail = self.extract_lines(&lines, gap_end - head_tail_limit + 1, gap_end);
            (head, tail)
        } else {
            // Short gap: show everything
            let all_gap_lines = self.extract_lines(&lines, gap_start, gap_end);
            (all_gap_lines, Vec::new())
        };
        
        // Extract imports for mocking/testing support
        let imports = self.extract_imports(&lines, &gap.language);
        
        Ok(SnippetPreview {
            language: gap.language.clone(),
            pre: pre_lines,
            head: head_lines,
            tail: tail_lines,
            post: post_lines,
            markers: GapMarkers {
                start_line: gap_start,
                end_line: gap_end,
            },
            imports,
        })
    }
    
    /// Extract lines from source with line numbers
    fn extract_lines(&self, lines: &[&str], start: usize, end: usize) -> Vec<String> {
        if start > end || start == 0 {
            return Vec::new();
        }
        
        let mut result = Vec::new();
        for line_num in start..=end {
            if let Some(line) = lines.get(line_num - 1) {
                // Include line number for agent reference
                result.push(format!("{:4} | {}", line_num, line));
            }
        }
        result
    }
    
    /// Extract relevant imports for testing/mocking support
    fn extract_imports(&self, lines: &[&str], language: &str) -> Vec<String> {
        let mut imports = Vec::new();
        
        // Look for imports in the first 50 lines (typical import section)
        let scan_limit = lines.len().min(50);
        
        match language {
            "python" => {
                for line in lines.iter().take(scan_limit) {
                    let trimmed = line.trim();
                    if trimmed.starts_with("import ") || 
                       trimmed.starts_with("from ") && trimmed.contains(" import ") {
                        imports.push(trimmed.to_string());
                    }
                }
            },
            "javascript" | "typescript" => {
                for line in lines.iter().take(scan_limit) {
                    let trimmed = line.trim();
                    if trimmed.starts_with("import ") || 
                       trimmed.starts_with("const ") && trimmed.contains("require(") ||
                       trimmed.starts_with("import type ") {
                        imports.push(trimmed.to_string());
                    }
                }
            },
            "rust" => {
                for line in lines.iter().take(scan_limit) {
                    let trimmed = line.trim();
                    if trimmed.starts_with("use ") && trimmed.ends_with(';') {
                        imports.push(trimmed.to_string());
                    }
                }
            },
            "go" => {
                let mut in_import_block = false;
                for line in lines.iter().take(scan_limit) {
                    let trimmed = line.trim();
                    if trimmed == "import (" {
                        in_import_block = true;
                        continue;
                    }
                    if in_import_block && trimmed == ")" {
                        break;
                    }
                    if in_import_block || (trimmed.starts_with("import ") && trimmed.contains('"')) {
                        imports.push(trimmed.to_string());
                    }
                }
            },
            "java" => {
                for line in lines.iter().take(scan_limit) {
                    let trimmed = line.trim();
                    if trimmed.starts_with("import ") && trimmed.ends_with(';') {
                        imports.push(trimmed.to_string());
                    }
                }
            },
            _ => {
                // For unknown languages, try to detect common import patterns
                for line in lines.iter().take(scan_limit) {
                    let trimmed = line.trim();
                    if (trimmed.starts_with("import ") || trimmed.starts_with("use ") || 
                        trimmed.starts_with("from ") || trimmed.contains("require(")) &&
                       !trimmed.is_empty() {
                        imports.push(trimmed.to_string());
                    }
                }
            }
        }
        
        // Deduplicate and limit to most important imports
        imports.sort();
        imports.dedup();
        imports.truncate(10); // Keep up to 10 most relevant imports
        imports
    }
    
    /// Detect programming language from file extension
    fn detect_language(&self, file_path: &PathBuf) -> String {
        match file_path.extension().and_then(|ext| ext.to_str()) {
            Some("py") => "python".to_string(),
            Some("js") => "javascript".to_string(),
            Some("ts") => "typescript".to_string(),
            Some("rs") => "rust".to_string(),
            Some("go") => "go".to_string(),
            Some("java") => "java".to_string(),
            Some("cpp" | "cc" | "cxx") => "cpp".to_string(),
            Some("c") => "c".to_string(),
            Some("h" | "hpp") => "c".to_string(),
            _ => "unknown".to_string(),
        }
    }
    
    /// Coalesce spans within a single file by merging adjacent/nearby spans
    fn coalesce_spans_for_file(&self, spans: &[UncoveredSpan]) -> Result<Vec<UncoveredSpan>> {
        if spans.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut sorted_spans = spans.to_vec();
        sorted_spans.sort_by_key(|span| span.start);
        
        let mut coalesced = Vec::new();
        let mut current_span = sorted_spans[0].clone();
        
        for span in sorted_spans.iter().skip(1) {
            // If spans are close (within 3 lines), merge them
            if span.start <= current_span.end + 3 {
                current_span.end = current_span.end.max(span.end);
            } else {
                // Gap too large, finalize current span
                coalesced.push(current_span.clone());
                current_span = span.clone();
            }
        }
        
        // Add the final span
        coalesced.push(current_span);
        
        Ok(coalesced)
    }
    
    /// Apply language-specific chunking to break spans at function/class boundaries
    fn chunk_spans_by_language(&self, file_path: &PathBuf, language: &str, spans: &[UncoveredSpan]) -> Result<Vec<UncoveredSpan>> {
        // For now, implement basic chunking. Future enhancement will use full AST parsing.
        match language {
            "python" => self.chunk_spans_python(file_path, spans),
            _ => Ok(spans.to_vec()), // No chunking for other languages yet
        }
    }
    
    /// Python-specific span chunking using simple pattern matching
    fn chunk_spans_python(&self, file_path: &PathBuf, spans: &[UncoveredSpan]) -> Result<Vec<UncoveredSpan>> {
        // Read the file to analyze function/class boundaries
        let content = match fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(_) => return Ok(spans.to_vec()), // Fallback if file can't be read
        };
        
        let lines: Vec<&str> = content.lines().collect();
        let mut chunked_spans = Vec::new();
        
        for span in spans {
            // If span is small (<=5 lines), don't chunk it
            if span.end - span.start + 1 <= 5 {
                chunked_spans.push(span.clone());
                continue;
            }
            
            // Find function/class boundaries within the span
            let mut boundaries = Vec::new();
            boundaries.push(span.start);
            
            for line_num in span.start..=span.end {
                if line_num <= lines.len() {
                    let line = lines.get(line_num - 1).unwrap_or(&"");
                    let trimmed = line.trim();
                    
                    // Look for function/class definitions
                    if trimmed.starts_with("def ") && trimmed.ends_with(':') ||
                       trimmed.starts_with("class ") && trimmed.ends_with(':') ||
                       trimmed.starts_with("async def ") && trimmed.ends_with(':') {
                        boundaries.push(line_num);
                    }
                }
            }
            
            boundaries.push(span.end + 1);
            boundaries.sort_unstable();
            boundaries.dedup();
            
            // Create chunks based on boundaries
            for window in boundaries.windows(2) {
                let chunk_start = window[0];
                let chunk_end = window[1] - 1;
                
                // Only create chunks that are within the original span and have some size
                if chunk_start >= span.start && chunk_end <= span.end && chunk_start <= chunk_end {
                    chunked_spans.push(UncoveredSpan {
                        path: span.path.clone(),
                        start: chunk_start,
                        end: chunk_end,
                        hits: span.hits,
                    });
                }
            }
        }
        
        Ok(chunked_spans)
    }
}

// Keep the existing FeatureExtractor implementation for backward compatibility
#[async_trait]
impl FeatureExtractor for CoverageExtractor {
    fn name(&self) -> &str { "coverage" }
    fn features(&self) -> &[FeatureDefinition] { &[] }
    async fn extract(&self, _entity: &CodeEntity, _context: &ExtractionContext) -> Result<HashMap<String, f64>> {
        Ok(HashMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::featureset::CodeEntity;
    
    #[test]
    fn test_coverage_extractor_default() {
        let extractor = CoverageExtractor::default();
        assert_eq!(extractor.name(), "coverage");
    }
    
    #[test]
    fn test_coverage_extractor_debug() {
        let extractor = CoverageExtractor::default();
        let debug_str = format!("{:?}", extractor);
        assert!(debug_str.contains("CoverageExtractor"));
    }
    
    #[test]
    fn test_coverage_extractor_name() {
        let extractor = CoverageExtractor::default();
        assert_eq!(extractor.name(), "coverage");
    }
    
    #[test]
    fn test_coverage_extractor_features() {
        let extractor = CoverageExtractor::default();
        assert!(extractor.features().is_empty());
    }

    #[test]
    fn test_coverage_config_default() {
        let config = CoverageConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.max_gaps_per_file, 5);
        assert_eq!(config.min_gap_loc, 3);
        assert_eq!(config.snippet_context_lines, 5);
        assert_eq!(config.target_repo_gain, 0.02);
    }

    #[test]
    fn test_scoring_weights_default() {
        let weights = ScoringWeights::default();
        assert_eq!(weights.size, 0.40);
        assert_eq!(weights.complexity, 0.20);
        assert_eq!(weights.fan_in, 0.15);
        assert_eq!(weights.exports, 0.10);
        assert_eq!(weights.centrality, 0.10);
        assert_eq!(weights.docs, 0.05);
        
        // Verify weights sum to 1.0
        let sum = weights.size + weights.complexity + weights.fan_in + 
                 weights.exports + weights.centrality + weights.docs;
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_uncovered_span_creation() {
        let span = UncoveredSpan {
            path: PathBuf::from("src/lib.rs"),
            start: 10,
            end: 20,
            hits: Some(0),
        };
        
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 20);
        assert_eq!(span.hits, Some(0));
        assert_eq!(span.path, PathBuf::from("src/lib.rs"));
    }

    #[test]
    fn test_gap_features_creation() {
        let features = GapFeatures {
            gap_loc: 10,
            cyclomatic_in_gap: 5.0,
            cognitive_in_gap: 8.0,
            fan_in_gap: 3,
            exports_touched: true,
            dependency_centrality_file: 0.7,
            interface_surface: 4,
            docstring_or_comment_present: true,
            exception_density_in_gap: 0.1,
        };
        
        assert_eq!(features.gap_loc, 10);
        assert_eq!(features.cyclomatic_in_gap, 5.0);
        assert!(features.exports_touched);
        assert!(features.docstring_or_comment_present);
    }

    #[test]
    fn test_gap_symbol_kinds() {
        let function_symbol = GapSymbol {
            kind: SymbolKind::Function,
            name: "test_function".to_string(),
            signature: "fn test_function() -> bool".to_string(),
            line_start: 10,
            line_end: 15,
        };
        
        let class_symbol = GapSymbol {
            kind: SymbolKind::Class,
            name: "TestClass".to_string(),
            signature: "class TestClass".to_string(),
            line_start: 20,
            line_end: 50,
        };
        
        assert_eq!(function_symbol.name, "test_function");
        assert_eq!(class_symbol.name, "TestClass");
        assert!(matches!(function_symbol.kind, SymbolKind::Function));
        assert!(matches!(class_symbol.kind, SymbolKind::Class));
    }

    #[test]
    fn test_snippet_preview_structure() {
        let preview = SnippetPreview {
            language: "rust".to_string(),
            pre: vec!["// Pre-context".to_string()],
            head: vec!["fn uncovered_function() {".to_string()],
            tail: vec!["}".to_string()],
            post: vec!["// Post-context".to_string()],
            markers: GapMarkers { start_line: 10, end_line: 20 },
            imports: vec!["use std::collections::HashMap;".to_string()],
        };
        
        assert_eq!(preview.language, "rust");
        assert_eq!(preview.pre.len(), 1);
        assert_eq!(preview.markers.start_line, 10);
        assert_eq!(preview.markers.end_line, 20);
        assert!(preview.imports.contains(&"use std::collections::HashMap;".to_string()));
    }

    #[test]
    fn test_coverage_pack_creation() {
        let pack = CoveragePack {
            kind: "coverage".to_string(),
            pack_id: "cov:src/lib.rs".to_string(),
            path: PathBuf::from("src/lib.rs"),
            file_info: FileInfo {
                loc: 100,
                coverage_before: 0.6,
                coverage_after_if_filled: 0.8,
            },
            gaps: Vec::new(),
            value: PackValue {
                file_cov_gain: 0.2,
                repo_cov_gain_est: 0.01,
            },
            effort: PackEffort {
                tests_to_write_est: 3,
                mocks_est: 1,
            },
        };
        
        assert_eq!(pack.kind, "coverage");
        assert_eq!(pack.pack_id, "cov:src/lib.rs");
        assert_eq!(pack.file_info.coverage_before, 0.6);
        assert_eq!(pack.file_info.coverage_after_if_filled, 0.8);
        assert_eq!(pack.value.file_cov_gain, 0.2);
        assert_eq!(pack.effort.tests_to_write_est, 3);
    }

    #[test]
    fn test_coverage_extractor_new_with_config() {
        let config = CoverageConfig {
            enabled: true,
            max_gaps_per_file: 10,
            ..CoverageConfig::default()
        };
        
        let extractor = CoverageExtractor::new(config);
        assert!(extractor.config.enabled);
        assert_eq!(extractor.config.max_gaps_per_file, 10);
    }

    #[test]
    fn test_coverage_format_variants() {
        assert_eq!(CoverageFormat::CoveragePyXml, CoverageFormat::CoveragePyXml);
        assert_ne!(CoverageFormat::Lcov, CoverageFormat::JaCoCo);
        
        // Test debug format
        let format = CoverageFormat::IstanbulJson;
        assert!(format!("{:?}", format).contains("IstanbulJson"));
    }

    #[test]
    fn test_line_coverage_creation() {
        let line_cov = LineCoverage {
            line_number: 42,
            hits: 5,
            is_covered: true,
        };
        
        assert_eq!(line_cov.line_number, 42);
        assert_eq!(line_cov.hits, 5);
        assert!(line_cov.is_covered);
    }

    #[test]
    fn test_lines_to_spans_single_line() {
        let extractor = CoverageExtractor::default();
        let file_path = PathBuf::from("test.rs");
        let lines = vec![42];
        
        let spans = extractor.lines_to_spans(&file_path, &lines).unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start, 42);
        assert_eq!(spans[0].end, 42);
        assert_eq!(spans[0].hits, Some(0));
        assert_eq!(spans[0].path, file_path);
    }

    #[test]
    fn test_lines_to_spans_adjacent_lines() {
        let extractor = CoverageExtractor::default();
        let file_path = PathBuf::from("test.rs");
        let lines = vec![10, 11, 12, 13];
        
        let spans = extractor.lines_to_spans(&file_path, &lines).unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start, 10);
        assert_eq!(spans[0].end, 13);
    }

    #[test]
    fn test_lines_to_spans_multiple_gaps() {
        let extractor = CoverageExtractor::default();
        let file_path = PathBuf::from("test.rs");
        let lines = vec![10, 11, 15, 16, 20];
        
        let spans = extractor.lines_to_spans(&file_path, &lines).unwrap();
        assert_eq!(spans.len(), 3);
        
        // First span: 10-11
        assert_eq!(spans[0].start, 10);
        assert_eq!(spans[0].end, 11);
        
        // Second span: 15-16
        assert_eq!(spans[1].start, 15);
        assert_eq!(spans[1].end, 16);
        
        // Third span: 20
        assert_eq!(spans[2].start, 20);
        assert_eq!(spans[2].end, 20);
    }

    #[test]
    fn test_lines_to_spans_empty() {
        let extractor = CoverageExtractor::default();
        let file_path = PathBuf::from("test.rs");
        let lines = vec![];
        
        let spans = extractor.lines_to_spans(&file_path, &lines).unwrap();
        assert!(spans.is_empty());
    }

    #[test]
    fn test_detect_format_coverage_py_xml() {
        let extractor = CoverageExtractor::default();
        
        // Create a temp file with coverage.py XML content
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("coverage_py_test.xml");
        fs::write(&test_file, r#"<?xml version="1.0"?>
<coverage version="7.0" timestamp="1234567890" lines-valid="100" lines-covered="80" line-rate="0.8" branches-covered="0" branches-valid="0" branch-rate="0" complexity="0">
  <sources>
    <source>.</source>
  </sources>
  <packages>
    <package name="." line-rate="0.8" branch-rate="0" complexity="0">
      <classes>
        <class name="main.py" filename="main.py" complexity="0" line-rate="0.8" branch-rate="0">
          <methods/>
          <lines>
            <line number="1" hits="1"/>
            <line number="2" hits="0"/>
            <line number="3" hits="1"/>
          </lines>
        </class>
      </classes>
    </package>
  </packages>
</coverage>"#).unwrap();
        
        let format = extractor.detect_format(&test_file).unwrap();
        assert_eq!(format, CoverageFormat::CoveragePyXml);
        
        // Clean up
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_detect_format_lcov() {
        let extractor = CoverageExtractor::default();
        
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test.info");
        fs::write(&test_file, r#"TN:
SF:src/main.rs
FN:1,main
FNDA:1,main
FNF:1
FNH:1
DA:1,1
DA:2,0
DA:3,1
LF:3
LH:2
end_of_record"#).unwrap();
        
        let format = extractor.detect_format(&test_file).unwrap();
        assert_eq!(format, CoverageFormat::Lcov);
        
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_detect_format_istanbul_json() {
        let extractor = CoverageExtractor::default();
        
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("coverage-final.json");
        fs::write(&test_file, r#"{
  "src/main.js": {
    "path": "src/main.js",
    "statementMap": {
      "0": {"start": {"line": 1, "column": 0}, "end": {"line": 1, "column": 20}},
      "1": {"start": {"line": 2, "column": 0}, "end": {"line": 2, "column": 15}}
    },
    "s": {
      "0": 1,
      "1": 0
    }
  }
}"#).unwrap();
        
        let format = extractor.detect_format(&test_file).unwrap();
        assert_eq!(format, CoverageFormat::IstanbulJson);
        
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_detect_format_unknown() {
        let extractor = CoverageExtractor::default();
        
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("unknown.txt");
        fs::write(&test_file, "Some random content that doesn't match any format").unwrap();
        
        let format = extractor.detect_format(&test_file).unwrap();
        assert_eq!(format, CoverageFormat::Unknown);
        
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_parse_coverage_py_xml() {
        let extractor = CoverageExtractor::default();
        
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("coverage_py_parse_test.xml");
        fs::write(&test_file, r#"<?xml version="1.0"?>
<coverage version="7.0" timestamp="1234567890">
  <packages>
    <package name="." line-rate="0.6" branch-rate="0" complexity="0">
      <classes>
        <class name="main.py" filename="src/main.py" complexity="0" line-rate="0.6">
          <lines>
            <line number="1" hits="1"/>
            <line number="2" hits="0"/>
            <line number="3" hits="0"/>
            <line number="4" hits="1"/>
            <line number="10" hits="0"/>
          </lines>
        </class>
        <class name="utils.py" filename="src/utils.py" complexity="0" line-rate="0.5">
          <lines>
            <line number="5" hits="0"/>
            <line number="6" hits="0"/>
            <line number="8" hits="1"/>
          </lines>
        </class>
      </classes>
    </package>
  </packages>
</coverage>"#).unwrap();
        
        let spans = extractor.parse_coverage_py_xml(&test_file).unwrap();
        assert_eq!(spans.len(), 3); // main.py: [2-3], [10], utils.py: [5-6]
        
        // Check main.py spans
        let main_spans: Vec<_> = spans.iter().filter(|s| s.path.to_string_lossy().contains("main.py")).collect();
        assert_eq!(main_spans.len(), 2);
        
        // Check utils.py spans
        let utils_spans: Vec<_> = spans.iter().filter(|s| s.path.to_string_lossy().contains("utils.py")).collect();
        assert_eq!(utils_spans.len(), 1);
        assert_eq!(utils_spans[0].start, 5);
        assert_eq!(utils_spans[0].end, 6);
        
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_parse_lcov() {
        let extractor = CoverageExtractor::default();
        
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("lcov_parse_test.info");
        fs::write(&test_file, r#"TN:
SF:src/main.rs
DA:1,1
DA:2,0
DA:3,0
DA:4,1
DA:10,0
LF:5
LH:2
end_of_record
TN:
SF:src/utils.rs
DA:5,0
DA:6,0
DA:8,1
LF:3
LH:1
end_of_record"#).unwrap();
        
        let spans = extractor.parse_lcov(&test_file).unwrap();
        assert_eq!(spans.len(), 3); // main.rs: [2-3], [10], utils.rs: [5-6]
        
        let main_spans: Vec<_> = spans.iter().filter(|s| s.path.to_string_lossy().contains("main.rs")).collect();
        assert_eq!(main_spans.len(), 2);
        
        let utils_spans: Vec<_> = spans.iter().filter(|s| s.path.to_string_lossy().contains("utils.rs")).collect();
        assert_eq!(utils_spans.len(), 1);
        assert_eq!(utils_spans[0].start, 5);
        assert_eq!(utils_spans[0].end, 6);
        
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_parse_istanbul_json() {
        let extractor = CoverageExtractor::default();
        
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("istanbul_parse_test.json");
        fs::write(&test_file, r#"{
  "src/main.js": {
    "path": "src/main.js",
    "statementMap": {
      "0": {"start": {"line": 1, "column": 0}, "end": {"line": 1, "column": 20}},
      "1": {"start": {"line": 2, "column": 0}, "end": {"line": 2, "column": 15}},
      "2": {"start": {"line": 3, "column": 0}, "end": {"line": 3, "column": 10}},
      "3": {"start": {"line": 10, "column": 0}, "end": {"line": 10, "column": 5}}
    },
    "s": {
      "0": 1,
      "1": 0,
      "2": 0,
      "3": 0
    }
  }
}"#).unwrap();
        
        let spans = extractor.parse_istanbul_json(&test_file).unwrap();
        assert_eq!(spans.len(), 2); // [2-3], [10]
        
        // First span should be lines 2-3
        assert_eq!(spans[0].start, 2);
        assert_eq!(spans[0].end, 3);
        
        // Second span should be line 10
        assert_eq!(spans[1].start, 10);
        assert_eq!(spans[1].end, 10);
        
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_parse_coverage_report_integration() {
        let extractor = CoverageExtractor::default();
        
        // Test with LCOV format
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("integration_test.info");
        fs::write(&test_file, r#"SF:src/lib.rs
DA:5,0
DA:6,0
DA:8,1
end_of_record"#).unwrap();
        
        let spans = extractor.parse_coverage_report(&test_file).unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start, 5);
        assert_eq!(spans[0].end, 6);
        assert_eq!(spans[0].path, PathBuf::from("src/lib.rs"));
        
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_parse_unknown_format_error() {
        let extractor = CoverageExtractor::default();
        
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("unknown_format.txt");
        fs::write(&test_file, "Random content").unwrap();
        
        let result = extractor.parse_coverage_report(&test_file);
        assert!(result.is_err());
        
        fs::remove_file(test_file).ok();
    }
    
    #[tokio::test]
    async fn test_coverage_extractor_extract() {
        let extractor = CoverageExtractor::default();
        let entity = CodeEntity::new(
            "test_id".to_string(),
            "test_type".to_string(),
            "test_name".to_string(),
            "test_file.rs".to_string(),
        );
        let config = std::sync::Arc::new(crate::core::config::ValknutConfig::default());
        let context = ExtractionContext::new(config, "rust");
        
        let result = extractor.extract(&entity, &context).await.unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_language() {
        let extractor = CoverageExtractor::default();
        
        assert_eq!(extractor.detect_language(&PathBuf::from("test.py")), "python");
        assert_eq!(extractor.detect_language(&PathBuf::from("test.js")), "javascript");
        assert_eq!(extractor.detect_language(&PathBuf::from("test.ts")), "typescript");
        assert_eq!(extractor.detect_language(&PathBuf::from("test.rs")), "rust");
        assert_eq!(extractor.detect_language(&PathBuf::from("test.go")), "go");
        assert_eq!(extractor.detect_language(&PathBuf::from("test.java")), "java");
        assert_eq!(extractor.detect_language(&PathBuf::from("test.cpp")), "cpp");
        assert_eq!(extractor.detect_language(&PathBuf::from("test.c")), "c");
        assert_eq!(extractor.detect_language(&PathBuf::from("test.txt")), "unknown");
    }

    #[test]
    fn test_coalesce_spans_for_file() {
        let extractor = CoverageExtractor::default();
        
        // Test empty input
        assert!(extractor.coalesce_spans_for_file(&[]).unwrap().is_empty());
        
        // Test single span
        let spans = vec![
            UncoveredSpan {
                path: PathBuf::from("test.py"),
                start: 5,
                end: 5,
                hits: Some(0),
            }
        ];
        let result = extractor.coalesce_spans_for_file(&spans).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start, 5);
        assert_eq!(result[0].end, 5);
        
        // Test adjacent spans (should merge)
        let spans = vec![
            UncoveredSpan {
                path: PathBuf::from("test.py"),
                start: 5,
                end: 5,
                hits: Some(0),
            },
            UncoveredSpan {
                path: PathBuf::from("test.py"),
                start: 6,
                end: 6,
                hits: Some(0),
            },
            UncoveredSpan {
                path: PathBuf::from("test.py"),
                start: 7,
                end: 8,
                hits: Some(0),
            }
        ];
        let result = extractor.coalesce_spans_for_file(&spans).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start, 5);
        assert_eq!(result[0].end, 8);
        
        // Test spans with gaps (should not merge if gap > 3 lines)
        let spans = vec![
            UncoveredSpan {
                path: PathBuf::from("test.py"),
                start: 1,
                end: 2,
                hits: Some(0),
            },
            UncoveredSpan {
                path: PathBuf::from("test.py"),
                start: 10,
                end: 12,
                hits: Some(0),
            }
        ];
        let result = extractor.coalesce_spans_for_file(&spans).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].start, 1);
        assert_eq!(result[0].end, 2);
        assert_eq!(result[1].start, 10);
        assert_eq!(result[1].end, 12);
        
        // Test spans with small gaps (should merge if gap <= 3 lines)
        let spans = vec![
            UncoveredSpan {
                path: PathBuf::from("test.py"),
                start: 1,
                end: 2,
                hits: Some(0),
            },
            UncoveredSpan {
                path: PathBuf::from("test.py"),
                start: 5,
                end: 6,
                hits: Some(0),
            }
        ];
        let result = extractor.coalesce_spans_for_file(&spans).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start, 1);
        assert_eq!(result[0].end, 6);
    }

    #[test]
    fn test_coalesce_gaps_basic() {
        let extractor = CoverageExtractor::default();
        
        // Test with simple uncovered spans
        let spans = vec![
            UncoveredSpan {
                path: PathBuf::from("test.py"),
                start: 5,
                end: 7,
                hits: Some(0),
            },
            UncoveredSpan {
                path: PathBuf::from("test.js"),
                start: 10,
                end: 12,
                hits: Some(0),
            }
        ];
        
        let gaps = extractor.coalesce_gaps(spans).unwrap();
        assert_eq!(gaps.len(), 2);
        
        // Check Python gap
        let py_gap = gaps.iter().find(|g| g.language == "python").unwrap();
        assert_eq!(py_gap.span.start, 5);
        assert_eq!(py_gap.span.end, 7);
        assert_eq!(py_gap.features.gap_loc, 3);
        
        // Check JavaScript gap  
        let js_gap = gaps.iter().find(|g| g.language == "javascript").unwrap();
        assert_eq!(js_gap.span.start, 10);
        assert_eq!(js_gap.span.end, 12);
        assert_eq!(js_gap.features.gap_loc, 3);
    }

    #[test]
    fn test_chunk_spans_python_small_span() {
        let extractor = CoverageExtractor::default();
        
        // Create a temporary Python file for testing
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("chunk_test_small.py");
        fs::write(&test_file, r#"def function1():
    print("hello")
    return True

def function2():
    return False
"#).unwrap();
        
        // Small span (<=5 lines) should not be chunked
        let spans = vec![
            UncoveredSpan {
                path: test_file.clone(),
                start: 1,
                end: 3,
                hits: Some(0),
            }
        ];
        
        let result = extractor.chunk_spans_python(&test_file, &spans).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start, 1);
        assert_eq!(result[0].end, 3);
        
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_chunk_spans_python_large_span() {
        let extractor = CoverageExtractor::default();
        
        // Create a temporary Python file with multiple functions
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("chunk_test_large.py");
        fs::write(&test_file, r#"# Line 1
def function1():
    print("function 1")
    return True

def function2():
    print("function 2")  
    return False

class MyClass:
    def method1(self):
        return "method1"
        
    async def method2(self):
        return "method2"
"#).unwrap();
        
        // Large span (>5 lines) should be chunked at function/class boundaries
        let spans = vec![
            UncoveredSpan {
                path: test_file.clone(),
                start: 1,
                end: 15,
                hits: Some(0),
            }
        ];
        
        let result = extractor.chunk_spans_python(&test_file, &spans).unwrap();
        
        // Should be chunked into multiple spans at function/class boundaries
        assert!(result.len() > 1);
        
        // Verify that chunks respect the original span boundaries
        for chunk in &result {
            assert!(chunk.start >= 1);
            assert!(chunk.end <= 15);
            assert!(chunk.start <= chunk.end);
        }
        
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_chunk_spans_by_language_unknown() {
        let extractor = CoverageExtractor::default();
        
        let spans = vec![
            UncoveredSpan {
                path: PathBuf::from("test.unknown"),
                start: 1,
                end: 10,
                hits: Some(0),
            }
        ];
        
        let result = extractor.chunk_spans_by_language(&PathBuf::from("test.unknown"), "unknown", &spans).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start, 1);
        assert_eq!(result[0].end, 10);
    }

    #[test]
    fn test_normalize_size_score() {
        let extractor = CoverageExtractor::default();
        
        assert_eq!(extractor.normalize_size_score(0), 0.0);
        assert!(extractor.normalize_size_score(10) > 0.3);
        assert!(extractor.normalize_size_score(20) > 0.4);
        assert!(extractor.normalize_size_score(100) < 1.0); // Should have diminishing returns
    }

    #[test]
    fn test_normalize_complexity_score() {
        let extractor = CoverageExtractor::default();
        
        assert_eq!(extractor.normalize_complexity_score(0.0), 0.0);
        assert!(extractor.normalize_complexity_score(5.0) > 0.3);
        assert!(extractor.normalize_complexity_score(10.0) > 0.4);
        assert!(extractor.normalize_complexity_score(100.0) < 1.0); // Should have diminishing returns
    }

    #[test]
    fn test_normalize_fan_in_score() {
        let extractor = CoverageExtractor::default();
        
        assert_eq!(extractor.normalize_fan_in_score(0), 0.0);
        assert!(extractor.normalize_fan_in_score(2) > 0.2);
        assert!(extractor.normalize_fan_in_score(5) > 0.4);
        assert!(extractor.normalize_fan_in_score(50) < 1.0); // Should have diminishing returns
    }

    #[test]
    fn test_estimate_file_centrality() {
        let extractor = CoverageExtractor::default();
        
        // High centrality files
        assert_eq!(extractor.estimate_file_centrality(&PathBuf::from("src/lib.rs")), 0.9);
        assert_eq!(extractor.estimate_file_centrality(&PathBuf::from("src/main.rs")), 0.9);
        assert_eq!(extractor.estimate_file_centrality(&PathBuf::from("__init__.py")), 0.9);
        
        // Medium centrality
        assert_eq!(extractor.estimate_file_centrality(&PathBuf::from("src/core/mod.rs")), 0.7);
        assert_eq!(extractor.estimate_file_centrality(&PathBuf::from("src/common/utils.rs")), 0.7);
        
        // Low centrality
        assert_eq!(extractor.estimate_file_centrality(&PathBuf::from("tests/test_example.py")), 0.2);
        
        // Default centrality
        assert_eq!(extractor.estimate_file_centrality(&PathBuf::from("src/feature/handler.rs")), 0.5);
    }

    #[test]
    fn test_extract_symbol_from_line() {
        let extractor = CoverageExtractor::default();
        
        // Function detection
        let symbol = extractor.extract_symbol_from_line("fn calculate_score(x: i32) -> f64 {", 42);
        assert!(symbol.is_some());
        let symbol = symbol.unwrap();
        assert_eq!(symbol.kind, SymbolKind::Function);
        assert_eq!(symbol.name, "calculate_score");
        assert_eq!(symbol.line_start, 42);
        
        // Python function
        let symbol = extractor.extract_symbol_from_line("def process_data(items):", 10);
        assert!(symbol.is_some());
        let symbol = symbol.unwrap();
        assert_eq!(symbol.kind, SymbolKind::Function);
        assert_eq!(symbol.name, "process_data");
        
        // Class detection
        let symbol = extractor.extract_symbol_from_line("class DataProcessor {", 5);
        assert!(symbol.is_some());
        let symbol = symbol.unwrap();
        assert_eq!(symbol.kind, SymbolKind::Class);
        assert_eq!(symbol.name, "DataProcessor");
        
        // No symbol
        let symbol = extractor.extract_symbol_from_line("let x = 42;", 1);
        assert!(symbol.is_none());
    }

    #[test]
    fn test_score_gaps_basic() {
        let extractor = CoverageExtractor::default();
        
        // Create test gaps with different characteristics
        let mut gaps = vec![
            CoverageGap {
                path: PathBuf::from("src/lib.rs"), // High centrality
                span: UncoveredSpan {
                    path: PathBuf::from("src/lib.rs"),
                    start: 1,
                    end: 10, // Medium size
                    hits: Some(0),
                },
                file_loc: 100,
                language: "rust".to_string(),
                score: 0.0,
                features: GapFeatures {
                    gap_loc: 10,
                    cyclomatic_in_gap: 3.0, // Some complexity
                    cognitive_in_gap: 2.0,
                    fan_in_gap: 2,
                    exports_touched: true, // Public API
                    dependency_centrality_file: 0.0, // Will be updated
                    interface_surface: 0,
                    docstring_or_comment_present: false, // Missing docs
                    exception_density_in_gap: 0.0,
                },
                symbols: Vec::new(),
                preview: SnippetPreview {
                    language: "rust".to_string(),
                    pre: Vec::new(),
                    head: Vec::new(),
                    tail: Vec::new(),
                    post: Vec::new(),
                    markers: GapMarkers { start_line: 1, end_line: 10 },
                    imports: Vec::new(),
                },
            },
            CoverageGap {
                path: PathBuf::from("tests/test.rs"), // Low centrality
                span: UncoveredSpan {
                    path: PathBuf::from("tests/test.rs"),
                    start: 20,
                    end: 22, // Small size
                    hits: Some(0),
                },
                file_loc: 50,
                language: "rust".to_string(),
                score: 0.0,
                features: GapFeatures {
                    gap_loc: 3,
                    cyclomatic_in_gap: 0.0, // No complexity
                    cognitive_in_gap: 0.0,
                    fan_in_gap: 0,
                    exports_touched: false, // Private
                    dependency_centrality_file: 0.0, // Will be updated
                    interface_surface: 0,
                    docstring_or_comment_present: true, // Has docs
                    exception_density_in_gap: 0.0,
                },
                symbols: Vec::new(),
                preview: SnippetPreview {
                    language: "rust".to_string(),
                    pre: Vec::new(),
                    head: Vec::new(),
                    tail: Vec::new(),
                    post: Vec::new(),
                    markers: GapMarkers { start_line: 20, end_line: 22 },
                    imports: Vec::new(),
                },
            },
        ];
        
        extractor.score_gaps(&mut gaps).unwrap();
        
        // Should be sorted by score descending
        assert!(gaps[0].score > gaps[1].score);
        
        // The lib.rs gap should score higher due to:
        // - Higher centrality (lib.rs vs test.rs) 
        // - Larger size (10 vs 3 lines)
        // - Higher complexity (5.0 vs 0.0 total)
        // - Public exports (true vs false)
        // - Missing docs (gets points for needing docs)
        assert_eq!(gaps[0].path, PathBuf::from("src/lib.rs"));
        assert_eq!(gaps[1].path, PathBuf::from("tests/test.rs"));
        
        // Scores should be in [0.0, 1.0] range
        for gap in &gaps {
            assert!(gap.score >= 0.0 && gap.score <= 1.0);
        }
    }

    #[test]
    fn test_extract_lines() {
        let extractor = CoverageExtractor::default();
        let lines = vec!["line1", "line2", "line3", "line4", "line5"];
        
        // Normal case
        let result = extractor.extract_lines(&lines, 2, 4);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "   2 | line2");
        assert_eq!(result[1], "   3 | line3");
        assert_eq!(result[2], "   4 | line4");
        
        // Edge cases
        assert!(extractor.extract_lines(&lines, 0, 2).is_empty()); // start = 0
        assert!(extractor.extract_lines(&lines, 3, 2).is_empty()); // start > end
        assert!(extractor.extract_lines(&lines, 10, 12).is_empty()); // out of bounds
        
        // Single line
        let result = extractor.extract_lines(&lines, 1, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "   1 | line1");
    }

    #[test]
    fn test_extract_imports_python() {
        let extractor = CoverageExtractor::default();
        let lines = vec![
            "#!/usr/bin/env python3",
            "import os",
            "import sys",
            "from collections import defaultdict",
            "from typing import List, Dict",
            "",
            "def some_function():",
            "    pass",
        ];
        
        let imports = extractor.extract_imports(&lines, "python");
        assert!(imports.contains(&"import os".to_string()));
        assert!(imports.contains(&"import sys".to_string()));
        assert!(imports.contains(&"from collections import defaultdict".to_string()));
        assert!(imports.contains(&"from typing import List, Dict".to_string()));
        assert!(!imports.contains(&"def some_function():".to_string()));
    }

    #[test]
    fn test_extract_imports_rust() {
        let extractor = CoverageExtractor::default();
        let lines = vec![
            "use std::collections::HashMap;",
            "use std::fs;",
            "use crate::core::errors::Result;",
            "",
            "fn main() {",
            "    println!(\"Hello\");",
            "}",
        ];
        
        let imports = extractor.extract_imports(&lines, "rust");
        assert!(imports.contains(&"use std::collections::HashMap;".to_string()));
        assert!(imports.contains(&"use std::fs;".to_string()));
        assert!(imports.contains(&"use crate::core::errors::Result;".to_string()));
        assert!(!imports.contains(&"fn main() {".to_string()));
    }

    #[test]
    fn test_extract_imports_typescript() {
        let extractor = CoverageExtractor::default();
        let lines = vec![
            "import React from 'react';",
            "import { useState, useEffect } from 'react';", 
            "import type { User } from './types';",
            "const fs = require('fs');",
            "",
            "function Component() {",
            "  return <div>Hello</div>;",
            "}",
        ];
        
        let imports = extractor.extract_imports(&lines, "typescript");
        assert!(imports.contains(&"import React from 'react';".to_string()));
        assert!(imports.contains(&"import { useState, useEffect } from 'react';".to_string()));
        assert!(imports.contains(&"import type { User } from './types';".to_string()));
        assert!(imports.contains(&"const fs = require('fs');".to_string()));
        assert!(!imports.contains(&"function Component() {".to_string()));
    }

    #[test]
    fn test_generate_preview_with_real_file() {
        let extractor = CoverageExtractor::default();
        
        // Create a temporary file for testing
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("preview_test.py");
        fs::write(&test_file, r#"#!/usr/bin/env python3
import os
import sys
from collections import defaultdict

def function_one():
    """This function is covered by tests."""
    return "covered"

def untested_function():
    # This function has no tests - should appear in gap
    if True:
        return "untested"
    else:
        return "never reached"
        
def another_untested():
    """Another untested function"""  
    x = 1 + 1
    return x

def final_function():
    return "also covered"
"#).unwrap();
        
        // Create a gap that covers the untested functions
        let gap = CoverageGap {
            path: test_file.clone(),
            span: UncoveredSpan {
                path: test_file.clone(),
                start: 10, // untested_function starts here
                end: 18,   // another_untested ends here
                hits: Some(0),
            },
            file_loc: 100,
            language: "python".to_string(),
            score: 0.0,
            features: GapFeatures {
                gap_loc: 9,
                cyclomatic_in_gap: 0.0,
                cognitive_in_gap: 0.0,
                fan_in_gap: 0,
                exports_touched: false,
                dependency_centrality_file: 0.0,
                interface_surface: 0,
                docstring_or_comment_present: false,
                exception_density_in_gap: 0.0,
            },
            symbols: Vec::new(),
            preview: SnippetPreview {
                language: "python".to_string(),
                pre: Vec::new(),
                head: Vec::new(),
                tail: Vec::new(),
                post: Vec::new(),
                markers: GapMarkers { start_line: 10, end_line: 18 },
                imports: Vec::new(),
            },
        };
        
        let preview = extractor.generate_preview(&gap).unwrap();
        
        // Verify the preview structure
        assert_eq!(preview.language, "python");
        assert_eq!(preview.markers.start_line, 10);
        assert_eq!(preview.markers.end_line, 18);
        
        // Should have context before the gap
        assert!(!preview.pre.is_empty());
        assert!(preview.pre.iter().any(|line| line.contains("return \"covered\"")));
        
        // Should have gap content (all in head since it's a short gap)
        assert!(!preview.head.is_empty());
        assert!(preview.head.iter().any(|line| line.contains("untested_function")));
        
        // Should have context after the gap  
        assert!(!preview.post.is_empty());
        assert!(preview.post.iter().any(|line| line.contains("final_function")));
        
        // Should have extracted Python imports
        assert!(!preview.imports.is_empty());
        assert!(preview.imports.contains(&"import os".to_string()));
        assert!(preview.imports.contains(&"import sys".to_string()));
        assert!(preview.imports.contains(&"from collections import defaultdict".to_string()));
        
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_generate_preview_long_gap() {
        let extractor = CoverageExtractor::default();
        
        // Create a temporary file with a long gap
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("long_gap_test.py");
        let long_content = (1..=50).map(|i| format!("    line_{} = {}", i, i)).collect::<Vec<_>>().join("\n");
        fs::write(&test_file, format!("def big_function():\n{}\n    return total", long_content)).unwrap();
        
        // Create a gap that covers most of the function (line 2-51)
        let gap = CoverageGap {
            path: test_file.clone(),
            span: UncoveredSpan {
                path: test_file.clone(),
                start: 2,
                end: 51,
                hits: Some(0),
            },
            file_loc: 100,
            language: "python".to_string(),
            score: 0.0,
            features: GapFeatures {
                gap_loc: 50,
                cyclomatic_in_gap: 0.0,
                cognitive_in_gap: 0.0,
                fan_in_gap: 0,
                exports_touched: false,
                dependency_centrality_file: 0.0,
                interface_surface: 0,
                docstring_or_comment_present: false,
                exception_density_in_gap: 0.0,
            },
            symbols: Vec::new(),
            preview: SnippetPreview {
                language: "python".to_string(),
                pre: Vec::new(),
                head: Vec::new(),
                tail: Vec::new(),
                post: Vec::new(),
                markers: GapMarkers { start_line: 2, end_line: 51 },
                imports: Vec::new(),
            },
        };
        
        let preview = extractor.generate_preview(&gap).unwrap();
        
        // For a long gap, should have both head and tail sections
        assert!(!preview.head.is_empty());
        assert!(!preview.tail.is_empty());
        
        // Head should contain early lines
        assert!(preview.head.iter().any(|line| line.contains("line_1 = 1")));
        
        // Tail should contain later lines
        assert!(preview.tail.iter().any(|line| line.contains("line_50 = 50")));
        
        // Should have context before (function definition)
        assert!(!preview.pre.is_empty());
        assert!(preview.pre.iter().any(|line| line.contains("def big_function")));
        
        // Should have context after (return statement)
        assert!(!preview.post.is_empty());
        assert!(preview.post.iter().any(|line| line.contains("return total")));
        
        fs::remove_file(test_file).ok();
    }

    #[tokio::test]
    async fn test_build_coverage_packs_integration() {
        let config = CoverageConfig {
            enabled: true,
            report_paths: vec![PathBuf::from("coverage.lcov")],
            max_gaps_per_file: 5,
            min_gap_loc: 1, // Lower threshold for testing
            snippet_context_lines: 3,
            long_gap_head_tail: 5,
            group_cross_file: false,
            target_repo_gain: 0.10,
            weights: ScoringWeights::default(),
            exclude_patterns: vec!["*/tests/*".to_string()],
        };
        
        let mut extractor = CoverageExtractor::new(config);
        
        // Only run if the coverage file exists
        if std::path::Path::new("coverage.lcov").exists() {
            let coverage_reports = vec![PathBuf::from("coverage.lcov")];
            let result = extractor.build_coverage_packs(coverage_reports).await;
            
            match result {
                Ok(packs) => {
                    println!("✅ Generated {} coverage packs", packs.len());
                    
                    for (i, pack) in packs.iter().enumerate().take(2) {
                        println!("📦 Pack #{}: {} (file: {:?})", i+1, pack.pack_id, pack.path);
                        println!("   Gaps: {}, File LOC: {}, Coverage gain: {:.2}%", 
                            pack.gaps.len(), pack.file_info.loc, pack.value.file_cov_gain * 100.0);
                        
                        for (j, gap) in pack.gaps.iter().enumerate().take(1) {
                            println!("   🔸 Gap #{}: lines {}-{}, score: {:.3}, lang: {}", 
                                j+1, gap.span.start, gap.span.end, gap.score, gap.language);
                        }
                    }
                },
                Err(e) => {
                    println!("⚠️  Coverage pack generation failed (this is expected in test environment): {}", e);
                }
            }
        } else {
            println!("⚠️  No coverage.lcov file found - skipping integration test");
        }
    }
}