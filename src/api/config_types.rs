//! Simplified configuration types for the public API.
//!
//! This module provides a clean, unified configuration interface that eliminates
//! complexity and duplication while maintaining backward compatibility.

use crate::core::config::ValknutConfig;
use crate::core::errors::{Result, ValknutError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unified analysis configuration for the public API
///
/// This is the main configuration interface for users. It provides a clean,
/// composable API that automatically handles internal configuration complexity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Analysis modules to enable
    pub modules: AnalysisModules,

    /// Language-specific settings
    pub languages: LanguageSettings,

    /// File discovery and filtering
    pub files: FileSettings,

    /// Quality thresholds and limits
    pub quality: QualitySettings,

    /// Coverage analysis configuration
    pub coverage: CoverageSettings,
}

/// Analysis modules that can be enabled/disabled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisModules {
    /// Enable complexity and scoring analysis
    pub complexity: bool,

    /// Enable dependency graph analysis
    pub dependencies: bool,

    /// Enable duplicate code detection
    pub duplicates: bool,

    /// Enable refactoring opportunity detection
    pub refactoring: bool,

    /// Enable code structure analysis
    pub structure: bool,

    /// Enable code coverage analysis
    pub coverage: bool,
}

/// Language configuration for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageSettings {
    /// Languages to analyze (if empty, auto-detect from files)
    pub enabled: Vec<String>,

    /// Maximum file size per language (in MB)
    pub max_file_size_mb: Option<f64>,

    /// Language-specific complexity thresholds
    pub complexity_thresholds: std::collections::HashMap<String, f64>,
}

/// File discovery and filtering settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSettings {
    /// Patterns to include in analysis
    pub include_patterns: Vec<String>,

    /// Patterns to exclude from analysis
    pub exclude_patterns: Vec<String>,

    /// Maximum number of files to analyze (None = unlimited)
    pub max_files: Option<usize>,

    /// Follow symbolic links during file discovery
    pub follow_symlinks: bool,
}

/// Quality thresholds and analysis limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualitySettings {
    /// Minimum confidence threshold for results (0.0-1.0)
    pub confidence_threshold: f64,

    /// Maximum analysis time per file (seconds)
    pub max_analysis_time_per_file: Option<u64>,

    /// Enable strict validation mode
    pub strict_mode: bool,
}

/// Coverage analysis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageSettings {
    /// Enable coverage analysis
    pub enabled: bool,

    /// Specific coverage file path (overrides auto discovery)
    pub file_path: Option<PathBuf>,

    /// Enable automatic coverage file discovery
    pub auto_discover: bool,

    /// Maximum age of coverage files in days (0 = no age limit)
    pub max_age_days: u32,

    /// Additional search paths for coverage files
    pub search_paths: Vec<String>,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            modules: AnalysisModules::default(),
            languages: LanguageSettings::default(),
            files: FileSettings::default(),
            quality: QualitySettings::default(),
            coverage: CoverageSettings::default(),
        }
    }
}

impl Default for AnalysisModules {
    fn default() -> Self {
        Self {
            complexity: true,
            dependencies: true,
            duplicates: false, // Disabled by default due to performance
            refactoring: true,
            structure: true,
            coverage: true,
        }
    }
}

impl Default for LanguageSettings {
    fn default() -> Self {
        Self {
            enabled: vec![
                "python".to_string(),
                "javascript".to_string(),
                "typescript".to_string(),
            ],
            max_file_size_mb: Some(10.0),
            complexity_thresholds: [
                ("python".to_string(), 10.0),
                ("javascript".to_string(), 10.0),
                ("typescript".to_string(), 10.0),
                ("rust".to_string(), 15.0),
                ("go".to_string(), 12.0),
            ]
            .iter()
            .cloned()
            .collect(),
        }
    }
}

impl Default for FileSettings {
    fn default() -> Self {
        Self {
            include_patterns: vec!["**/*".to_string()],
            exclude_patterns: vec![
                "*/node_modules/*".to_string(),
                "*/venv/*".to_string(),
                "*/target/*".to_string(),
                "*/__pycache__/*".to_string(),
                "*.min.js".to_string(),
            ],
            max_files: None,
            follow_symlinks: false,
        }
    }
}

impl Default for QualitySettings {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.7,
            max_analysis_time_per_file: Some(30),
            strict_mode: false,
        }
    }
}

impl Default for CoverageSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            file_path: None,
            auto_discover: true,
            max_age_days: 7,
            search_paths: vec![
                "./coverage/".to_string(),
                "./target/coverage/".to_string(),
                "./target/tarpaulin/".to_string(),
            ],
        }
    }
}

