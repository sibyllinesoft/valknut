//! Type definitions for cache and pattern analysis

use std::collections::{HashMap, HashSet};

/// AST pattern extracted from tree-sitter analysis
#[derive(Debug, Clone)]
pub struct AstPattern {
    /// Pattern identifier
    pub id: String,

    /// Pattern type
    pub pattern_type: AstPatternType,

    /// Node type (for NodeType patterns)
    pub node_type: Option<String>,

    /// Subtree structure (for SubtreePattern)
    pub subtree_signature: Option<String>,

    /// Token sequence (for TokenSequence patterns)
    pub token_sequence: Option<String>,

    /// Language where pattern was found
    pub language: String,

    /// Metadata about the pattern
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Types of AST patterns that can be extracted
#[derive(Debug, Clone, PartialEq)]
pub enum AstPatternType {
    /// Common AST node type
    NodeType,

    /// Structural subtree pattern
    SubtreePattern,

    /// Token sequence pattern
    TokenSequence,

    /// Control flow pattern
    ControlFlowPattern,

    /// Framework-specific pattern
    FrameworkPattern,
}

/// AST pattern extractor that analyzes parsed code
#[derive(Debug)]
pub struct AstPatternExtractor {
    /// Node type frequency tracking
    pub(crate) node_type_frequencies: HashMap<String, usize>,

    /// Subtree pattern frequencies
    pub(crate) subtree_frequencies: HashMap<String, usize>,

    /// Token sequence frequencies
    pub(crate) token_sequence_frequencies: HashMap<String, usize>,

    /// Pattern extraction configuration
    pub config: AstExtractionConfig,
}

/// Factory and frequency analysis methods for [`AstPatternExtractor`].
impl AstPatternExtractor {
    /// Create a new AST pattern extractor
    pub fn new(config: AstExtractionConfig) -> Self {
        Self {
            node_type_frequencies: HashMap::new(),
            subtree_frequencies: HashMap::new(),
            token_sequence_frequencies: HashMap::new(),
            config,
        }
    }

    /// Analyze frequencies of all extracted patterns
    pub fn analyze_pattern_frequencies(&mut self, patterns: &[AstPattern]) {
        self.node_type_frequencies.clear();
        self.subtree_frequencies.clear();
        self.token_sequence_frequencies.clear();

        for pattern in patterns {
            match &pattern.pattern_type {
                AstPatternType::NodeType => {
                    if let Some(ref node_type) = pattern.node_type {
                        *self
                            .node_type_frequencies
                            .entry(node_type.clone())
                            .or_insert(0) += 1;
                    }
                }
                AstPatternType::SubtreePattern => {
                    if let Some(ref signature) = pattern.subtree_signature {
                        *self
                            .subtree_frequencies
                            .entry(signature.clone())
                            .or_insert(0) += 1;
                    }
                }
                AstPatternType::TokenSequence => {
                    if let Some(ref sequence) = pattern.token_sequence {
                        *self
                            .token_sequence_frequencies
                            .entry(sequence.clone())
                            .or_insert(0) += 1;
                    }
                }
                AstPatternType::ControlFlowPattern | AstPatternType::FrameworkPattern => {
                    // Treat as subtree pattern for frequency analysis
                    if let Some(ref signature) = pattern.subtree_signature {
                        *self
                            .subtree_frequencies
                            .entry(signature.clone())
                            .or_insert(0) += 1;
                    }
                }
            }
        }
    }
}

/// Configuration for AST pattern extraction
#[derive(Debug, Clone)]
pub struct AstExtractionConfig {
    /// Minimum support count for patterns
    pub min_support: usize,

    /// Maximum subtree depth to analyze
    pub max_subtree_depth: usize,

    /// Token sequence length for analysis
    pub token_sequence_length: usize,

    /// Languages to process
    pub enabled_languages: HashSet<String>,
}

/// Default implementation for [`AstExtractionConfig`].
impl Default for AstExtractionConfig {
    /// Returns the default extraction configuration.
    fn default() -> Self {
        Self {
            min_support: 3,
            max_subtree_depth: 4,
            token_sequence_length: 5,
            enabled_languages: ["python", "javascript", "typescript", "rust", "go"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

/// Frequency thresholds for pattern selection
#[derive(Debug, Clone)]
pub struct PatternThresholds {
    /// Top percentile for node types (e.g., top 5%)
    pub node_type_percentile: f64,

    /// Top percentile for subtree patterns
    pub subtree_percentile: f64,

    /// Top percentile for token sequences
    pub token_sequence_percentile: f64,

    /// Minimum IDF score to consider a pattern rare enough
    pub min_idf_score: f64,
}

/// Default implementation for [`PatternThresholds`].
impl Default for PatternThresholds {
    /// Returns the default pattern thresholds.
    fn default() -> Self {
        Self {
            node_type_percentile: 0.95,
            subtree_percentile: 0.90,
            token_sequence_percentile: 0.95,
            min_idf_score: 0.1,
        }
    }
}
