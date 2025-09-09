//! Individual analysis stages for the pipeline.

use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use crate::core::errors::Result;
use crate::detectors::complexity::ComplexityAnalyzer;
use crate::detectors::structure::StructureExtractor;
use crate::detectors::refactoring::RefactoringAnalyzer;
use super::pipeline_results::{
    StructureAnalysisResults, ComplexityAnalysisResults, 
    RefactoringAnalysisResults, ImpactAnalysisResults
};

/// Handles all individual analysis stages
pub struct AnalysisStages {
    pub structure_extractor: StructureExtractor,
    pub complexity_analyzer: ComplexityAnalyzer,
    pub refactoring_analyzer: RefactoringAnalyzer,
}

impl AnalysisStages {
    /// Create new analysis stages with the given analyzers
    pub fn new(
        structure_extractor: StructureExtractor,
        complexity_analyzer: ComplexityAnalyzer,
        refactoring_analyzer: RefactoringAnalyzer,
    ) -> Self {
        Self {
            structure_extractor,
            complexity_analyzer,
            refactoring_analyzer,
        }
    }

    /// Run structure analysis
    pub async fn run_structure_analysis(&self, paths: &[PathBuf]) -> Result<StructureAnalysisResults> {
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
    pub async fn run_complexity_analysis(&self, files: &[PathBuf]) -> Result<ComplexityAnalysisResults> {
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
    pub async fn run_refactoring_analysis(&self, files: &[PathBuf]) -> Result<RefactoringAnalysisResults> {
        debug!("Running refactoring analysis on {} files", files.len());
        
        let detailed_results = self.refactoring_analyzer.analyze_files(files).await?;
        let opportunities_count = detailed_results.iter().map(|r| r.recommendations.len()).sum();

        Ok(RefactoringAnalysisResults {
            enabled: true,
            detailed_results,
            opportunities_count,
        })
    }

    /// Run impact analysis (placeholder for now)
    pub async fn run_impact_analysis(&self, _files: &[PathBuf]) -> Result<ImpactAnalysisResults> {
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
}