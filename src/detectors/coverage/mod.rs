//! Coverage analysis and gap generation using structured parsers and AST-backed metrics.

pub mod config;
pub use config::CoverageConfig;

mod gap_scoring;
mod parsers;
pub mod types;

pub use types::*;

use crate::core::ast_service::{AstService, CachedTree, DecisionKind};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::{CodeEntity, ExtractionContext, FeatureDefinition, FeatureExtractor};
use crate::lang::registry::detect_language_from_path;
use async_trait::async_trait;
use parsers::parse_report;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::warn;
use types::{FileCoverage, LineCoverage};

#[cfg(test)]
mod tests;

/// Intermediate struct for coverage statistics calculation.
struct CoverageStats {
    total_uncovered: usize,
    coverage_before: f64,
    coverage_after: f64,
    file_cov_gain: f64,
    repo_cov_gain_est: f64,
}

/// Primary entry point for coverage analysis.
#[derive(Debug)]
pub struct CoverageExtractor {
    pub config: CoverageConfig,
    ast_service: Arc<AstService>,
}

impl CoverageExtractor {
    pub fn new(config: CoverageConfig, ast_service: Arc<AstService>) -> Self {
        Self {
            config,
            ast_service,
        }
    }

    pub fn with_ast(ast_service: Arc<AstService>) -> Self {
        Self::new(CoverageConfig::default(), ast_service)
    }

    /// Build coverage packs from parsed coverage reports.
    pub async fn build_coverage_packs(&self, reports: Vec<PathBuf>) -> Result<Vec<CoveragePack>> {
        let per_file = self.aggregate_coverage_lines(reports)?;

        let mut packs = Vec::new();
        for (path, mut lines) in per_file {
            lines.sort_by_key(|line| line.line_number);

            if let Some(pack) = self.build_pack_for_file(&path, lines).await? {
                packs.push(pack);
            }
        }

        self.sort_packs_by_priority(&mut packs);
        Ok(packs)
    }

    /// Aggregate coverage lines from multiple reports into per-file collections.
    fn aggregate_coverage_lines(
        &self,
        reports: Vec<PathBuf>,
    ) -> Result<HashMap<PathBuf, Vec<LineCoverage>>> {
        let mut per_file: HashMap<PathBuf, Vec<LineCoverage>> = HashMap::new();

        for report_path in reports {
            if !report_path.exists() {
                continue;
            }

            let (_format, files) = parse_report(&report_path)?;
            for file in files {
                per_file.entry(file.path).or_default().extend(file.lines);
            }
        }

        Ok(per_file)
    }

    /// Build a coverage pack for a single file.
    async fn build_pack_for_file(
        &self,
        path: &PathBuf,
        lines: Vec<LineCoverage>,
    ) -> Result<Option<CoveragePack>> {
        let spans = self.lines_to_spans(path, &lines)?;
        if spans.is_empty() {
            return Ok(None);
        }

        let language = self.detect_language(path);
        let file_gaps = self.build_gaps_for_file(path, spans, &language).await?;
        if file_gaps.is_empty() {
            return Ok(None);
        }

        let file_loc = self.read_file_loc(path);
        let coverage_stats = self.calculate_coverage_stats(&file_gaps, file_loc);
        let effort = self.estimate_effort(&file_gaps, coverage_stats.total_uncovered);

        let mut gaps = file_gaps;
        gap_scoring::score_gaps(&self.config, &mut gaps)?;

        Ok(Some(CoveragePack {
            kind: "coverage".to_string(),
            pack_id: format!("cov:{}", path.display()),
            path: path.clone(),
            file_info: FileInfo {
                loc: file_loc,
                coverage_before: coverage_stats.coverage_before,
                coverage_after_if_filled: coverage_stats.coverage_after,
            },
            gaps,
            value: PackValue {
                file_cov_gain: coverage_stats.file_cov_gain,
                repo_cov_gain_est: coverage_stats.repo_cov_gain_est,
            },
            effort,
        }))
    }

