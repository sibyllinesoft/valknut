//! Comprehensive analysis pipeline that orchestrates all analyzers.
//!
//! This module provides the main analysis pipeline that coordinates:
//! - Structure analysis (directory organization, file splitting)
//! - Complexity analysis (cyclomatic, cognitive, technical debt)
//! - Refactoring analysis (improvement recommendations)
//! - Impact analysis (dependency cycles, chokepoints)
//! - Report generation and output formatting

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn, error};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tokio::fs;

use crate::core::config::ValknutConfig;
use crate::core::errors::{Result, ValknutError};
use crate::core::scoring::{Priority, ScoringResult};
use crate::core::featureset::FeatureVector;
use crate::detectors::complexity::{ComplexityAnalyzer, ComplexityAnalysisResult, ComplexityConfig};
use crate::detectors::structure::{StructureExtractor, StructureConfig};
use crate::detectors::refactoring::{RefactoringAnalyzer, RefactoringAnalysisResult, RefactoringConfig};
use crate::detectors::names_simple::{SimpleNameAnalyzer, NamingAnalysisResult, NamesConfig};

/// Comprehensive analysis result containing all analysis types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveAnalysisResult {
    /// Unique identifier for this analysis run
    pub analysis_id: String,
    /// Timestamp when analysis started
    pub timestamp: DateTime<Utc>,
    /// Total processing time in seconds
    pub processing_time: f64,
    /// Analysis configuration used
    pub config: AnalysisConfig,
    /// Summary statistics
    pub summary: AnalysisSummary,
    /// Structure analysis results
    pub structure: StructureAnalysisResults,
    /// Complexity analysis results
    pub complexity: ComplexityAnalysisResults,
    /// Refactoring analysis results
    pub refactoring: RefactoringAnalysisResults,
    /// Naming analysis results
    pub naming: NamingAnalysisResults,
    /// Impact analysis results  
    pub impact: ImpactAnalysisResults,
    /// Overall health metrics
    pub health_metrics: HealthMetrics,
}

/// Configuration for comprehensive analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Enable structure analysis
    pub enable_structure_analysis: bool,
    /// Enable complexity analysis
    pub enable_complexity_analysis: bool,
    /// Enable refactoring analysis
    pub enable_refactoring_analysis: bool,
    /// Enable naming analysis
    pub enable_naming_analysis: bool,
    /// Enable impact analysis
    pub enable_impact_analysis: bool,
    /// File extensions to include
    pub file_extensions: Vec<String>,
    /// Directories to exclude
    pub exclude_directories: Vec<String>,
    /// Maximum files to analyze (0 = no limit)
    pub max_files: usize,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            enable_structure_analysis: true,
            enable_complexity_analysis: true,
            enable_refactoring_analysis: true,
            enable_naming_analysis: true,
            enable_impact_analysis: true,
            file_extensions: vec![
                "py".to_string(),
                "js".to_string(),
                "ts".to_string(),
                "tsx".to_string(),
                "jsx".to_string(),
                "rs".to_string(),
                "go".to_string(),
                "java".to_string(),
            ],
            exclude_directories: vec![
                "node_modules".to_string(),
                "target".to_string(),
                "__pycache__".to_string(),
                ".git".to_string(),
                "dist".to_string(),
                "build".to_string(),
            ],
            max_files: 1000,
        }
    }
}

/// Summary statistics for the analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSummary {
    /// Total files analyzed
    pub total_files: usize,
    /// Total entities analyzed (functions, classes, etc.)
    pub total_entities: usize,
    /// Total lines of code
    pub total_lines_of_code: usize,
    /// Languages detected
    pub languages: Vec<String>,
    /// Total issues found
    pub total_issues: usize,
    /// High-priority issues
    pub high_priority_issues: usize,
    /// Critical issues
    pub critical_issues: usize,
}

/// Structure analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Directory reorganization recommendations
    pub directory_recommendations: Vec<serde_json::Value>,
    /// File splitting recommendations
    pub file_splitting_recommendations: Vec<serde_json::Value>,
    /// Structure issues count
    pub issues_count: usize,
}

/// Complexity analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Detailed complexity results per file/entity
    pub detailed_results: Vec<ComplexityAnalysisResult>,
    /// Average cyclomatic complexity
    pub average_cyclomatic_complexity: f64,
    /// Average cognitive complexity
    pub average_cognitive_complexity: f64,
    /// Average technical debt score
    pub average_technical_debt_score: f64,
    /// Average maintainability index
    pub average_maintainability_index: f64,
    /// Complexity issues count
    pub issues_count: usize,
}

