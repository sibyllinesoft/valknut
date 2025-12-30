//! Coverage analysis and gap generation using structured parsers and AST-backed metrics.

pub mod config;
pub use config::CoverageConfig;

mod parsers;
pub mod types;

pub use types::*;

use crate::core::ast_service::{AstService, CachedTree, DecisionKind};
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::{CodeEntity, ExtractionContext, FeatureDefinition, FeatureExtractor};
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
        let mut per_file: HashMap<PathBuf, Vec<LineCoverage>> = HashMap::new();

        for report_path in reports {
            if !report_path.exists() {
                continue;
            }

            let (_format, files) = parse_report(&report_path)?;
            for file in files {
                let entry = per_file.entry(file.path).or_default();
                entry.extend(file.lines);
            }
        }

        let mut packs = Vec::new();
        for (path, mut lines) in per_file {
            lines.sort_by_key(|line| line.line_number);
            let spans = self.lines_to_spans(&path, &lines)?;
            if spans.is_empty() {
                continue;
            }

            let language = self.detect_language(&path);
            let file_gaps = self.build_gaps_for_file(&path, spans, &language).await?;
            if file_gaps.is_empty() {
                continue;
            }

            let file_loc = self.read_file_loc(&path);
            let total_uncovered: usize = file_gaps.iter().map(|gap| gap.features.gap_loc).sum();
            let coverage_before = if file_loc > 0 {
                1.0 - (total_uncovered as f64 / file_loc as f64)
            } else {
                1.0
            };
            let coverage_after_if_filled =
                1.0_f64.min(coverage_before + (total_uncovered as f64 / file_loc.max(1) as f64));
            let file_cov_gain = (coverage_after_if_filled - coverage_before).max(0.0);
            let repo_cov_gain_est = file_cov_gain * (file_loc as f64 / 10_000_f64);

            let tests_to_write_est = file_gaps.len().max(total_uncovered / 5).max(1);
            let mocks_est = file_gaps
                .iter()
                .flat_map(|gap| gap.symbols.iter())
                .filter(|symbol| matches!(symbol.kind, SymbolKind::Class | SymbolKind::Module))
                .count()
                .min(5);

            let mut gaps = file_gaps;
            self.score_gaps(&mut gaps)?;

            packs.push(CoveragePack {
                kind: "coverage".to_string(),
                pack_id: format!("cov:{}", path.display()),
                path,
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
            });
        }

        packs.sort_by(|a, b| {
            let score_a = a.value.repo_cov_gain_est / (a.effort.tests_to_write_est as f64 + 1.0);
            let score_b = b.value.repo_cov_gain_est / (b.effort.tests_to_write_est as f64 + 1.0);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(packs)
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
                if let Some(span) = current.take() {
                    if (span.end - span.start + 1) >= self.config.min_gap_loc {
                        uncovered.push(span);
                    }
                }
                continue;
            }

            match &mut current {
                Some(span) => {
                    if line.line_number == span.end + 1 {
                        span.end = line.line_number;
                    } else {
                        if (span.end - span.start + 1) >= self.config.min_gap_loc {
                            uncovered.push(span.clone());
                        }
                        *span = UncoveredSpan {
                            path: path.clone(),
                            start: line.line_number,
                            end: line.line_number,
                            hits: Some(line.hits),
                        };
                    }
                }
                None => {
                    current = Some(UncoveredSpan {
                        path: path.clone(),
                        start: line.line_number,
                        end: line.line_number,
                        hits: Some(line.hits),
                    });
                }
            }
        }

        if let Some(span) = current {
            if (span.end - span.start + 1) >= self.config.min_gap_loc {
                uncovered.push(span);
            }
        }

        Ok(uncovered)
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

        let mut result = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            let line_no = idx + 1;
            if line_no < start {
                continue;
            }
            if line_no > end {
                break;
            }
            result.push(format!("{:>4}: {}", line_no, line));
        }
        result
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
        content
            .lines()
            .enumerate()
            .filter_map(|(idx, line)| {
                let line_no = idx + 1;
                if line_no < start || line_no > end {
                    None
                } else {
                    Some(line.to_string())
                }
            })
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

    fn score_gaps(&self, gaps: &mut [CoverageGap]) -> Result<()> {
        let weights = &self.config.weights;
        let file_metrics = self.calculate_file_metrics(gaps)?;

        for gap in gaps.iter_mut() {
            if let Some(metrics) = file_metrics.get(&gap.path) {
                gap.features.dependency_centrality_file = metrics.centrality;
                gap.file_loc = gap.file_loc.max(metrics.total_gap_loc);
            }

            let size_score = self.normalize_size_score(gap.features.gap_loc);
            let complexity_score = self.normalize_complexity_score(
                gap.features.cyclomatic_in_gap + gap.features.cognitive_in_gap,
            );
            let fan_in_score = self.normalize_fan_in_score(gap.features.fan_in_gap);
            let exports_score = if gap.features.exports_touched {
                1.0
            } else {
                0.0
            };
            let centrality_score = gap.features.dependency_centrality_file;
            let docs_score = if gap.features.docstring_or_comment_present {
                0.0
            } else {
                1.0
            };

            gap.score = (size_score * weights.size)
                + (complexity_score * weights.complexity)
                + (fan_in_score * weights.fan_in)
                + (exports_score * weights.exports)
                + (centrality_score * weights.centrality)
                + (docs_score * weights.docs);

            gap.score = gap.score.clamp(0.0, 1.0);
        }

        gaps.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(())
    }

    fn calculate_file_metrics(
        &self,
        gaps: &[CoverageGap],
    ) -> Result<HashMap<PathBuf, FileMetrics>> {
        let mut metrics = HashMap::new();
        let mut grouped: HashMap<PathBuf, Vec<&CoverageGap>> = HashMap::new();
        for gap in gaps {
            grouped.entry(gap.path.clone()).or_default().push(gap);
        }

        for (path, file_gaps) in grouped {
            let total_gap_loc: usize = file_gaps.iter().map(|g| g.features.gap_loc).sum();
            let avg_complexity = if file_gaps.is_empty() {
                0.0
            } else {
                file_gaps
                    .iter()
                    .map(|g| g.features.cyclomatic_in_gap + g.features.cognitive_in_gap)
                    .sum::<f64>()
                    / file_gaps.len() as f64
            };

            let centrality = self.estimate_file_centrality(&path);

            metrics.insert(
                path,
                FileMetrics {
                    total_gap_loc,
                    avg_complexity,
                    centrality,
                    gap_count: file_gaps.len(),
                },
            );
        }

        Ok(metrics)
    }

    fn estimate_file_centrality(&self, file_path: &PathBuf) -> f64 {
        let path_str = file_path.to_string_lossy().to_lowercase();
        if path_str.contains("lib.rs")
            || path_str.contains("main.rs")
            || path_str.contains("__init__.py")
            || path_str.contains("index.")
        {
            return 0.9;
        }
        if path_str.contains("core")
            || path_str.contains("base")
            || path_str.contains("common")
            || path_str.contains("util")
        {
            return 0.7;
        }
        if path_str.contains("test") || path_str.contains("example") {
            return 0.2;
        }
        0.5
    }

    fn normalize_size_score(&self, gap_loc: usize) -> f64 {
        let x = gap_loc as f64;
        1.0 - (-x / 20.0).exp()
    }

    fn normalize_complexity_score(&self, complexity: f64) -> f64 {
        1.0 - (-complexity / 10.0).exp()
    }

    fn normalize_fan_in_score(&self, fan_in: usize) -> f64 {
        let x = fan_in as f64;
        (x / (x + 5.0)).clamp(0.0, 1.0)
    }
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