    /// Calculate coverage statistics for a file.
    fn calculate_coverage_stats(&self, gaps: &[CoverageGap], file_loc: usize) -> CoverageStats {
        let total_uncovered: usize = gaps.iter().map(|gap| gap.features.gap_loc).sum();
        let coverage_before = if file_loc > 0 {
            1.0 - (total_uncovered as f64 / file_loc as f64)
        } else {
            1.0
        };
        let coverage_after =
            1.0_f64.min(coverage_before + (total_uncovered as f64 / file_loc.max(1) as f64));
        let file_cov_gain = (coverage_after - coverage_before).max(0.0);
        let repo_cov_gain_est = file_cov_gain * (file_loc as f64 / 10_000_f64);

        CoverageStats {
            total_uncovered,
            coverage_before,
            coverage_after,
            file_cov_gain,
            repo_cov_gain_est,
        }
    }

    /// Estimate effort required to cover gaps.
    fn estimate_effort(&self, gaps: &[CoverageGap], total_uncovered: usize) -> PackEffort {
        let tests_to_write_est = gaps.len().max(total_uncovered / 5).max(1);
        let mocks_est = gaps
            .iter()
            .flat_map(|gap| gap.symbols.iter())
            .filter(|symbol| matches!(symbol.kind, SymbolKind::Class | SymbolKind::Module))
            .count()
            .min(5);

        PackEffort {
            tests_to_write_est,
            mocks_est,
        }
    }

