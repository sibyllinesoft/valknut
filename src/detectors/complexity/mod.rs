//! AST-based complexity analysis detector - CORRECT implementation
//!
//! This module replaces the text-based complexity analysis with proper AST-based
//! calculation using the central AST service for accurate complexity metrics.

mod extractor;
mod halstead;
pub mod types;

pub use extractor::AstComplexityExtractor;

use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::core::ast_service::{AstService, ComplexityMetrics as AstComplexityMetrics};
use crate::core::ast_utils::find_entity_node;
use crate::core::errors::Result;
use crate::core::featureset::{CodeEntity, EntityId};

// Re-export types from submodule
pub use types::{
    ComplexityAnalysisResult, ComplexityConfig, ComplexityIssue, ComplexityIssueType,
    ComplexityMetrics, ComplexitySeverity, ComplexityThresholds, DecisionPointInfo,
    HalsteadMetrics,
};

/// AST-based complexity analyzer - the CORRECT implementation
#[derive(Clone)]
pub struct AstComplexityAnalyzer {
    config: ComplexityConfig,
    ast_service: Arc<AstService>,
}

/// Type alias for backwards compatibility
pub type ComplexityAnalyzer = AstComplexityAnalyzer;

/// Factory, analysis, and metrics calculation methods for [`AstComplexityAnalyzer`].
impl AstComplexityAnalyzer {
    /// Create new AST-based complexity analyzer
    pub fn new(config: ComplexityConfig, ast_service: Arc<AstService>) -> Self {
        Self {
            config,
            ast_service,
        }
    }

    /// Analyze multiple files for compatibility with pipeline
    pub async fn analyze_files(
        &self,
        file_paths: &[&std::path::Path],
    ) -> Result<Vec<crate::detectors::complexity::ComplexityAnalysisResult>> {
        use tokio::fs;

        let mut all_results = Vec::new();

        for file_path in file_paths {
            match fs::read_to_string(file_path).await {
                Ok(source) => {
                    match self
                        .analyze_file_with_results(file_path.to_string_lossy().as_ref(), &source)
                        .await
                    {
                        Ok(mut results) => all_results.extend(results),
                        Err(e) => warn!("Failed to analyze {}: {}", file_path.display(), e),
                    }
                }
                Err(e) => warn!("Failed to read {}: {}", file_path.display(), e),
            }
        }

        Ok(all_results)
    }

