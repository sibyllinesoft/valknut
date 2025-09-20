//! Refactoring analysis detector for identifying code improvement opportunities.

use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::core::ast_service::AstService;
use crate::core::ast_utils::{find_entity_node, node_text};
use crate::core::errors::Result;
use crate::core::featureset::{CodeEntity, ExtractionContext, FeatureDefinition, FeatureExtractor};
use crate::core::file_utils::FileReader;
use crate::detectors::complexity::{
    AstComplexityAnalyzer, ComplexityAnalysisResult, ComplexityConfig,
    ComplexityMetrics as AnalyzerComplexityMetrics,
};
use crate::lang::{adapter_for_file, EntityKind, ParseIndex, ParsedEntity};

/// Minimum tokens required before we consider a block a meaningful duplication target
const DUPLICATE_MIN_TOKEN_COUNT: usize = 10;
/// Minimum lines required to consider a block large enough for duplication checks
const DUPLICATE_MIN_LINE_COUNT: usize = 4;
/// Threshold for marking a function as long
const LONG_METHOD_LINE_THRESHOLD: usize = 50;
/// Threshold for marking a class as too large
const LARGE_CLASS_LINE_THRESHOLD: usize = 200;
/// Threshold for number of member entities in a class before recommending extraction
const LARGE_CLASS_MEMBER_THRESHOLD: usize = 12;
/// Logical operator count that suggests a complex conditional
const COMPLEX_CONDITIONAL_THRESHOLD: usize = 4;

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

