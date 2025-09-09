//! Refactoring analysis detector for identifying code improvement opportunities.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::featureset::{FeatureExtractor, FeatureDefinition, CodeEntity, ExtractionContext};
use crate::core::errors::Result;
use crate::core::file_utils::FileReader;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl RefactoringAnalyzer {
    /// Create new refactoring analyzer
    pub fn new(config: RefactoringConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(RefactoringConfig::default())
    }

    /// Analyze files for refactoring opportunities
    pub async fn analyze_files(&self, file_paths: &[PathBuf]) -> Result<Vec<RefactoringAnalysisResult>> {
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
                },
                Err(e) => warn!("Refactoring analysis failed for {}: {}", file_path.display(), e),
            }
        }

        info!("Refactoring analysis found {} files with opportunities", results.len());
        Ok(results)
    }

    /// Analyze a single file for refactoring opportunities
    async fn analyze_file(&self, file_path: &Path) -> Result<RefactoringAnalysisResult> {
        debug!("Analyzing refactoring opportunities for: {}", file_path.display());

        let content = FileReader::read_to_string(file_path)?;

        let mut recommendations = Vec::new();

        // Analyze for various refactoring opportunities
        recommendations.extend(self.detect_long_methods(&content));
        recommendations.extend(self.detect_complex_conditionals(&content));
        recommendations.extend(self.detect_duplicate_code(&content));
        recommendations.extend(self.detect_large_classes(&content));
        recommendations.extend(self.detect_dead_code(&content));

        // Filter by minimum impact threshold
        recommendations.retain(|rec| rec.estimated_impact >= self.config.min_impact_threshold);

        // Sort by priority (highest first)
        recommendations.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap());

        // Calculate overall refactoring score
        let refactoring_score = self.calculate_refactoring_score(&recommendations, &content);

        Ok(RefactoringAnalysisResult {
            file_path: file_path.to_string_lossy().to_string(),
            recommendations,
            refactoring_score,
        })
    }

    /// Detect long methods that should be extracted
    fn detect_long_methods(&self, content: &str) -> Vec<RefactoringRecommendation> {
        let mut recommendations = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        
        let mut current_method_start = None;
        let mut brace_count = 0;
        
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            
            // Simple method detection (language-agnostic)
            if self.is_method_start(trimmed) && current_method_start.is_none() {
                current_method_start = Some(line_num + 1);
                brace_count = 0;
            }
            
            // Count braces to track method end
            brace_count += trimmed.matches('{').count() as i32;
            brace_count -= trimmed.matches('}').count() as i32;
            
            // Method ended
            if current_method_start.is_some() && brace_count == 0 && trimmed.contains('}') {
                let start_line = current_method_start.unwrap();
                let end_line = line_num + 1;
                let method_length = end_line - start_line;
                
                if method_length > 30 { // Long method threshold
                    let impact = (method_length as f64 / 10.0).min(10.0);
                    let effort = 6.0; // Medium effort
                    
                    recommendations.push(RefactoringRecommendation {
                        refactoring_type: RefactoringType::ExtractMethod,
                        description: format!("Long method ({} lines) should be broken down into smaller methods", method_length),
                        estimated_impact: impact,
                        estimated_effort: effort,
                        priority_score: impact / effort,
                        location: (start_line, end_line),
                    });
                }
                
                current_method_start = None;
            }
        }
        
        recommendations
    }

    /// Detect complex conditional statements
    fn detect_complex_conditionals(&self, content: &str) -> Vec<RefactoringRecommendation> {
        let mut recommendations = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            
            // Count logical operators in conditional statements
            if trimmed.starts_with("if ") || trimmed.contains(" if ") {
                let and_count = trimmed.matches("&&").count() + trimmed.matches(" and ").count();
                let or_count = trimmed.matches("||").count() + trimmed.matches(" or ").count();
                let total_operators = and_count + or_count;
                
                if total_operators >= 3 {
                    let impact = (total_operators as f64 * 2.0).min(10.0);
                    let effort = 4.0;
                    
                    recommendations.push(RefactoringRecommendation {
                        refactoring_type: RefactoringType::SimplifyConditionals,
                        description: format!("Complex conditional with {} logical operators should be simplified", total_operators),
                        estimated_impact: impact,
                        estimated_effort: effort,
                        priority_score: impact / effort,
                        location: (line_num + 1, line_num + 1),
                    });
                }
            }
        }
        
        recommendations
    }

    /// Detect duplicate code patterns (simplified)
    fn detect_duplicate_code(&self, content: &str) -> Vec<RefactoringRecommendation> {
        let mut recommendations = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        
        // Look for repeated blocks of code (simplified detection)
        let mut line_groups: HashMap<String, Vec<usize>> = HashMap::new();
        
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.len() > 10 && !trimmed.starts_with("//") && !trimmed.starts_with("#") {
                // Also check for repeated string literals (a common duplication pattern)
                if let Some(string_literal) = extract_string_literal(trimmed) {
                    if string_literal.len() > 15 { // Only long strings are worth deduplicating
                        line_groups.entry(format!("STRING_LITERAL: {}", string_literal)).or_insert_with(Vec::new).push(line_num + 1);
                    }
                }
                line_groups.entry(trimmed.to_string()).or_insert_with(Vec::new).push(line_num + 1);
                if trimmed.contains("this appears multiple times") {
                    println!("Found potential duplicate line {}: '{}'", line_num + 1, trimmed);
                }
            }
        }
        
        // Helper function to extract string literals from a line
        fn extract_string_literal(line: &str) -> Option<String> {
            // Look for quoted strings
            if let Some(start) = line.find('"') {
                if let Some(end) = line.rfind('"') {
                    if start != end {
                        return Some(line[start+1..end].to_string());
                    }
                }
            }
            if let Some(start) = line.find('\'') {
                if let Some(end) = line.rfind('\'') {
                    if start != end {
                        return Some(line[start+1..end].to_string());
                    }
                }
            }
            None
        }
        
        for (line_content, occurrences) in line_groups {
            if occurrences.len() >= 3 { // 3+ occurrences indicate duplication
                let impact = (occurrences.len() as f64).min(10.0);
                let effort = 5.0;
                
                recommendations.push(RefactoringRecommendation {
                    refactoring_type: RefactoringType::EliminateDuplication,
                    description: format!("Duplicate code pattern found {} times: '{}'", occurrences.len(), 
                                       line_content.chars().take(50).collect::<String>()),
                    estimated_impact: impact,
                    estimated_effort: effort,
                    priority_score: impact / effort,
                    location: (occurrences[0], occurrences[occurrences.len() - 1]),
                });
            }
        }
        
        recommendations
    }

    /// Detect large classes that should be split
    fn detect_large_classes(&self, content: &str) -> Vec<RefactoringRecommendation> {
        let mut recommendations = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        
        let mut current_class_start = None;
        let mut brace_count = 0;
        
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            
            if self.is_class_start(trimmed) && current_class_start.is_none() {
                current_class_start = Some(line_num + 1);
                brace_count = 0;
            }
            
            brace_count += trimmed.matches('{').count() as i32;
            brace_count -= trimmed.matches('}').count() as i32;
            
            if current_class_start.is_some() && brace_count == 0 && trimmed.contains('}') {
                let start_line = current_class_start.unwrap();
                let end_line = line_num + 1;
                let class_length = end_line - start_line;
                
                if class_length > 100 { // Large class threshold
                    let impact = (class_length as f64 / 20.0).min(10.0);
                    let effort = 8.0; // High effort
                    
                    recommendations.push(RefactoringRecommendation {
                        refactoring_type: RefactoringType::ExtractClass,
                        description: format!("Large class ({} lines) should be split into smaller classes", class_length),
                        estimated_impact: impact,
                        estimated_effort: effort,
                        priority_score: impact / effort,
                        location: (start_line, end_line),
                    });
                }
                
                current_class_start = None;
            }
        }
        
        recommendations
    }

    /// Detect potential dead code
    fn detect_dead_code(&self, content: &str) -> Vec<RefactoringRecommendation> {
        let mut recommendations = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        
        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            
            // Look for commented-out code
            if (trimmed.starts_with("//") || trimmed.starts_with("#")) && trimmed.len() > 20 {
                // Check if it looks like code (has parentheses, operators, etc.)
                if trimmed.contains('(') && (trimmed.contains('=') || trimmed.contains("def ") || trimmed.contains("function")) {
                    recommendations.push(RefactoringRecommendation {
                        refactoring_type: RefactoringType::RemoveDeadCode,
                        description: "Commented-out code should be removed".to_string(),
                        estimated_impact: 3.0,
                        estimated_effort: 1.0, // Very low effort
                        priority_score: 3.0,
                        location: (line_num + 1, line_num + 1),
                    });
                }
            }
        }
        
        recommendations
    }

    /// Check if a line starts a method
    fn is_method_start(&self, line: &str) -> bool {
        let trimmed = line.trim();
        
        // Skip comments
        if trimmed.starts_with("//") || trimmed.starts_with("#") {
            return false;
        }
        
        trimmed.contains("def ") || // Python
        trimmed.contains("function ") || // JavaScript
        (trimmed.contains("fn ") && !trimmed.contains("->")) || // Rust function declaration
        (trimmed.contains("func ") && trimmed.contains("(")) // Go
    }

    /// Check if a line starts a class
    fn is_class_start(&self, line: &str) -> bool {
        let trimmed = line.trim();
        
        // Skip comments
        if trimmed.starts_with("//") || trimmed.starts_with("#") {
            return false;
        }
        
        trimmed.starts_with("class ") || // Python, JavaScript
        trimmed.starts_with("struct ") || // Rust, Go
        trimmed.starts_with("type ") // Go
    }

    /// Calculate overall refactoring score for the file
    fn calculate_refactoring_score(&self, recommendations: &[RefactoringRecommendation], content: &str) -> f64 {
        let total_lines = content.lines().count() as f64;
        let total_impact: f64 = recommendations.iter().map(|r| r.estimated_impact).sum();
        
        // Normalize by file size
        let base_score = total_impact / (total_lines / 100.0).max(1.0);
        
        // Cap at 100
        base_score.min(100.0)
    }
}

