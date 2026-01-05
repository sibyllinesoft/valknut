//! AST Stop-Motif Miner using tree-sitter analysis.
//!
//! This module provides the `AstStopMotifMiner` for mining AST-based patterns.

use std::collections::{HashMap, HashSet};

use super::language_adapters::{
    GoLanguageAdapter, JavaScriptLanguageAdapter, LanguageAdapter, PythonLanguageAdapter,
    RustLanguageAdapter, TypeScriptLanguageAdapter,
};
use super::types::{AstExtractionConfig, AstPattern, AstPatternExtractor, AstPatternType, PatternThresholds};
use super::{AstPatternCategory, AstStopMotifEntry, FunctionInfo};
use crate::core::errors::Result;
use crate::lang::registry::detect_language_from_path;

/// Phase 3: AST Stop-Motif Miner using tree-sitter analysis
pub struct AstStopMotifMiner {
    /// Language adapters for AST parsing
    language_adapters: HashMap<String, Box<dyn LanguageAdapter>>,

    /// Pattern extractor for AST analysis
    pattern_extractor: AstPatternExtractor,

    /// Frequency thresholds for pattern selection
    frequency_thresholds: PatternThresholds,
}

/// Factory and AST pattern mining methods for [`AstStopMotifMiner`].
impl AstStopMotifMiner {
    /// Create a new AST stop-motif miner
    pub fn new() -> Self {
        let mut language_adapters: HashMap<String, Box<dyn LanguageAdapter>> = HashMap::new();

        // Initialize language adapters
        if let Ok(python_adapter) = PythonLanguageAdapter::new() {
            language_adapters.insert("python".to_string(), Box::new(python_adapter));
        }

        if let Ok(js_adapter) = JavaScriptLanguageAdapter::new() {
            language_adapters.insert("javascript".to_string(), Box::new(js_adapter));
        }

        if let Ok(ts_adapter) = TypeScriptLanguageAdapter::new() {
            language_adapters.insert("typescript".to_string(), Box::new(ts_adapter));
        }

        if let Ok(rust_adapter) = RustLanguageAdapter::new() {
            language_adapters.insert("rust".to_string(), Box::new(rust_adapter));
        }

        if let Ok(go_adapter) = GoLanguageAdapter::new() {
            language_adapters.insert("go".to_string(), Box::new(go_adapter));
        }

        let config = AstExtractionConfig::default();
        let thresholds = PatternThresholds::default();

        Self {
            language_adapters,
            pattern_extractor: AstPatternExtractor::new(config),
            frequency_thresholds: thresholds,
        }
    }

    /// Mine AST stop-motifs from codebase functions
    pub fn mine_ast_stop_motifs(
        &mut self,
        functions: &[FunctionInfo],
    ) -> Result<Vec<AstStopMotifEntry>> {
        let start_time = std::time::Instant::now();
        let mut all_patterns = Vec::new();
        let mut languages_processed = HashSet::new();

        // Extract patterns from all functions
        for function in functions {
            let language = self.detect_language(&function.file_path);
            let Some(adapter) = self.language_adapters.get_mut(&language) else {
                continue;
            };
            languages_processed.insert(language.clone());

            let Ok(parse_index) = adapter.parse_source(&function.source_code, &function.file_path)
            else {
                eprintln!("Failed to parse source code for {}", function.id);
                continue;
            };

            match adapter.extract_ast_patterns(&parse_index, &function.source_code) {
                Ok(patterns) => all_patterns.extend(patterns),
                Err(e) => eprintln!("Failed to extract AST patterns from {}: {:?}", function.id, e),
            }
        }

        // Analyze pattern frequencies
        self.pattern_extractor
            .analyze_pattern_frequencies(&all_patterns);

        // Select stop-motifs based on frequency thresholds
        let stop_motifs = self.select_stop_motifs(&all_patterns)?;

        let duration = start_time.elapsed();
        println!(
            "AST stop-motif mining completed in {:?}ms",
            duration.as_millis()
        );
        println!(
            "Found {} AST patterns, selected {} as stop-motifs",
            all_patterns.len(),
            stop_motifs.len()
        );
        println!("Languages processed: {:?}", languages_processed);

        Ok(stop_motifs)
    }

    /// Detect programming language from file path
    fn detect_language(&self, file_path: &str) -> String {
        detect_language_from_path(file_path)
    }

    /// Select stop-motifs based on frequency analysis
    fn select_stop_motifs(&self, patterns: &[AstPattern]) -> Result<Vec<AstStopMotifEntry>> {
        let mut stop_motifs = Vec::new();

        // Calculate pattern frequencies by type
        let mut pattern_frequencies: HashMap<String, usize> = HashMap::new();
        for pattern in patterns {
            *pattern_frequencies.entry(pattern.id.clone()).or_insert(0) += 1;
        }

        // Sort patterns by frequency
        let mut frequency_pairs: Vec<(String, usize)> = pattern_frequencies.into_iter().collect();
        frequency_pairs.sort_by(|a, b| b.1.cmp(&a.1));

        let total_patterns = frequency_pairs.len();

        // Select top percentile patterns as stop-motifs
        for (i, (pattern_id, support)) in frequency_pairs.iter().enumerate() {
            if let Some(pattern) = patterns.iter().find(|p| &p.id == pattern_id) {
                let percentile_threshold = match pattern.pattern_type {
                    AstPatternType::NodeType => self.frequency_thresholds.node_type_percentile,
                    AstPatternType::SubtreePattern => self.frequency_thresholds.subtree_percentile,
                    AstPatternType::TokenSequence => {
                        self.frequency_thresholds.token_sequence_percentile
                    }
                    AstPatternType::ControlFlowPattern => {
                        self.frequency_thresholds.subtree_percentile
                    }
                    AstPatternType::FrameworkPattern => {
                        self.frequency_thresholds.subtree_percentile
                    }
                };

                // Calculate which percentile this pattern falls into
                let pattern_rank = i + 1;

                let pattern_percentile = 1.0 - (pattern_rank as f64 / total_patterns as f64);

                if pattern_percentile >= percentile_threshold
                    && *support >= self.pattern_extractor.config.min_support
                {
                    // Calculate IDF score
                    let total_functions = patterns.len();
                    let idf_score = (total_functions as f64 / *support as f64).ln();

                    if idf_score >= self.frequency_thresholds.min_idf_score {
                        let category = match pattern.pattern_type {
                            AstPatternType::NodeType => AstPatternCategory::NodeType,
                            AstPatternType::SubtreePattern => AstPatternCategory::SubtreePattern,
                            AstPatternType::TokenSequence => AstPatternCategory::TokenSequence,
                            AstPatternType::ControlFlowPattern => {
                                AstPatternCategory::ControlFlowPattern
                            }
                            AstPatternType::FrameworkPattern => {
                                AstPatternCategory::FrameworkPattern
                            }
                        };

                        let stop_motif = AstStopMotifEntry {
                            pattern: pattern.id.clone(),
                            support: *support,
                            idf_score,
                            weight_multiplier: 0.2, // Common weight for stop-motifs
                            category,
                            language: pattern.language.clone(),
                            metadata: pattern.metadata.clone(),
                        };

                        stop_motifs.push(stop_motif);
                    }
                }
            }
        }

        Ok(stop_motifs)
    }
}

/// Default implementation for [`AstStopMotifMiner`].
impl Default for AstStopMotifMiner {
    /// Returns a new miner with default settings.
    fn default() -> Self {
        Self::new()
    }
}