/// Refactoring analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Detailed refactoring results
    pub detailed_results: Vec<RefactoringAnalysisResult>,
    /// Refactoring opportunities count
    pub opportunities_count: usize,
}

/// Naming analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamingAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Detailed naming results
    pub detailed_results: Vec<NamingAnalysisResult>,
    /// Naming issues count
    pub issues_count: usize,
}

/// Impact analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysisResults {
    /// Enabled flag
    pub enabled: bool,
    /// Dependency cycles detected
    pub dependency_cycles: Vec<serde_json::Value>,
    /// Chokepoint modules
    pub chokepoints: Vec<serde_json::Value>,
    /// Clone groups
    pub clone_groups: Vec<serde_json::Value>,
    /// Impact issues count
    pub issues_count: usize,
}

/// Overall health metrics for the codebase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// Overall health score (0-100, higher is better)
    pub overall_health_score: f64,
    /// Maintainability score (0-100, higher is better)
    pub maintainability_score: f64,
    /// Technical debt ratio (0-100, lower is better)
    pub technical_debt_ratio: f64,
    /// Complexity score (0-100, lower is better)
    pub complexity_score: f64,
    /// Structure quality score (0-100, higher is better)
    pub structure_quality_score: f64,
}

impl From<ValknutConfig> for AnalysisConfig {
    fn from(_valknut_config: ValknutConfig) -> Self {
        Self::default()
    }
}

/// Progress callback function type
pub type ProgressCallback = Box<dyn Fn(&str, f64) + Send + Sync>;

/// Main analysis pipeline that orchestrates all analyzers
pub struct AnalysisPipeline {
    config: AnalysisConfig,
    complexity_analyzer: ComplexityAnalyzer,
    structure_extractor: StructureExtractor,
    refactoring_analyzer: RefactoringAnalyzer,
    name_analyzer: SimpleNameAnalyzer,
}

