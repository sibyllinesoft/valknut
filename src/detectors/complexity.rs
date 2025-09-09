//! Complexity analysis detectors for various code complexity metrics.
//!
//! This module implements comprehensive complexity analysis including:
//! - Cyclomatic complexity (McCabe complexity)
//! - Cognitive complexity (human-readable complexity)
//! - Halstead complexity metrics
//! - Nesting depth analysis
//! - Parameter count analysis
//! - Technical debt scoring

use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use crate::core::errors::{Result, ValknutError};
use crate::core::file_utils::FileReader;

/// Configuration for complexity analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityConfig {
    /// Enable complexity analysis
    pub enabled: bool,
    /// Cyclomatic complexity thresholds
    pub cyclomatic_thresholds: ComplexityThresholds,
    /// Cognitive complexity thresholds
    pub cognitive_thresholds: ComplexityThresholds,
    /// Nesting depth thresholds
    pub nesting_thresholds: ComplexityThresholds,
    /// Parameter count thresholds
    pub parameter_thresholds: ComplexityThresholds,
    /// File length thresholds (lines)
    pub file_length_thresholds: ComplexityThresholds,
    /// Function length thresholds (lines)
    pub function_length_thresholds: ComplexityThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityThresholds {
    pub low: f64,
    pub moderate: f64,
    pub high: f64,
    pub very_high: f64,
}

impl Default for ComplexityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cyclomatic_thresholds: ComplexityThresholds {
                low: 5.0,
                moderate: 10.0,
                high: 15.0,
                very_high: 25.0,
            },
            cognitive_thresholds: ComplexityThresholds {
                low: 5.0,
                moderate: 15.0,
                high: 25.0,
                very_high: 50.0,
            },
            nesting_thresholds: ComplexityThresholds {
                low: 2.0,
                moderate: 4.0,
                high: 6.0,
                very_high: 8.0,
            },
            parameter_thresholds: ComplexityThresholds {
                low: 3.0,
                moderate: 5.0,
                high: 8.0,
                very_high: 12.0,
            },
            file_length_thresholds: ComplexityThresholds {
                low: 100.0,
                moderate: 250.0,
                high: 500.0,
                very_high: 1000.0,
            },
            function_length_thresholds: ComplexityThresholds {
                low: 10.0,
                moderate: 25.0,
                high: 50.0,
                very_high: 100.0,
            },
        }
    }
}

/// Comprehensive complexity metrics for a code entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityMetrics {
    /// Cyclomatic complexity (decision points + 1)
    pub cyclomatic: f64,
    /// Cognitive complexity (weighted by human understanding difficulty)
    pub cognitive: f64,
    /// Maximum nesting depth
    pub max_nesting_depth: f64,
    /// Number of parameters
    pub parameter_count: f64,
    /// Lines of code
    pub lines_of_code: f64,
    /// Number of statements
    pub statement_count: f64,
    /// Halstead complexity metrics
    pub halstead: HalsteadMetrics,
    /// Technical debt score (0-100, higher is worse)
    pub technical_debt_score: f64,
    /// Maintainability index (0-100, higher is better)
    pub maintainability_index: f64,
}

/// Halstead complexity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalsteadMetrics {
    /// Number of distinct operators
    pub distinct_operators: f64,
    /// Number of distinct operands
    pub distinct_operands: f64,
    /// Total number of operators
    pub total_operators: f64,
    /// Total number of operands
    pub total_operands: f64,
    /// Program length
    pub program_length: f64,
    /// Program vocabulary
    pub vocabulary: f64,
    /// Program volume
    pub volume: f64,
    /// Program difficulty
    pub difficulty: f64,
    /// Programming effort
    pub effort: f64,
    /// Time required to program
    pub time: f64,
    /// Number of delivered bugs
    pub bugs: f64,
}

impl Default for ComplexityMetrics {
    fn default() -> Self {
        Self {
            cyclomatic: 1.0,
            cognitive: 0.0,
            max_nesting_depth: 0.0,
            parameter_count: 0.0,
            lines_of_code: 0.0,
            statement_count: 0.0,
            halstead: HalsteadMetrics::default(),
            technical_debt_score: 0.0,
            maintainability_index: 100.0,
        }
    }
}

