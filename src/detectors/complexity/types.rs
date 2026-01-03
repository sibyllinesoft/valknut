//! Complexity analysis types and configuration.

use serde::{Deserialize, Serialize};

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

/// Default implementation for [`ComplexityConfig`].
impl Default for ComplexityConfig {
    /// Returns the default complexity analysis configuration.
    fn default() -> Self {
        Self {
            enabled: true,
            cyclomatic_thresholds: ComplexityThresholds::default_cyclomatic(),
            cognitive_thresholds: ComplexityThresholds::default_cognitive(),
            nesting_thresholds: ComplexityThresholds::default_nesting(),
            parameter_thresholds: ComplexityThresholds::default_parameters(),
            file_length_thresholds: ComplexityThresholds::default_file_length(),
            function_length_thresholds: ComplexityThresholds::default_function_length(),
        }
    }
}

/// Complexity thresholds for various metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityThresholds {
    pub low: f64,
    pub medium: f64,
    pub high: f64,
    pub very_high: f64,
}

/// Factory methods for standard complexity thresholds.
impl ComplexityThresholds {
    /// Returns default thresholds for cyclomatic complexity.
    pub fn default_cyclomatic() -> Self {
        Self {
            low: 5.0,
            medium: 10.0,
            high: 15.0,
            very_high: 25.0,
        }
    }

    /// Returns default thresholds for cognitive complexity.
    pub fn default_cognitive() -> Self {
        Self {
            low: 5.0,
            medium: 15.0,
            high: 25.0,
            very_high: 50.0,
        }
    }

    /// Returns default thresholds for nesting depth.
    pub fn default_nesting() -> Self {
        Self {
            low: 2.0,
            medium: 4.0,
            high: 6.0,
            very_high: 10.0,
        }
    }

    /// Returns default thresholds for parameter count.
    pub fn default_parameters() -> Self {
        Self {
            low: 3.0,
            medium: 5.0,
            high: 8.0,
            very_high: 12.0,
        }
    }

    /// Returns default thresholds for file length in lines.
    pub fn default_file_length() -> Self {
        Self {
            low: 100.0,
            medium: 300.0,
            high: 500.0,
            very_high: 1000.0,
        }
    }

    /// Returns default thresholds for function length in lines.
    pub fn default_function_length() -> Self {
        Self {
            low: 15.0,
            medium: 30.0,
            high: 50.0,
            very_high: 100.0,
        }
    }
}

/// Complexity severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplexitySeverity {
    Low,
    Medium,
    Moderate, // Alias for Medium
    High,
    VeryHigh,
    Critical,
}

/// Severity classification methods for [`ComplexitySeverity`].
impl ComplexitySeverity {
    /// Determines severity level based on a value and thresholds.
    pub fn from_value(value: f64, thresholds: &ComplexityThresholds) -> Self {
        if value <= thresholds.low {
            Self::Low
        } else if value <= thresholds.medium {
            Self::Medium
        } else if value <= thresholds.high {
            Self::High
        } else if value <= thresholds.very_high {
            Self::VeryHigh
        } else {
            Self::Critical
        }
    }
}

/// Analysis result for complexity detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityAnalysisResult {
    pub entity_id: String,
    pub file_path: String,
    pub line_number: usize,
    pub start_line: usize,
    pub entity_name: String,
    pub entity_type: String,
    pub metrics: ComplexityMetrics,
    pub issues: Vec<ComplexityIssue>,
    pub severity: ComplexitySeverity,
    pub recommendations: Vec<String>,
}

/// Issue type for complexity problems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplexityIssueType {
    HighCyclomaticComplexity,
    HighCognitiveComplexity,
    ExcessiveNesting,
    DeepNesting,
    TooManyParameters,
    LongFunction,
    LongFile,
    HighTechnicalDebt,
}

/// Enhanced complexity metrics from AST analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityMetrics {
    /// Real cyclomatic complexity from AST
    pub cyclomatic_complexity: f64,
    /// Cognitive complexity with nesting weights
    pub cognitive_complexity: f64,
    /// Maximum nesting depth
    pub max_nesting_depth: f64,
    /// Number of parameters in functions
    pub parameter_count: f64,
    /// Lines of code (non-comment, non-blank)
    pub lines_of_code: f64,
    /// Number of statements
    pub statement_count: f64,
    /// Halstead complexity metrics
    pub halstead: HalsteadMetrics,
    /// Technical debt score
    pub technical_debt_score: f64,
    /// Maintainability index
    pub maintainability_index: f64,
    /// Decision points breakdown
    pub decision_points: Vec<DecisionPointInfo>,
}

/// Accessor methods for [`ComplexityMetrics`].
impl ComplexityMetrics {
    /// Alias for cyclomatic complexity for compatibility
    pub fn cyclomatic(&self) -> f64 {
        self.cyclomatic_complexity
    }

    /// Alias for cognitive complexity for compatibility
    pub fn cognitive(&self) -> f64 {
        self.cognitive_complexity
    }
}

/// Information about each decision point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPointInfo {
    pub kind: String,
    pub line: usize,
    pub column: usize,
    pub nesting_level: u32,
}

/// Halstead complexity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalsteadMetrics {
    pub n1: f64,                // Number of distinct operators
    pub n2: f64,                // Number of distinct operands
    pub n_1: f64,               // Total number of operators
    pub n_2: f64,               // Total number of operands
    pub vocabulary: f64,        // n1 + n2
    pub length: f64,            // N1 + N2
    pub calculated_length: f64, // n1 * log2(n1) + n2 * log2(n2)
    pub volume: f64,            // length * log2(vocabulary)
    pub difficulty: f64,        // (n1/2) * (N2/n2)
    pub effort: f64,            // difficulty * volume
    pub time: f64,              // effort / 18
    pub bugs: f64,              // volume / 3000
}

/// Default implementation for [`HalsteadMetrics`].
impl Default for HalsteadMetrics {
    /// Returns zeroed Halstead metrics.
    fn default() -> Self {
        Self {
            n1: 0.0,
            n2: 0.0,
            n_1: 0.0,
            n_2: 0.0,
            vocabulary: 0.0,
            length: 0.0,
            calculated_length: 0.0,
            volume: 0.0,
            difficulty: 0.0,
            effort: 0.0,
            time: 0.0,
            bugs: 0.0,
        }
    }
}

/// Complexity issue for refactoring suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityIssue {
    pub entity_id: String,
    pub issue_type: String,
    pub severity: String,
    pub description: String,
    pub recommendation: String,
    pub location: String,
    pub metric_value: f64,
    pub threshold: f64,
}
