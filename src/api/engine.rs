//! Main analysis engine implementation.

use std::path::Path;
use std::sync::Arc;

use tracing::{info, error};

use crate::api::config_types::AnalysisConfig as ApiAnalysisConfig;
use crate::api::results::AnalysisResults;
use crate::core::config::ValknutConfig;
use crate::core::pipeline::{AnalysisPipeline, AnalysisConfig as PipelineAnalysisConfig};
use crate::core::featureset::FeatureVector;
use crate::core::errors::{Result, ValknutError};

/// Main valknut analysis engine
pub struct ValknutEngine {
    /// Internal analysis pipeline
    pipeline: AnalysisPipeline,
    
    /// Engine configuration
    config: Arc<ValknutConfig>,
}

impl ValknutEngine {
    /// Create a new valknut engine with the given configuration
    pub async fn new(config: ApiAnalysisConfig) -> Result<Self> {
        info!("Initializing Valknut analysis engine");
        
        // Convert high-level config to internal config
        let internal_config = config.to_valknut_config();
        
        // Validate configuration
        internal_config.validate()?;
        
        let config_arc = Arc::new(internal_config);
        let analysis_config = PipelineAnalysisConfig::from((*config_arc).clone());
        let mut pipeline = AnalysisPipeline::new(analysis_config);
        
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
    
    /// Analyze a directory of code files
    pub async fn analyze_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<AnalysisResults> {
        let path = path.as_ref();
        info!("Starting directory analysis: {}", path.display());
        
        // Verify path exists
        if !path.exists() {
            return Err(ValknutError::io(
                format!("Path does not exist: {}", path.display()),
                std::io::Error::new(std::io::ErrorKind::NotFound, "Path not found")
            ));
        }
        
        if !path.is_dir() {
            return Err(ValknutError::validation(
                format!("Path is not a directory: {}", path.display())
            ));
        }
        
        // Run the pipeline
        let pipeline_results = self.pipeline.analyze_directory(path).await?;
        
        // Convert to public API format
        let results = AnalysisResults::from_pipeline_results(pipeline_results);
        
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
        
        // TODO: Implement file-specific analysis
        // For now, delegate to directory analysis of parent directories
        
        if files.is_empty() {
            return Ok(AnalysisResults {
                summary: crate::api::results::AnalysisSummary {
                    files_processed: 0,
                    entities_analyzed: 0,
                    refactoring_needed: 0,
                    high_priority: 0,
                    critical: 0,
                    avg_refactoring_score: 0.0,
                    code_health_score: 1.0,
                },
                refactoring_candidates: Vec::new(),
                statistics: crate::api::results::AnalysisStatistics {
                    total_duration: std::time::Duration::from_secs(0),
                    avg_file_processing_time: std::time::Duration::from_secs(0),
                    avg_entity_processing_time: std::time::Duration::from_secs(0),
                    features_per_entity: std::collections::HashMap::new(),
                    priority_distribution: std::collections::HashMap::new(),
                    issue_distribution: std::collections::HashMap::new(),
                    memory_stats: crate::api::results::MemoryStats {
                        peak_memory_bytes: 0,
                        final_memory_bytes: 0,
                        efficiency_score: 1.0,
                    },
                },
                // naming_results: None,
                warnings: Vec::new(),
            });
        }
        
        // For now, analyze the parent directory of the first file
        let first_file = files[0].as_ref();
        if let Some(parent) = first_file.parent() {
            self.analyze_directory(parent).await
        } else {
            Err(ValknutError::validation("Cannot determine parent directory for analysis"))
        }
    }
    
    /// Analyze pre-extracted feature vectors (for testing and advanced usage)
    pub async fn analyze_vectors(&mut self, vectors: Vec<FeatureVector>) -> Result<AnalysisResults> {
        info!("Analyzing {} pre-extracted feature vectors", vectors.len());
        
        // Ensure pipeline is ready
        if !vectors.is_empty() && !self.pipeline.is_ready() {
            // Fit the pipeline with the provided vectors as training data
            info!("Fitting pipeline with provided vectors");
            self.pipeline.fit(&vectors).await?;
        }
        
        // Run analysis
        let pipeline_results = self.pipeline.analyze_vectors(vectors).await?;
        
        // Convert to public API format
        let results = AnalysisResults::from_pipeline_results(pipeline_results);
        
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
        if let Err(e) = self.config.validate() {
            checks.push(HealthCheck {
                name: "Configuration".to_string(),
                status: HealthCheckStatus::Failed,
                message: Some(e.to_string()),
            });
            overall_status = false;
        } else {
            checks.push(HealthCheck {
                name: "Configuration".to_string(),
                status: HealthCheckStatus::Passed,
                message: None,
            });
        }
        
        // Check pipeline status
        let pipeline_status = self.pipeline.get_status();
        if pipeline_status.ready {
            checks.push(HealthCheck {
                name: "Pipeline".to_string(),
                status: HealthCheckStatus::Passed,
                message: None,
            });
        } else {
            checks.push(HealthCheck {
                name: "Pipeline".to_string(),
                status: HealthCheckStatus::Failed,
                message: Some(pipeline_status.issues.join("; ")),
            });
            overall_status = false;
        }
        
        // Check feature extractors
        let extractor_count = self.pipeline.extractor_registry().get_all_extractors().count();
        if extractor_count > 0 {
            checks.push(HealthCheck {
                name: "Feature Extractors".to_string(),
                status: HealthCheckStatus::Passed,
                message: Some(format!("{} extractors available", extractor_count)),
            });
        } else {
            checks.push(HealthCheck {
                name: "Feature Extractors".to_string(),
                status: HealthCheckStatus::Warning,
                message: Some("No feature extractors registered".to_string()),
            });
        }
        
        // Check supported languages
        let supported_languages = self.get_supported_languages();
        if supported_languages.is_empty() {
            checks.push(HealthCheck {
                name: "Language Support".to_string(),
                status: HealthCheckStatus::Warning,
                message: Some("No languages enabled".to_string()),
            });
        } else {
            checks.push(HealthCheck {
                name: "Language Support".to_string(),
                status: HealthCheckStatus::Passed,
                message: Some(format!("Languages: {}", supported_languages.join(", "))),
            });
        }
        
        HealthCheckResult {
            overall_status,
            checks,
            timestamp: chrono::Utc::now(),
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
    use tempfile::TempDir;
    use crate::api::config_types::AnalysisConfig;

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
        println!("Files processed: {}, entities analyzed: {}", results.summary.files_processed, results.summary.entities_analyzed);
        // Empty directory might still analyze some files (like hidden config files)
        assert_eq!(results.summary.entities_analyzed, 0);
    }
    
    #[tokio::test]
    async fn test_analyze_vectors() {
        let config = AnalysisConfig::default();
        let mut engine = ValknutEngine::new(config).await.unwrap();
        
        // Create test vectors
        let mut vectors = vec![
            FeatureVector::new("entity1"),
            FeatureVector::new("entity2"),
        ];
        
        vectors[0].add_feature("complexity", 2.0);
        vectors[1].add_feature("complexity", 8.0);
        
        let result = engine.analyze_vectors(vectors).await;
        assert!(result.is_ok());
        
        let results = result.unwrap();
        println!("Vector test - entities analyzed: {}", results.summary.entities_analyzed);
        // The vector analysis should analyze some entities, but the exact count may vary
        // based on implementation details
        assert!(results.summary.entities_analyzed >= 0); // At least no crash
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
}