impl Default for RefactoringConfig {
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
pub struct RefactoringAnalyzer {
    config: RefactoringConfig,
    ast_service: Arc<AstService>,
    complexity_analyzer: AstComplexityAnalyzer,
}

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
            if let Some(metrics) = self.lookup_complexity_metrics(entity, start_line, complexity) {
                if let Ok(value) = serde_json::to_value(&metrics) {
                    code_entity.add_property(PROP_COMPLEXITY_METRICS, value);
                }
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
            for child_id in &entity.children {
                if let Some(child) = index.entities.get(child_id) {
                    if matches!(child.kind, EntityKind::Function | EntityKind::Method) {
                        *counts.entry(entity.id.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        counts
    }

    fn is_function_entity(entity: &CodeEntity) -> bool {
        let kind = entity.entity_type.as_str();
        kind.eq_ignore_ascii_case("function") || kind.eq_ignore_ascii_case("method")
    }

    fn is_type_entity(entity: &CodeEntity) -> bool {
        let kind = entity.entity_type.as_str();
        kind.eq_ignore_ascii_case("class")
            || kind.eq_ignore_ascii_case("struct")
            || kind.eq_ignore_ascii_case("interface")
            || kind.eq_ignore_ascii_case("enum")
    }

    fn entity_complexity(entity: &CodeEntity) -> Option<AnalyzerComplexityMetrics> {
        entity
            .properties
            .get(PROP_COMPLEXITY_METRICS)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }

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

    fn member_count_from_entity(entity: &CodeEntity) -> usize {
        entity
            .properties
            .get(PROP_MEMBER_COUNT)
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(0)
    }

    fn entity_location(entity: &CodeEntity) -> (usize, usize) {
        entity
            .line_range
            .map(|(start, end)| (start, end.max(start)))
            .unwrap_or((1, entity.line_count()))
    }

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
        match kind {
            "comment" | "block_comment" | "line_comment" => return,
            "identifier"
            | "field_identifier"
            | "property_identifier"
            | "shorthand_property_identifier_pattern"
            | "member_expression"
            | "scoped_identifier" => tokens.push("IDENT".to_string()),
            "type_identifier" | "primitive_type" => tokens.push("TYPE".to_string()),
            "string" | "string_literal" | "raw_string_literal" => tokens.push("STRING".to_string()),
            "number" | "integer" | "float" | "decimal_literal" | "float_literal" => {
                tokens.push("NUMBER".to_string())
            }
            "true" | "false" => tokens.push("BOOL".to_string()),
            "null" | "nil" => tokens.push("NULL".to_string()),
            _ => tokens.push(kind.to_string()),
        }

        if matches!(
            kind,
            "binary_expression" | "assignment_expression" | "logical_expression"
        ) {
            if let Some(operator) = node.child_by_field_name("operator") {
                if let Some(text) = node_text(operator, source) {
                    tokens.push(format!("OP:{}", text.trim()));
                }
            }
        }

        if matches!(kind, "call_expression" | "call") {
            let arg_count = node
                .child_by_field_name("arguments")
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
                });
            tokens.push(format!("CALL_ARGS:{}", arg_count));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_fingerprint_tokens(child, source, tokens);
        }
    }

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

    fn detect_long_methods(&self, functions: &[CodeEntity]) -> Vec<RefactoringRecommendation> {
        let mut recommendations = Vec::new();

        for function in functions {
            let complexity = Self::entity_complexity(function);
            let loc = complexity
                .as_ref()
                .map(|metrics| metrics.lines_of_code.max(function.line_count() as f64))
                .unwrap_or(function.line_count() as f64);

            if loc < LONG_METHOD_LINE_THRESHOLD as f64 {
                continue;
            }

            let cyclomatic = complexity
                .as_ref()
                .map(|metrics| metrics.cyclomatic_complexity)
                .unwrap_or(0.0);

            let impact = ((loc / 8.0) + (cyclomatic / 2.0)).min(10.0);
            let effort = 4.0 + (loc / 70.0).min(4.0);
            let priority = (impact / effort).max(0.1);
            let loc_display = loc.round() as usize;
            let complexity_note = if cyclomatic > 0.0 {
                format!(" with cyclomatic {:.1}", cyclomatic)
            } else {
                String::new()
            };

            recommendations.push(RefactoringRecommendation {
                refactoring_type: RefactoringType::ExtractMethod,
                description: format!(
                    "Function `{}` spans {} lines{}. Extract helper functions to improve cohesion.",
                    function.name, loc_display, complexity_note
                ),
                estimated_impact: impact,
                estimated_effort: effort,
                priority_score: priority,
                location: Self::entity_location(function),
            });
        }

        recommendations
    }

    fn detect_complex_conditionals(
        &self,
        functions: &[CodeEntity],
    ) -> Vec<RefactoringRecommendation> {
        let mut recommendations = Vec::new();

        for function in functions {
            let operator_complexity = estimate_logical_operator_complexity(&function.source_code);
            let complexity = Self::entity_complexity(function);
            let (logical_complexity, cognitive_complexity) = match &complexity {
                Some(metrics) => {
                    let combined = metrics.decision_points.len().max(operator_complexity);
                    let cognitive = if metrics.cognitive_complexity > 0.0 {
                        metrics.cognitive_complexity
                    } else {
                        combined as f64
                    };
                    (combined, cognitive)
                }
                None => (operator_complexity, operator_complexity as f64),
            };

            if logical_complexity < COMPLEX_CONDITIONAL_THRESHOLD {
                continue;
            }

            let impact = (cognitive_complexity * 1.5).min(10.0).max(5.0);
            let effort = 3.5;
            let priority = (impact / effort).max(0.1);

            recommendations.push(RefactoringRecommendation {
                refactoring_type: RefactoringType::SimplifyConditionals,
                description: format!(
                    "Function `{}` contains {} decision points (cognitive {:.1}). Consider guard clauses or breaking the logic into smaller helpers.",
                    function.name, logical_complexity, cognitive_complexity
                ),
                estimated_impact: impact,
                estimated_effort: effort,
                priority_score: priority,
                location: Self::entity_location(function),
            });
        }

        recommendations
    }

    fn detect_duplicate_code(&self, functions: &[CodeEntity]) -> Vec<RefactoringRecommendation> {
        let mut buckets: HashMap<u64, Vec<&CodeEntity>> = HashMap::new();

        for function in functions {
            if function.line_count() < DUPLICATE_MIN_LINE_COUNT {
                continue;
            }

            if let Some((fingerprint, complexity)) = Self::duplicate_signature(function) {
                if complexity >= DUPLICATE_MIN_TOKEN_COUNT {
                    buckets.entry(fingerprint).or_default().push(function);
                }
            }
        }

        let mut recommendations = Vec::new();

        for duplicates in buckets.values() {
            if duplicates.len() < 2 {
                continue;
            }

            let names: Vec<String> = duplicates.iter().map(|f| f.name.clone()).collect();
            let names_display = names.join(", ");

            for function in duplicates {
                let impact = (function.line_count() as f64 / 8.0).min(10.0).max(6.0);
                let effort = 5.5;
                let priority = (impact / effort).max(0.1);

                recommendations.push(RefactoringRecommendation {
                    refactoring_type: RefactoringType::EliminateDuplication,
                    description: format!(
                        "Function `{}` shares near-identical implementation with [{}]. Consolidate shared logic into a reusable helper.",
                        function.name, names_display
                    ),
                    estimated_impact: impact,
                    estimated_effort: effort,
                    priority_score: priority,
                    location: Self::entity_location(function),
                });
            }
        }

        recommendations
    }

    fn detect_large_types(&self, types: &[CodeEntity]) -> Vec<RefactoringRecommendation> {
        let mut recommendations = Vec::new();

        for entity in types {
            let line_count = entity.line_count();
            let member_count = Self::member_count_from_entity(entity);

            if line_count < LARGE_CLASS_LINE_THRESHOLD
                && member_count < LARGE_CLASS_MEMBER_THRESHOLD
            {
                continue;
            }

            let impact = ((line_count as f64 / 20.0) + member_count as f64 * 0.5)
                .min(10.0)
                .max(5.0);
            let effort = 7.5;
            let priority = (impact / effort).max(0.1);

            recommendations.push(RefactoringRecommendation {
                refactoring_type: RefactoringType::ExtractClass,
                description: format!(
                    "Type `{}` spans {} lines with {} members. Split responsibilities into focused components.",
                    entity.name, line_count, member_count
                ),
                estimated_impact: impact,
                estimated_effort: effort,
                priority_score: priority,
                location: Self::entity_location(entity),
            });
        }

        recommendations
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

pub struct RefactoringExtractor {
    analyzer: Arc<RefactoringAnalyzer>,
    feature_definitions: Vec<FeatureDefinition>,
    file_cache: DashMap<String, Arc<RefactoringAnalysisResult>>,
}

impl RefactoringExtractor {
    /// Create a refactoring extractor backed by the provided analyzer
    pub fn new(analyzer: RefactoringAnalyzer) -> Self {
        let feature_definitions = vec![
            FeatureDefinition::new(
                "refactoring_recommendation_count",
                "Number of refactoring opportunities detected for this entity",
            )
            .with_range(0.0, 50.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_total_impact",
                "Sum of estimated impact values for matching refactoring recommendations",
            )
            .with_range(0.0, 200.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_avg_impact",
                "Average estimated impact for matching refactoring recommendations",
            )
            .with_range(0.0, 10.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_avg_priority",
                "Average priority score for matching refactoring recommendations",
            )
            .with_range(0.0, 10.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_max_priority",
                "Highest priority score among matching refactoring recommendations",
            )
            .with_range(0.0, 10.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_file_score",
                "Overall refactoring score for the containing file",
            )
            .with_range(0.0, 100.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_extract_method_count",
                "Occurrences of extract-method opportunities",
            )
            .with_range(0.0, 50.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_extract_class_count",
                "Occurrences of extract-class opportunities",
            )
            .with_range(0.0, 50.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_duplicate_code_count",
                "Occurrences of duplicate-code elimination opportunities",
            )
            .with_range(0.0, 50.0)
            .with_default(0.0),
            FeatureDefinition::new(
                "refactoring_simplify_conditionals_count",
                "Occurrences of complex conditional simplification opportunities",
            )
            .with_range(0.0, 50.0)
            .with_default(0.0),
        ];

        Self {
            analyzer: Arc::new(analyzer),
            feature_definitions,
            file_cache: DashMap::new(),
        }
    }

    /// Construct an extractor with explicit configuration and AST service
    pub fn with_config(config: RefactoringConfig, ast_service: Arc<AstService>) -> Self {
        Self::new(RefactoringAnalyzer::new(config, ast_service))
    }

    /// Fetch (and cache) the refactoring analysis for a file
    async fn file_analysis(&self, file_path: &str) -> Result<Arc<RefactoringAnalysisResult>> {
        let key = normalize_path(file_path);

        if let Some(entry) = self.file_cache.get(&key) {
            return Ok(entry.clone());
        }

        let path = PathBuf::from(file_path);
        match self.analyzer.analyze_file(&path).await {
            Ok(result) => {
                let arc = Arc::new(result);
                self.file_cache.insert(key, arc.clone());
                Ok(arc)
            }
            Err(error) => {
                warn!(
                    "Refactoring extractor failed to analyze {}: {}",
                    file_path, error
                );
                let placeholder = Arc::new(RefactoringAnalysisResult {
                    file_path: file_path.to_string(),
                    recommendations: Vec::new(),
                    refactoring_score: 0.0,
                });
                self.file_cache.insert(key, placeholder.clone());
                Ok(placeholder)
            }
        }
    }

    /// Initialise the feature vector with configured defaults
    fn initialise_feature_map(&self) -> HashMap<String, f64> {
        let mut map = HashMap::with_capacity(self.feature_definitions.len());
        for definition in &self.feature_definitions {
            map.insert(definition.name.clone(), definition.default_value);
        }
        map
    }
}

impl Default for RefactoringExtractor {
    fn default() -> Self {
        Self::new(RefactoringAnalyzer::default())
    }
}

#[async_trait]
impl FeatureExtractor for RefactoringExtractor {
    fn name(&self) -> &str {
        "refactoring"
    }
    fn features(&self) -> &[FeatureDefinition] {
        &self.feature_definitions
    }
    async fn extract(
        &self,
        entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = self.initialise_feature_map();

        // Attempt to load analysis for the containing file
        let analysis = self.file_analysis(&entity.file_path).await?;

        let entity_range = entity.line_range.unwrap_or_else(|| {
            let lines = entity.line_count().max(1);
            (1, lines)
        });

        let mut total_impact = 0.0_f64;
        let mut total_priority = 0.0_f64;
        let mut recommendations_considered = 0.0_f64;
        let mut max_priority = 0.0_f64;
        let mut extract_method = 0.0_f64;
        let mut extract_class = 0.0_f64;
        let mut eliminate_duplication = 0.0_f64;
        let mut simplify_conditionals = 0.0_f64;

        for recommendation in &analysis.recommendations {
            let location = recommendation.location;
            if !ranges_overlap(entity_range, location) {
                continue;
            }

            recommendations_considered += 1.0;
            total_impact += recommendation.estimated_impact;
            total_priority += recommendation.priority_score;
            max_priority = max_priority.max(recommendation.priority_score);

            match recommendation.refactoring_type {
                RefactoringType::ExtractMethod => extract_method += 1.0,
                RefactoringType::ExtractClass => extract_class += 1.0,
                RefactoringType::EliminateDuplication => {
                    eliminate_duplication += 1.0;
                }
                RefactoringType::SimplifyConditionals => simplify_conditionals += 1.0,
                RefactoringType::ReduceComplexity
                | RefactoringType::ImproveNaming
                | RefactoringType::RemoveDeadCode => {
                    // Keep hook for future detailed features
                }
            }
        }

        if recommendations_considered > 0.0 {
            let avg_impact = total_impact / recommendations_considered;
            let avg_priority = total_priority / recommendations_considered;

            features.insert(
                "refactoring_recommendation_count".to_string(),
                recommendations_considered,
            );
            features.insert("refactoring_total_impact".to_string(), total_impact);
            features.insert("refactoring_avg_impact".to_string(), avg_impact);
            features.insert("refactoring_avg_priority".to_string(), avg_priority);
            features.insert("refactoring_max_priority".to_string(), max_priority);
            features.insert(
                "refactoring_extract_method_count".to_string(),
                extract_method,
            );
            features.insert("refactoring_extract_class_count".to_string(), extract_class);
            features.insert(
                "refactoring_duplicate_code_count".to_string(),
                eliminate_duplication,
            );
            features.insert(
                "refactoring_simplify_conditionals_count".to_string(),
                simplify_conditionals,
            );
        }

        // Propagate the file-level refactoring score regardless of overlap results
        features.insert(
            "refactoring_file_score".to_string(),
            analysis.refactoring_score,
        );

        Ok(features)
    }
}

fn ranges_overlap(lhs: (usize, usize), rhs: (usize, usize)) -> bool {
    let (lhs_start, lhs_end) = lhs;
    let (rhs_start, rhs_end) = rhs;

    lhs_start <= rhs_end && rhs_start <= lhs_end
}

fn normalize_path(path: &str) -> String {
    Path::new(path).to_string_lossy().into_owned()
}

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
fn estimate_logical_operator_complexity(snippet: &str) -> usize {
    let mut count = 0;

    for line in snippet.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }

        count += trimmed.matches("&&").count();
        count += trimmed.matches("||").count();
    }

    count
        + snippet
            .split(|c: char| !c.is_alphabetic())
            .filter(|token| matches!(token.to_ascii_lowercase().as_str(), "and" | "or"))
            .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    use crate::core::config::ValknutConfig;
    use crate::core::featureset::{CodeEntity, ExtractionContext};

    fn analyzer() -> RefactoringAnalyzer {
        RefactoringAnalyzer::new(RefactoringConfig::default(), Arc::new(AstService::new()))
    }

    #[test]
    fn test_refactoring_config_default() {
        let config = RefactoringConfig::default();
        assert!(config.enabled);
        assert_eq!(config.min_impact_threshold, 5.0);
    }

    #[test]
    fn test_refactoring_analyzer_creation() {
        let ast_service = Arc::new(AstService::new());
        let analyzer = RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service);
        assert!(analyzer.config.enabled);
    }

