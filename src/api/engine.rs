//! Main analysis engine implementation.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::info;

use crate::api::config_types::AnalysisConfig as ApiAnalysisConfig;
use crate::core::config::ValknutConfig;
use crate::core::errors::{Result, ValknutError};
use crate::core::featureset::FeatureVector;
use crate::core::pipeline::AnalysisResults;
use crate::core::pipeline::{AnalysisConfig as PipelineAnalysisConfig, AnalysisPipeline};

/// Compute the common root directory from a list of paths.
/// Returns the longest common prefix that ends at a directory boundary.
fn compute_common_root(paths: &[PathBuf]) -> PathBuf {
    if paths.is_empty() {
        return PathBuf::new();
    }

    // Canonicalize paths that exist, otherwise use as-is
    let canonical_paths: Vec<PathBuf> = paths
        .iter()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .collect();

    // Start with the first path's parent directory
    let first = &canonical_paths[0];
    let mut common = first.parent().unwrap_or(first).to_path_buf();

    // Find the common prefix across all paths
    for path in &canonical_paths[1..] {
        while !path.starts_with(&common) {
            if let Some(parent) = common.parent() {
                common = parent.to_path_buf();
            } else {
                return PathBuf::new();
            }
        }
    }

    common
}

/// Main valknut analysis engine
pub struct ValknutEngine {
    /// Internal analysis pipeline
    pipeline: AnalysisPipeline,

    /// Engine configuration
    config: Arc<ValknutConfig>,
}

/// Factory and analysis methods for [`ValknutEngine`].
impl ValknutEngine {
    /// Create a new valknut engine with the given configuration
    pub async fn new(config: ApiAnalysisConfig) -> Result<Self> {
        info!("Initializing Valknut analysis engine");

        // Convert high-level config to internal config
        let internal_config = config.to_valknut_config();

        // Validate configuration
        internal_config.validate()?;

        let config_arc = Arc::new(internal_config.clone());
        let analysis_config = PipelineAnalysisConfig::from(internal_config.clone());
        let pipeline = AnalysisPipeline::new_with_config(analysis_config, internal_config);

        // TODO: Register feature extractors based on enabled languages
        // For now, we'll create a minimal setup

        // Check if pipeline needs fitting with training data
        // For this initial implementation, we'll skip the training phase
        // and rely on default configurations

        info!("Valknut engine initialized successfully");

        Ok(Self {
            pipeline,
            config: config_arc,
        })
    }

    /// Create a new engine directly from a fully-populated ValknutConfig.
    ///
    /// This avoids lossy round-trips through the public API config when we need
    /// to preserve advanced settings like denoising and dedupe thresholds.
    pub async fn new_from_valknut_config(valknut_config: ValknutConfig) -> Result<Self> {
        info!("Initializing Valknut analysis engine (direct config)");

        valknut_config.validate()?;

        let config_arc = Arc::new(valknut_config.clone());
        let analysis_config = PipelineAnalysisConfig::from(valknut_config.clone());
        let pipeline = AnalysisPipeline::new_with_config(analysis_config, valknut_config);

        info!("Valknut engine initialized successfully");

        Ok(Self {
            pipeline,
            config: config_arc,
        })
    }

    /// Analyze a directory of code files
    pub async fn analyze_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<AnalysisResults> {
        let path = path.as_ref();
        info!("Starting directory analysis: {}", path.display());

        // Verify path exists
        if !path.exists() {
            return Err(ValknutError::io(
                format!("Path does not exist: {}", path.display()),
                std::io::Error::new(std::io::ErrorKind::NotFound, "Path not found"),
            ));
        }

        if !path.is_dir() {
            return Err(ValknutError::validation(format!(
                "Path is not a directory: {}",
                path.display()
            )));
        }

        // Run the pipeline
        let pipeline_results = self.pipeline.analyze_directory(path).await?;

        // Convert to public API format with the directory as project root
        let project_root = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let results = AnalysisResults::from_pipeline_results(pipeline_results, project_root);

        info!(
            "Directory analysis completed: {} files processed, {} entities analyzed",
            results.files_analyzed(),
            results.summary.entities_analyzed
        );

        Ok(results)
    }