impl Default for HalsteadMetrics {
    fn default() -> Self {
        Self {
            distinct_operators: 0.0,
            distinct_operands: 0.0,
            total_operators: 0.0,
            total_operands: 0.0,
            program_length: 0.0,
            vocabulary: 0.0,
            volume: 0.0,
            difficulty: 0.0,
            effort: 0.0,
            time: 0.0,
            bugs: 0.0,
        }
    }
}

/// Complexity severity level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplexitySeverity {
    Low,
    Moderate,
    High,
    VeryHigh,
    Critical,
}

/// Complexity analysis result for a single code entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityAnalysisResult {
    /// Entity identifier
    pub entity_id: String,
    /// Entity name
    pub entity_name: String,
    /// File path
    pub file_path: String,
    /// Line number where entity starts
    pub start_line: usize,
    /// Complexity metrics
    pub metrics: ComplexityMetrics,
    /// Overall complexity severity
    pub severity: ComplexitySeverity,
    /// Issues detected
    pub issues: Vec<ComplexityIssue>,
    /// Refactoring recommendations
    pub recommendations: Vec<ComplexityRecommendation>,
}

/// Complexity issue detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityIssue {
    /// Type of complexity issue
    pub issue_type: ComplexityIssueType,
    /// Description of the issue
    pub description: String,
    /// Severity level
    pub severity: ComplexitySeverity,
    /// Metric value that triggered this issue
    pub metric_value: f64,
    /// Threshold that was exceeded
    pub threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplexityIssueType {
    HighCyclomaticComplexity,
    HighCognitiveComplexity,
    DeepNesting,
    TooManyParameters,
    LongFunction,
    LongFile,
    HighTechnicalDebt,
    LowMaintainability,
}

/// Refactoring recommendation to reduce complexity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityRecommendation {
    /// Type of refactoring
    pub refactoring_type: RefactoringType,
    /// Description of recommended change
    pub description: String,
    /// Expected complexity reduction
    pub expected_reduction: f64,
    /// Effort required (1-10 scale)
    pub effort: u32,
    /// Priority (higher means more important)
    pub priority: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RefactoringType {
    ExtractMethod,
    SimplifyConditions,
    ReduceNesting,
    SplitFunction,
    ExtractClass,
    ReduceParameters,
    SimplifyExpressions,
    RemoveDeadCode,
}

/// Complexity analyzer that implements various complexity metrics
pub struct ComplexityAnalyzer {
    config: ComplexityConfig,
}

impl ComplexityAnalyzer {
    /// Create new complexity analyzer
    pub fn new(config: ComplexityConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(ComplexityConfig::default())
    }

