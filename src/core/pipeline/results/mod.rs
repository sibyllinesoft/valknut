//! Pipeline result types and conversions.
//!
//! This module contains all result-related types for the analysis pipeline:
//! - Result type definitions
//! - Result conversions between formats
//! - Pipeline results aggregation
//! - Normalized types for scoring

pub mod normalized_types;
pub mod pipeline_results;
pub mod result_builder;
pub mod result_conversions;
pub mod result_types;

#[cfg(test)]
mod result_conversions_tests;
#[cfg(test)]
mod result_types_tests;

// Explicit re-exports to avoid name collisions
pub use normalized_types::*;
pub use pipeline_results::{
    CloneVerificationResults, ComplexityAnalysisResults, ComprehensiveAnalysisResult,
    CoverageAnalysisResults, CoverageFileInfo, DocumentationAnalysisResults, FileScore,
    HealthMetrics, ImpactAnalysisResults, LshAnalysisResults, MemoryStats, PipelineResults,
    PipelineStatistics, PipelineStatus, RefactoringAnalysisResults, ResultSummary, ScoringResults,
    StructureAnalysisResults, TfIdfStats,
};
pub use result_builder::*;
pub use result_conversions::*;
// Re-export result_types but exclude MemoryStats to avoid conflict with pipeline_results::MemoryStats
pub use result_types::{
    AnalysisResults, AnalysisStatistics, AnalysisSummary, CloneAnalysisPerformance,
    CloneAnalysisResults, CodeDefinition, CodeDictionary, DepthHealthStats,
    DirectoryHealthScore, DirectoryHealthTree, DirectoryHotspot, DirectoryIssueSummary,
    DocumentationResults, FeatureContribution, FileRefactoringGroup, PhaseFilteringStats,
    RefactoringCandidate, RefactoringIssue, RefactoringSuggestion, TreeStatistics,
};