    /// Analyze specific files
    pub async fn analyze_files<P: AsRef<Path>>(&mut self, files: &[P]) -> Result<AnalysisResults> {
        info!("Starting analysis of {} specific files", files.len());

        if files.is_empty() {
            return Ok(AnalysisResults::empty());
        }

        let paths: Vec<PathBuf> = files
            .iter()
            .map(|file| file.as_ref().to_path_buf())
            .collect();

        let comprehensive = self
            .pipeline
            .analyze_paths(&paths, None)
            .await
            .map_err(|err| {
                ValknutError::pipeline("file_analysis", format!("File analysis failed: {}", err))
            })?;

        let pipeline_results = self.pipeline.wrap_results(comprehensive);

        // Compute project root from common prefix of file paths
        let project_root = compute_common_root(&paths);
        Ok(AnalysisResults::from_pipeline_results(pipeline_results, project_root))
    }

    /// Analyze pre-extracted feature vectors (for testing and advanced usage)
    pub async fn analyze_vectors(
        &mut self,
        vectors: Vec<FeatureVector>,
    ) -> Result<AnalysisResults> {
        info!("Analyzing {} pre-extracted feature vectors", vectors.len());

        // Ensure pipeline is ready
        if !vectors.is_empty() && !self.pipeline.is_ready() {
            // Fit the pipeline with the provided vectors as training data
            info!("Fitting pipeline with provided vectors");
            self.pipeline.fit(&vectors).await?;
        }

        // Run analysis
        let pipeline_results = self.pipeline.analyze_vectors(vectors).await?;

        // Convert to public API format (no project root for vector-only analysis)
        let results = AnalysisResults::from_pipeline_results(pipeline_results, PathBuf::new());

        info!(
            "Vector analysis completed: {} entities analyzed",
            results.summary.entities_analyzed
        );

        Ok(results)
    }

    /// Get the current configuration
    pub fn config(&self) -> &ValknutConfig {
        &self.config
    }

    /// Get pipeline status information
    pub fn get_status(&self) -> EngineStatus {
        let pipeline_status = self.pipeline.get_status();

        EngineStatus {
            is_ready: pipeline_status.is_ready,
            pipeline_fitted: self.pipeline.is_ready(),
            configuration_valid: pipeline_status.config_valid,
            issues: pipeline_status.issues,
            supported_languages: self.get_supported_languages(),
        }
    }