impl AnalysisPipeline {
    /// Create new analysis pipeline with configuration
    pub fn new(config: AnalysisConfig) -> Self {
        let complexity_config = ComplexityConfig::default();
        let structure_config = StructureConfig::default();
        let refactoring_config = RefactoringConfig::default();
        let names_config = NamesConfig::default();

        Self {
            config,
            complexity_analyzer: ComplexityAnalyzer::new(complexity_config),
            structure_extractor: StructureExtractor::with_config(structure_config),
            refactoring_analyzer: RefactoringAnalyzer::new(refactoring_config),
            name_analyzer: SimpleNameAnalyzer::new(names_config),
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(AnalysisConfig::default())
    }

    /// Run comprehensive analysis on the given paths
    pub async fn analyze_paths(
        &self, 
        paths: &[PathBuf],
        progress_callback: Option<ProgressCallback>,
    ) -> Result<ComprehensiveAnalysisResult> {
        let start_time = Instant::now();
        let analysis_id = Uuid::new_v4().to_string();
        
        info!("Starting comprehensive analysis {} for {} paths", analysis_id, paths.len());

        // Update progress
        if let Some(ref callback) = progress_callback {
            callback("Discovering files...", 0.0);
        }

        // Stage 1: File discovery
        let files = self.discover_files(paths).await?;
        info!("Discovered {} files for analysis", files.len());

        if let Some(ref callback) = progress_callback {
            callback("Analyzing file structure...", 10.0);
        }

        // Stage 2: Structure analysis
        let structure_results = if self.config.enable_structure_analysis {
            self.run_structure_analysis(paths).await?
        } else {
            StructureAnalysisResults {
                enabled: false,
                directory_recommendations: Vec::new(),
                file_splitting_recommendations: Vec::new(),
                issues_count: 0,
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Analyzing code complexity...", 30.0);
        }

        // Stage 3: Complexity analysis
        let complexity_results = if self.config.enable_complexity_analysis {
            self.run_complexity_analysis(&files).await?
        } else {
            ComplexityAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                average_cyclomatic_complexity: 0.0,
                average_cognitive_complexity: 0.0,
                average_technical_debt_score: 0.0,
                average_maintainability_index: 100.0,
                issues_count: 0,
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Analyzing refactoring opportunities...", 50.0);
        }

        // Stage 4: Refactoring analysis
        let refactoring_results = if self.config.enable_refactoring_analysis {
            self.run_refactoring_analysis(&files).await?
        } else {
            RefactoringAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                opportunities_count: 0,
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Analyzing function naming...", 65.0);
        }

        // Stage 5: Naming analysis
        let naming_results = if self.config.enable_naming_analysis {
            self.run_naming_analysis(&files).await?
        } else {
            NamingAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                issues_count: 0,
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Analyzing dependencies and impact...", 80.0);
        }

        // Stage 6: Impact analysis
        let impact_results = if self.config.enable_impact_analysis {
            self.run_impact_analysis(&files).await?
        } else {
            ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            }
        };

        if let Some(ref callback) = progress_callback {
            callback("Calculating health metrics...", 90.0);
        }

        // Stage 7: Calculate summary and health metrics
        let summary = self.calculate_summary(&files, &structure_results, &complexity_results, &refactoring_results, &naming_results, &impact_results);
        let health_metrics = self.calculate_health_metrics(&complexity_results, &structure_results, &impact_results);

        if let Some(ref callback) = progress_callback {
            callback("Analysis complete", 100.0);
        }

        let processing_time = start_time.elapsed().as_secs_f64();
        
        info!("Comprehensive analysis completed in {:.2}s", processing_time);
        info!("Total issues found: {}", summary.total_issues);
        info!("Overall health score: {:.1}", health_metrics.overall_health_score);

        Ok(ComprehensiveAnalysisResult {
            analysis_id,
            timestamp: Utc::now(),
            processing_time,
            config: self.config.clone(),
            summary,
            structure: structure_results,
            complexity: complexity_results,
            refactoring: refactoring_results,
            naming: naming_results,
            impact: impact_results,
            health_metrics,
        })
    }

    /// Discover files to analyze
    async fn discover_files(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        for path in paths {
            if path.is_file() {
                if self.should_include_file(path) {
                    files.push(path.clone());
                }
            } else if path.is_dir() {
                self.discover_files_recursive(path, &mut files).await?;
            }
        }

        // Limit files if configured
        if self.config.max_files > 0 && files.len() > self.config.max_files {
            warn!("Limiting analysis to {} files (found {})", self.config.max_files, files.len());
            files.truncate(self.config.max_files);
        }

        Ok(files)
    }

    /// Recursively discover files in a directory
    fn discover_files_recursive<'a>(&'a self, dir: &'a Path, files: &'a mut Vec<PathBuf>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
        let mut entries = fs::read_dir(dir).await
            .map_err(|e| ValknutError::io(format!("Failed to read directory {}: {}", dir.display(), e), e))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| ValknutError::io("Failed to read directory entry".to_string(), e))? {
            
            let path = entry.path();
            
            if path.is_file() && self.should_include_file(&path) {
                files.push(path);
            } else if path.is_dir() && self.should_include_directory(&path) {
                self.discover_files_recursive(&path, files).await?;
            }
        }

