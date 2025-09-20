//! Individual analysis stages for the pipeline.

// use chrono::{DateTime, Utc}; // Unused imports
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use futures::future;

use super::pipeline_results::{
    ComplexityAnalysisResults, CoverageAnalysisResults, CoverageFileInfo, ImpactAnalysisResults,
    LshAnalysisResults, RefactoringAnalysisResults, StructureAnalysisResults,
};
use crate::core::ast_service::AstService;
use crate::core::config::CoverageConfig;
use crate::core::dependency::ProjectDependencyAnalysis;
use crate::core::errors::Result;
use crate::core::featureset::FeatureExtractor;
use crate::core::file_utils::{CoverageDiscovery, CoverageFile, CoverageFormat};
use crate::detectors::complexity::{AstComplexityAnalyzer, ComplexityAnalyzer};
use crate::detectors::coverage::CoverageExtractor;
use crate::detectors::lsh::LshExtractor;
use crate::detectors::refactoring::RefactoringAnalyzer;
use crate::detectors::structure::StructureExtractor;
use std::sync::Arc;

/// Handles all individual analysis stages
pub struct AnalysisStages {
    pub structure_extractor: StructureExtractor,
    pub complexity_analyzer: ComplexityAnalyzer,
    pub ast_complexity_analyzer: AstComplexityAnalyzer,
    pub refactoring_analyzer: RefactoringAnalyzer,
    pub lsh_extractor: Option<LshExtractor>,
    pub coverage_extractor: CoverageExtractor,
    pub ast_service: Arc<AstService>,
}

impl AnalysisStages {
    /// Create new analysis stages with the given analyzers
    pub fn new(
        structure_extractor: StructureExtractor,
        complexity_analyzer: ComplexityAnalyzer,
        refactoring_analyzer: RefactoringAnalyzer,
        ast_service: Arc<AstService>,
    ) -> Self {
        let ast_complexity_analyzer = AstComplexityAnalyzer::new(
            crate::detectors::complexity::ComplexityConfig::default(),
            ast_service.clone(),
        );

        Self {
            structure_extractor,
            complexity_analyzer,
            ast_complexity_analyzer,
            refactoring_analyzer,
            lsh_extractor: None,
            coverage_extractor: CoverageExtractor::new(Default::default(), ast_service.clone()),
            ast_service,
        }
    }

    /// Create new analysis stages with LSH support
    pub fn new_with_lsh(
        structure_extractor: StructureExtractor,
        complexity_analyzer: ComplexityAnalyzer,
        refactoring_analyzer: RefactoringAnalyzer,
        lsh_extractor: LshExtractor,
        ast_service: Arc<AstService>,
    ) -> Self {
        let ast_complexity_analyzer = AstComplexityAnalyzer::new(
            crate::detectors::complexity::ComplexityConfig::default(),
            ast_service.clone(),
        );

        Self {
            structure_extractor,
            complexity_analyzer,
            ast_complexity_analyzer,
            refactoring_analyzer,
            lsh_extractor: Some(lsh_extractor),
            coverage_extractor: CoverageExtractor::new(Default::default(), ast_service.clone()),
            ast_service,
        }
    }

