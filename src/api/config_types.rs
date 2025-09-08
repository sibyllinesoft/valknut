//! High-level configuration types for the public API.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use crate::core::config::{ValknutConfig, ScoringConfig, NormalizationScheme, WeightsConfig};

/// High-level analysis configuration for easy API usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Languages to analyze
    pub languages: Vec<String>,
    
    /// Enable scoring analysis
    pub enable_scoring: bool,
    
    /// Enable graph analysis
    pub enable_graph_analysis: bool,
    
    /// Enable LSH-based duplicate detection
    pub enable_lsh_analysis: bool,
    
    /// Enable refactoring analysis
    pub enable_refactoring_analysis: bool,
    
    /// Confidence threshold for results
    pub confidence_threshold: f64,
    
    /// Maximum number of files to analyze
    pub max_files: Option<usize>,
    
    /// Patterns to exclude from analysis
    pub exclude_patterns: Vec<String>,
    
    /// Patterns to include in analysis
    pub include_patterns: Vec<String>,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            languages: vec!["python".to_string(), "javascript".to_string(), "typescript".to_string()],
            enable_scoring: true,
            enable_graph_analysis: true,
            enable_lsh_analysis: false,
            enable_refactoring_analysis: true,
            confidence_threshold: 0.7,
            max_files: None,
            exclude_patterns: vec![
                "*/node_modules/*".to_string(),
                "*/venv/*".to_string(),
                "*/__pycache__/*".to_string(),
                "*.min.js".to_string(),
            ],
            include_patterns: vec!["**/*".to_string()],
        }
    }
}

impl AnalysisConfig {
    /// Create a new analysis configuration
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the languages to analyze
    pub fn with_languages(mut self, languages: Vec<String>) -> Self {
        self.languages = languages;
        self
    }
    
    /// Add a language to analyze
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.languages.push(language.into());
        self
    }
    
    /// Enable scoring analysis
    pub fn with_scoring_enabled(mut self) -> Self {
        self.enable_scoring = true;
        self
    }
    
    /// Enable graph analysis
    pub fn with_graph_analysis(mut self) -> Self {
        self.enable_graph_analysis = true;
        self
    }
    
    /// Enable LSH analysis
    pub fn with_lsh_analysis(mut self) -> Self {
        self.enable_lsh_analysis = true;
        self
    }
    
    /// Enable refactoring analysis
    pub fn with_refactoring_analysis(mut self) -> Self {
        self.enable_refactoring_analysis = true;
        self
    }
    
    /// Set confidence threshold
    pub fn with_confidence_threshold(mut self, threshold: f64) -> Self {
        self.confidence_threshold = threshold;
        self
    }
    
    /// Set maximum number of files to analyze
    pub fn with_max_files(mut self, max_files: usize) -> Self {
        self.max_files = Some(max_files);
        self
    }
    
    /// Add an exclusion pattern
    pub fn exclude_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }
    
    /// Add an inclusion pattern
    pub fn include_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.include_patterns.push(pattern.into());
        self
    }
    
    /// Convert to internal ValknutConfig
    pub(crate) fn to_valknut_config(self) -> ValknutConfig {
        let mut config = ValknutConfig::default();
        
        // Map high-level settings to detailed configuration
        config.analysis.enable_scoring = self.enable_scoring;
        config.analysis.enable_graph_analysis = self.enable_graph_analysis;
        config.analysis.enable_lsh_analysis = self.enable_lsh_analysis;
        config.analysis.enable_refactoring_analysis = self.enable_refactoring_analysis;
        config.analysis.confidence_threshold = self.confidence_threshold;
        config.analysis.max_files = self.max_files.unwrap_or(0);
        config.analysis.exclude_patterns = self.exclude_patterns;
        config.analysis.include_patterns = self.include_patterns;
        
        // Enable languages
        for language in &self.languages {
            if let Some(lang_config) = config.languages.get_mut(language) {
                lang_config.enabled = true;
            }
        }
        
        config
    }
}