    /// Analyze complexity of a source file using AST and return structured results
    pub async fn analyze_file_with_results(
        &self,
        file_path: &str,
        source: &str,
    ) -> Result<Vec<crate::detectors::complexity::ComplexityAnalysisResult>> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }

        debug!("Analyzing complexity for file: {}", file_path);

        let cached_tree = self.ast_service.get_ast(file_path, source).await?;
        let context = self.ast_service.create_context(&cached_tree, file_path);
        let ast_metrics = self.ast_service.calculate_complexity(&context)?;
        let entities = self.extract_entities_from_ast(&context)?;

        let mut results = Vec::new();
        for entity in entities {
            let metrics = self.calculate_entity_ast_metrics(&entity, &ast_metrics, &context)?;
            let result = self.build_analysis_result(&entity, file_path, metrics);
            results.push(result);
        }

        Ok(results)
    }

    /// Build a ComplexityAnalysisResult from an entity and its metrics.
    fn build_analysis_result(
        &self,
        entity: &CodeEntity,
        file_path: &str,
        metrics: ComplexityMetrics,
    ) -> ComplexityAnalysisResult {
        let issues = self.generate_issues_from_metrics(&entity.id, &metrics);
        let start_line = entity.line_range.map(|(start, _)| start).unwrap_or(1);

        ComplexityAnalysisResult {
            entity_id: entity.id.clone(),
            entity_name: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            file_path: file_path.to_string(),
            line_number: start_line,
            start_line,
            metrics: metrics.clone(),
            severity: self.determine_complexity_severity(&metrics),
            issues: issues
                .into_iter()
                .map(|issue| self.convert_issue(&entity.id, file_path, start_line, issue))
                .collect(),
            recommendations: Vec::new(),
        }
    }

    /// Convert an internal ComplexityIssue to the output format.
    fn convert_issue(
        &self,
        entity_id: &str,
        file_path: &str,
        start_line: usize,
        issue: ComplexityIssue,
    ) -> ComplexityIssue {
        let issue_type = Self::parse_issue_type(&issue.issue_type);
        let severity = Self::parse_severity(&issue.severity);

        ComplexityIssue {
            entity_id: entity_id.to_string(),
            issue_type: format!("{:?}", issue_type),
            description: issue.description,
            severity: format!("{:?}", severity),
            recommendation: issue.recommendation,
            location: format!("{}:{}", file_path, start_line),
            metric_value: issue.metric_value,
            threshold: issue.threshold,
        }
    }

    /// Parse issue type string to enum.
    fn parse_issue_type(type_str: &str) -> ComplexityIssueType {
        match type_str {
            "high_cyclomatic_complexity" => ComplexityIssueType::HighCyclomaticComplexity,
            "high_cognitive_complexity" => ComplexityIssueType::HighCognitiveComplexity,
            "excessive_nesting" => ComplexityIssueType::DeepNesting,
            "too_many_parameters" => ComplexityIssueType::TooManyParameters,
            "large_file" => ComplexityIssueType::LongFile,
            _ => ComplexityIssueType::HighTechnicalDebt,
        }
    }

    /// Parse severity string to enum.
    fn parse_severity(severity_str: &str) -> ComplexitySeverity {
        match severity_str {
            "low" => ComplexitySeverity::Low,
            "medium" => ComplexitySeverity::Moderate,
            "high" => ComplexitySeverity::High,
            "critical" => ComplexitySeverity::Critical,
            _ => ComplexitySeverity::Moderate,
        }
    }

    /// Determine complexity severity based on metrics
    fn determine_complexity_severity(
        &self,
        metrics: &ComplexityMetrics,
    ) -> crate::detectors::complexity::ComplexitySeverity {
        if metrics.cyclomatic_complexity >= self.config.cyclomatic_thresholds.very_high
            || metrics.cognitive_complexity >= self.config.cognitive_thresholds.very_high
        {
            crate::detectors::complexity::ComplexitySeverity::Critical
        } else if metrics.cyclomatic_complexity >= self.config.cyclomatic_thresholds.high
            || metrics.cognitive_complexity >= self.config.cognitive_thresholds.high
        {
            crate::detectors::complexity::ComplexitySeverity::High
        } else if metrics.cyclomatic_complexity >= self.config.cyclomatic_thresholds.medium
            || metrics.cognitive_complexity >= self.config.cognitive_thresholds.medium
        {
            crate::detectors::complexity::ComplexitySeverity::Moderate
        } else {
            crate::detectors::complexity::ComplexitySeverity::Low
        }
    }

    /// Analyze complexity of a source file using AST
    pub async fn analyze_file(
        &self,
        file_path: &str,
        source: &str,
    ) -> Result<Vec<ComplexityIssue>> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }

        debug!("Analyzing complexity for file: {}", file_path);

        // Get AST from service
        let cached_tree = self.ast_service.get_ast(file_path, source).await?;
        let context = self.ast_service.create_context(&cached_tree, file_path);

        // Calculate real AST-based complexity
        let ast_metrics = self.ast_service.calculate_complexity(&context)?;

        // Extract entities and calculate per-entity metrics
        let entities = self.extract_entities_from_ast(&context)?;
        let mut issues = Vec::new();

        for entity in entities {
            let metrics = self.calculate_entity_ast_metrics(&entity, &ast_metrics, &context)?;
            let entity_issues = self.generate_issues_from_metrics(&entity.id, &metrics);
            issues.extend(entity_issues);
        }

        // Add file-level complexity issues
        let file_issues = self.generate_file_level_issues(file_path, source, &ast_metrics)?;
        issues.extend(file_issues);

        info!("Found {} complexity issues in {}", issues.len(), file_path);
        Ok(issues)
    }

    /// Extract entities from AST context
    fn extract_entities_from_ast(
        &self,
        context: &crate::core::ast_service::AstContext<'_>,
    ) -> Result<Vec<CodeEntity>> {
        let mut entities = Vec::new();
        let root_node = context.tree.root_node();

        self.traverse_for_entities(&root_node, context, &mut entities, 0)?;

        Ok(entities)
    }

    /// Recursively traverse AST to extract code entities
    fn traverse_for_entities(
        &self,
        node: &tree_sitter::Node,
        context: &crate::core::ast_service::AstContext<'_>,
        entities: &mut Vec<CodeEntity>,
        depth: usize,
    ) -> Result<()> {
        // Extract functions, methods, classes - supporting multiple languages
        match node.kind() {
            // Python function patterns
            "function_definition" 
            // JavaScript/TypeScript function patterns
            | "function_declaration" | "function_expression" | "arrow_function" | "method_definition"
            // Rust function patterns  
            | "function_item" 
            // Go function patterns
            | "method_declaration" => {
                if let Some(entity) = self.extract_function_entity(node, context, depth)? {
                    entities.push(entity);
                }
            }
            // Python/JavaScript class patterns
            "class_definition" | "class_declaration"
            // Rust struct/impl patterns 
            | "struct_item" | "impl_item" => {
                if let Some(entity) = self.extract_class_entity(node, context, depth)? {
                    entities.push(entity);
                }
            }
            _ => {}
        }

        // Continue traversing children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.traverse_for_entities(&child, context, entities, depth + 1)?;
        }

        Ok(())
    }

    /// Extract function entity from AST node
    fn extract_function_entity(
        &self,
        node: &tree_sitter::Node,
        context: &crate::core::ast_service::AstContext<'_>,
        depth: usize,
    ) -> Result<Option<CodeEntity>> {
        // Get function name
        let name = if let Some(name_node) = node.child_by_field_name("name") {
            self.get_node_text(name_node, context.source)
        } else {
            format!("anonymous_function_{}", node.start_position().row)
        };

        // Get function body
        let body_text = self.get_node_text(*node, context.source);

        let mut entity = CodeEntity::new(
            format!(
                "{}:{}:{}",
                context.file_path,
                name,
                node.start_position().row
            ),
            "function",
            name,
            context.file_path,
        )
        .with_line_range(node.start_position().row + 1, node.end_position().row + 1)
        .with_source_code(body_text);

        entity.add_property("start_byte", json!(node.start_byte()));
        entity.add_property("end_byte", json!(node.end_byte()));
        entity.add_property("ast_kind", json!(node.kind()));

        Ok(Some(entity))
    }

    /// Extract class entity from AST node
    fn extract_class_entity(
        &self,
        node: &tree_sitter::Node,
        context: &crate::core::ast_service::AstContext<'_>,
        depth: usize,
    ) -> Result<Option<CodeEntity>> {
        let name = if let Some(name_node) = node.child_by_field_name("name") {
            self.get_node_text(name_node, context.source)
        } else {
            format!("anonymous_class_{}", node.start_position().row)
        };

        let body_text = self.get_node_text(*node, context.source);

        let mut entity = CodeEntity::new(
            format!(
                "{}:{}:{}",
                context.file_path,
                name,
                node.start_position().row
            ),
            "class",
            name,
            context.file_path,
        )
        .with_line_range(node.start_position().row + 1, node.end_position().row + 1)
        .with_source_code(body_text);

        entity.add_property("start_byte", json!(node.start_byte()));
        entity.add_property("end_byte", json!(node.end_byte()));
        entity.add_property("ast_kind", json!(node.kind()));

        Ok(Some(entity))
    }

    /// Get text content of an AST node
    fn get_node_text(&self, node: tree_sitter::Node, source: &str) -> String {
        let start = node.start_byte();
        let end = node.end_byte();

        // Ensure bounds are valid - clamp and check
        let source_len = source.len();
        let clamped_start = std::cmp::min(start as usize, source_len);
        let clamped_end = std::cmp::min(end as usize, source_len);

        if clamped_start > clamped_end {
            debug!(
                "Invalid node range: start={}, end={}, source_len={}",
                start, end, source_len
            );
            return String::new();
        }

        source[clamped_start..clamped_end].to_string()
    }

    /// Calculate AST-based complexity metrics for an entity
    fn calculate_entity_ast_metrics(
        &self,
        entity: &CodeEntity,
        ast_metrics: &AstComplexityMetrics,
        context: &crate::core::ast_service::AstContext<'_>,
    ) -> Result<ComplexityMetrics> {
        // Convert AST metrics to our format
        let decision_points: Vec<DecisionPointInfo> = ast_metrics
            .decision_points
            .iter()
            .filter(|dp| {
                // Filter decision points that belong to this entity
                entity.line_range.map_or(false, |(start, end)| {
                    dp.location.start_line >= start && dp.location.end_line <= end
                })
            })
            .map(|dp| DecisionPointInfo {
                kind: format!("{:?}", dp.kind),
                line: dp.location.start_line,
                column: dp.location.start_column,
                nesting_level: dp.nesting_level,
            })
            .collect();

        // Calculate entity-specific metrics
        let entity_cyclomatic = if decision_points.is_empty() {
            1.0
        } else {
            1.0 + decision_points.len() as f64
        };
        let entity_cognitive = decision_points
            .iter()
            .map(|dp| 1.0 + dp.nesting_level as f64)
            .sum::<f64>();
        let entity_nesting = decision_points
            .iter()
            .map(|dp| dp.nesting_level as f64)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        // Calculate additional metrics
        let lines_of_code = entity.line_count() as f64;
        let parameter_count = self.count_parameters_in_entity(entity, context)?;
        let statement_count = self.count_statements_in_entity(entity, context)?;
        let halstead = self.calculate_halstead_for_entity(entity, context)?;
        let maintainability_index =
            self.calculate_maintainability_index(entity_cyclomatic, lines_of_code, &halstead);

        let metrics = ComplexityMetrics {
            cyclomatic_complexity: entity_cyclomatic,
            cognitive_complexity: entity_cognitive,
            max_nesting_depth: entity_nesting,
            parameter_count,
            lines_of_code,
            statement_count,
            halstead,
            technical_debt_score: self.calculate_technical_debt(
                entity_cyclomatic,
                entity_cognitive,
                lines_of_code,
            ),
            maintainability_index,
            decision_points,
        };

        Ok(metrics)
    }

    /// Count parameters in a function entity
    fn count_parameters_in_entity(
        &self,
        entity: &CodeEntity,
        context: &crate::core::ast_service::AstContext<'_>,
    ) -> Result<f64> {
        if entity.entity_type != "function" && entity.entity_type != "method" {
            return Ok(0.0);
        }

        let Some(node) = find_entity_node(context, entity) else {
            return Ok(0.0);
        };

        if let Some(params_node) = self.locate_parameters_node(&node) {
            let count = self.count_parameter_entries(&params_node);
            return Ok(count as f64);
        }

        Ok(0.0)
    }

    /// Count statements in an entity
    fn count_statements_in_entity(
        &self,
        entity: &CodeEntity,
        context: &crate::core::ast_service::AstContext<'_>,
    ) -> Result<f64> {
        let Some(node) = find_entity_node(context, entity) else {
            return Ok(0.0);
        };

        let (start_line, end_line) = entity.line_range.unwrap_or((0, 0));
        let count = self.count_statement_nodes(&node, start_line, end_line);

        Ok(count as f64)
    }

    /// Calculate Halstead metrics for an entity
    fn calculate_halstead_for_entity(
        &self,
        entity: &CodeEntity,
        context: &crate::core::ast_service::AstContext<'_>,
    ) -> Result<HalsteadMetrics> {
        let Some(root_node) = find_entity_node(context, entity) else {
            return Ok(HalsteadMetrics::default());
        };

        Ok(halstead::calculate_halstead_for_node(root_node, context.source))
    }

    /// Locates the parameters node for a function AST node.
    fn locate_parameters_node<'a>(
        &self,
        node: &tree_sitter::Node<'a>,
    ) -> Option<tree_sitter::Node<'a>> {
        for field in [
            "parameters",
            "parameter_list",
            "parameter_clause",
            "formal_parameters",
        ] {
            if let Some(child) = node.child_by_field_name(field) {
                return Some(child);
            }
        }

        let mut cursor = node.walk();
        let mut candidate = None;
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind.contains("parameter_list")
                || kind == "parameters"
                || kind == "formal_parameters"
                || kind == "lambda_parameters"
                || kind == "parameter_clause"
            {
                candidate = Some(child);
                break;
            }
        }
        drop(cursor);
        candidate
    }

    /// Counts parameter entries within a parameters node.
    fn count_parameter_entries(&self, node: &tree_sitter::Node) -> usize {
        if self.is_parameter_entry(node) {
            return 1;
        }

        let mut count = 0;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_named() {
                continue;
            }

            if self.is_parameter_entry(&child) {
                count += 1;
            } else {
                count += self.count_parameter_entries(&child);
            }
        }

        count
    }

    /// Checks if an AST node represents a parameter entry.
    fn is_parameter_entry(&self, node: &tree_sitter::Node) -> bool {
        let kind = node.kind();
        if kind.ends_with("_parameters") {
            return false;
        }

        if kind.ends_with("_parameter") {
            return true;
        }

        matches!(
            kind,
            "parameter"
                | "required_parameter"
                | "optional_parameter"
                | "default_parameter"
                | "typed_parameter"
                | "parameter_declaration"
                | "parameter_specification"
                | "self_parameter"
                | "rest_parameter"
                | "identifier"
        )
    }

    /// Counts statement nodes within a given line range.
    fn count_statement_nodes(
        &self,
        node: &tree_sitter::Node,
        start_line: usize,
        end_line: usize,
    ) -> usize {
        let mut total = 0;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_named() {
                continue;
            }

            let child_start = child.start_position().row + 1;
            let child_end = child.end_position().row + 1;
            if child_end < start_line || child_start > end_line {
                continue;
            }

            if self.is_statement_kind(child.kind()) {
                total += 1;
            }

            total += self.count_statement_nodes(&child, start_line, end_line);
        }

        total
    }

    /// Checks if an AST node kind represents a statement.
    fn is_statement_kind(&self, kind: &str) -> bool {
        kind.ends_with("_statement")
            || kind.ends_with("_declaration")
            || matches!(
                kind,
                "expression_statement"
                    | "return_statement"
                    | "break_statement"
                    | "continue_statement"
                    | "yield_statement"
                    | "throw_statement"
                    | "raise_statement"
                    | "import_statement"
                    | "pass_statement"
                    | "variable_declaration"
                    | "lexical_declaration"
                    | "const_declaration"
            )
    }

    /// Calculate technical debt score
    fn calculate_technical_debt(&self, cyclomatic: f64, cognitive: f64, lines: f64) -> f64 {
        // Weighted combination of complexity factors
        let complexity_weight = 0.4;
        let cognitive_weight = 0.4;
        let size_weight = 0.2;

        let normalized_cyclomatic = (cyclomatic / 20.0).min(1.0); // Normalize to 0-1
        let normalized_cognitive = (cognitive / 50.0).min(1.0); // Normalize to 0-1
        let normalized_size = (lines / 100.0).min(1.0); // Normalize to 0-1

        (normalized_cyclomatic * complexity_weight
            + normalized_cognitive * cognitive_weight
            + normalized_size * size_weight)
            * 100.0
    }

    /// Calculate maintainability index
    fn calculate_maintainability_index(
        &self,
        cyclomatic: f64,
        lines: f64,
        halstead: &HalsteadMetrics,
    ) -> f64 {
        // Microsoft maintainability index formula
        let volume = if halstead.volume > 0.0 {
            halstead.volume
        } else {
            1.0
        };
        let mi = 171.0 - 5.2 * volume.ln() - 0.23 * cyclomatic - 16.2 * lines.ln();
        mi.max(0.0).min(100.0)
    }

    /// Generate complexity issues from metrics
    fn generate_issues_from_metrics(
        &self,
        entity_id: &EntityId,
        metrics: &ComplexityMetrics,
    ) -> Vec<ComplexityIssue> {
        let mut issues = Vec::new();

        self.check_metric_threshold(
            &mut issues,
            entity_id,
            metrics.cyclomatic_complexity,
            &self.config.cyclomatic_thresholds,
            "high_cyclomatic_complexity",
            "Cyclomatic complexity",
            "Consider breaking this function into smaller, more focused functions",
        );

        self.check_metric_threshold(
            &mut issues,
            entity_id,
            metrics.cognitive_complexity,
            &self.config.cognitive_thresholds,
            "high_cognitive_complexity",
            "Cognitive complexity",
            "Reduce nesting levels and simplify conditional logic",
        );

        self.check_metric_threshold(
            &mut issues,
            entity_id,
            metrics.max_nesting_depth,
            &self.config.nesting_thresholds,
            "excessive_nesting",
            "Maximum nesting depth",
            "Reduce nesting by using early returns or extracting functions",
        );

        issues
    }

    /// Check a metric against thresholds and add an issue if exceeded.
    fn check_metric_threshold(
        &self,
        issues: &mut Vec<ComplexityIssue>,
        entity_id: &EntityId,
        value: f64,
        thresholds: &ComplexityThresholds,
        issue_type: &str,
        metric_name: &str,
        recommendation: &str,
    ) {
        if value <= thresholds.high {
            return;
        }

        issues.push(ComplexityIssue {
            entity_id: entity_id.clone(),
            issue_type: issue_type.to_string(),
            severity: self.determine_severity(value, thresholds),
            description: format!("{} of {:.1} exceeds threshold", metric_name, value),
            recommendation: recommendation.to_string(),
            location: entity_id.clone(),
            metric_value: value,
            threshold: thresholds.high,
        });
    }

    /// Generate file-level complexity issues
    fn generate_file_level_issues(
        &self,
        file_path: &str,
        source: &str,
        ast_metrics: &AstComplexityMetrics,
    ) -> Result<Vec<ComplexityIssue>> {
        let mut issues = Vec::new();
        let line_count = source.lines().count() as f64;

        // Check file length
        if line_count > self.config.file_length_thresholds.high {
            issues.push(ComplexityIssue {
                entity_id: format!("file:{}", file_path),
                issue_type: "large_file".to_string(),
                severity: self.determine_severity(line_count, &self.config.file_length_thresholds),
                description: format!("File length of {:.0} lines exceeds threshold", line_count),
                recommendation: "Consider splitting this file into smaller, more focused modules"
                    .to_string(),
                location: file_path.to_string(),
                metric_value: line_count,
                threshold: self.config.file_length_thresholds.high,
            });
        }

        Ok(issues)
    }

    /// Determine severity based on thresholds
    fn determine_severity(&self, value: f64, thresholds: &ComplexityThresholds) -> String {
        if value >= thresholds.very_high {
            "critical".to_string()
        } else if value >= thresholds.high {
            "high".to_string()
        } else if value >= thresholds.medium {
            "medium".to_string()
        } else {
            "low".to_string()
        }
    }
}

#[cfg(test)]
mod tests;