        Ok(())
        })
    }

    /// Check if a file should be included in analysis
    fn should_include_file(&self, file: &Path) -> bool {
        if let Some(extension) = file.extension().and_then(|ext| ext.to_str()) {
            self.config.file_extensions.contains(&extension.to_string())
        } else {
            false
        }
    }

    /// Check if a directory should be included in analysis
    fn should_include_directory(&self, dir: &Path) -> bool {
        if let Some(dir_name) = dir.file_name().and_then(|name| name.to_str()) {
            !self.config.exclude_directories.contains(&dir_name.to_string())
        } else {
            true
        }
    }

    /// Run structure analysis
    async fn run_structure_analysis(&self, paths: &[PathBuf]) -> Result<StructureAnalysisResults> {
        debug!("Running structure analysis");
        
        let mut all_recommendations = Vec::new();
        let mut file_splitting_recommendations = Vec::new();
        
        for path in paths {
            match self.structure_extractor.generate_recommendations(path).await {
                Ok(recommendations) => {
                    for rec in recommendations {
                        match rec.get("kind") {
                            Some(serde_json::Value::String(kind)) if kind == "file_split" => {
                                file_splitting_recommendations.push(rec);
                            },
                            _ => {
                                all_recommendations.push(rec);
                            }
                        }
                    }
                },
                Err(e) => warn!("Structure analysis failed for {}: {}", path.display(), e),
            }
        }

        let issues_count = all_recommendations.len() + file_splitting_recommendations.len();

        Ok(StructureAnalysisResults {
            enabled: true,
            directory_recommendations: all_recommendations,
            file_splitting_recommendations,
            issues_count,
        })
    }

    /// Run complexity analysis
    async fn run_complexity_analysis(&self, files: &[PathBuf]) -> Result<ComplexityAnalysisResults> {
        debug!("Running complexity analysis on {} files", files.len());
        
        let file_refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
        let detailed_results = self.complexity_analyzer.analyze_files(&file_refs).await?;

        // Calculate averages
        let count = detailed_results.len() as f64;
        let total_cyclomatic: f64 = detailed_results.iter().map(|r| r.metrics.cyclomatic).sum();
        let total_cognitive: f64 = detailed_results.iter().map(|r| r.metrics.cognitive).sum();
        let total_debt: f64 = detailed_results.iter().map(|r| r.metrics.technical_debt_score).sum();
        let total_maintainability: f64 = detailed_results.iter().map(|r| r.metrics.maintainability_index).sum();

        let average_cyclomatic_complexity = if count > 0.0 { total_cyclomatic / count } else { 0.0 };
        let average_cognitive_complexity = if count > 0.0 { total_cognitive / count } else { 0.0 };
        let average_technical_debt_score = if count > 0.0 { total_debt / count } else { 0.0 };
        let average_maintainability_index = if count > 0.0 { total_maintainability / count } else { 100.0 };

        // Count issues
        let issues_count = detailed_results.iter().map(|r| r.issues.len()).sum();

        Ok(ComplexityAnalysisResults {
            enabled: true,
            detailed_results,
            average_cyclomatic_complexity,
            average_cognitive_complexity,
            average_technical_debt_score,
            average_maintainability_index,
            issues_count,
        })
    }

    /// Run refactoring analysis
    async fn run_refactoring_analysis(&self, files: &[PathBuf]) -> Result<RefactoringAnalysisResults> {
        debug!("Running refactoring analysis on {} files", files.len());
        
        let detailed_results = self.refactoring_analyzer.analyze_files(files).await?;
        let opportunities_count = detailed_results.iter().map(|r| r.recommendations.len()).sum();

        Ok(RefactoringAnalysisResults {
            enabled: true,
            detailed_results,
            opportunities_count,
        })
    }

    /// Run naming analysis
    async fn run_naming_analysis(&self, files: &[PathBuf]) -> Result<NamingAnalysisResults> {
        debug!("Running naming analysis on {} files", files.len());
        
        let file_refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
        let detailed_results = self.name_analyzer.analyze_files(&file_refs).await?;
        let issues_count = detailed_results.len();

        Ok(NamingAnalysisResults {
            enabled: true,
            detailed_results,
            issues_count,
        })
    }

    /// Run impact analysis (placeholder for now)
    async fn run_impact_analysis(&self, _files: &[PathBuf]) -> Result<ImpactAnalysisResults> {
        debug!("Running impact analysis (placeholder implementation)");
        
        // TODO: Implement dependency cycle detection, chokepoint analysis, clone detection
        Ok(ImpactAnalysisResults {
            enabled: true,
            dependency_cycles: Vec::new(),
            chokepoints: Vec::new(),
            clone_groups: Vec::new(),
            issues_count: 0,
        })
    }

    /// Calculate analysis summary
    fn calculate_summary(
        &self,
        files: &[PathBuf],
        structure: &StructureAnalysisResults,
        complexity: &ComplexityAnalysisResults,
        refactoring: &RefactoringAnalysisResults,
        naming: &NamingAnalysisResults,
        impact: &ImpactAnalysisResults,
    ) -> AnalysisSummary {
        let total_files = files.len();
        let total_entities = complexity.detailed_results.len(); // Approximate
        let total_lines_of_code = complexity.detailed_results
            .iter()
            .map(|r| r.metrics.lines_of_code as usize)
            .sum();

        // Extract languages from file extensions
        let mut languages = std::collections::HashSet::new();
        for file in files {
            if let Some(extension) = file.extension().and_then(|ext| ext.to_str()) {
                let language = match extension {
                    "py" => "Python",
                    "js" | "jsx" => "JavaScript",
                    "ts" | "tsx" => "TypeScript", 
                    "rs" => "Rust",
                    "go" => "Go",
                    "java" => "Java",
                    _ => continue,
                };
                languages.insert(language.to_string());
            }
        }

        let total_issues = structure.issues_count + complexity.issues_count + naming.issues_count + impact.issues_count;
        
        // Count high-priority and critical issues from complexity analysis
        let mut high_priority_issues = 0;
        let mut critical_issues = 0;
        
        for result in &complexity.detailed_results {
            for issue in &result.issues {
                match issue.severity {
                    crate::detectors::complexity::ComplexitySeverity::High => high_priority_issues += 1,
                    crate::detectors::complexity::ComplexitySeverity::VeryHigh => high_priority_issues += 1,
                    crate::detectors::complexity::ComplexitySeverity::Critical => critical_issues += 1,
                    _ => {}
                }
            }
        }

        AnalysisSummary {
            total_files,
            total_entities,
            total_lines_of_code,
            languages: languages.into_iter().collect(),
            total_issues,
            high_priority_issues,
            critical_issues,
        }
    }

    /// Calculate overall health metrics
    fn calculate_health_metrics(
        &self,
        complexity: &ComplexityAnalysisResults,
        structure: &StructureAnalysisResults,
        impact: &ImpactAnalysisResults,
    ) -> HealthMetrics {
        // Complexity score (0-100, lower is better)
        let complexity_score = if complexity.enabled {
            let avg_complexity = (complexity.average_cyclomatic_complexity + complexity.average_cognitive_complexity) / 2.0;
            (avg_complexity * 4.0).min(100.0) // Scale to 0-100
        } else {
            0.0
        };

        // Technical debt ratio (average of technical debt scores)
        let technical_debt_ratio = if complexity.enabled {
            complexity.average_technical_debt_score
        } else {
            0.0
        };

        // Maintainability score (average maintainability index)
        let maintainability_score = if complexity.enabled {
            complexity.average_maintainability_index
        } else {
            100.0
        };

        // Structure quality score (based on issues found)
        let structure_quality_score = if structure.enabled {
            let issue_penalty = structure.issues_count as f64 * 5.0;
            (100.0 - issue_penalty).max(0.0)
        } else {
            100.0
        };

        // Overall health score (weighted average)
        let overall_health_score = (
            maintainability_score * 0.3 +
            structure_quality_score * 0.3 +
            (100.0 - complexity_score) * 0.2 +
            (100.0 - technical_debt_ratio) * 0.2
        ).max(0.0).min(100.0);

        HealthMetrics {
            overall_health_score,
            maintainability_score,
            technical_debt_ratio,
            complexity_score,
            structure_quality_score,
        }
    }

    /// Get pipeline status for API layer
    pub fn get_status(&self) -> PipelineStatus {
        let is_ready = self.is_ready();
        PipelineStatus {
            initialized: true,
            ready: is_ready,
            is_ready,
            config_valid: true,  // Assume config is valid if pipeline was initialized
            current_stage: None,
            processed_files: 0,
            total_files: 0,
            issues: Vec::new(),
        }
    }

    /// Check if pipeline is ready for analysis
    pub fn is_ready(&self) -> bool {
        true
    }

    /// Get extractor registry (placeholder)
    pub fn extractor_registry(&self) -> ExtractorRegistry {
        ExtractorRegistry::new()
    }
}

