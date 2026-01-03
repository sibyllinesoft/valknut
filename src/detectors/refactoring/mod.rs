//! Refactoring analysis detector for identifying code improvement opportunities.

mod detection_rules;
mod extractor;

pub use detection_rules::{
    COMPLEX_CONDITIONAL_THRESHOLD, DUPLICATE_MIN_LINE_COUNT, DUPLICATE_MIN_TOKEN_COUNT,
    LARGE_CLASS_LINE_THRESHOLD, LARGE_CLASS_MEMBER_THRESHOLD, LONG_METHOD_LINE_THRESHOLD,
};
pub use extractor::RefactoringExtractor;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::core::ast_service::AstService;
use crate::core::ast_utils::{find_entity_node, node_text};
use crate::core::errors::Result;
use crate::core::featureset::CodeEntity;
use crate::core::file_utils::FileReader;
use crate::detectors::complexity::{
    AstComplexityAnalyzer, ComplexityAnalysisResult, ComplexityConfig,
    ComplexityMetrics as AnalyzerComplexityMetrics,
};
use crate::lang::{adapter_for_file, EntityKind, ParseIndex, ParsedEntity};

// Detection thresholds are now defined in detection_rules module

const PROP_DUPLICATE_FINGERPRINT: &str = "duplicate_fingerprint";
const PROP_FINGERPRINT_TOKENS: &str = "duplicate_token_count";
const PROP_MEMBER_COUNT: &str = "member_count";
const PROP_COMPLEXITY_METRICS: &str = "complexity_metrics";

/// Configuration for refactoring analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringConfig {
    /// Enable refactoring analysis
    pub enabled: bool,
    /// Minimum impact threshold to report refactoring opportunities
    pub min_impact_threshold: f64,
}

/// Default implementation for [`RefactoringConfig`].
impl Default for RefactoringConfig {
    /// Returns the default refactoring analysis configuration.
    fn default() -> Self {
        Self {
            enabled: true,
            min_impact_threshold: 5.0,
        }
    }
}

/// Type of refactoring opportunity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RefactoringType {
    ExtractMethod,
    ExtractClass,
    ReduceComplexity,
    EliminateDuplication,
    ImproveNaming,
    SimplifyConditionals,
    RemoveDeadCode,
}

/// Refactoring recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringRecommendation {
    /// Type of refactoring
    pub refactoring_type: RefactoringType,
    /// Description of the opportunity
    pub description: String,
    /// Estimated impact (1-10 scale)
    pub estimated_impact: f64,
    /// Estimated effort (1-10 scale)
    pub estimated_effort: f64,
    /// Priority score (impact/effort ratio)
    pub priority_score: f64,
    /// Location in file (line numbers)
    pub location: (usize, usize), // start_line, end_line
}

/// Refactoring analysis result for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringAnalysisResult {
    /// File path
    pub file_path: String,
    /// Refactoring recommendations
    pub recommendations: Vec<RefactoringRecommendation>,
    /// Overall refactoring score (0-100, higher means more refactoring needed)
    pub refactoring_score: f64,
}

/// Main refactoring analyzer
#[derive(Clone)]
pub struct RefactoringAnalyzer {
    config: RefactoringConfig,
    ast_service: Arc<AstService>,
    complexity_analyzer: AstComplexityAnalyzer,
}

/// Analysis and detection methods for [`RefactoringAnalyzer`].
impl RefactoringAnalyzer {
    /// Create new refactoring analyzer
    pub fn new(config: RefactoringConfig, ast_service: Arc<AstService>) -> Self {
        let complexity_analyzer =
            AstComplexityAnalyzer::new(ComplexityConfig::default(), ast_service.clone());

        Self {
            config,
            ast_service,
            complexity_analyzer,
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(RefactoringConfig::default(), Arc::new(AstService::new()))
    }

    /// Analyze files for refactoring opportunities
    pub async fn analyze_files(
        &self,
        file_paths: &[PathBuf],
    ) -> Result<Vec<RefactoringAnalysisResult>> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }

        info!("Running refactoring analysis on {} files", file_paths.len());
        let mut results = Vec::new();

        for file_path in file_paths {
            match self.analyze_file(file_path).await {
                Ok(result) => {
                    if !result.recommendations.is_empty() {
                        results.push(result);
                    }
                }
                Err(e) => warn!(
                    "Refactoring analysis failed for {}: {}",
                    file_path.display(),
                    e
                ),
            }
        }