    /// Analyze complexity of code in a file
    pub async fn analyze_file(&self, file_path: &Path) -> Result<Vec<ComplexityAnalysisResult>> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }

        info!("Analyzing complexity for file: {}", file_path.display());

        // Read and parse the file
        let content = FileReader::read_to_string(file_path)?;

        // Detect language from file extension
        let language = self.detect_language(file_path)?;

        // Parse and analyze based on language
        match language.as_str() {
            "python" => self.analyze_python_file(file_path, &content).await,
            "javascript" | "typescript" => self.analyze_js_file(file_path, &content).await,
            "rust" => self.analyze_rust_file(file_path, &content).await,
            "go" => self.analyze_go_file(file_path, &content).await,
            _ => {
                warn!("Unsupported language {} for file {}", language, file_path.display());
                Ok(Vec::new())
            }
        }
    }

    /// Analyze complexity of multiple files
    pub async fn analyze_files(&self, file_paths: &[&Path]) -> Result<Vec<ComplexityAnalysisResult>> {
        let mut all_results = Vec::new();

        for file_path in file_paths {
            match self.analyze_file(file_path).await {
                Ok(mut results) => all_results.append(&mut results),
                Err(e) => warn!("Failed to analyze {}: {}", file_path.display(), e),
            }
        }

        Ok(all_results)
    }

    /// Detect programming language from file extension
    fn detect_language(&self, file_path: &Path) -> Result<String> {
        let extension = file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let language = match extension.to_lowercase().as_str() {
            "py" => "python",
            "js" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            "rs" => "rust",
            "go" => "go",
            "java" => "java",
            "cpp" | "cxx" | "cc" => "cpp",
            "c" | "h" => "c",
            "cs" => "csharp",
            _ => "unknown",
        };

        if language == "unknown" {
            return Err(ValknutError::unsupported(format!("Unsupported file extension: {}", extension)));
        }

        Ok(language.to_string())
    }

    /// Analyze Python file complexity
    async fn analyze_python_file(&self, file_path: &Path, content: &str) -> Result<Vec<ComplexityAnalysisResult>> {
        debug!("Analyzing Python file: {}", file_path.display());
        
        // For now, implement a basic line-based analysis
        // In a full implementation, this would use tree-sitter or AST parsing
        let mut results = Vec::new();

        // Analyze the file as a single entity for now
        let metrics = self.calculate_basic_metrics(content, "python");
        let severity = self.calculate_severity(&metrics);
        let issues = self.detect_issues(&metrics);
        let recommendations = self.generate_recommendations(&metrics, &issues);

        results.push(ComplexityAnalysisResult {
            entity_id: format!("{}:file", file_path.display()),
            entity_name: file_path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            start_line: 1,
            metrics,
            severity,
            issues,
            recommendations,
        });

        Ok(results)
    }

    /// Analyze JavaScript/TypeScript file complexity
    async fn analyze_js_file(&self, file_path: &Path, content: &str) -> Result<Vec<ComplexityAnalysisResult>> {
        debug!("Analyzing JavaScript/TypeScript file: {}", file_path.display());
        
        let mut results = Vec::new();

        // Basic file-level analysis
        let metrics = self.calculate_basic_metrics(content, "javascript");
        let severity = self.calculate_severity(&metrics);
        let issues = self.detect_issues(&metrics);
        let recommendations = self.generate_recommendations(&metrics, &issues);

        results.push(ComplexityAnalysisResult {
            entity_id: format!("{}:file", file_path.display()),
            entity_name: file_path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            start_line: 1,
            metrics,
            severity,
            issues,
            recommendations,
        });

        Ok(results)
    }

    /// Analyze Rust file complexity
    async fn analyze_rust_file(&self, file_path: &Path, content: &str) -> Result<Vec<ComplexityAnalysisResult>> {
        debug!("Analyzing Rust file: {}", file_path.display());
        
        let mut results = Vec::new();

        let metrics = self.calculate_basic_metrics(content, "rust");
        let severity = self.calculate_severity(&metrics);
        let issues = self.detect_issues(&metrics);
        let recommendations = self.generate_recommendations(&metrics, &issues);

        results.push(ComplexityAnalysisResult {
            entity_id: format!("{}:file", file_path.display()),
            entity_name: file_path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            start_line: 1,
            metrics,
            severity,
            issues,
            recommendations,
        });

        Ok(results)
    }

    /// Analyze Go file complexity
    async fn analyze_go_file(&self, file_path: &Path, content: &str) -> Result<Vec<ComplexityAnalysisResult>> {
        debug!("Analyzing Go file: {}", file_path.display());
        
        let mut results = Vec::new();

        let metrics = self.calculate_basic_metrics(content, "go");
        let severity = self.calculate_severity(&metrics);
        let issues = self.detect_issues(&metrics);
        let recommendations = self.generate_recommendations(&metrics, &issues);

        results.push(ComplexityAnalysisResult {
            entity_id: format!("{}:file", file_path.display()),
            entity_name: file_path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            start_line: 1,
            metrics,
            severity,
            issues,
            recommendations,
        });

        Ok(results)
    }

    /// Calculate basic complexity metrics from source code text
    fn calculate_basic_metrics(&self, content: &str, language: &str) -> ComplexityMetrics {
        let lines: Vec<&str> = content.lines().collect();
        let lines_of_code = lines.len() as f64;
        
        // Count decision points for cyclomatic complexity
        let decision_keywords = match language {
            "python" => vec!["if", "elif", "while", "for", "and", "or", "except"],
            "javascript" | "typescript" => vec!["if", "while", "for", "&&", "||", "case", "catch"],
            "rust" => vec!["if", "while", "for", "match", "&&", "||"],
            "go" => vec!["if", "for", "switch", "case", "&&", "||"],
            _ => vec!["if", "while", "for", "&&", "||"],
        };

        let mut cyclomatic = 1.0; // Base complexity
        let mut max_nesting = 0;
        let mut current_nesting = 0;

        for line in &lines {
            let trimmed = line.trim();
            
            // Skip comments and empty lines
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#") {
                continue;
            }

            // Count decision points
            for keyword in &decision_keywords {
                cyclomatic += trimmed.matches(keyword).count() as f64;
            }

            // Estimate nesting depth (simplified)
            let open_braces = trimmed.matches('{').count();
            let close_braces = trimmed.matches('}').count();
            let indent_level = (line.len() - line.trim_start().len()) / 4; // Assume 4-space indentation
            
            current_nesting += open_braces as i32;
            current_nesting -= close_braces as i32;
            max_nesting = max_nesting.max(current_nesting.max(indent_level as i32));
        }

        // Estimate cognitive complexity (simplified - would need AST for accuracy)
        let cognitive = cyclomatic * 0.8; // Rough approximation
        
        // Calculate Halstead metrics (simplified)
        let halstead = self.calculate_halstead_metrics(content, language);
        
        // Calculate technical debt score and maintainability index
        let technical_debt_score = self.calculate_technical_debt_score(cyclomatic, cognitive, lines_of_code);
        let maintainability_index = self.calculate_maintainability_index(cyclomatic, lines_of_code, &halstead);

        ComplexityMetrics {
            cyclomatic,
            cognitive,
            max_nesting_depth: max_nesting as f64,
            parameter_count: 0.0, // Would need AST parsing
            lines_of_code,
            statement_count: lines.iter().filter(|line| !line.trim().is_empty()).count() as f64,
            halstead,
            technical_debt_score,
            maintainability_index,
        }
    }

    /// Calculate Halstead complexity metrics (simplified implementation)
    fn calculate_halstead_metrics(&self, content: &str, language: &str) -> HalsteadMetrics {
        // This is a simplified implementation
        // A full implementation would need proper tokenization and AST analysis
        
        let operators = match language {
            "python" => vec!["+", "-", "*", "/", "=", "==", "!=", "and", "or", "not"],
            "javascript" | "typescript" => vec!["+", "-", "*", "/", "=", "==", "!=", "&&", "||", "!"],
            "rust" => vec!["+", "-", "*", "/", "=", "==", "!=", "&&", "||", "!"],
            _ => vec!["+", "-", "*", "/", "=", "==", "!="],
        };

        let mut operator_counts: HashMap<&str, usize> = HashMap::new();
        let mut operand_counts: HashMap<String, usize> = HashMap::new();
        let mut total_operators = 0.0;
        let mut total_operands = 0.0;

        // Count operators and operands (very simplified)
        for line in content.lines() {
            for &operator in &operators {
                let count = line.matches(operator).count();
                *operator_counts.entry(operator).or_insert(0) += count;
                total_operators += count as f64;
            }
            
            // Count identifiers as operands (simplified)
            let words: Vec<&str> = line.split_whitespace().collect();
            for word in words {
                if word.chars().all(|c| c.is_alphanumeric() || c == '_') && !word.chars().all(|c| c.is_numeric()) {
                    *operand_counts.entry(word.to_string()).or_insert(0) += 1;
                    total_operands += 1.0;
                }
            }
        }

        let distinct_operators = operator_counts.len() as f64;
        let distinct_operands = operand_counts.len() as f64;
        let program_length = total_operators + total_operands;
        let vocabulary = distinct_operators + distinct_operands;
        
        let volume = if vocabulary > 0.0 {
            program_length * vocabulary.log2()
        } else {
            0.0
        };

        let difficulty = if distinct_operands > 0.0 {
            (distinct_operators / 2.0) * (total_operands / distinct_operands)
        } else {
            0.0
        };

        let effort = difficulty * volume;
        let time = effort / 18.0; // Stroud number
        let bugs = volume / 3000.0; // Empirical constant

        HalsteadMetrics {
            distinct_operators,
            distinct_operands,
            total_operators,
            total_operands,
            program_length,
            vocabulary,
            volume,
            difficulty,
            effort,
            time,
            bugs,
        }
    }

    /// Calculate technical debt score (0-100, higher is worse)
    fn calculate_technical_debt_score(&self, cyclomatic: f64, cognitive: f64, loc: f64) -> f64 {
        let complexity_factor = (cyclomatic + cognitive) / 2.0;
        let size_factor = loc / 100.0;
        
        let score = (complexity_factor * 5.0) + (size_factor * 2.0);
        score.min(100.0).max(0.0)
    }

    /// Calculate maintainability index (0-100, higher is better)
    fn calculate_maintainability_index(&self, cyclomatic: f64, loc: f64, halstead: &HalsteadMetrics) -> f64 {
        // Microsoft's maintainability index formula (adapted)
        let volume = halstead.volume;
        let mi = 171.0 
            - 5.2 * volume.ln()
            - 0.23 * cyclomatic
            - 16.2 * loc.ln();
            
        // Normalize to 0-100 scale
        (mi * 100.0 / 171.0).min(100.0).max(0.0)
    }

    /// Calculate overall complexity severity
    fn calculate_severity(&self, metrics: &ComplexityMetrics) -> ComplexitySeverity {
        // Use the highest severity from any metric
        let mut max_severity = ComplexitySeverity::Low;

        if metrics.cyclomatic >= self.config.cyclomatic_thresholds.very_high {
            max_severity = ComplexitySeverity::VeryHigh;
        } else if metrics.cyclomatic >= self.config.cyclomatic_thresholds.high {
            max_severity = ComplexitySeverity::High;
        } else if metrics.cyclomatic >= self.config.cyclomatic_thresholds.moderate {
            max_severity = ComplexitySeverity::Moderate;
        }

        if metrics.cognitive >= self.config.cognitive_thresholds.very_high {
            max_severity = ComplexitySeverity::VeryHigh;
        } else if metrics.cognitive >= self.config.cognitive_thresholds.high {
            max_severity = ComplexitySeverity::High;
        }

        if metrics.technical_debt_score >= 80.0 {
            max_severity = ComplexitySeverity::Critical;
        }

        max_severity
    }

    /// Detect complexity issues
    fn detect_issues(&self, metrics: &ComplexityMetrics) -> Vec<ComplexityIssue> {
        let mut issues = Vec::new();

        // Check cyclomatic complexity
        if metrics.cyclomatic >= self.config.cyclomatic_thresholds.very_high {
            issues.push(ComplexityIssue {
                issue_type: ComplexityIssueType::HighCyclomaticComplexity,
                description: format!("Very high cyclomatic complexity: {:.1}", metrics.cyclomatic),
                severity: ComplexitySeverity::VeryHigh,
                metric_value: metrics.cyclomatic,
                threshold: self.config.cyclomatic_thresholds.very_high,
            });
        } else if metrics.cyclomatic >= self.config.cyclomatic_thresholds.high {
            issues.push(ComplexityIssue {
                issue_type: ComplexityIssueType::HighCyclomaticComplexity,
                description: format!("High cyclomatic complexity: {:.1}", metrics.cyclomatic),
                severity: ComplexitySeverity::High,
                metric_value: metrics.cyclomatic,
                threshold: self.config.cyclomatic_thresholds.high,
            });
        }

        // Check cognitive complexity
        if metrics.cognitive >= self.config.cognitive_thresholds.high {
            issues.push(ComplexityIssue {
                issue_type: ComplexityIssueType::HighCognitiveComplexity,
                description: format!("High cognitive complexity: {:.1}", metrics.cognitive),
                severity: ComplexitySeverity::High,
                metric_value: metrics.cognitive,
                threshold: self.config.cognitive_thresholds.high,
            });
        }

        // Check nesting depth
        if metrics.max_nesting_depth >= self.config.nesting_thresholds.high {
            issues.push(ComplexityIssue {
                issue_type: ComplexityIssueType::DeepNesting,
                description: format!("Deep nesting: {} levels", metrics.max_nesting_depth),
                severity: ComplexitySeverity::High,
                metric_value: metrics.max_nesting_depth,
                threshold: self.config.nesting_thresholds.high,
            });
        }

        // Check file length
        if metrics.lines_of_code >= self.config.file_length_thresholds.high {
            issues.push(ComplexityIssue {
                issue_type: ComplexityIssueType::LongFile,
                description: format!("Long file: {:.0} lines", metrics.lines_of_code),
                severity: ComplexitySeverity::Moderate,
                metric_value: metrics.lines_of_code,
                threshold: self.config.file_length_thresholds.high,
            });
        }

        // Check technical debt
        if metrics.technical_debt_score >= 80.0 {
            issues.push(ComplexityIssue {
                issue_type: ComplexityIssueType::HighTechnicalDebt,
                description: format!("High technical debt score: {:.1}", metrics.technical_debt_score),
                severity: ComplexitySeverity::Critical,
                metric_value: metrics.technical_debt_score,
                threshold: 80.0,
            });
        }

        // Check maintainability
        if metrics.maintainability_index < 20.0 {
            issues.push(ComplexityIssue {
                issue_type: ComplexityIssueType::LowMaintainability,
                description: format!("Low maintainability index: {:.1}", metrics.maintainability_index),
                severity: ComplexitySeverity::High,
                metric_value: metrics.maintainability_index,
                threshold: 20.0,
            });
        }

        issues
    }

    /// Generate refactoring recommendations based on complexity issues
    fn generate_recommendations(&self, metrics: &ComplexityMetrics, issues: &[ComplexityIssue]) -> Vec<ComplexityRecommendation> {
        let mut recommendations = Vec::new();

        for issue in issues {
            match issue.issue_type {
                ComplexityIssueType::HighCyclomaticComplexity => {
                    recommendations.push(ComplexityRecommendation {
                        refactoring_type: RefactoringType::ExtractMethod,
                        description: "Extract complex logic into smaller methods".to_string(),
                        expected_reduction: metrics.cyclomatic * 0.3,
                        effort: 4,
                        priority: issue.metric_value / issue.threshold,
                    });
                    
                    recommendations.push(ComplexityRecommendation {
                        refactoring_type: RefactoringType::SimplifyConditions,
                        description: "Simplify complex conditional expressions".to_string(),
                        expected_reduction: metrics.cyclomatic * 0.2,
                        effort: 3,
                        priority: issue.metric_value / issue.threshold * 0.8,
                    });
                },
                ComplexityIssueType::HighCognitiveComplexity => {
                    recommendations.push(ComplexityRecommendation {
                        refactoring_type: RefactoringType::ReduceNesting,
                        description: "Reduce nesting levels using early returns or guard clauses".to_string(),
                        expected_reduction: metrics.cognitive * 0.4,
                        effort: 3,
                        priority: issue.metric_value / issue.threshold,
                    });
                },
                ComplexityIssueType::DeepNesting => {
                    recommendations.push(ComplexityRecommendation {
                        refactoring_type: RefactoringType::ReduceNesting,
                        description: "Extract nested logic into separate functions".to_string(),
                        expected_reduction: metrics.max_nesting_depth * 0.5,
                        effort: 4,
                        priority: issue.metric_value / issue.threshold,
                    });
                },
                ComplexityIssueType::LongFile => {
                    recommendations.push(ComplexityRecommendation {
                        refactoring_type: RefactoringType::ExtractClass,
                        description: "Split file into smaller, focused modules".to_string(),
                        expected_reduction: metrics.lines_of_code * 0.3,
                        effort: 6,
                        priority: issue.metric_value / issue.threshold * 0.7,
                    });
                },
                ComplexityIssueType::HighTechnicalDebt => {
                    recommendations.push(ComplexityRecommendation {
                        refactoring_type: RefactoringType::SimplifyExpressions,
                        description: "Refactor complex expressions and improve code clarity".to_string(),
                        expected_reduction: metrics.technical_debt_score * 0.4,
                        effort: 5,
                        priority: issue.metric_value / 100.0,
                    });
                },
                _ => {}
            }
        }

        // Sort by priority (highest first)
        recommendations.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap());
        
        // Limit to top 5 recommendations
        recommendations.into_iter().take(5).collect()
    }
}