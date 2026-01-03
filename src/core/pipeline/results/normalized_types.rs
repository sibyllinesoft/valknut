//! Normalized analysis result types for legacy report compatibility.
//!
//! These types provide a backwards-compatible representation of analysis results
//! for report generators and downstream consumers that expect a specific format.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::scoring::Priority;
use super::result_types::CodeDictionary;

/// Simplified normalized issue used for report compatibility
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormalizedIssue {
    pub code: String,
    pub category: String,
    pub severity: f64,
}

/// Simplified normalized suggestion used for report compatibility
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormalizedSuggestion {
    #[serde(rename = "type")]
    pub refactoring_type: String,
    pub code: String,
    pub priority: f64,
    pub effort: f64,
    pub impact: f64,
}

/// Normalized entity representation for legacy report consumers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEntity {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub line_range: Option<(usize, usize)>,
    pub score: f64,
    #[serde(default = "default_priority_low")]
    pub priority: Priority,
    #[serde(default)]
    pub metrics: Option<serde_json::Value>,
    pub issues: Vec<NormalizedIssue>,
    pub suggestions: Vec<NormalizedSuggestion>,
    #[serde(default)]
    pub issue_count: usize,
    #[serde(default)]
    pub suggestion_count: usize,
}

/// Returns the default priority (Low) for deserialization.
fn default_priority_low() -> Priority {
    Priority::Low
}

/// Default implementation for [`NormalizedEntity`].
impl Default for NormalizedEntity {
    /// Returns a default empty normalized entity.
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            file_path: None,
            file: None,
            kind: None,
            line_range: None,
            score: 0.0,
            priority: Priority::Low,
            metrics: None,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 0,
            suggestion_count: 0,
        }
    }
}

/// Normalized meta summary used for legacy report structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedMeta {
    pub files_scanned: usize,
    pub entities_analyzed: usize,
    pub code_health: f64,
    pub languages: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub issues: NormalizedIssues,
}

/// Normalized issue counts
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormalizedIssues {
    pub total: usize,
    pub high: usize,
    pub critical: usize,
}

/// Backwards-compatible alias for normalized issue totals
pub type NormalizedIssueTotals = NormalizedIssues;

/// Backwards-compatible alias for normalized meta summary
pub type NormalizedSummary = NormalizedMeta;

/// Conversion from `(String, f64)` tuple to [`NormalizedIssue`].
impl From<(String, f64)> for NormalizedIssue {
    /// Creates a normalized issue from a code/severity tuple.
    fn from(value: (String, f64)) -> Self {
        NormalizedIssue {
            code: value.0,
            category: String::new(),
            severity: value.1,
        }
    }
}

/// Conversion from `(&str, f64)` tuple to [`NormalizedIssue`].
impl From<(&str, f64)> for NormalizedIssue {
    /// Creates a normalized issue from a code/severity tuple.
    fn from(value: (&str, f64)) -> Self {
        NormalizedIssue {
            code: value.0.to_string(),
            category: String::new(),
            severity: value.1,
        }
    }
}

/// Normalized analysis results used by report generator compatibility path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedAnalysisResults {
    pub meta: NormalizedMeta,
    pub entities: Vec<NormalizedEntity>,
    #[serde(default)]
    pub clone: Option<serde_json::Value>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub dictionary: CodeDictionary,
}

/// Default implementation for [`NormalizedMeta`].
impl Default for NormalizedMeta {
    /// Returns default meta summary with current timestamp.
    fn default() -> Self {
        Self {
            files_scanned: 0,
            entities_analyzed: 0,
            code_health: 1.0,
            languages: Vec::new(),
            timestamp: Utc::now(),
            issues: NormalizedIssues::default(),
        }
    }
}

/// Default implementation for [`NormalizedAnalysisResults`].
impl Default for NormalizedAnalysisResults {
    /// Returns empty analysis results with default meta.
    fn default() -> Self {
        Self {
            meta: NormalizedMeta::default(),
            entities: Vec::new(),
            clone: None,
            warnings: Vec::new(),
            dictionary: CodeDictionary::default(),
        }
    }
}