    /// Sort packs by priority (highest value/effort ratio first).
    fn sort_packs_by_priority(&self, packs: &mut [CoveragePack]) {
        packs.sort_by(|a, b| {
            let score_a = a.value.repo_cov_gain_est / (a.effort.tests_to_write_est as f64 + 1.0);
            let score_b = b.value.repo_cov_gain_est / (b.effort.tests_to_write_est as f64 + 1.0);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    async fn build_gaps_for_file(
        &self,
        path: &PathBuf,
        spans: Vec<UncoveredSpan>,
        language: &str,
    ) -> Result<Vec<CoverageGap>> {
        let coalesced = self.coalesce_spans_for_file(&spans)?;
        let chunked = self.chunk_spans_by_language(path, language, &coalesced)?;

        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(err) => {
                warn!(
                    "Skipping coverage gaps for missing source file {}: {}",
                    path.display(),
                    err
                );
                return Ok(Vec::new());
            }
        };

        let cached_tree = if content.trim().is_empty() {
            None
        } else {
            Some(
                self.ast_service
                    .get_ast(&path.to_string_lossy(), &content)
                    .await?,
            )
        };

        let mut gaps = Vec::new();
        for span in chunked {
            if span.end < span.start {
                continue;
            }
            if (span.end - span.start + 1) < self.config.min_gap_loc {
                continue;
            }

            let mut gap = CoverageGap {
                path: path.clone(),
                span: span.clone(),
                file_loc: content.lines().count(),
                language: language.to_string(),
                score: 0.0,
                features: GapFeatures {
                    gap_loc: span.end - span.start + 1,
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
                    language: language.to_string(),
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

            self.generate_preview(&content, &mut gap)?;
            self.analyze_gap_code(&content, cached_tree.as_ref(), &mut gap)?;
            gaps.push(gap);
        }

        Ok(gaps)
    }

    fn read_file_loc(&self, path: &PathBuf) -> usize {
        fs::read_to_string(path)
            .map(|content| content.lines().count())
            .unwrap_or(0)
    }

    fn lines_to_spans(&self, path: &PathBuf, lines: &[LineCoverage]) -> Result<Vec<UncoveredSpan>> {
        let mut uncovered = Vec::new();
        let mut current: Option<UncoveredSpan> = None;

        for line in lines {
            if line.is_covered {
                self.maybe_push_span(&mut uncovered, current.take());
                continue;
            }

            current = Some(self.process_uncovered_line(&mut uncovered, current, path, line));
        }

        self.maybe_push_span(&mut uncovered, current);
        Ok(uncovered)
    }

    /// Push a span to the list if it meets the minimum size requirement.
    fn maybe_push_span(&self, spans: &mut Vec<UncoveredSpan>, span: Option<UncoveredSpan>) {
        if let Some(span) = span {
            if self.is_span_large_enough(&span) {
                spans.push(span);
            }
        }
    }

    /// Check if a span meets the minimum size requirement.
    fn is_span_large_enough(&self, span: &UncoveredSpan) -> bool {
        (span.end - span.start + 1) >= self.config.min_gap_loc
    }

    /// Process an uncovered line, either extending the current span or starting a new one.
    fn process_uncovered_line(
        &self,
        uncovered: &mut Vec<UncoveredSpan>,
        current: Option<UncoveredSpan>,
        path: &PathBuf,
        line: &LineCoverage,
    ) -> UncoveredSpan {
        match current {
            Some(mut span) if line.line_number == span.end + 1 => {
                span.end = line.line_number;
                span
            }
            Some(old_span) => {
                // Non-contiguous line - push old span if valid, start new one
                self.maybe_push_span(uncovered, Some(old_span));
                self.create_span(path, line)
            }
            None => self.create_span(path, line),
        }
    }

    /// Create a new uncovered span from a line.
    fn create_span(&self, path: &PathBuf, line: &LineCoverage) -> UncoveredSpan {
        UncoveredSpan {
            path: path.clone(),
            start: line.line_number,
            end: line.line_number,
            hits: Some(line.hits),
        }
    }

    fn coalesce_spans_for_file(&self, spans: &[UncoveredSpan]) -> Result<Vec<UncoveredSpan>> {
        if spans.is_empty() {
            return Ok(Vec::new());
        }

        let mut sorted = spans.to_vec();
        sorted.sort_by_key(|span| span.start);

        let mut merged = Vec::new();
        let mut current = sorted[0].clone();

        for span in sorted.into_iter().skip(1) {
            if span.start <= current.end + 2 {
                current.end = current.end.max(span.end);
            } else {
                merged.push(current);
                current = span;
            }
        }

        merged.push(current);
        Ok(merged)
    }

    fn chunk_spans_by_language(
        &self,
        path: &PathBuf,
        language: &str,
        spans: &[UncoveredSpan],
    ) -> Result<Vec<UncoveredSpan>> {
        match language {
            "python" => self.chunk_spans_python(path, spans),
            _ => Ok(spans.to_vec()),
        }
    }

    fn chunk_spans_python(
        &self,
        path: &PathBuf,
        spans: &[UncoveredSpan],
    ) -> Result<Vec<UncoveredSpan>> {
        let content = fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = content.lines().collect();
        let mut chunked = Vec::new();

        for span in spans {
            let mut boundaries = HashSet::new();
            boundaries.insert(span.start);
            boundaries.insert(span.end + 1);

            for line_no in span.start..=span.end {
                if let Some(line) = lines.get(line_no.saturating_sub(1)) {
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
                        boundaries.insert(line_no);
                    }
                }
            }

            let mut boundary_list: Vec<usize> = boundaries.into_iter().collect();
            boundary_list.sort_unstable();

            for window in boundary_list.windows(2) {
                let start = window[0];
                let end = window[1].saturating_sub(1);
                if start <= end {
                    chunked.push(UncoveredSpan {
                        path: span.path.clone(),
                        start,
                        end,
                        hits: span.hits,
                    });
                }
            }
        }

        Ok(chunked)
    }

    fn detect_language(&self, file_path: &PathBuf) -> String {
        detect_language_from_path(&file_path.to_string_lossy())
    }

    fn generate_preview(&self, content: &str, gap: &mut CoverageGap) -> Result<()> {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        let gap_start = gap.span.start;
        let gap_end = gap.span.end;
        let gap_size = gap_end.saturating_sub(gap_start).saturating_add(1);

        gap.preview.pre.clear();
        gap.preview.head.clear();
        gap.preview.tail.clear();
        gap.preview.post.clear();

        // Helper to push a single line by number
        let mut push_line = |vec: &mut Vec<String>, line_no: usize| {
            if line_no == 0 || line_no > total_lines {
                return;
            }
            if let Some(line) = lines.get(line_no - 1) {
                vec.push(format!("{:>4}: {}", line_no, line));
            }
        };

        if gap_size >= 3 {
            // Show only the uncovered lines
            gap.preview.head = self.extract_lines(&lines, gap_start, gap_end);
        } else if gap_size == 2 {
            // Include one line of context before (or after if start is 1)
            if gap_start > 1 {
                push_line(&mut gap.preview.pre, gap_start - 1);
            } else if gap_end < total_lines {
                push_line(&mut gap.preview.post, gap_end + 1);
            }
            gap.preview.head = self.extract_lines(&lines, gap_start, gap_end);
        } else {
            // gap_size == 1
            if gap_start > 1 {
                push_line(&mut gap.preview.pre, gap_start - 1);
            }
            gap.preview.head = self.extract_lines(&lines, gap_start, gap_end);
            if gap_end < total_lines {
                push_line(&mut gap.preview.post, gap_end + 1);
            }
            // ensure at least 3 lines of snippet by adding trailing context if available
            let mut next = gap_end + 2;
            while (gap.preview.pre.len() + gap.preview.head.len() + gap.preview.post.len()) < 3
                && next <= total_lines
            {
                push_line(&mut gap.preview.post, next);
                next += 1;
            }
        }

        gap.preview.imports = self.extract_imports(&lines, &gap.language);

        Ok(())
    }

    fn extract_lines(&self, lines: &[&str], start: usize, end: usize) -> Vec<String> {
        if start == 0 || start > end {
            return Vec::new();
        }

        lines
            .iter()
            .enumerate()
            .skip(start - 1)
            .take(end - start + 1)
            .map(|(idx, line)| format!("{:>4}: {}", idx + 1, line))
            .collect()
    }

    fn extract_imports(&self, lines: &[&str], language: &str) -> Vec<String> {
        let mut imports = Vec::new();

        for line in lines.iter().take(200) {
            let trimmed = line.trim();
            match language {
                "python" => {
                    if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                        imports.push(trimmed.to_string());
                    }
                }
                "javascript" | "typescript" => {
                    if trimmed.starts_with("import ")
                        || trimmed.starts_with("const ") && trimmed.contains("require(")
                    {
                        imports.push(trimmed.to_string());
                    }
                }
                "rust" => {
                    if trimmed.starts_with("use ") {
                        imports.push(trimmed.to_string());
                    }
                }
                _ => {}
            }
        }

        imports
    }

    fn analyze_gap_code(
        &self,
        content: &str,
        cached_tree: Option<&Arc<crate::core::ast_service::CachedTree>>,
        gap: &mut CoverageGap,
    ) -> Result<()> {
        let Some(cached_tree) = cached_tree else {
            return Ok(());
        };

        let path_repr = gap.path.to_string_lossy().to_string();
        let context = self.ast_service.create_context(cached_tree, &path_repr);
        let metrics = self.ast_service.calculate_complexity(&context)?;

        let decision_points = self.filter_decision_points_in_span(&metrics, &gap.span);
        self.populate_complexity_features(&decision_points, gap);

        let snippet = self.extract_snippet(content, gap.span.start, gap.span.end);
        self.populate_code_style_features(&snippet, gap);

        gap.symbols =
            self.extract_symbols_from_ast(content, cached_tree, gap.span.start, gap.span.end);
        self.populate_symbol_features(content, gap);
        self.populate_exception_density(&snippet, gap);

        Ok(())
    }

    /// Filter decision points that fall within the gap span.
    fn filter_decision_points_in_span<'a>(
        &self,
        metrics: &'a crate::core::ast_service::ComplexityMetrics,
        span: &UncoveredSpan,
    ) -> Vec<&'a crate::core::ast_service::DecisionPoint> {
        metrics
            .decision_points
            .iter()
            .filter(|dp| dp.location.start_line >= span.start && dp.location.end_line <= span.end)
            .collect()
    }

