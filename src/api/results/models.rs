//! Re-export analysis result structures from the core pipeline module.

pub use crate::core::pipeline::{
    AnalysisResults, AnalysisStatistics, AnalysisSummary, CloneAnalysisPerformance,
    CloneAnalysisResults, FeatureContribution, FileRefactoringGroup,
    PhaseFilteringStats, RefactoringCandidate, RefactoringIssue, RefactoringSuggestion,
    StageResultsBundle,
};
// Use the 3-field MemoryStats from result_types (matches AnalysisStatistics.memory_stats)
pub use crate::core::pipeline::results::result_types::MemoryStats;
