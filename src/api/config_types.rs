//! Simplified configuration types for the public API.
//!
//! This module provides a clean, unified configuration interface that eliminates
//! complexity and duplication while maintaining backward compatibility.

use crate::core::config::{validate_unit_range, ValknutConfig};
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

    /// Maximum file size in bytes (None = unlimited, default = 500KB)
    /// Files larger than this are skipped during analysis
    pub max_file_size_bytes: Option<u64>,

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

/// Default implementation for [`AnalysisConfig`].
impl Default for AnalysisConfig {
    /// Returns the default analysis configuration.
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

/// Default implementation for [`AnalysisModules`].
impl Default for AnalysisModules {
    /// Returns the default analysis modules configuration.
    fn default() -> Self {
        Self {
            complexity: true,
            dependencies: true,
            duplicates: false,
            refactoring: true,
            structure: true,
            coverage: true,
        }
    }
}

/// Default implementation for [`LanguageSettings`].
impl Default for LanguageSettings {
    /// Returns the default language settings.
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

/// Default implementation for [`FileSettings`].
impl Default for FileSettings {
    /// Returns the default file settings.
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
            max_file_size_bytes: Some(500 * 1024), // 500KB default
            follow_symlinks: false,
        }
    }
}

/// Default implementation for [`QualitySettings`].
impl Default for QualitySettings {
    /// Returns the default quality settings.
    fn default() -> Self {
        Self {
            confidence_threshold: 0.7,
            max_analysis_time_per_file: Some(30),
            strict_mode: false,
        }
    }
}

/// Default implementation for [`CoverageSettings`].
impl Default for CoverageSettings {
    /// Returns the default coverage settings.
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

/// Constructor and fluent builder methods for [`AnalysisConfig`].
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
        validate_unit_range(self.quality.confidence_threshold, "confidence_threshold")?;

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
        let modules_enabled = [
            self.modules.complexity,
            self.modules.dependencies,
            self.modules.duplicates,
            self.modules.refactoring,
            self.modules.structure,
            self.modules.coverage,
        ];
        if !modules_enabled.iter().any(|&enabled| enabled) {
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
    pub fn to_valknut_config(self) -> ValknutConfig {
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
                max_file_size_bytes: if valknut_config.analysis.max_file_size_bytes == 0 {
                    None
                } else {
                    Some(valknut_config.analysis.max_file_size_bytes)
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

/// Factory methods for [`AnalysisModules`] presets.
impl AnalysisModules {
    /// Creates a configuration with all analysis modules enabled.
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

    /// Creates a configuration with only essential modules for fast analysis.
    ///
    /// Only enables complexity analysis, which provides basic code health metrics
    /// with minimal overhead.
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

    /// Creates a configuration focused on code quality analysis.
    ///
    /// Enables complexity, duplicate detection, and refactoring modules.
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

/// Builder methods for [`LanguageSettings`].
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

/// Builder methods for [`FileSettings`].
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

/// Builder methods for [`QualitySettings`].
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

/// Factory and builder methods for [`CoverageSettings`].
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
#[path = "config_types_tests.rs"]
mod tests;
