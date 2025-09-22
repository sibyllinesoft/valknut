//! Individual analysis stages for the pipeline.

use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use futures::future;

use super::pipeline_results::{
    ComplexityAnalysisResults, CoverageAnalysisResults, CoverageFileInfo, ImpactAnalysisResults,
    LshAnalysisResults, RefactoringAnalysisResults, StructureAnalysisResults,
};
use crate::core::arena_analysis::{ArenaFileAnalyzer, ArenaBatchAnalyzer};
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
    pub arena_analyzer: ArenaFileAnalyzer,
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
            arena_analyzer: ArenaFileAnalyzer::with_ast_service(ast_service.clone()),
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
            arena_analyzer: ArenaFileAnalyzer::with_ast_service(ast_service.clone()),
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

    /// Run complexity analysis from pre-extracted arena results (optimized path)
    pub async fn run_complexity_analysis_from_arena_results(
        &self,
        arena_results: &[crate::core::arena_analysis::ArenaAnalysisResult],
    ) -> Result<ComplexityAnalysisResults> {
        debug!("Running complexity analysis from {} arena results", arena_results.len());
    
        // Use the configured analyzer instance and run analyses in parallel.
        let analysis_futures = arena_results.iter().map(|arena_result| {
            let analyzer = self.ast_complexity_analyzer.clone(); // Clone Arc'd analyzer
            let file_path_str = arena_result.file_path_str().to_string();
            // Source code is not in ArenaAnalysisResult, so we must read it.
            // A better optimization would be to pass the source map down.
            let file_path = PathBuf::from(&file_path_str);
    
            tokio::spawn(async move {
                match tokio::fs::read_to_string(&file_path).await {
                    Ok(source) => analyzer.analyze_file_with_results(&file_path_str, &source).await,
                    Err(e) => {
                        warn!("Could not read file for complexity analysis {}: {}", file_path.display(), e);
                        Ok(Vec::new())
                    }
                }
            })
        });

        let results_of_results = future::join_all(analysis_futures).await;
    
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
        let (
            total_cyclomatic,
            total_cognitive,
            total_debt,
            total_maintainability,
        ) = if count > 0.0 {
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
            (total_cyclomatic, total_cognitive, total_debt, total_maintainability)
        } else {
            (0.0, 0.0, 0.0, 100.0)
        };
    
        let issues_count = detailed_results.iter().map(|r| r.issues.len()).sum();
    
        debug!(
            "Complexity analysis completed: {} entities, avg cyclomatic: {:.2}, avg cognitive: {:.2}",
            detailed_results.len(),
            if count > 0.0 { total_cyclomatic / count } else { 0.0 },
            if count > 0.0 { total_cognitive / count } else { 0.0 }
        );
    
        Ok(ComplexityAnalysisResults {
            enabled: true,
            detailed_results,
            average_cyclomatic_complexity: if count > 0.0 { total_cyclomatic / count } else { 0.0 },
            average_cognitive_complexity: if count > 0.0 { total_cognitive / count } else { 0.0 },
            average_technical_debt_score: if count > 0.0 { total_debt / count } else { 0.0 },
            average_maintainability_index: if count > 0.0 { total_maintainability / count } else { 100.0 },
            issues_count,
        })
    }

    /// Run complexity analysis (legacy path - re-parses files)
    pub async fn run_complexity_analysis(
        &self,
        files: &[PathBuf],
    ) -> Result<ComplexityAnalysisResults> {
        debug!("Running complexity analysis on {} files", files.len());

        // Parallelize file analysis using tokio::spawn
        let analysis_futures = files.iter().map(|file_path| {
            let analyzer = self.ast_complexity_analyzer.clone();
            let path = file_path.clone();
            
            tokio::spawn(async move {
                let file_refs = vec![path.as_path()];
                analyzer.analyze_files(&file_refs).await
            })
        });

        // Wait for all concurrent analyses to complete
        let results_of_results = future::join_all(analysis_futures).await;

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
            // Clone the Arc'd analyzer, which is cheap
            let analyzer = self.refactoring_analyzer.clone();
            let path = file_path.clone();
            
            tokio::spawn(async move {
                analyzer.analyze_files(&[path]).await
            })
        });

        // Wait for all concurrent analyses to complete
        let results_of_results = future::join_all(analysis_futures).await;

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

            // Extract real entities from files using language adapters
            let mut entities = Vec::new();
            let mut entity_index = HashMap::new();

            for file_path in files.iter() {
                if let Ok(content) = tokio::fs::read_to_string(file_path).await {
                    // Extract entities using appropriate language adapter
                    if let Some(extracted_entities) = self.extract_entities_from_file(file_path, &content).await {
                        for entity in extracted_entities {
                            entity_index.insert(entity.id.clone(), entity.clone());
                            entities.push(entity);
                        }
                    }
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

    /// Extract entities from a file using appropriate language adapter
    async fn extract_entities_from_file(&self, file_path: &Path, content: &str) -> Option<Vec<crate::core::featureset::CodeEntity>> {
        use crate::lang::registry::adapter_for_file;
        
        // Get appropriate language adapter
        let mut adapter = match adapter_for_file(file_path) {
            Ok(adapter) => adapter,
            Err(e) => {
                debug!("No language adapter for {}: {}", file_path.display(), e);
                return None;
            }
        };

        // Extract entities using the standardized interface
        match adapter.extract_code_entities(content, &file_path.to_string_lossy()) {
            Ok(entities) => {
                debug!("Extracted {} entities from {}", entities.len(), file_path.display());
                Some(entities)
            }
            Err(e) => {
                warn!("Failed to extract entities from {}: {}", file_path.display(), e);
                None
            }
        }
    }

    /// Run arena-based file analysis for optimal memory performance
    /// 
    /// This method demonstrates arena allocation benefits by processing files
    /// with minimal memory allocation overhead using bump-pointer allocation.
    pub async fn run_arena_file_analysis(&self, files: &[PathBuf]) -> Result<Vec<crate::core::arena_analysis::ArenaAnalysisResult>> {
        debug!("Running arena-based file analysis on {} files", files.len());
        
        use tokio::fs;
        
        // Prepare file paths and sources for batch arena analysis
        let mut file_sources = Vec::with_capacity(files.len());
        
        for file_path in files {
            match fs::read_to_string(file_path).await {
                Ok(source) => {
                    file_sources.push((file_path.as_path(), source));
                }
                Err(e) => {
                    warn!("Failed to read file {}: {}", file_path.display(), e);
                    continue;
                }
            }
        }
        
        if file_sources.is_empty() {
            info!("No files could be read for arena analysis");
            return Ok(Vec::new());
        }
        
        // Use ArenaBatchAnalyzer for optimal memory usage
        let batch_analyzer = ArenaBatchAnalyzer::new();
        
        // Convert to the format expected by batch analyzer
        let file_refs: Vec<(&std::path::Path, &str)> = file_sources
            .iter()
            .map(|(path, source)| (*path, source.as_str()))
            .collect();
            
        let batch_result = batch_analyzer.analyze_batch(file_refs).await?;
        
        info!(
            "Arena batch analysis completed: {} files, {} entities, {:.2} KB arena usage, {:.1} entities/sec",
            batch_result.total_files,
            batch_result.total_entities,
            batch_result.total_arena_kb(),
            batch_result.entities_per_second()
        );
        
        info!(
            "Estimated malloc savings: {:.2} KB overhead reduction vs traditional allocation",
            batch_result.estimated_malloc_savings()
        );
        
        Ok(batch_result.file_results)
    }

    /// Run arena-based file analysis with pre-loaded file contents (performance optimized)
    pub async fn run_arena_file_analysis_with_content(&self, file_contents: &[(PathBuf, String)]) -> Result<Vec<crate::core::arena_analysis::ArenaAnalysisResult>> {
        debug!("Running arena-based file analysis on {} pre-loaded files", file_contents.len());
        
        if file_contents.is_empty() {
            info!("No files provided for arena analysis");
            return Ok(Vec::new());
        }
        
        // Use ArenaBatchAnalyzer for optimal memory usage
        let batch_analyzer = ArenaBatchAnalyzer::new();
        
        // Convert to the format expected by batch analyzer
        let file_refs: Vec<(&std::path::Path, &str)> = file_contents
            .iter()
            .map(|(path, content)| (path.as_path(), content.as_str()))
            .collect();
        
        let batch_result = batch_analyzer.analyze_batch(file_refs).await?;
        
        info!(
            "Arena analysis completed: {} files, {} entities, {:.2} KB arena memory, {:.1} entities/sec",
            batch_result.total_files,
            batch_result.total_entities,
            batch_result.total_arena_kb(),
            batch_result.entities_per_second()
        );
        
        info!(
            "Estimated malloc savings: {:.2} KB overhead reduction vs traditional allocation",
            batch_result.estimated_malloc_savings()
        );
        
        Ok(batch_result.file_results)
    }
}