    #[tokio::test]
    async fn test_analyze_files_disabled() {
        let config = RefactoringConfig {
            enabled: false,
            min_impact_threshold: 5.0,
        };
        let analyzer = RefactoringAnalyzer::new(config, Arc::new(AstService::new()));

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        fs::write(&file_path, "def test_function():\n    pass").unwrap();

        let paths = vec![file_path];
        let results = analyzer.analyze_files(&paths).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_detects_long_method() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("long_function.py");
        let mut content = String::from("def long_function():\n");
        for i in 0..65 {
            content.push_str(&format!("    value = {}\n", i));
        }
        fs::write(&file_path, content).unwrap();

        let analyzer = analyzer();
        let results = analyzer.analyze_files(&[file_path.clone()]).await.unwrap();
        assert_eq!(results.len(), 1);
        let has_extract_method = results[0]
            .recommendations
            .iter()
            .any(|rec| rec.refactoring_type == RefactoringType::ExtractMethod);
        assert!(has_extract_method, "Expected long method recommendation");
    }

    #[tokio::test]
    async fn test_detects_complex_conditionals() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("complex_condition.py");
        let content = r#"
def complex_condition(a, b, c, d):
    if (a and b) or (c and d) or (a and c and d):
        return True
    return False
"#;
        fs::write(&file_path, content).unwrap();