#[derive(Debug, Default)]
pub struct RefactoringExtractor;

#[async_trait]
impl FeatureExtractor for RefactoringExtractor {
    fn name(&self) -> &str { "refactoring" }
    fn features(&self) -> &[FeatureDefinition] { &[] }
    async fn extract(&self, _entity: &CodeEntity, _context: &ExtractionContext) -> Result<HashMap<String, f64>> {
        Ok(HashMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_refactoring_config_default() {
        let config = RefactoringConfig::default();
        assert!(config.enabled);
        assert_eq!(config.min_impact_threshold, 5.0);
    }

    #[test]
    fn test_refactoring_analyzer_creation() {
        let analyzer = RefactoringAnalyzer::default();
        assert!(analyzer.config.enabled);
        
        let custom_config = RefactoringConfig {
            enabled: false,
            min_impact_threshold: 8.0,
        };
        let analyzer = RefactoringAnalyzer::new(custom_config);
        assert!(!analyzer.config.enabled);
        assert_eq!(analyzer.config.min_impact_threshold, 8.0);
    }

    #[tokio::test]
    async fn test_analyze_files_disabled() {
        let config = RefactoringConfig {
            enabled: false,
            min_impact_threshold: 5.0,
        };
        let analyzer = RefactoringAnalyzer::new(config);
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        fs::write(&file_path, "def test_function():\n    pass").unwrap();
        
        let paths = vec![file_path];
        let results = analyzer.analyze_files(&paths).await.unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_is_method_start() {
        let analyzer = RefactoringAnalyzer::default();
        
        // Python
        assert!(analyzer.is_method_start("def test_function():"));
        assert!(analyzer.is_method_start("    def inner_function():"));
        
        // JavaScript
        assert!(analyzer.is_method_start("function testFunction() {"));
        
        // Rust
        assert!(analyzer.is_method_start("fn test_function() {"));
        assert!(!analyzer.is_method_start("fn test() -> bool {")); // Has return type
        
        // Go
        assert!(analyzer.is_method_start("func testFunction() {"));
        
        // Not methods
        assert!(!analyzer.is_method_start("if condition {"));
        assert!(!analyzer.is_method_start("// def commented_function():"));
    }

    #[test]
    fn test_is_class_start() {
        let analyzer = RefactoringAnalyzer::default();
        
        // Python
        assert!(analyzer.is_class_start("class TestClass:"));
        assert!(analyzer.is_class_start("class TestClass(BaseClass):"));
        
        // Rust/Go structs
        assert!(analyzer.is_class_start("struct TestStruct {"));
        assert!(analyzer.is_class_start("type TestType struct {"));
        
        // Not classes
        assert!(!analyzer.is_class_start("def function():"));
        assert!(!analyzer.is_class_start("if class_name:"));
    }

    #[test]
    fn test_detect_long_methods() {
        let analyzer = RefactoringAnalyzer::default();
        
        // Create a long method with JavaScript syntax (uses braces)
        let mut content = String::from("function long_method() {\n");
        for i in 1..=35 {
            content.push_str(&format!("    line_{};\n", i));
        }
        content.push_str("}\n");
        
        let recommendations = analyzer.detect_long_methods(&content);
        assert!(!recommendations.is_empty());
        
        let long_method_rec = &recommendations[0];
        assert!(matches!(long_method_rec.refactoring_type, RefactoringType::ExtractMethod));
        assert!(long_method_rec.description.contains("Long method"));
        assert!(long_method_rec.estimated_impact > 0.0);
    }

    #[test]
    fn test_detect_complex_conditionals() {
        let analyzer = RefactoringAnalyzer::default();
        
        let content = "if condition1 && condition2 || condition3 && condition4 || condition5:\n    pass";
        
        let recommendations = analyzer.detect_complex_conditionals(content);
        assert!(!recommendations.is_empty());
        
        let complex_conditional = &recommendations[0];
        assert!(matches!(complex_conditional.refactoring_type, RefactoringType::SimplifyConditionals));
        assert!(complex_conditional.description.contains("Complex conditional"));
        assert!(complex_conditional.estimated_impact > 0.0);
    }

    #[test]
    fn test_detect_duplicate_code() {
        let analyzer = RefactoringAnalyzer::default();
        
        let content = r#"
        result = calculate_something(param1, param2)
        some_other_line = different
        result = calculate_something(param1, param2)
        another_line = also_different
        result = calculate_something(param1, param2)
        "#;
        
        let recommendations = analyzer.detect_duplicate_code(content);
        assert!(!recommendations.is_empty());
        
        let duplicate_rec = &recommendations[0];
        assert!(matches!(duplicate_rec.refactoring_type, RefactoringType::EliminateDuplication));
        assert!(duplicate_rec.description.contains("Duplicate code pattern"));
        assert!(duplicate_rec.estimated_impact > 0.0);
    }

    #[test]
    fn test_detect_large_classes() {
        let analyzer = RefactoringAnalyzer::default();
        
        // Create a large class with JavaScript syntax (uses braces)
        let mut content = String::from("class LargeClass {\n");
        for i in 1..=105 {
            content.push_str(&format!("    line_{};\n", i));
        }
        content.push_str("}\n");
        
        let recommendations = analyzer.detect_large_classes(&content);
        assert!(!recommendations.is_empty());
        
        let large_class_rec = &recommendations[0];
        assert!(matches!(large_class_rec.refactoring_type, RefactoringType::ExtractClass));
        assert!(large_class_rec.description.contains("Large class"));
        assert!(large_class_rec.estimated_impact > 0.0);
    }

    #[test]
    fn test_detect_dead_code() {
        let analyzer = RefactoringAnalyzer::default();
        
        let content = r#"
        active_line = "this is active code"
        // def commented_out_function(param1, param2):
        //     return param1 + param2
        # def another_commented_function():
        #     some_variable = calculate_value()
        "#;
        
        let recommendations = analyzer.detect_dead_code(content);
        assert!(!recommendations.is_empty());
        
        let dead_code_rec = &recommendations[0];
        assert!(matches!(dead_code_rec.refactoring_type, RefactoringType::RemoveDeadCode));
        assert!(dead_code_rec.description.contains("Commented-out code"));
        assert_eq!(dead_code_rec.estimated_impact, 3.0);
        assert_eq!(dead_code_rec.estimated_effort, 1.0);
    }

    #[tokio::test]
    async fn test_analyze_file_integration() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.js");
        
        // Create a file with multiple refactoring opportunities using JavaScript syntax
        let content = r#"
class LargeClass {
    longMethodWithIssues() {
        // This is a very long method that should be refactored
        if (condition1 && condition2 && condition3 && condition4) {
            return;
        }
        let duplicate_line = "this appears multiple times";
        for (let i = 0; i < 50; i++) {
            console.log("Line " + i);
        }
        let duplicate_line2 = "this appears multiple times";
        // function commented_out_function() {
        //     return "should be removed";
        // }
        let duplicate_line3 = "this appears multiple times";
        return result;
    }
}
"#;
        fs::write(&file_path, content).unwrap();
        
        let config = RefactoringConfig {
            enabled: true,
            min_impact_threshold: 2.0, // Lower threshold for test
        };
        let analyzer = RefactoringAnalyzer::new(config);
        let result = analyzer.analyze_file(&file_path).await.unwrap();
        
        assert!(result.refactoring_score > 0.0);
        assert!(!result.recommendations.is_empty());
        
        // Should find multiple types of issues
        let types: Vec<_> = result.recommendations.iter()
            .map(|r| &r.refactoring_type)
            .collect();
        
        // We should find at least some of these types
        println!("Found types: {:?}", types);
        println!("Recommendations: {:?}", result.recommendations);
        assert!(types.iter().any(|t| matches!(t, RefactoringType::SimplifyConditionals)));
        assert!(types.iter().any(|t| matches!(t, RefactoringType::EliminateDuplication)));
    }

    #[test]
    fn test_calculate_refactoring_score() {
        let analyzer = RefactoringAnalyzer::default();
        
        let recommendations = vec![
            RefactoringRecommendation {
                refactoring_type: RefactoringType::ExtractMethod,
                description: "Test".to_string(),
                estimated_impact: 8.0,
                estimated_effort: 4.0,
                priority_score: 2.0,
                location: (1, 10),
            },
            RefactoringRecommendation {
                refactoring_type: RefactoringType::SimplifyConditionals,
                description: "Test".to_string(),
                estimated_impact: 6.0,
                estimated_effort: 3.0,
                priority_score: 2.0,
                location: (15, 15),
            },
        ];
        
        let content = "line1\nline2\nline3\n"; // 3 lines
        let score = analyzer.calculate_refactoring_score(&recommendations, content);
        
        // Total impact: 8.0 + 6.0 = 14.0
        // Normalized by file size: 14.0 / (3/100).max(1) = 14.0 / 1 = 14.0
        assert_eq!(score, 14.0);
        
        // Test with empty recommendations
        let empty_recommendations = vec![];
        let empty_score = analyzer.calculate_refactoring_score(&empty_recommendations, content);
        assert_eq!(empty_score, 0.0);
    }

    #[test]
    fn test_refactoring_type_variants() {
        // Test all RefactoringType variants can be created
        let _extract_method = RefactoringType::ExtractMethod;
        let _extract_class = RefactoringType::ExtractClass;
        let _reduce_complexity = RefactoringType::ReduceComplexity;
        let _eliminate_duplication = RefactoringType::EliminateDuplication;
        let _improve_naming = RefactoringType::ImproveNaming;
        let _simplify_conditionals = RefactoringType::SimplifyConditionals;
        let _remove_dead_code = RefactoringType::RemoveDeadCode;
    }

    #[test]
    fn test_feature_extractor_implementation() {
        let extractor = RefactoringExtractor::default();
        assert_eq!(extractor.name(), "refactoring");
        assert!(extractor.features().is_empty());
    }

    #[tokio::test]
    async fn test_feature_extractor_extract() {
        let extractor = RefactoringExtractor::default();
        let entity = CodeEntity::new(
            "test_id".to_string(),
            "Function".to_string(),
            "test_func".to_string(),
            "test.py".to_string(),
        );
        let context = ExtractionContext::new(
            std::sync::Arc::new(crate::core::config::ValknutConfig::default()),
            "rust".to_string(),
        );
        
        let result = extractor.extract(&entity, &context).await.unwrap();
        assert!(result.is_empty());
    }
}