        info!(
            "Refactoring analysis found {} files with opportunities",
            results.len()
        );
        Ok(results)
    }

    /// Analyze a single file for refactoring opportunities
    async fn analyze_file(&self, file_path: &Path) -> Result<RefactoringAnalysisResult> {
        debug!(
            "Analyzing refactoring opportunities for: {}",
            file_path.display()
        );

        let content = FileReader::read_to_string(file_path)?;
        let file_path_str = file_path.to_string_lossy().to_string();

        let complexity_results = match self
            .complexity_analyzer
            .analyze_file_with_results(&file_path_str, &content)
            .await
        {
            Ok(results) => results,
            Err(err) => {
                warn!(
                    "Complexity analysis failed for {}: {}",
                    file_path.display(),
                    err
                );
                Vec::new()
            }
        };
        let complexity_by_id: HashMap<String, ComplexityAnalysisResult> = complexity_results
            .into_iter()
            .map(|res| (res.entity_id.clone(), res))
            .collect();

        let mut adapter = match adapter_for_file(file_path) {
            Ok(adapter) => adapter,
            Err(err) => {
                warn!("No language adapter for {}: {}", file_path.display(), err);
                return Ok(RefactoringAnalysisResult {
                    file_path: file_path_str,
                    recommendations: Vec::new(),
                    refactoring_score: 0.0,
                });
            }
        };

        let parse_index = adapter.parse_source(&content, &file_path_str)?;
        let cached_tree = self.ast_service.get_ast(&file_path_str, &content).await?;
        let ast_context = self
            .ast_service
            .create_context(&cached_tree, &file_path_str);
        let entity_summaries =
            self.collect_entity_summaries(&parse_index, &content, &complexity_by_id, &ast_context)?;

        if entity_summaries.is_empty() {
            return Ok(RefactoringAnalysisResult {
                file_path: file_path_str,
                recommendations: Vec::new(),
                refactoring_score: 0.0,
            });
        }

        let functions: Vec<_> = entity_summaries
            .iter()
            .filter(|e| Self::is_function_entity(e))
            .cloned()
            .collect();

        let type_like_entities: Vec<_> = entity_summaries
            .iter()
            .filter(|e| Self::is_type_entity(e))
            .cloned()
            .collect();

        let mut recommendations = Vec::new();
        recommendations.extend(self.detect_long_methods(&functions));
        recommendations.extend(self.detect_complex_conditionals(&functions));
        recommendations.extend(self.detect_duplicate_code(&functions));
        recommendations.extend(self.detect_large_types(&type_like_entities));

        recommendations.retain(|rec| rec.estimated_impact >= self.config.min_impact_threshold);
        recommendations.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap());

        let refactoring_score = self.calculate_refactoring_score(&recommendations, &content);

        Ok(RefactoringAnalysisResult {
            file_path: file_path_str,
            recommendations,
            refactoring_score,
        })
    }

    /// Collect entity summaries from the parse index for later analysis
    fn collect_entity_summaries(
        &self,
        index: &ParseIndex,
        content: &str,
        complexity: &HashMap<String, ComplexityAnalysisResult>,
        ast_context: &crate::core::ast_service::AstContext<'_>,
    ) -> Result<Vec<CodeEntity>> {
        let lines: Vec<&str> = content.lines().collect();
        let child_function_counts = self.count_child_functions(index);
        let mut summaries = Vec::new();

        for entity in index.entities.values() {
            let start_line = entity.location.start_line;
            let end_line = entity.location.end_line;

            if start_line == 0 || end_line == 0 || start_line > lines.len() + 1 {
                continue;
            }

            let end_line = end_line.min(lines.len());
            let snippet = extract_lines(&lines, start_line, end_line);

            let mut code_entity = CodeEntity::new(
                entity.id.clone(),
                format!("{:?}", entity.kind),
                entity.name.clone(),
                entity.location.file_path.clone(),
            )
            .with_line_range(start_line, end_line)
            .with_source_code(snippet.clone());

            if let Some(range) = entity.metadata.get("byte_range") {
                code_entity.add_property("byte_range", range.clone());
            }
            if let Some(kind) = entity.metadata.get("node_kind") {
                code_entity.add_property("node_kind", kind.clone());
            }

            let (fingerprint, complexity_score) =
                self.compute_duplicate_fingerprint_for_entity(&code_entity, ast_context)?;
            if let Some(hash) = fingerprint {
                code_entity.add_property(PROP_DUPLICATE_FINGERPRINT, json!(hash));
            }
            if let Some(tokens) = complexity_score {
                code_entity.add_property(PROP_FINGERPRINT_TOKENS, json!(tokens));
            }
            if let Some(value) = self
                .lookup_complexity_metrics(entity, start_line, complexity)
                .and_then(|m| serde_json::to_value(&m).ok())
            {
                code_entity.add_property(PROP_COMPLEXITY_METRICS, value);
            }
            if let Some(count) = child_function_counts.get(&entity.id) {
                code_entity.add_property(PROP_MEMBER_COUNT, json!(count));
            }

            summaries.push(code_entity);
        }

        Ok(summaries)
    }

    /// Count child functions for each entity to help with class size detection
    fn count_child_functions(&self, index: &ParseIndex) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();

        for entity in index.entities.values() {
            let function_children = entity.children.iter().filter(|child_id| {
                index
                    .entities
                    .get(*child_id)
                    .map(|c| matches!(c.kind, EntityKind::Function | EntityKind::Method))
                    .unwrap_or(false)
            });
            let count = function_children.count();
            if count > 0 {
                counts.insert(entity.id.clone(), count);
            }
        }

        counts
    }

    /// Checks if an entity represents a function or method.
    fn is_function_entity(entity: &CodeEntity) -> bool {
        let kind = entity.entity_type.as_str();
        kind.eq_ignore_ascii_case("function") || kind.eq_ignore_ascii_case("method")
    }

    /// Checks if an entity represents a type (class, struct, interface, enum).
    fn is_type_entity(entity: &CodeEntity) -> bool {
        let kind = entity.entity_type.as_str();
        kind.eq_ignore_ascii_case("class")
            || kind.eq_ignore_ascii_case("struct")
            || kind.eq_ignore_ascii_case("interface")
            || kind.eq_ignore_ascii_case("enum")
    }

    /// Extracts complexity metrics from an entity's properties.
    fn entity_complexity(entity: &CodeEntity) -> Option<AnalyzerComplexityMetrics> {
        entity
            .properties
            .get(PROP_COMPLEXITY_METRICS)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }

    /// Extracts duplicate detection signature (hash, token count) from an entity.
    fn duplicate_signature(entity: &CodeEntity) -> Option<(u64, usize)> {
        let hash = entity
            .properties
            .get(PROP_DUPLICATE_FINGERPRINT)?
            .as_u64()? as u64;
        let tokens = entity
            .properties
            .get(PROP_FINGERPRINT_TOKENS)
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as usize;
        Some((hash, tokens))
    }

    /// Gets the member count from an entity's properties.
    fn member_count_from_entity(entity: &CodeEntity) -> usize {
        entity
            .properties
            .get(PROP_MEMBER_COUNT)
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(0)
    }

    /// Gets the line range (start, end) for an entity.
    fn entity_location(entity: &CodeEntity) -> (usize, usize) {
        entity
            .line_range
            .map(|(start, end)| (start, end.max(start)))
            .unwrap_or((1, entity.line_count()))
    }

    /// Computes a fingerprint hash and token count for duplicate detection.
    fn compute_duplicate_fingerprint_for_entity(
        &self,
        entity: &CodeEntity,
        context: &crate::core::ast_service::AstContext<'_>,
    ) -> Result<(Option<u64>, Option<usize>)> {
        let Some(node) = find_entity_node(context, entity) else {
            return Ok((None, None));
        };

        let mut tokens = Vec::new();
        self.collect_fingerprint_tokens(node, context.source, &mut tokens);

        if tokens.is_empty() {
            return Ok((None, None));
        }

        let token_count = tokens.len();
        if token_count < DUPLICATE_MIN_TOKEN_COUNT {
            return Ok((None, Some(token_count)));
        }

        let normalized = tokens.join(" ");
        let hash = blake3::hash(normalized.as_bytes());
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&hash.as_bytes()[..8]);

        Ok((Some(u64::from_le_bytes(bytes)), Some(token_count)))
    }

    /// Comment node kinds to skip during fingerprinting.
    const COMMENT_KINDS: [&'static str; 3] = ["comment", "block_comment", "line_comment"];

    /// Mapping from node kinds to normalized token names.
    const TOKEN_MAPPINGS: [(&'static str, &'static [&'static str]); 6] = [
        ("IDENT", &["identifier", "field_identifier", "property_identifier",
                   "shorthand_property_identifier_pattern", "member_expression", "scoped_identifier"]),
        ("TYPE", &["type_identifier", "primitive_type"]),
        ("STRING", &["string", "string_literal", "raw_string_literal"]),
        ("NUMBER", &["number", "integer", "float", "decimal_literal", "float_literal"]),
        ("BOOL", &["true", "false"]),
        ("NULL", &["null", "nil"]),
    ];

    /// Normalize a node kind to a token name, or None if it should be skipped.
    fn normalize_token_kind(kind: &str) -> Option<String> {
        if Self::COMMENT_KINDS.contains(&kind) {
            return None;
        }
        for (token, kinds) in &Self::TOKEN_MAPPINGS {
            if kinds.contains(&kind) {
                return Some((*token).to_string());
            }
        }
        Some(kind.to_string())
    }

    /// Count arguments in a call expression.
    fn count_call_arguments(node: &tree_sitter::Node<'_>) -> usize {
        node.child_by_field_name("arguments")
            .map(|args| args.named_child_count())
            .unwrap_or_else(|| {
                let mut cnt = 0;
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind().ends_with("argument") {
                        cnt += 1;
                    }
                }
                cnt
            })
    }

    /// Recursively collects normalized tokens from an AST node for fingerprinting.
    fn collect_fingerprint_tokens(
        &self,
        node: tree_sitter::Node<'_>,
        source: &str,
        tokens: &mut Vec<String>,
    ) {
        if !node.is_named() {
            return;
        }

        let kind = node.kind();
        if let Some(token) = Self::normalize_token_kind(kind) {
            tokens.push(token);
        } else {
            return; // Comment node, skip children too
        }

        if matches!(kind, "binary_expression" | "assignment_expression" | "logical_expression") {
            if let Some(op_text) = node
                .child_by_field_name("operator")
                .and_then(|op| node_text(op, source))
            {
                tokens.push(format!("OP:{}", op_text.trim()));
            }
        }

        if matches!(kind, "call_expression" | "call") {
            tokens.push(format!("CALL_ARGS:{}", Self::count_call_arguments(&node)));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_fingerprint_tokens(child, source, tokens);
        }
    }

    /// Looks up complexity metrics for an entity by ID or name/line match.
    fn lookup_complexity_metrics(
        &self,
        entity: &ParsedEntity,
        start_line: usize,
        complexity: &HashMap<String, ComplexityAnalysisResult>,
    ) -> Option<AnalyzerComplexityMetrics> {
        if let Some(result) = complexity.get(&entity.id) {
            return Some(result.metrics.clone());
        }

        complexity
            .values()
            .find(|result| result.entity_name == entity.name && result.start_line == start_line)
            .map(|result| result.metrics.clone())
    }

    /// Detects methods exceeding the line count threshold.
    fn detect_long_methods(&self, functions: &[CodeEntity]) -> Vec<RefactoringRecommendation> {
        detection_rules::detect_long_methods(
            functions,
            Self::entity_complexity,
            Self::entity_location,
        )
    }

    /// Detects functions with overly complex conditional logic.
    fn detect_complex_conditionals(
        &self,
        functions: &[CodeEntity],
    ) -> Vec<RefactoringRecommendation> {
        detection_rules::detect_complex_conditionals(
            functions,
            Self::entity_complexity,
            Self::entity_location,
        )
    }

    /// Detects duplicate code blocks based on fingerprint matching.
    fn detect_duplicate_code(&self, functions: &[CodeEntity]) -> Vec<RefactoringRecommendation> {
        detection_rules::detect_duplicate_code(
            functions,
            Self::duplicate_signature,
            Self::entity_location,
        )
    }

    /// Detects types exceeding member count or line thresholds.
    fn detect_large_types(&self, types: &[CodeEntity]) -> Vec<RefactoringRecommendation> {
        detection_rules::detect_large_types(
            types,
            Self::member_count_from_entity,
            Self::entity_location,
        )
    }

    /// Calculate overall refactoring score for the file
    fn calculate_refactoring_score(
        &self,
        recommendations: &[RefactoringRecommendation],
        content: &str,
    ) -> f64 {
        if recommendations.is_empty() {
            return 0.0;
        }

        let total_lines = content.lines().count().max(1) as f64;
        let total_impact: f64 = recommendations.iter().map(|r| r.estimated_impact).sum();

        // Normalize by file size and cap at 100
        let base_score = (total_impact / total_lines) * 120.0;
        base_score.min(100.0)
    }
}

/// Extracts a range of lines from a slice, joining them with newlines.
fn extract_lines(lines: &[&str], start_line: usize, end_line: usize) -> String {
    let start_idx = start_line.saturating_sub(1);
    let end_idx = end_line
        .saturating_sub(1)
        .min(lines.len().saturating_sub(1));

    if start_idx > end_idx || start_idx >= lines.len() {
        return String::new();
    }

    lines[start_idx..=end_idx].join("\n")
}
// estimate_logical_operator_complexity is now in detection_rules module

#[cfg(test)]
mod tests;