        let analyzer = analyzer();
        let results = analyzer.analyze_files(&[file_path.clone()]).await.unwrap();
        assert_eq!(results.len(), 1);
        let has_complexity = results[0]
            .recommendations
            .iter()
            .any(|rec| rec.refactoring_type == RefactoringType::SimplifyConditionals);
        assert!(
            has_complexity,
            "Expected complex conditional recommendation"
        );
    }

    #[tokio::test]
    async fn test_detects_duplicate_functions() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("duplicates.py");
        let content = r#"
def helper():
    total = 0
    for i in range(10):
        total += i * 2
        if total % 3 == 0:
            total -= 1
        else:
            total += 1
    return total

def helper_copy():
    total = 0
    for i in range(10):
        total += i * 2
        if total % 3 == 0:
            total -= 1
        else:
            total += 1
    return total
"#;
        fs::write(&file_path, content).unwrap();

        let analyzer = analyzer();
        let source = fs::read_to_string(&file_path).unwrap();
        let mut adapter = crate::lang::python::PythonAdapter::new().unwrap();
        let file_path_str = file_path.to_string_lossy().to_string();
        let parse_index = adapter.parse_source(&source, &file_path_str).unwrap();
        let ast_service = Arc::new(AstService::new());
        let cached_tree = ast_service.get_ast(&file_path_str, &source).await.unwrap();
        let ast_context = ast_service.create_context(&cached_tree, &file_path_str);
        let complexity_map = HashMap::<String, ComplexityAnalysisResult>::new();
        let summaries = analyzer
            .collect_entity_summaries(&parse_index, &source, &complexity_map, &ast_context)
            .unwrap();
        assert!(
            summaries
                .iter()
                .filter(|s| RefactoringAnalyzer::is_function_entity(s))
                .count()
                >= 2
        );
        let duplicate_ready = summaries
            .iter()
            .filter(|s| RefactoringAnalyzer::is_function_entity(s))
            .filter(|s| RefactoringAnalyzer::duplicate_signature(s).is_some())
            .count();
        assert!(
            duplicate_ready >= 2,
            "expected duplicate fingerprints to be present"
        );

        let results = analyzer.analyze_files(&[file_path.clone()]).await.unwrap();
        assert_eq!(results.len(), 1);
        let has_duplicate = results[0]
            .recommendations
            .iter()
            .any(|rec| rec.refactoring_type == RefactoringType::EliminateDuplication);
        assert!(has_duplicate, "Expected duplicate code recommendation");
    }

    #[tokio::test]
    async fn test_detects_large_class() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("large_class.py");
        let mut content = String::from("class HugeClass:\n");
        for i in 0..30 {
            content.push_str(&format!("    def method_{}(self):\n", i));
            content.push_str("        result = 0\n");
            for j in 0..10 {
                content.push_str(&format!("        result += {}\n", j));
            }
            content.push_str("        return result\n\n");
        }
        fs::write(&file_path, content).unwrap();

        let analyzer = analyzer();
        let results = analyzer.analyze_files(&[file_path.clone()]).await.unwrap();
        assert_eq!(results.len(), 1);
        let has_large_class = results[0]
            .recommendations
            .iter()
            .any(|rec| rec.refactoring_type == RefactoringType::ExtractClass);
        assert!(has_large_class, "Expected large class recommendation");
    }

    #[tokio::test]
    async fn test_refactoring_extractor_produces_features() {
        use crate::core::config::ValknutConfig;
        use crate::core::featureset::{CodeEntity, ExtractionContext};

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("long_refactor.py");

        let mut content = String::from("def long_function():\n");
        for i in 0..70 {
            content.push_str(&format!("    value = {}\n", i));
        }
        tokio::fs::write(&file_path, &content).await.unwrap();

        let entity = CodeEntity::new(
            "entity::long_function",
            "function",
            "long_function",
            file_path.to_string_lossy(),
        )
        .with_line_range(1, content.lines().count())
        .with_source_code(content.clone());

        let mut context = ExtractionContext::new(Arc::new(ValknutConfig::default()), "python");
        context.add_entity(entity.clone());

        let extractor = RefactoringExtractor::default();
        let features = extractor.extract(&entity, &context).await.unwrap();

        let recommendation_count = features
            .get("refactoring_recommendation_count")
            .copied()
            .unwrap_or_default();
        assert!(recommendation_count >= 1.0);

        assert!(
            features
                .get("refactoring_file_score")
                .copied()
                .unwrap_or_default()
                >= 0.0
        );
    }
}
