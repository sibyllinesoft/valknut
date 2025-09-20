//! Re-export analysis result structures from the core pipeline module.

pub use crate::core::pipeline::{
    AnalysisResults,
    AnalysisStatistics,
    AnalysisSummary,
    CloneAnalysisPerformance,
    CloneAnalysisResults,
    DepthHealthStats,
    DirectoryHealthScore,
    DirectoryHealthTree,
    DirectoryHotspot,
    DirectoryIssueSummary,
    FeatureContribution,
    FileRefactoringGroup,
    MemoryStats,
    PhaseFilteringStats,
    RefactoringCandidate,
    RefactoringIssue,
    RefactoringSuggestion,
    TreeStatistics,
};
