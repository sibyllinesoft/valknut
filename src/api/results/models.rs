//! Re-export analysis result structures from the core pipeline module.

pub use crate::core::pipeline::{
    AnalysisResults, AnalysisStatistics, AnalysisSummary, CloneAnalysisPerformance,
    CloneAnalysisResults, FeatureContribution, FileRefactoringGroup, MemoryStats,
    PhaseFilteringStats, RefactoringCandidate, RefactoringIssue, RefactoringSuggestion,
    StageResultsBundle,
};