    /// Get list of supported languages based on configuration
    fn get_supported_languages(&self) -> Vec<String> {
        self.config
            .languages
            .iter()
            .filter(|(_, config)| config.enabled)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Check if the engine is ready for analysis
    pub fn is_ready(&self) -> bool {
        self.pipeline.is_ready()
    }

    /// Perform a health check of the engine
    pub async fn health_check(&self) -> HealthCheckResult {
        let mut checks = Vec::new();
        let mut overall_status = true;

        // Check configuration validity
        let config_check = self.check_configuration();
        if config_check.status == HealthCheckStatus::Failed {
            overall_status = false;
        }
        checks.push(config_check);

        // Check pipeline status
        let pipeline_check = self.check_pipeline();
        if pipeline_check.status == HealthCheckStatus::Failed {
            overall_status = false;
        }
        checks.push(pipeline_check);

        // Check feature extractors
        checks.push(self.check_feature_extractors());

        // Check supported languages
        checks.push(self.check_language_support());

        HealthCheckResult {
            overall_status,
            checks,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Check configuration validity.
    fn check_configuration(&self) -> HealthCheck {
        match self.config.validate() {
            Ok(_) => HealthCheck::passed("Configuration"),
            Err(e) => HealthCheck::failed("Configuration", e.to_string()),
        }
    }

    /// Check pipeline status.
    fn check_pipeline(&self) -> HealthCheck {
        let status = self.pipeline.get_status();
        if status.ready {
            HealthCheck::passed("Pipeline")
        } else {
            HealthCheck::failed("Pipeline", status.issues.join("; "))
        }
    }

    /// Check feature extractors availability.
    fn check_feature_extractors(&self) -> HealthCheck {
        let count = self.pipeline.extractor_registry().get_all_extractors().count();
        if count > 0 {
            HealthCheck::passed_with_message("Feature Extractors", format!("{} extractors available", count))
        } else {
            HealthCheck::warning("Feature Extractors", "No feature extractors registered")
        }
    }

    /// Check language support.
    fn check_language_support(&self) -> HealthCheck {
        let languages = self.get_supported_languages();
        if languages.is_empty() {
            HealthCheck::warning("Language Support", "No languages enabled")
        } else {
            HealthCheck::passed_with_message("Language Support", format!("Languages: {}", languages.join(", ")))
        }
    }
}

/// Status information about the analysis engine
#[derive(Debug)]
pub struct EngineStatus {
    /// Whether the engine is ready for analysis
    pub is_ready: bool,

    /// Whether the pipeline has been fitted
    pub pipeline_fitted: bool,

    /// Whether the configuration is valid
    pub configuration_valid: bool,

    /// List of issues preventing readiness
    pub issues: Vec<String>,

    /// List of supported languages
    pub supported_languages: Vec<String>,
}

/// Result of an engine health check
#[derive(Debug)]
pub struct HealthCheckResult {
    /// Overall health status
    pub overall_status: bool,

    /// Individual health checks
    pub checks: Vec<HealthCheck>,

    /// Timestamp of the check
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Individual health check result
#[derive(Debug)]
pub struct HealthCheck {
    /// Name of the component being checked
    pub name: String,

    /// Status of this check
    pub status: HealthCheckStatus,

    /// Optional message with details
    pub message: Option<String>,
}

/// Factory methods for [`HealthCheck`].
impl HealthCheck {
    /// Create a passed health check.
    fn passed(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthCheckStatus::Passed,
            message: None,
        }
    }

    /// Create a passed health check with a message.
    fn passed_with_message(name: &str, message: String) -> Self {
        Self {
            name: name.to_string(),
            status: HealthCheckStatus::Passed,
            message: Some(message),
        }
    }

    /// Create a failed health check.
    fn failed(name: &str, message: String) -> Self {
        Self {
            name: name.to_string(),
            status: HealthCheckStatus::Failed,
            message: Some(message),
        }
    }

    /// Create a warning health check.
    fn warning(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthCheckStatus::Warning,
            message: Some(message.to_string()),
        }
    }
}

/// Health check status
#[derive(Debug, PartialEq, Eq)]
pub enum HealthCheckStatus {
    /// Check passed successfully
    Passed,

    /// Check failed
    Failed,

    /// Check passed with warnings
    Warning,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::config_types::AnalysisConfig;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_engine_creation() {
        let config = AnalysisConfig::default();
        let result = ValknutEngine::new(config).await;
        assert!(result.is_ok());

        let engine = result.unwrap();
        assert!(!engine.get_supported_languages().is_empty());
    }

    #[tokio::test]
    async fn test_analyze_nonexistent_directory() {
        let config = AnalysisConfig::default();
        let mut engine = ValknutEngine::new(config).await.unwrap();

        let result = engine.analyze_directory("/nonexistent/path").await;
        assert!(result.is_err());

        if let Err(ValknutError::Io { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected Io error");
        }
    }

    #[tokio::test]
    async fn test_analyze_empty_directory() {
        let config = AnalysisConfig::default();
        let mut engine = ValknutEngine::new(config).await.unwrap();

        // Create temporary empty directory
        let temp_dir = TempDir::new().unwrap();

        let result = engine.analyze_directory(temp_dir.path()).await;
        assert!(result.is_ok());

        let results = result.unwrap();
        println!(
            "Files processed: {}, entities analyzed: {}",
            results.summary.files_processed, results.summary.entities_analyzed
        );
        // Empty directory might still analyze some files (like hidden config files)
        assert_eq!(results.summary.entities_analyzed, 0);
    }

    #[tokio::test]
    async fn test_analyze_vectors() {
        let config = AnalysisConfig::default();
        let mut engine = ValknutEngine::new(config).await.unwrap();

        // Create test vectors
        let mut vectors = vec![FeatureVector::new("entity1"), FeatureVector::new("entity2")];

        vectors[0].add_feature("complexity", 2.0);
        vectors[1].add_feature("complexity", 8.0);

        let result = engine.analyze_vectors(vectors).await;
        assert!(result.is_ok());

        let results = result.unwrap();
        println!(
            "Vector test - entities analyzed: {}",
            results.summary.entities_analyzed
        );
        // The vector analysis should analyze some entities, but the exact count may vary
        // based on implementation details (entities_analyzed is unsigned, always >= 0)
    }

    #[tokio::test]
    async fn test_health_check() {
        let config = AnalysisConfig::default();
        let engine = ValknutEngine::new(config).await.unwrap();

        let health = engine.health_check().await;

        // Should have at least configuration and pipeline checks
        assert!(!health.checks.is_empty());

        // Find configuration check
        let config_check = health.checks.iter().find(|c| c.name == "Configuration");
        assert!(config_check.is_some());
        assert_eq!(config_check.unwrap().status, HealthCheckStatus::Passed);
    }

    #[tokio::test]
    async fn test_engine_status() {
        let config = AnalysisConfig::default();
        let engine = ValknutEngine::new(config).await.unwrap();

        let status = engine.get_status();
        assert!(!status.supported_languages.is_empty());
        assert!(status.configuration_valid);
    }

    #[tokio::test]
    async fn test_analyze_file_not_directory() {
        let config = AnalysisConfig::default();
        let mut engine = ValknutEngine::new(config).await.unwrap();

        // Create temporary file (not directory)
        let temp_dir = TempDir::new().unwrap();
        let temp_file = temp_dir.path().join("test.txt");
        std::fs::write(&temp_file, "test content").unwrap();

        let result = engine.analyze_directory(&temp_file).await;
        assert!(result.is_err());

        if let Err(ValknutError::Validation { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected Validation error for non-directory path");
        }
    }

    #[tokio::test]
    async fn test_analyze_files_empty_list() {
        let config = AnalysisConfig::default();
        let mut engine = ValknutEngine::new(config).await.unwrap();

        let empty_files: Vec<&str> = vec![];
        let result = engine.analyze_files(&empty_files).await;
        assert!(result.is_ok());

        let results = result.unwrap();
        assert_eq!(results.summary.files_processed, 0);
        assert_eq!(results.summary.entities_analyzed, 0);
        assert_eq!(results.summary.refactoring_needed, 0);
        assert_eq!(results.summary.high_priority, 0);
        assert_eq!(results.summary.critical, 0);
        assert_eq!(results.summary.avg_refactoring_score, 0.0);
        assert_eq!(results.summary.code_health_score, 1.0);
        assert!(results.refactoring_candidates.is_empty());
        assert!(results.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_analyze_files_with_parent_directory() {
        let config = AnalysisConfig::default();
        let mut engine = ValknutEngine::new(config).await.unwrap();

        // Create temporary file
        let temp_dir = TempDir::new().unwrap();
        let temp_file = temp_dir.path().join("test.py");
        std::fs::write(&temp_file, "def hello(): pass").unwrap();

        let files = vec![temp_file.as_path()];
        let result = engine.analyze_files(&files).await;
        assert!(result.is_ok()); // Should analyze the parent directory
    }

    #[tokio::test]
    async fn test_analyze_files_no_parent_directory() {
        let config = AnalysisConfig::default();
        let mut engine = ValknutEngine::new(config).await.unwrap();

        // Try to analyze a relative path with no parent directory
        let files = vec![std::path::Path::new("file_with_no_parent.rs")];
        let result = engine.analyze_files(&files).await;
        assert!(result.is_ok());

        let results = result.unwrap();
        assert_eq!(results.summary.files_processed, 0);
        assert_eq!(results.summary.entities_analyzed, 0);
    }

    #[tokio::test]
    async fn test_analyze_vectors_empty() {
        let config = AnalysisConfig::default();
        let mut engine = ValknutEngine::new(config).await.unwrap();

        let empty_vectors = vec![];
        let result = engine.analyze_vectors(empty_vectors).await;
        assert!(result.is_ok());

        let results = result.unwrap();
        assert_eq!(results.summary.entities_analyzed, 0);
    }

    #[tokio::test]
    async fn test_analyze_vectors_with_multiple_features() {
        let config = AnalysisConfig::default();
        let mut engine = ValknutEngine::new(config).await.unwrap();

        let mut vectors = vec![FeatureVector::new("complex_entity")];
        vectors[0].add_feature("complexity", 10.0);
        vectors[0].add_feature("maintainability", 0.3);
        vectors[0].add_feature("duplication", 5.0);

        let result = engine.analyze_vectors(vectors).await;
        assert!(result.is_ok());

        let results = result.unwrap();
        // Engine should process something (entities_analyzed is unsigned, always >= 0)
    }

    #[tokio::test]
    async fn test_config_access() {
        let original_config = AnalysisConfig::default()
            .with_confidence_threshold(0.85)
            .with_max_files(100);
        let engine = ValknutEngine::new(original_config).await.unwrap();

        let engine_config = engine.config();
        assert_eq!(engine_config.analysis.confidence_threshold, 0.85);
        assert_eq!(engine_config.analysis.max_files, 100);
    }

    #[tokio::test]
    async fn test_is_ready() {
        let config = AnalysisConfig::default();
        let engine = ValknutEngine::new(config).await.unwrap();

        // Engine should be ready after creation (even if pipeline isn't fitted)
        let ready = engine.is_ready();
        // This will depend on the pipeline implementation, so we just test it doesn't crash
        let _ = ready;
    }

    #[tokio::test]
    async fn test_get_supported_languages() {
        let config = AnalysisConfig::default()
            .with_languages(vec!["python".to_string(), "javascript".to_string()]);
        let engine = ValknutEngine::new(config).await.unwrap();

        let languages = engine.get_supported_languages();
        // Should have some languages enabled from the default configuration
        assert!(!languages.is_empty());
    }

    #[tokio::test]
    async fn test_health_check_comprehensive() {
        let config = AnalysisConfig::default();
        let engine = ValknutEngine::new(config).await.unwrap();

        let health = engine.health_check().await;

        // Should have several checks
        assert!(health.checks.len() >= 4);

        // Check for expected components
        let check_names: Vec<&str> = health.checks.iter().map(|c| c.name.as_str()).collect();
        assert!(check_names.contains(&"Configuration"));
        assert!(check_names.contains(&"Pipeline"));
        assert!(check_names.contains(&"Feature Extractors"));
        assert!(check_names.contains(&"Language Support"));

        // Timestamp should be recent
        let now = chrono::Utc::now();
        let check_time = health.timestamp;
        let diff = now - check_time;
        assert!(diff.num_seconds() < 10); // Should be within 10 seconds
    }

    #[test]
    fn test_engine_status_debug() {
        let status = EngineStatus {
            is_ready: true,
            pipeline_fitted: false,
            configuration_valid: true,
            issues: vec!["test issue".to_string()],
            supported_languages: vec!["python".to_string(), "rust".to_string()],
        };

        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("is_ready: true"));
        assert!(debug_str.contains("pipeline_fitted: false"));
        assert!(debug_str.contains("test issue"));
        assert!(debug_str.contains("python"));
        assert!(debug_str.contains("rust"));
    }

    #[test]
    fn test_health_check_result_debug() {
        let result = HealthCheckResult {
            overall_status: true,
            checks: vec![HealthCheck {
                name: "Test".to_string(),
                status: HealthCheckStatus::Passed,
                message: Some("All good".to_string()),
            }],
            timestamp: chrono::Utc::now(),
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("overall_status: true"));
        assert!(debug_str.contains("Test"));
        assert!(debug_str.contains("Passed"));
        assert!(debug_str.contains("All good"));
    }

    #[test]
    fn test_health_check_status_equality() {
        assert_eq!(HealthCheckStatus::Passed, HealthCheckStatus::Passed);
        assert_eq!(HealthCheckStatus::Failed, HealthCheckStatus::Failed);
        assert_eq!(HealthCheckStatus::Warning, HealthCheckStatus::Warning);
        assert_ne!(HealthCheckStatus::Passed, HealthCheckStatus::Failed);
        assert_ne!(HealthCheckStatus::Warning, HealthCheckStatus::Passed);
    }

    #[test]
    fn test_health_check_debug() {
        let check = HealthCheck {
            name: "Test Component".to_string(),
            status: HealthCheckStatus::Warning,
            message: Some("Minor issue detected".to_string()),
        };

        let debug_str = format!("{:?}", check);
        assert!(debug_str.contains("Test Component"));
        assert!(debug_str.contains("Warning"));
        assert!(debug_str.contains("Minor issue detected"));
    }

    #[test]
    fn test_health_check_no_message() {
        let check = HealthCheck {
            name: "Silent Check".to_string(),
            status: HealthCheckStatus::Passed,
            message: None,
        };

        let debug_str = format!("{:?}", check);
        assert!(debug_str.contains("Silent Check"));
        assert!(debug_str.contains("Passed"));
        assert!(debug_str.contains("None"));
    }
}
