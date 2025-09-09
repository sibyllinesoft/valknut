//! Refactoring analysis detector for identifying code improvement opportunities.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::featureset::{FeatureExtractor, FeatureDefinition, CodeEntity, ExtractionContext};
use crate::core::errors::{Result, ValknutError};
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
                line_groups.entry(trimmed.to_string()).or_insert_with(Vec::new).push(line_num + 1);
            }
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
        line.contains("def ") || // Python
        line.contains("function ") || // JavaScript
        (line.contains("fn ") && !line.contains("->")) || // Rust function declaration
        (line.contains("func ") && line.contains("(")) // Go
    }

    /// Check if a line starts a class
    fn is_class_start(&self, line: &str) -> bool {
        line.starts_with("class ") || // Python, JavaScript
        line.starts_with("struct ") || // Rust, Go
        line.starts_with("type ") // Go
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