/// Placeholder extractor registry
pub struct ExtractorRegistry {
}

impl ExtractorRegistry {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_all_extractors(&self) -> std::iter::Empty<()> {
        std::iter::empty()
    }
}

impl AnalysisPipeline {
    /// Analyze directory - wrapper around analyze_paths
    pub async fn analyze_directory(&self, path: &Path) -> Result<PipelineResults> {
        let paths = vec![path.to_path_buf()];
        let results = self.analyze_paths(&paths, None).await?;
        
        Ok(PipelineResults {
            analysis_id: results.analysis_id.clone(),
            timestamp: results.timestamp,
            results,
            statistics: PipelineStatistics {
                memory_stats: MemoryStats {
                    current_memory_bytes: 0,
                    peak_memory_bytes: 0,
                },
                files_processed: 1,
                total_duration_ms: 0,
            },
            errors: Vec::new(),
            scoring_results: ScoringResults {
                files: Vec::new(),
            },
            feature_vectors: Vec::new(),
        })
    }

    /// Fit pipeline with training data (placeholder)
    pub async fn fit(&mut self, _vectors: &[FeatureVector]) -> Result<()> {
        // Placeholder implementation
        Ok(())
    }

    /// Analyze vectors (placeholder)
    pub async fn analyze_vectors(&self, _vectors: Vec<FeatureVector>) -> Result<PipelineResults> {
        // For now, create empty results
        let results = ComprehensiveAnalysisResult {
            analysis_id: "placeholder".to_string(),
            timestamp: Utc::now(),
            processing_time: 0.0,
            config: self.config.clone(),
            summary: AnalysisSummary {
                total_files: 0,
                total_entities: 0,
                total_lines_of_code: 0,
                languages: Vec::new(),
                total_issues: 0,
                high_priority_issues: 0,
                critical_issues: 0,
            },
            structure: StructureAnalysisResults {
                enabled: false,
                directory_recommendations: Vec::new(),
                file_splitting_recommendations: Vec::new(),
                issues_count: 0,
            },
            complexity: ComplexityAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                average_cyclomatic_complexity: 0.0,
                average_cognitive_complexity: 0.0,
                average_technical_debt_score: 0.0,
                average_maintainability_index: 0.0,
                issues_count: 0,
            },
            refactoring: RefactoringAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                opportunities_count: 0,
            },
            naming: NamingAnalysisResults {
                enabled: false,
                detailed_results: Vec::new(),
                issues_count: 0,
            },
            impact: ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            },
            health_metrics: HealthMetrics {
                overall_health_score: 100.0,
                maintainability_score: 100.0,
                technical_debt_ratio: 0.0,
                complexity_score: 0.0,
                structure_quality_score: 100.0,
            },
        };

        Ok(PipelineResults {
            analysis_id: "placeholder".to_string(),
            timestamp: Utc::now(),
            results,
            statistics: PipelineStatistics {
                memory_stats: MemoryStats {
                    current_memory_bytes: 0,
                    peak_memory_bytes: 0,
                },
                files_processed: 1,
                total_duration_ms: 0,
            },
            errors: Vec::new(),
            scoring_results: ScoringResults {
                files: Vec::new(),
            },
            feature_vectors: Vec::new(),
        })
    }
}