    /// Run structure analysis
    pub async fn run_structure_analysis(
        &self,
        paths: &[PathBuf],
    ) -> Result<StructureAnalysisResults> {
        debug!("Running structure analysis");

        let mut all_recommendations = Vec::new();
        let mut file_splitting_recommendations = Vec::new();

        for path in paths {
            match self
                .structure_extractor
                .generate_recommendations(path)
                .await
            {
                Ok(recommendations) => {
                    for rec in recommendations {
                        match rec.get("kind") {
                            Some(serde_json::Value::String(kind)) if kind == "file_split" => {
                                file_splitting_recommendations.push(rec);
                            }
                            _ => {
                                all_recommendations.push(rec);
                            }
                        }
                    }
                }
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
    pub async fn run_complexity_analysis(
        &self,
        files: &[PathBuf],
    ) -> Result<ComplexityAnalysisResults> {
        debug!("Running complexity analysis on {} files", files.len());

        // Parallelize file analysis using tokio::spawn
        // Since AstComplexityAnalyzer contains Arc<AstService>, we need to access the shared service
        let analysis_futures = files.iter().map(|file_path| {
            let ast_service = self.ast_service.clone();
            let config = crate::detectors::complexity::ComplexityConfig::default();
            let path = file_path.clone();
            
            tokio::spawn(async move {
                // Create a local analyzer for this file
                let analyzer = crate::detectors::complexity::AstComplexityAnalyzer::new(config, ast_service);
                let file_refs = vec![path.as_path()];
                analyzer.analyze_files(&file_refs).await
            })
        });

        // Wait for all concurrent analyses to complete
        let results_of_results = futures::future::join_all(analysis_futures).await;

        // Collect and flatten the results
        let mut detailed_results = Vec::new();
        for result in results_of_results {
            match result {
                Ok(Ok(file_results)) => detailed_results.extend(file_results),
                Ok(Err(e)) => warn!("Complexity analysis task failed: {}", e),
                Err(e) => warn!("Tokio spawn failed for complexity analysis: {}", e),
            }
        }

        // Calculate averages
        let count = detailed_results.len() as f64;
        let total_cyclomatic: f64 = detailed_results
            .iter()
            .map(|r| r.metrics.cyclomatic())
            .sum();
        let total_cognitive: f64 = detailed_results.iter().map(|r| r.metrics.cognitive()).sum();
        let total_debt: f64 = detailed_results
            .iter()
            .map(|r| r.metrics.technical_debt_score)
            .sum();
        let total_maintainability: f64 = detailed_results
            .iter()
            .map(|r| r.metrics.maintainability_index)
            .sum();

        let average_cyclomatic_complexity = if count > 0.0 {
            total_cyclomatic / count
        } else {
            0.0
        };
        let average_cognitive_complexity = if count > 0.0 {
            total_cognitive / count
        } else {
            0.0
        };
        let average_technical_debt_score = if count > 0.0 { total_debt / count } else { 0.0 };
        let average_maintainability_index = if count > 0.0 {
            total_maintainability / count
        } else {
            100.0
        };

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
    pub async fn run_refactoring_analysis(
        &self,
        files: &[PathBuf],
    ) -> Result<RefactoringAnalysisResults> {
        debug!("Running refactoring analysis on {} files", files.len());

        // Parallelize file analysis using tokio::spawn
        let analysis_futures = files.iter().map(|file_path| {
            let ast_service = self.ast_service.clone();
            let config = crate::detectors::refactoring::RefactoringConfig::default();
            let path = file_path.clone();
            
            tokio::spawn(async move {
                // Create a local analyzer for this file
                let analyzer = crate::detectors::refactoring::RefactoringAnalyzer::new(config, ast_service);
                analyzer.analyze_files(&[path]).await
            })
        });

        // Wait for all concurrent analyses to complete
        let results_of_results = futures::future::join_all(analysis_futures).await;

        // Collect and flatten the results
        let mut detailed_results = Vec::new();
        for result in results_of_results {
            match result {
                Ok(Ok(file_results)) => detailed_results.extend(file_results),
                Ok(Err(e)) => warn!("Refactoring analysis task failed: {}", e),
                Err(e) => warn!("Tokio spawn failed for refactoring analysis: {}", e),
            }
        }
        let opportunities_count = detailed_results
            .iter()
            .map(|r| r.recommendations.len())
            .sum();

        Ok(RefactoringAnalysisResults {
            enabled: true,
            detailed_results,
            opportunities_count,
        })
    }

    /// Run impact analysis powered by the dependency graph
    pub async fn run_impact_analysis(&self, files: &[PathBuf]) -> Result<ImpactAnalysisResults> {
        debug!(
            "Running dependency impact analysis across {} files",
            files.len()
        );

        if files.is_empty() {
            return Ok(ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            });
        }

        let analysis = ProjectDependencyAnalysis::analyze(files)?;

        if analysis.is_empty() {
            return Ok(ImpactAnalysisResults {
                enabled: false,
                dependency_cycles: Vec::new(),
                chokepoints: Vec::new(),
                clone_groups: Vec::new(),
                issues_count: 0,
            });
        }

        let dependency_cycles = analysis
            .cycles()
            .iter()
            .map(|cycle| {
                serde_json::json!({
                    "size": cycle.len(),
                    "members": cycle
                        .iter()
                        .map(|node| serde_json::json!({
                            "name": node.name,
                            "file": node.file_path,
                            "start_line": node.start_line,
                        }))
                        .collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();

        let chokepoints = analysis
            .chokepoints()
            .iter()
            .map(|chokepoint| {
                serde_json::json!({
                    "name": chokepoint.node.name,
                    "file": chokepoint.node.file_path,
                    "start_line": chokepoint.node.start_line,
                    "score": chokepoint.score,
                })
            })
            .collect::<Vec<_>>();

        let issues_count = dependency_cycles.len() + chokepoints.len();

        Ok(ImpactAnalysisResults {
            enabled: true,
            dependency_cycles,
            chokepoints,
            clone_groups: Vec::new(),
            issues_count,
        })
    }

    /// Run LSH analysis for clone detection
    pub async fn run_lsh_analysis(
        &self,
        files: &[PathBuf],
        denoise_enabled: bool,
    ) -> Result<LshAnalysisResults> {
        debug!(
            "Running LSH analysis for clone detection on {} files",
            files.len()
        );

        if let Some(ref lsh_extractor) = self.lsh_extractor {
            use crate::core::config::ValknutConfig;
            use crate::core::featureset::{CodeEntity, ExtractionContext};
            use std::collections::HashMap;
            use std::sync::Arc;

            // Create extraction context
            let config = Arc::new(ValknutConfig::default());
            let context = ExtractionContext::new(config, "mixed");

            // Convert files to CodeEntity objects for LSH analysis
            let mut entities = Vec::new();
            let mut entity_index = HashMap::new();

            for (i, file_path) in files.iter().enumerate() {
                if let Ok(content) = tokio::fs::read_to_string(file_path).await {
                    let entity_id = format!("entity_{}", i);
                    let entity = CodeEntity::new(
                        &entity_id,
                        "function", // Simplified - in real implementation would parse AST
                        &format!("file_{}", i),
                        &file_path.to_string_lossy().to_string(),
                    )
                    .with_source_code(&content);

                    entity_index.insert(entity_id.clone(), entity.clone());
                    entities.push(entity);
                }
            }

            // Update context with entities
            let context = ExtractionContext {
                entity_index,
                ..context
            };

            // Run LSH analysis on each entity in parallel
            // Note: LSH analysis will remain sequential for now due to shared state concerns
            // but could be optimized further with proper Arc wrapping of the extractor
            let mut all_similarities = Vec::new();
            let mut max_similarity: f64 = 0.0;
            let mut total_similarity = 0.0;
            let mut duplicate_count = 0;
            
            // For now, we'll keep LSH sequential but optimize other stages
            // TODO: Parallelize LSH with proper shared state management
            for entity in &entities {
                if let Ok(features) = lsh_extractor.extract(entity, &context).await {
                    if let Some(similarity) = features.get("max_similarity") {
                        all_similarities.push(*similarity);
                        max_similarity = max_similarity.max(*similarity);
                        total_similarity += *similarity;

                        if *similarity > 0.8 {
                            duplicate_count += 1;
                        }
                    }
                }
            }

            let avg_similarity = if !all_similarities.is_empty() {
                total_similarity / all_similarities.len() as f64
            } else {
                0.0
            };

            // Collect TF-IDF stats if denoising was enabled
            let tfidf_stats = if denoise_enabled {
                use super::pipeline_results::TfIdfStats;

                // These would be populated by the weighted analyzer
                Some(TfIdfStats {
                    total_grams: 0,            // TODO: Get from WeightedShingleAnalyzer
                    unique_grams: 0,           // TODO: Get from WeightedShingleAnalyzer
                    top1pct_contribution: 0.0, // TODO: Get from WeightedShingleAnalyzer
                })
            } else {
                None
            };

            Ok(LshAnalysisResults {
                enabled: true,
                clone_pairs: Vec::new(), // TODO: Collect actual clone pairs
                max_similarity,
                avg_similarity,
                duplicate_count,
                denoising_enabled: denoise_enabled,
                tfidf_stats,
            })
        } else {
            // LSH extractor not available
            Ok(LshAnalysisResults {
                enabled: false,
                clone_pairs: Vec::new(),
                max_similarity: 0.0,
                avg_similarity: 0.0,
                duplicate_count: 0,
                denoising_enabled: false,
                tfidf_stats: None,
            })
        }
    }

    /// Run coverage analysis with automatic file discovery
    pub async fn run_coverage_analysis(
        &self,
        root_path: &Path,
        coverage_config: &CoverageConfig,
    ) -> Result<CoverageAnalysisResults> {
        debug!("Running coverage analysis with auto-discovery");

        // Discover coverage files
        let discovered_files =
            CoverageDiscovery::discover_coverage_files(root_path, coverage_config)?;

        if discovered_files.is_empty() {
            info!("No coverage files found - analysis disabled");
            return Ok(CoverageAnalysisResults {
                enabled: false,
                coverage_files_used: Vec::new(),
                coverage_gaps: Vec::new(),
                gaps_count: 0,
                overall_coverage_percentage: None,
                analysis_method: "no_coverage_files_found".to_string(),
            });
        }

        // Convert discovered files to info structs
        let coverage_files_info: Vec<CoverageFileInfo> = discovered_files
            .iter()
            .map(|file| CoverageFileInfo {
                path: file.path.display().to_string(),
                format: format!("{:?}", file.format),
                size: file.size,
                modified: format!("{:?}", file.modified),
            })
            .collect();

        // Log which files are being used
        for file in &discovered_files {
            info!(
                "Using coverage file: {} (format: {:?})",
                file.path.display(),
                file.format
            );
        }

        // Run comprehensive coverage analysis using CoverageExtractor
        let gaps_count = self.analyze_coverage_gaps(&discovered_files).await?;

        // Build actual coverage packs for detailed analysis
        let mut all_coverage_packs = Vec::new();
        for file in &discovered_files {
            let packs = self
                .coverage_extractor
                .build_coverage_packs(vec![file.path.clone()])
                .await?;
            all_coverage_packs.extend(packs);
        }

        // Calculate overall coverage percentage from LCOV data
        let overall_coverage_percentage = if !discovered_files.is_empty() {
            self.calculate_overall_coverage(&discovered_files).await?
        } else {
            None
        };

        let analysis_method = if discovered_files.len() == 1 {
            format!("single_file_{:?}", discovered_files[0].format)
        } else {
            format!("multi_file_{}_sources", discovered_files.len())
        };

        // Convert CoveragePacks to JSON for storage in coverage_gaps
        let coverage_gaps: Vec<serde_json::Value> = all_coverage_packs
            .iter()
            .map(|pack| serde_json::to_value(pack).unwrap_or(serde_json::Value::Null))
            .collect();

        Ok(CoverageAnalysisResults {
            enabled: true,
            coverage_files_used: coverage_files_info,
            coverage_gaps,
            gaps_count,
            overall_coverage_percentage,
            analysis_method,
        })
    }

    /// Analyze coverage gaps from discovered coverage files
    async fn analyze_coverage_gaps(&self, coverage_files: &[CoverageFile]) -> Result<usize> {
        // Basic implementation - count files that could have coverage gaps
        // This is a placeholder for the more sophisticated coverage analysis

        let mut total_gaps = 0;

        for coverage_file in coverage_files {
            match coverage_file.format {
                CoverageFormat::CoveragePyXml
                | CoverageFormat::Cobertura
                | CoverageFormat::JaCoCo => {
                    // XML-based coverage files
                    total_gaps += self.analyze_xml_coverage(&coverage_file.path).await?;
                }
                CoverageFormat::Lcov => {
                    // LCOV format
                    total_gaps += self.analyze_lcov_coverage(&coverage_file.path).await?;
                }
                CoverageFormat::IstanbulJson => {
                    // JSON format
                    total_gaps += self.analyze_json_coverage(&coverage_file.path).await?;
                }
                CoverageFormat::Unknown => {
                    warn!(
                        "Unknown coverage format, skipping: {}",
                        coverage_file.path.display()
                    );
                }
            }
        }

        Ok(total_gaps)
    }

    /// Calculate overall coverage percentage from coverage files
    async fn calculate_overall_coverage(
        &self,
        coverage_files: &[CoverageFile],
    ) -> Result<Option<f64>> {
        for coverage_file in coverage_files {
            if matches!(coverage_file.format, CoverageFormat::Lcov) {
                // Parse LCOV file to calculate coverage percentage
                if let Ok(content) = std::fs::read_to_string(&coverage_file.path) {
                    let mut total_lines = 0;
                    let mut covered_lines = 0;

                    for line in content.lines() {
                        if line.starts_with("DA:") {
                            let parts: Vec<&str> = line[3..].split(',').collect();
                            if parts.len() >= 2 {
                                total_lines += 1;
                                if let Ok(hits) = parts[1].parse::<usize>() {
                                    if hits > 0 {
                                        covered_lines += 1;
                                    }
                                }
                            }
                        }
                    }

                    if total_lines > 0 {
                        let coverage_percentage =
                            (covered_lines as f64 / total_lines as f64) * 100.0;
                        debug!(
                            "Calculated coverage: {:.2}% ({}/{} lines)",
                            coverage_percentage, covered_lines, total_lines
                        );
                        return Ok(Some(coverage_percentage));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Analyze XML-based coverage files
    async fn analyze_xml_coverage(&self, coverage_path: &Path) -> Result<usize> {
        use std::fs;

        // Read and parse XML coverage file
        let xml_content = match fs::read_to_string(coverage_path) {
            Ok(content) => content,
            Err(e) => {
                warn!(
                    "Failed to read coverage file {}: {}",
                    coverage_path.display(),
                    e
                );
                return Ok(0);
            }
        };

        // Simple XML parsing to extract uncovered lines
        let mut uncovered_count = 0;

        for line in xml_content.lines() {
            // Count lines with hits="0" (uncovered lines)
            if line.trim().contains("<line number=") && line.contains("hits=\"0\"") {
                uncovered_count += 1;
            }
        }

        debug!(
            "Analyzed XML coverage file: {} uncovered lines found",
            uncovered_count
        );

        // Return a reasonable gap count - group consecutive uncovered lines into gaps
        // Assume average gap spans 2-3 lines, so divide by 2
        Ok((uncovered_count / 2).max(1))
    }

    /// Analyze LCOV coverage files
    async fn analyze_lcov_coverage(&self, coverage_path: &Path) -> Result<usize> {
        debug!("Analyzing LCOV coverage file: {:?}", coverage_path);

        // Use the CoverageExtractor to parse the LCOV file and build coverage packs
        let coverage_packs = self
            .coverage_extractor
            .build_coverage_packs(vec![coverage_path.to_path_buf()])
            .await?;

        // Count the total gaps across all packs
        let total_gaps: usize = coverage_packs.iter().map(|pack| pack.gaps.len()).sum();

        info!("Found {} coverage gaps in LCOV file", total_gaps);
        Ok(total_gaps)
    }

    /// Analyze JSON coverage files
    async fn analyze_json_coverage(&self, _coverage_path: &Path) -> Result<usize> {
        // Placeholder implementation
        // Future: Parse JSON coverage and identify gaps
        debug!("Analyzing JSON coverage file");
        Ok(0)
    }
}
