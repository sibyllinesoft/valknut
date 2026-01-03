//! Oracle types for configuration and responses.

use serde::{Deserialize, Serialize};

use crate::core::errors::{Result, ValknutError};

/// Configuration for the refactoring oracle
#[derive(Debug, Clone)]
pub struct OracleConfig {
    /// Gemini API key
    pub api_key: String,
    /// Maximum tokens to send to Gemini for full codebase analysis (default: 400_000)
    pub max_tokens: usize,
    /// Gemini API endpoint
    pub api_endpoint: String,
    /// Model name to use for full codebase analysis
    pub model: String,
    /// Whether to use sliced analysis for large codebases
    pub enable_slicing: bool,
    /// Token budget per slice (default: 200_000)
    pub slice_token_budget: usize,
    /// Model to use for slice analysis (default: gemini-2.0-flash)
    pub slice_model: String,
    /// Threshold for enabling slicing (if total tokens > this, use slices)
    pub slicing_threshold: usize,
}

/// Factory and builder methods for [`OracleConfig`].
impl OracleConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("GEMINI_API_KEY").map_err(|_| {
            ValknutError::config("GEMINI_API_KEY environment variable not set".to_string())
        })?;

        Ok(Self {
            api_key,
            max_tokens: 400_000, // Default 400k tokens for codebase bundle
            api_endpoint: "https://generativelanguage.googleapis.com/v1beta/models".to_string(),
            model: "gemini-3-flash-preview".to_string(),
            enable_slicing: true,
            slice_token_budget: 200_000,
            slice_model: "gemini-3-flash-preview".to_string(),
            slicing_threshold: 300_000, // Use slicing if codebase > 300k tokens
        })
    }

    /// Sets the maximum token limit for codebase analysis.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Sets the token budget for each analysis slice.
    pub fn with_slice_budget(mut self, budget: usize) -> Self {
        self.slice_token_budget = budget;
        self
    }

    /// Sets the model to use for slice analysis.
    pub fn with_slice_model(mut self, model: String) -> Self {
        self.slice_model = model;
        self
    }

    /// Enables or disables sliced analysis for large codebases.
    pub fn with_slicing(mut self, enabled: bool) -> Self {
        self.enable_slicing = enabled;
        self
    }
}

/// Response from the AI refactoring oracle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringOracleResponse {
    /// Overall assessment of the codebase
    pub assessment: CodebaseAssessment,
    /// Flat list of tasks (new schema)
    #[serde(default)]
    pub tasks: Vec<RefactoringTask>,
    /// Legacy: flat list in roadmap wrapper (for backwards compat)
    #[serde(default)]
    pub refactoring_roadmap: Option<RefactoringRoadmap>,
}

/// Accessor methods for [`RefactoringOracleResponse`].
impl RefactoringOracleResponse {
    /// Get all tasks, whether from new `tasks` field or legacy `refactoring_roadmap`
    pub fn all_tasks(&self) -> &[RefactoringTask] {
        if !self.tasks.is_empty() {
            &self.tasks
        } else if let Some(ref roadmap) = self.refactoring_roadmap {
            &roadmap.tasks
        } else {
            &[]
        }
    }
}

/// Assessment of overall codebase quality from the oracle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseAssessment {
    /// Brief summary of code quality (new schema)
    #[serde(default)]
    pub summary: Option<String>,
    /// Legacy: narrative field
    #[serde(default)]
    pub architectural_narrative: Option<String>,
    /// Legacy: architectural style
    #[serde(default)]
    pub architectural_style: Option<String>,
    /// Code strengths identified
    #[serde(default)]
    pub strengths: Vec<String>,
    /// Key issues identified
    #[serde(default)]
    pub issues: Vec<String>,
}

/// Accessor methods for [`CodebaseAssessment`].
impl CodebaseAssessment {
    /// Get summary text, preferring new field, falling back to legacy
    pub fn get_summary(&self) -> &str {
        self.summary
            .as_deref()
            .or(self.architectural_narrative.as_deref())
            .unwrap_or("No summary provided")
    }
}

/// Legacy container for refactoring tasks in execution order.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RefactoringRoadmap {
    /// Flat list of tasks in safe execution order
    #[serde(default)]
    pub tasks: Vec<RefactoringTask>,
}

/// A single refactoring task recommended by the oracle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringTask {
    /// Task ID (e.g., "T1", "T2")
    pub id: String,
    pub title: String,
    pub description: String,
    /// Category code (C1-C7) or legacy string
    pub category: String,
    pub files: Vec<String>,
    /// Risk code (R1-R3) - new field name
    #[serde(default, alias = "risk_level")]
    pub risk: Option<String>,
    /// Legacy risk_level field
    #[serde(default)]
    pub risk_level: Option<String>,
    /// Impact code (I1-I3)
    #[serde(default)]
    pub impact: Option<String>,
    /// Effort code (E1-E3)
    #[serde(default)]
    pub effort: Option<String>,
    /// Mitigation strategy for this task's risks
    #[serde(default)]
    pub mitigation: Option<String>,
    /// Whether this task is required (legacy, optional now)
    #[serde(default)]
    pub required: Option<bool>,
    /// Dependencies on other task IDs that must be completed first
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Expected benefits from this change (legacy, optional now)
    #[serde(default)]
    pub benefits: Vec<String>,
}

/// Accessor methods for [`RefactoringTask`].
impl RefactoringTask {
    /// Get risk level, checking both new and legacy field names
    pub fn get_risk(&self) -> Option<&str> {
        self.risk.as_deref().or(self.risk_level.as_deref())
    }
}