// Additional types needed by API layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResults {
    pub analysis_id: String,
    pub timestamp: DateTime<Utc>,
    pub results: ComprehensiveAnalysisResult,
    pub statistics: PipelineStatistics,
    pub errors: Vec<String>,
    pub scoring_results: ScoringResults,
    pub feature_vectors: Vec<FeatureVector>,
}

impl PipelineResults {
    /// Generate a summary of pipeline results for API layer
    pub fn summary(&self) -> ResultSummary {
        let refactoring_needed = self.scoring_results
            .files
            .iter()
            .filter(|result| result.needs_refactoring())
            .count();
        
        let avg_score = if !self.scoring_results.files.is_empty() {
            self.scoring_results.files.iter().map(|r| r.overall_score).sum::<f64>() / self.scoring_results.files.len() as f64
        } else {
            0.0
        };
        
        ResultSummary {
            files_analyzed: self.statistics.files_processed,
            issues_found: refactoring_needed,
            health_score: 100.0 - (avg_score * 20.0).min(100.0), // Convert score to health percentage
            processing_time: self.statistics.total_duration_ms as f64 / 1000.0, // Convert to seconds
            total_entities: self.scoring_results.files.len(),
            refactoring_needed,
            avg_score,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringResults {
    pub files: Vec<ScoringResult>,
}

impl ScoringResults {
    pub fn iter(&self) -> std::slice::Iter<'_, ScoringResult> {
        self.files.iter()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileScore {
    pub file_path: String,
    pub score: f64,
    pub priority: Priority,
    pub entity_id: String,
    // Additional fields expected by API layer
    pub category_scores: HashMap<String, f64>,
    pub feature_contributions: HashMap<String, f64>,
    pub overall_score: f64,
    pub confidence: f64,
}

impl FileScore {
    pub fn needs_refactoring(&self) -> bool {
        !matches!(self.priority, Priority::None) && self.score > 1.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatistics {
    pub memory_stats: MemoryStats,
    pub files_processed: usize,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub current_memory_bytes: u64,
    pub peak_memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultSummary {
    pub files_analyzed: usize,
    pub issues_found: usize,
    pub health_score: f64,
    pub processing_time: f64,
    pub total_entities: usize,
    pub refactoring_needed: usize,
    pub avg_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatus {
    pub initialized: bool,
    pub ready: bool,
    pub is_ready: bool,  // Alias for ready - used by API layer
    pub config_valid: bool,  // Configuration validation status
    pub current_stage: Option<String>,
    pub processed_files: usize,
    pub total_files: usize,
    pub issues: Vec<String>,
}