impl AnalysisConfig {
    /// Create a new analysis configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable/disable analysis modules with a fluent interface
    pub fn modules(mut self, f: impl FnOnce(AnalysisModules) -> AnalysisModules) -> Self {
        self.modules = f(self.modules);
        self
    }

    /// Configure languages with a fluent interface
    pub fn languages(mut self, f: impl FnOnce(LanguageSettings) -> LanguageSettings) -> Self {
        self.languages = f(self.languages);
        self
    }

    /// Configure file settings with a fluent interface
    pub fn files(mut self, f: impl FnOnce(FileSettings) -> FileSettings) -> Self {
        self.files = f(self.files);
        self
    }

    /// Configure quality settings with a fluent interface
    pub fn quality(mut self, f: impl FnOnce(QualitySettings) -> QualitySettings) -> Self {
        self.quality = f(self.quality);
        self
    }

    /// Configure coverage settings with a fluent interface
    pub fn coverage(mut self, f: impl FnOnce(CoverageSettings) -> CoverageSettings) -> Self {
        self.coverage = f(self.coverage);
        self
    }

    // Convenience methods for common operations

    /// Set the languages to analyze
    pub fn with_languages(mut self, languages: Vec<String>) -> Self {
        self.languages.enabled = languages;
        self
    }

    /// Add a language to analyze
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.languages.enabled.push(language.into());
        self
    }

    /// Set confidence threshold
    pub fn with_confidence_threshold(mut self, threshold: f64) -> Self {
        self.quality.confidence_threshold = threshold;
        self
    }

    /// Set maximum number of files to analyze
    pub fn with_max_files(mut self, max_files: usize) -> Self {
        self.files.max_files = Some(max_files);
        self
    }

    /// Add an exclusion pattern
    pub fn exclude_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.files.exclude_patterns.push(pattern.into());
        self
    }

    /// Add an inclusion pattern
    pub fn include_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.files.include_patterns.push(pattern.into());
        self
    }

    /// Enable all analysis modules
    pub fn enable_all_modules(mut self) -> Self {
        self.modules.complexity = true;
        self.modules.dependencies = true;
        self.modules.duplicates = true;
        self.modules.refactoring = true;
        self.modules.structure = true;
        self.modules.coverage = true;
        self
    }

    /// Disable all analysis modules (useful for selective enabling)
    pub fn disable_all_modules(mut self) -> Self {
        self.modules.complexity = false;
        self.modules.dependencies = false;
        self.modules.duplicates = false;
        self.modules.refactoring = false;
        self.modules.structure = false;
        self.modules.coverage = false;
        self
    }

    /// Enable only essential modules for fast analysis
    pub fn essential_modules_only(mut self) -> Self {
        self.modules.complexity = true;
        self.modules.dependencies = false;
        self.modules.duplicates = false;
        self.modules.refactoring = false;
        self.modules.structure = false;
        self.modules.coverage = false;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate confidence threshold
        if !(0.0..=1.0).contains(&self.quality.confidence_threshold) {
            return Err(ValknutError::validation(format!(
                "confidence_threshold must be between 0.0 and 1.0, got {}",
                self.quality.confidence_threshold
            )));
        }

        // Validate file limits
        if let Some(max_files) = self.files.max_files {
            if max_files == 0 {
                return Err(ValknutError::validation(
                    "max_files must be greater than 0 when specified",
                ));
            }
        }

        // Validate file size limits
        if let Some(max_size) = self.languages.max_file_size_mb {
            if max_size <= 0.0 {
                return Err(ValknutError::validation(
                    "max_file_size_mb must be positive when specified",
                ));
            }
        }

        // Validate coverage age
        if self.coverage.enabled && self.coverage.max_age_days == 0 && self.coverage.auto_discover {
            // This is actually fine - 0 means no age limit
        }

        // Validate that at least one module is enabled
        if !self.modules.complexity
            && !self.modules.dependencies
            && !self.modules.duplicates
            && !self.modules.refactoring
            && !self.modules.structure
            && !self.modules.coverage
        {
            return Err(ValknutError::validation(
                "At least one analysis module must be enabled",
            ));
        }

        Ok(())
    }

    /// Convert to internal ValknutConfig
    ///
    /// This method handles the complexity of mapping the clean public API
    /// to the detailed internal configuration structure.
    pub(crate) fn to_valknut_config(self) -> ValknutConfig {
        let mut config = ValknutConfig::default();

        // Map analysis modules to internal flags
        config.analysis.enable_scoring = self.modules.complexity;
        config.analysis.enable_graph_analysis = self.modules.dependencies;
        config.analysis.enable_lsh_analysis = self.modules.duplicates;
        config.analysis.enable_refactoring_analysis = self.modules.refactoring;
        config.analysis.enable_structure_analysis = self.modules.structure;
        config.analysis.enable_coverage_analysis = self.modules.coverage;

        // Map quality settings
        config.analysis.confidence_threshold = self.quality.confidence_threshold;

        // Map file settings
        config.analysis.max_files = self.files.max_files.unwrap_or(0);
        config.analysis.exclude_patterns = self.files.exclude_patterns;
        config.analysis.include_patterns = self.files.include_patterns;

        // Map coverage configuration
        config.coverage.coverage_file = self.coverage.file_path;
        config.coverage.auto_discover = self.coverage.auto_discover;
        config.coverage.max_age_days = self.coverage.max_age_days;
        config.coverage.search_paths = self.coverage.search_paths;

        // Configure languages
        for language in &self.languages.enabled {
            if let Some(lang_config) = config.languages.get_mut(language) {
                lang_config.enabled = true;

                // Apply language-specific settings
                if let Some(max_size) = self.languages.max_file_size_mb {
                    lang_config.max_file_size_mb = max_size;
                }

                if let Some(&threshold) = self.languages.complexity_thresholds.get(language) {
                    lang_config.complexity_threshold = threshold;
                }
            }
        }

        // Set performance configuration based on quality settings
        if let Some(timeout) = self.quality.max_analysis_time_per_file {
            config.performance.file_timeout_seconds = timeout;
        }

        config
    }

    /// Create from ValknutConfig
    ///
    /// This method handles the reverse conversion from the detailed internal
    /// configuration to the simplified public API.
    pub fn from_valknut_config(valknut_config: ValknutConfig) -> Result<Self> {
        // Extract enabled languages and their settings
        let enabled_languages: Vec<String> = valknut_config
            .languages
            .iter()
            .filter_map(|(name, config)| {
                if config.enabled {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();

        // Extract complexity thresholds
        let complexity_thresholds: std::collections::HashMap<String, f64> = valknut_config
            .languages
            .iter()
            .filter(|(_, config)| config.enabled)
            .map(|(name, config)| (name.clone(), config.complexity_threshold))
            .collect();

        // Extract file size limit (use first enabled language's limit)
        let max_file_size_mb = valknut_config
            .languages
            .values()
            .find(|config| config.enabled)
            .map(|config| config.max_file_size_mb);

        Ok(Self {
            modules: AnalysisModules {
                complexity: valknut_config.analysis.enable_scoring,
                dependencies: valknut_config.analysis.enable_graph_analysis,
                duplicates: valknut_config.analysis.enable_lsh_analysis,
                refactoring: valknut_config.analysis.enable_refactoring_analysis,
                structure: valknut_config.analysis.enable_structure_analysis,
                coverage: valknut_config.analysis.enable_coverage_analysis,
            },
            languages: LanguageSettings {
                enabled: enabled_languages,
                max_file_size_mb,
                complexity_thresholds,
            },
            files: FileSettings {
                include_patterns: valknut_config.analysis.include_patterns,
                exclude_patterns: valknut_config.analysis.exclude_patterns,
                max_files: if valknut_config.analysis.max_files == 0 {
                    None
                } else {
                    Some(valknut_config.analysis.max_files)
                },
                follow_symlinks: false, // Default value, not stored in ValknutConfig
            },
            quality: QualitySettings {
                confidence_threshold: valknut_config.analysis.confidence_threshold,
                max_analysis_time_per_file: Some(valknut_config.performance.file_timeout_seconds),
                strict_mode: false, // Default value, not stored in ValknutConfig
            },
            coverage: CoverageSettings {
                enabled: valknut_config.analysis.enable_coverage_analysis,
                file_path: valknut_config.coverage.coverage_file,
                auto_discover: valknut_config.coverage.auto_discover,
                max_age_days: valknut_config.coverage.max_age_days,
                search_paths: valknut_config.coverage.search_paths,
            },
        })
    }
}

// Additional convenience implementations for the new config components

impl AnalysisModules {
    /// Enable all modules
    pub fn all() -> Self {
        Self {
            complexity: true,
            dependencies: true,
            duplicates: true,
            refactoring: true,
            structure: true,
            coverage: true,
        }
    }

    /// Enable only essential modules for fast analysis
    pub fn essential() -> Self {
        Self {
            complexity: true,
            dependencies: false,
            duplicates: false,
            refactoring: false,
            structure: false,
            coverage: false,
        }
    }

    /// Enable complexity and refactoring analysis
    pub fn code_quality() -> Self {
        Self {
            complexity: true,
            dependencies: false,
            duplicates: true,
            refactoring: true,
            structure: false,
            coverage: false,
        }
    }
}

impl LanguageSettings {
    /// Add a language to the enabled list
    pub fn add_language(mut self, language: impl Into<String>) -> Self {
        self.enabled.push(language.into());
        self
    }

    /// Set complexity threshold for a specific language
    pub fn with_complexity_threshold(
        mut self,
        language: impl Into<String>,
        threshold: f64,
    ) -> Self {
        self.complexity_thresholds
            .insert(language.into(), threshold);
        self
    }

    /// Set maximum file size
    pub fn with_max_file_size_mb(mut self, size_mb: f64) -> Self {
        self.max_file_size_mb = Some(size_mb);
        self
    }
}

impl FileSettings {
    /// Add multiple exclusion patterns
    pub fn exclude_patterns(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns.extend(patterns);
        self
    }

    /// Add multiple inclusion patterns
    pub fn include_patterns(mut self, patterns: Vec<String>) -> Self {
        self.include_patterns.extend(patterns);
        self
    }

    /// Set maximum files to analyze
    pub fn with_max_files(mut self, max_files: usize) -> Self {
        self.max_files = Some(max_files);
        self
    }
}

impl QualitySettings {
    /// Enable strict validation mode
    pub fn strict(mut self) -> Self {
        self.strict_mode = true;
        self
    }

    /// Set analysis timeout per file
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.max_analysis_time_per_file = Some(seconds);
        self
    }
}

impl CoverageSettings {
    /// Disable coverage analysis
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Use a specific coverage file
    pub fn with_file(mut self, path: PathBuf) -> Self {
        self.file_path = Some(path);
        self.auto_discover = false;
        self
    }

    /// Add additional search paths
    pub fn with_search_paths(mut self, paths: Vec<String>) -> Self {
        self.search_paths.extend(paths);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_config_default() {
        let config = AnalysisConfig::default();

        // Check module defaults
        assert!(config.modules.complexity);
        assert!(config.modules.dependencies);
        assert!(!config.modules.duplicates); // Should be false by default
        assert!(config.modules.refactoring);
        assert!(config.modules.structure);
        assert!(config.modules.coverage);

        // Check language defaults
        assert_eq!(
            config.languages.enabled,
            vec!["python", "javascript", "typescript"]
        );
        assert_eq!(config.languages.max_file_size_mb, Some(10.0));

        // Check quality defaults
        assert_eq!(config.quality.confidence_threshold, 0.7);
        assert!(!config.quality.strict_mode);

        // Check file defaults
        assert!(config
            .files
            .exclude_patterns
            .contains(&"*/node_modules/*".to_string()));
        assert_eq!(config.files.include_patterns, vec!["**/*"]);
    }

    #[test]
    fn test_fluent_interface() {
        let config = AnalysisConfig::new()
            .modules(|_| AnalysisModules::code_quality())
            .languages(|l| {
                l.add_language("rust")
                    .with_complexity_threshold("rust", 15.0)
            })
            .files(|f| {
                f.with_max_files(1000)
                    .exclude_patterns(vec!["*/target/*".to_string()])
            })
            .quality(|q| q.strict().with_timeout(60))
            .coverage(|c| c.with_search_paths(vec!["./coverage/".to_string()]));

        // Verify modules
        assert!(config.modules.complexity);
        assert!(config.modules.duplicates);
        assert!(config.modules.refactoring);
        assert!(!config.modules.dependencies);

        // Verify languages
        assert!(config.languages.enabled.contains(&"rust".to_string()));
        assert_eq!(
            config.languages.complexity_thresholds.get("rust"),
            Some(&15.0)
        );

        // Verify files
        assert_eq!(config.files.max_files, Some(1000));
        assert!(config
            .files
            .exclude_patterns
            .contains(&"*/target/*".to_string()));

        // Verify quality
        assert!(config.quality.strict_mode);
        assert_eq!(config.quality.max_analysis_time_per_file, Some(60));

        // Verify coverage
        assert!(config
            .coverage
            .search_paths
            .contains(&"./coverage/".to_string()));
    }

    #[test]
    fn test_convenience_methods() {
        let config = AnalysisConfig::new()
            .with_languages(vec!["rust".to_string(), "go".to_string()])
            .with_confidence_threshold(0.85)
            .with_max_files(500)
            .exclude_pattern("*/tests/*")
            .include_pattern("src/**/*.rs");

        assert_eq!(config.languages.enabled, vec!["rust", "go"]);
        assert_eq!(config.quality.confidence_threshold, 0.85);
        assert_eq!(config.files.max_files, Some(500));
        assert!(config
            .files
            .exclude_patterns
            .contains(&"*/tests/*".to_string()));
        assert!(config
            .files
            .include_patterns
            .contains(&"src/**/*.rs".to_string()));
    }

    #[test]
    fn test_module_presets() {
        let essential = AnalysisModules::essential();
        assert!(essential.complexity);
        assert!(!essential.dependencies);
        assert!(!essential.duplicates);

        let all = AnalysisModules::all();
        assert!(all.complexity);
        assert!(all.dependencies);
        assert!(all.duplicates);
        assert!(all.refactoring);
        assert!(all.structure);
        assert!(all.coverage);

        let code_quality = AnalysisModules::code_quality();
        assert!(code_quality.complexity);
        assert!(code_quality.duplicates);
        assert!(code_quality.refactoring);
        assert!(!code_quality.dependencies);
    }

    #[test]
    fn test_validation() {
        // Valid config should pass
        let valid_config = AnalysisConfig::default();
        assert!(valid_config.validate().is_ok());

        // Invalid confidence threshold
        let invalid_config = AnalysisConfig::new().with_confidence_threshold(1.5);
        assert!(invalid_config.validate().is_err());

        // No modules enabled should fail
        let no_modules_config = AnalysisConfig::new().disable_all_modules();
        assert!(no_modules_config.validate().is_err());

        // Zero max files should fail
        let zero_files_config = AnalysisConfig::new().files(|f| f.with_max_files(0));
        assert!(zero_files_config.validate().is_err());
    }

    #[test]
    fn test_config_conversion() {
        let original_config = AnalysisConfig::new()
            .with_languages(vec!["python".to_string(), "rust".to_string()])
            .modules(|_| AnalysisModules::code_quality())
            .with_confidence_threshold(0.8)
            .with_max_files(200);

        // Convert to ValknutConfig and back
        let valknut_config = original_config.clone().to_valknut_config();
        let converted_back = AnalysisConfig::from_valknut_config(valknut_config).unwrap();

        // Check that key settings are preserved
        assert_eq!(converted_back.quality.confidence_threshold, 0.8);
        assert_eq!(converted_back.files.max_files, Some(200));
        assert!(converted_back
            .languages
            .enabled
            .contains(&"python".to_string()));
        assert!(converted_back
            .languages
            .enabled
            .contains(&"rust".to_string()));
        assert!(converted_back.modules.complexity);
        assert!(converted_back.modules.duplicates);
        assert!(converted_back.modules.refactoring);
    }

    #[test]
    fn test_serialization() {
        let config = AnalysisConfig::new()
            .with_language("rust")
            .with_confidence_threshold(0.75);

        // Test that it can be serialized and deserialized
        let json = serde_json::to_string(&config).expect("Should serialize");
        let deserialized: AnalysisConfig = serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(
            config.quality.confidence_threshold,
            deserialized.quality.confidence_threshold
        );
        assert!(deserialized.languages.enabled.contains(&"rust".to_string()));
    }

    #[test]
    fn test_builder_pattern_immutability() {
        let original = AnalysisConfig::new();
        let modified = original.clone().with_confidence_threshold(0.9);

        // Original should remain unchanged
        assert_eq!(original.quality.confidence_threshold, 0.7);
        assert_eq!(modified.quality.confidence_threshold, 0.9);
    }

    #[test]
    fn test_backward_compatibility() {
        // Test that old-style method calls still work
        let config = AnalysisConfig::new()
            .with_languages(vec!["rust".to_string()])
            .with_confidence_threshold(0.9)
            .with_max_files(500)
            .exclude_pattern("*/tests/*")
            .include_pattern("src/**/*.rs");

        assert_eq!(config.languages.enabled, vec!["rust"]);
        assert_eq!(config.quality.confidence_threshold, 0.9);
        assert_eq!(config.files.max_files, Some(500));
        assert!(config
            .files
            .exclude_patterns
            .contains(&"*/tests/*".to_string()));
        assert!(config
            .files
            .include_patterns
            .contains(&"src/**/*.rs".to_string()));
    }

    #[test]
    fn test_module_convenience_methods() {
        let config = AnalysisConfig::new()
            .enable_all_modules()
            .disable_all_modules()
            .essential_modules_only();

        assert!(config.modules.complexity);
        assert!(!config.modules.dependencies);
        assert!(!config.modules.duplicates);
        assert!(!config.modules.refactoring);
    }
}
