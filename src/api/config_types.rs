//! High-level configuration types for the public API.

use serde::{Deserialize, Serialize};

use crate::core::config::ValknutConfig;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_config_default() {
        let config = AnalysisConfig::default();
        
        assert_eq!(config.languages, vec!["python", "javascript", "typescript"]);
        assert!(config.enable_scoring);
        assert!(config.enable_graph_analysis);
        assert!(!config.enable_lsh_analysis);
        assert!(config.enable_refactoring_analysis);
        assert_eq!(config.confidence_threshold, 0.7);
        assert_eq!(config.max_files, None);
        assert!(config.exclude_patterns.contains(&"*/node_modules/*".to_string()));
        assert!(config.exclude_patterns.contains(&"*/venv/*".to_string()));
        assert!(config.exclude_patterns.contains(&"*/__pycache__/*".to_string()));
        assert!(config.exclude_patterns.contains(&"*.min.js".to_string()));
        assert_eq!(config.include_patterns, vec!["**/*"]);
    }

    #[test]
    fn test_analysis_config_new() {
        let config = AnalysisConfig::new();
        let default_config = AnalysisConfig::default();
        
        assert_eq!(config.languages, default_config.languages);
        assert_eq!(config.enable_scoring, default_config.enable_scoring);
        assert_eq!(config.confidence_threshold, default_config.confidence_threshold);
    }

    #[test]
    fn test_with_languages() {
        let config = AnalysisConfig::new()
            .with_languages(vec!["rust".to_string(), "go".to_string()]);
        
        assert_eq!(config.languages, vec!["rust", "go"]);
    }

    #[test]
    fn test_with_language() {
        let config = AnalysisConfig::new()
            .with_language("rust")
            .with_language("go");
        
        assert!(config.languages.contains(&"rust".to_string()));
        assert!(config.languages.contains(&"go".to_string()));
        // Should include original default languages too
        assert!(config.languages.len() >= 2);
    }

    #[test]
    fn test_with_scoring_enabled() {
        let config = AnalysisConfig::new()
            .with_scoring_enabled();
        
        assert!(config.enable_scoring);
    }

    #[test]
    fn test_with_graph_analysis() {
        let config = AnalysisConfig::new()
            .with_graph_analysis();
        
        assert!(config.enable_graph_analysis);
    }

    #[test]
    fn test_with_lsh_analysis() {
        let config = AnalysisConfig::new()
            .with_lsh_analysis();
        
        assert!(config.enable_lsh_analysis);
    }

    #[test]
    fn test_with_refactoring_analysis() {
        let config = AnalysisConfig::new()
            .with_refactoring_analysis();
        
        assert!(config.enable_refactoring_analysis);
    }

    #[test]
    fn test_with_confidence_threshold() {
        let config = AnalysisConfig::new()
            .with_confidence_threshold(0.85);
        
        assert_eq!(config.confidence_threshold, 0.85);
    }

    #[test]
    fn test_with_max_files() {
        let config = AnalysisConfig::new()
            .with_max_files(1000);
        
        assert_eq!(config.max_files, Some(1000));
    }

    #[test]
    fn test_exclude_pattern() {
        let config = AnalysisConfig::new()
            .exclude_pattern("*/target/*")
            .exclude_pattern("*.tmp");
        
        assert!(config.exclude_patterns.contains(&"*/target/*".to_string()));
        assert!(config.exclude_patterns.contains(&"*.tmp".to_string()));
        // Should still have default exclusions
        assert!(config.exclude_patterns.contains(&"*/node_modules/*".to_string()));
    }

    #[test]
    fn test_include_pattern() {
        let config = AnalysisConfig::new()
            .include_pattern("src/**/*.rs")
            .include_pattern("lib/**/*.rs");
        
        assert!(config.include_patterns.contains(&"src/**/*.rs".to_string()));
        assert!(config.include_patterns.contains(&"lib/**/*.rs".to_string()));
        // Should still have default inclusion
        assert!(config.include_patterns.contains(&"**/*".to_string()));
    }

    #[test]
    fn test_method_chaining() {
        let config = AnalysisConfig::new()
            .with_languages(vec!["rust".to_string()])
            .with_lsh_analysis()
            .with_confidence_threshold(0.9)
            .with_max_files(500)
            .exclude_pattern("*/tests/*")
            .include_pattern("src/**/*.rs");
        
        assert_eq!(config.languages, vec!["rust"]);
        assert!(config.enable_lsh_analysis);
        assert_eq!(config.confidence_threshold, 0.9);
        assert_eq!(config.max_files, Some(500));
        assert!(config.exclude_patterns.contains(&"*/tests/*".to_string()));
        assert!(config.include_patterns.contains(&"src/**/*.rs".to_string()));
    }

    #[test]
    fn test_to_valknut_config() {
        let config = AnalysisConfig::new()
            .with_languages(vec!["python".to_string(), "rust".to_string()])
            .with_lsh_analysis()
            .with_confidence_threshold(0.8)
            .with_max_files(200)
            .exclude_pattern("*/build/*")
            .include_pattern("**/*.py");
        
        let valknut_config = config.to_valknut_config();
        
        assert_eq!(valknut_config.analysis.confidence_threshold, 0.8);
        assert_eq!(valknut_config.analysis.max_files, 200);
        assert!(valknut_config.analysis.enable_lsh_analysis);
        assert!(valknut_config.analysis.exclude_patterns.contains(&"*/build/*".to_string()));
        assert!(valknut_config.analysis.include_patterns.contains(&"**/*.py".to_string()));
    }

    #[test]
    fn test_serialization() {
        let config = AnalysisConfig::new()
            .with_language("rust")
            .with_confidence_threshold(0.75);
        
        // Test that it can be serialized and deserialized
        let json = serde_json::to_string(&config).expect("Should serialize");
        let deserialized: AnalysisConfig = serde_json::from_str(&json).expect("Should deserialize");
        
        assert_eq!(config.confidence_threshold, deserialized.confidence_threshold);
        assert!(deserialized.languages.contains(&"rust".to_string()));
    }

    #[test]
    fn test_builder_pattern_immutability() {
        let original = AnalysisConfig::new();
        let modified = original.clone().with_confidence_threshold(0.9);
        
        // Original should remain unchanged
        assert_eq!(original.confidence_threshold, 0.7);
        assert_eq!(modified.confidence_threshold, 0.9);
    }

    #[test]
    fn test_empty_languages_list() {
        let config = AnalysisConfig::new()
            .with_languages(vec![]);
        
        assert!(config.languages.is_empty());
    }

    #[test]
    fn test_max_files_none_conversion() {
        let config = AnalysisConfig::new(); // max_files is None by default
        let valknut_config = config.to_valknut_config();
        
        assert_eq!(valknut_config.analysis.max_files, 0); // None should convert to 0
    }

    #[test]
    fn test_pattern_string_conversion() {
        let config = AnalysisConfig::new()
            .exclude_pattern("test_string")
            .include_pattern(String::from("another_string"));
        
        assert!(config.exclude_patterns.contains(&"test_string".to_string()));
        assert!(config.include_patterns.contains(&"another_string".to_string()));
    }
}