    /// Populate cyclomatic and cognitive complexity features from decision points.
    fn populate_complexity_features(
        &self,
        decision_points: &[&crate::core::ast_service::DecisionPoint],
        gap: &mut CoverageGap,
    ) {
        gap.features.cyclomatic_in_gap = if decision_points.is_empty() {
            0.0
        } else {
            1.0 + decision_points.len() as f64
        };

        gap.features.cognitive_in_gap = decision_points
            .iter()
            .map(|dp| self.cognitive_weight(&dp.kind) as f64 + dp.nesting_level as f64)
            .sum();
    }

    /// Populate exports and documentation features from snippet.
    fn populate_code_style_features(&self, snippet: &[String], gap: &mut CoverageGap) {
        gap.features.exports_touched = snippet.iter().any(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("pub ")
                || trimmed.starts_with("export ")
                || trimmed.starts_with("public ")
                || trimmed.contains("__all__")
        });

        gap.features.docstring_or_comment_present = snippet.iter().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('#')
                || trimmed.starts_with("///")
                || trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with("\"\"\"")
        });
    }

    /// Populate interface surface and fan-in features from symbols.
    fn populate_symbol_features(&self, content: &str, gap: &mut CoverageGap) {
        gap.features.interface_surface = gap
            .symbols
            .iter()
            .map(|symbol| symbol.signature.matches(',').count() + 1)
            .sum();

        if gap.symbols.is_empty() {
            return;
        }

        let rest = self.remove_span_from_content(content, gap.span.start, gap.span.end);
        let fan_in: usize = gap.symbols.iter().map(|s| rest.matches(&s.name).count()).sum();
        gap.features.fan_in_gap = fan_in.max(gap.symbols.len());
    }

    /// Populate exception density feature from snippet.
    fn populate_exception_density(&self, snippet: &[String], gap: &mut CoverageGap) {
        if snippet.is_empty() {
            return;
        }

        const EXCEPTION_KEYWORDS: &[&str] = &["except", "catch", "Result<", "Err("];
        let exceptions = snippet
            .iter()
            .filter(|line| EXCEPTION_KEYWORDS.iter().any(|kw| line.contains(kw)))
            .count();
        gap.features.exception_density_in_gap =
            exceptions as f64 / gap.features.gap_loc.max(1) as f64;
    }

    fn extract_snippet(&self, content: &str, start: usize, end: usize) -> Vec<String> {
        if start == 0 || end < start {
            return Vec::new();
        }
        content
            .lines()
            .skip(start - 1)
            .take(end - start + 1)
            .map(|line| line.to_string())
            .collect()
    }

    fn remove_span_from_content(&self, content: &str, start: usize, end: usize) -> String {
        let mut result = String::with_capacity(content.len());
        for (idx, line) in content.lines().enumerate() {
            let line_no = idx + 1;
            if line_no < start || line_no > end {
                result.push_str(line);
                result.push('\n');
            }
        }
        result
    }

    fn extract_symbols_from_ast(
        &self,
        content: &str,
        cached_tree: &Arc<crate::core::ast_service::CachedTree>,
        start_line: usize,
        end_line: usize,
    ) -> Vec<GapSymbol> {
        let mut symbols = Vec::new();
        let tree = &cached_tree.tree;
        let mut cursor = tree.walk();
        let source_bytes = content.as_bytes();

        fn node_text(node: &tree_sitter::Node, source: &[u8]) -> String {
            let range = node.byte_range();
            std::str::from_utf8(&source[range])
                .unwrap_or("")
                .trim()
                .to_string()
        }

        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            if node.start_position().row + 1 > end_line {
                continue;
            }
            if node.end_position().row + 1 < start_line {
                continue;
            }

            let kind = node.kind();
            if let Some(symbol_kind) = symbol_kind_from_node(kind) {
                let name = node
                    .child_by_field_name("name")
                    .map(|n| node_text(&n, source_bytes))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| {
                        node_text(&node, source_bytes)
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_string()
                    });

                if !name.is_empty() {
                    symbols.push(GapSymbol {
                        kind: symbol_kind,
                        name,
                        signature: node_text(&node, source_bytes),
                        line_start: node.start_position().row + 1,
                        line_end: node.end_position().row + 1,
                    });
                }
            }

            let mut child_cursor = node.walk();
            for child in node.children(&mut child_cursor) {
                stack.push(child);
            }
        }

        symbols
            .into_iter()
            .filter(|symbol| symbol.line_start >= start_line && symbol.line_end <= end_line)
            .collect()
    }

    fn cognitive_weight(&self, kind: &DecisionKind) -> u32 {
        match kind {
            DecisionKind::If | DecisionKind::ElseIf => 1,
            DecisionKind::While | DecisionKind::For => 1,
            DecisionKind::Match => 1,
            DecisionKind::Try | DecisionKind::Catch => 1,
            DecisionKind::LogicalAnd | DecisionKind::LogicalOr => 1,
            DecisionKind::ConditionalExpression => 1,
        }
    }

    // Gap scoring methods have been moved to gap_scoring module
}

fn symbol_kind_from_node(kind: &str) -> Option<SymbolKind> {
    match kind {
        "function_definition" | "function_item" | "function_declaration" | "method_definition" => {
            Some(SymbolKind::Function)
        }
        "class_definition" | "class_declaration" | "struct_item" => Some(SymbolKind::Class),
        "module" | "module_declaration" => Some(SymbolKind::Module),
        _ => None,
    }
}

#[async_trait]
impl FeatureExtractor for CoverageExtractor {
    fn name(&self) -> &str {
        "coverage"
    }

    fn features(&self) -> &[FeatureDefinition] {
        &[]
    }

    async fn extract(
        &self,
        _entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        Ok(HashMap::new())
    }
}
