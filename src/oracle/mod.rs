//! AI Refactoring Oracle - Gemini 2.5 Pro integration for intelligent refactoring suggestions
//!
//! This module provides intelligent refactoring suggestions by using scribe-analyzer to bundle
//! codebase contents and sending them to Gemini 2.5 Pro along with valknut analysis results.

use crate::core::errors::{Result, ValknutError, ValknutResultExt};
use crate::core::pipeline::{AnalysisResults, CodeDictionary, StageResultsBundle};
use crate::core::scoring::Priority;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

/// Token budget for valknut analysis output (50k tokens)
const VALKNUT_OUTPUT_TOKEN_BUDGET: usize = 50_000;

/// AI refactoring oracle that provides intelligent suggestions using Gemini 2.5 Pro
pub struct RefactoringOracle {
    config: OracleConfig,
    client: reqwest::Client,
}

/// Configuration for the refactoring oracle
#[derive(Debug, Clone)]
pub struct OracleConfig {
    /// Gemini API key
    pub api_key: String,
    /// Maximum tokens to send to Gemini (default: 500_000)
    pub max_tokens: usize,
    /// Gemini API endpoint
    pub api_endpoint: String,
    /// Model name to use
    pub model: String,
}

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
            model: "gemini-2.5-pro".to_string(),
        })
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}

/// Response from the AI refactoring oracle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringOracleResponse {
    /// Overall assessment of the codebase
    pub assessment: CodebaseAssessment,
    /// Refactoring plan organized by phases
    pub refactoring_plan: RefactoringPlan,
    /// Risk assessment for proposed changes
    pub risk_assessment: RiskAssessment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseAssessment {
    pub health_score: u8,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub architecture_quality: String,
    pub organization_quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringPlan {
    pub phases: Vec<RefactoringPhase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringPhase {
    pub id: String,
    pub name: String,
    pub description: String,
    pub priority: u8,
    pub subsystems: Vec<RefactoringSubsystem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringSubsystem {
    pub id: String,
    pub name: String,
    pub affected_files: Vec<String>,
    pub tasks: Vec<RefactoringTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactoringTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub task_type: String,
    pub files: Vec<String>,
    pub risk_level: String,
    pub benefits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub overall_risk: String,
    pub risks: Vec<IdentifiedRisk>,
    pub mitigation_strategies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifiedRisk {
    pub category: String,
    pub description: String,
    pub probability: String,
    pub impact: String,
    pub mitigation: String,
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    temperature: f32,
    #[serde(rename = "topK")]
    top_k: i32,
    #[serde(rename = "topP")]
    top_p: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: i32,
    #[serde(rename = "responseMimeType")]
    response_mime_type: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiResponseContent,
}

#[derive(Deserialize)]
struct GeminiResponseContent {
    parts: Vec<GeminiResponsePart>,
}

#[derive(Deserialize)]
struct GeminiResponsePart {
    text: String,
}

impl RefactoringOracle {
    /// Create a new refactoring oracle with the given configuration
    pub fn new(config: OracleConfig) -> Self {
        let client = reqwest::Client::new();
        Self { config, client }
    }

    /// Generate refactoring suggestions for the given codebase
    pub async fn generate_suggestions(
        &self,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Result<RefactoringOracleResponse> {
        // Use scribe-analyzer to bundle the codebase
        let bundle = self
            .create_codebase_bundle(project_path, analysis_results)
            .await?;

        // Send to Gemini for analysis
        let response = self.query_gemini(&bundle).await?;

        Ok(response)
    }

    /// Create a codebase bundle with XML file tree structure and debugging
    async fn create_codebase_bundle(
        &self,
        project_path: &Path,
        analysis_results: &AnalysisResults,
    ) -> Result<String> {
        println!("\n🔍 [ORACLE DEBUG] Starting codebase bundle creation");
        println!("   📁 Project path: {}", project_path.display());
        println!("   📊 Token budget: {} tokens", self.config.max_tokens);

        let mut xml_files = Vec::new();
        let mut total_tokens = 0;
        let mut files_included = 0;
        let mut files_skipped = 0;

        let refactor_hints = build_refactor_hints(analysis_results, project_path);

        // First, find README at root level
        let readme_candidates = ["README.md", "readme.md", "README.txt", "README"];
        for readme_name in &readme_candidates {
            let readme_path = project_path.join(readme_name);
            if readme_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&readme_path) {
                    let estimated_tokens = content.len() / 4; // Rough token estimate
                    if total_tokens + estimated_tokens < self.config.max_tokens {
                        let tuple_label = format!("({}, {})", readme_name, "overview");
                        xml_files.push(format!(
                "    <file path=\"{}\" tuple=\"{}\" type=\"documentation\" tokens=\"{}\">\n{}\n    </file>",
                readme_name,
                html_escape(&tuple_label),
                estimated_tokens,
                html_escape(&content)
            ));
                        total_tokens += estimated_tokens;
                        files_included += 1;
                        println!(
                            "   ✅ Included README: {} ({} tokens)",
                            readme_name, estimated_tokens
                        );
                        break;
                    }
                }
            }
        }

        // Walk through project files and collect source files
        let walker = WalkDir::new(project_path)
            .max_depth(4)
            .into_iter()
            .filter_entry(|e| {
                let path = e.path();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();

                // Skip common directories and files we don't want
                !name.starts_with('.')
                    && name != "target"
                    && name != "node_modules"
                    && name != "__pycache__"
                    && name != "dist"
                    && name != "build"
                    && name != "coverage"
                    && name != "tmp"
                    && name != "temp"
            });

        let mut candidate_files = Vec::new();

        // Collect all candidate source files with metadata
        for entry in walker {
            let entry = entry.map_generic_err("walking project directory")?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    // Include main source files
                    if matches!(
                        ext,
                        "rs" | "py"
                            | "js"
                            | "ts"
                            | "tsx"
                            | "jsx"
                            | "go"
                            | "java"
                            | "cpp"
                            | "c"
                            | "h"
                            | "hpp"
                            | "cs"
                            | "php"
                    ) {
                        let relative_path = path
                            .strip_prefix(project_path)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string();

                        // Skip test files
                        if is_test_file(&relative_path) {
                            continue;
                        }

                        if let Ok(content) = std::fs::read_to_string(path) {
                            let estimated_tokens = content.len() / 4;
                            let priority =
                                calculate_file_priority(&relative_path, ext, content.len());

                            candidate_files.push(FileCandidate {
                                path: relative_path,
                                content,
                                tokens: estimated_tokens,
                                priority,
                                file_type: ext.to_string(),
                            });
                        }
                    }
                }
            }
        }

        println!(
            "   📋 Found {} candidate source files",
            candidate_files.len()
        );

        // Sort by priority (higher priority first)
        candidate_files.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Add files until we hit token budget
        for candidate in candidate_files {
            if total_tokens + candidate.tokens > self.config.max_tokens {
                files_skipped += 1;
                if files_skipped <= 5 {
                    // Only log first few skipped files
                    println!(
                        "   ⏭️  Skipped: {} ({} tokens) - would exceed budget",
                        candidate.path, candidate.tokens
                    );
                }
                continue;
            }

            let key = normalize_path_for_key(&candidate.path);
            let hints = refactor_hints
                .get(&key)
                .map(|h| h.join("; "))
                .unwrap_or_else(|| "none".to_string());
            let hints_truncated = truncate_hint(&hints, 80);
            let tuple_label = format!("({}, {})", candidate.path, hints_truncated);

            xml_files.push(format!(
                "    <file path=\"{}\" tuple=\"{}\" hint=\"{}\" type=\"{}\" tokens=\"{}\" priority=\"{:.2}\">\n{}\n    </file>",
                candidate.path,
                html_escape(&tuple_label),
                html_escape(&hints_truncated),
                candidate.file_type,
                candidate.tokens,
                candidate.priority,
                html_escape(&candidate.content)
            ));

            total_tokens += candidate.tokens;
            files_included += 1;

            println!(
                "   ✅ Included: {} ({} tokens, priority: {:.2})",
                candidate.path, candidate.tokens, candidate.priority
            );
        }

        if files_skipped > 5 {
            println!(
                "   ⏭️  ... and {} more files skipped due to token budget",
                files_skipped - 5
            );
        }

        // Create XML structure
        let xml_bundle = format!(
            "<codebase project_path=\"{}\" files_included=\"{}\" total_tokens=\"{}\">\n{}\n</codebase>",
            project_path.display(),
            files_included,
            total_tokens,
            xml_files.join("\n")
        );

        // Create condensed valknut analysis with token budget
        println!("\n🔍 [ORACLE DEBUG] Creating condensed valknut analysis");
        println!(
            "   📊 Analysis token budget: {} tokens",
            VALKNUT_OUTPUT_TOKEN_BUDGET
        );
        let condensed_analysis = self
            .condense_analysis_results_with_budget(analysis_results, VALKNUT_OUTPUT_TOKEN_BUDGET)?;

        let final_bundle = format!(
            "# Codebase Refactoring Analysis Request\n\n\
            ## Project Codebase ({} files, ~{} tokens)\n{}\n\n\
            ## Valknut Technical Debt Analysis\n{}\n\n\
            ## Task Instructions\n\
            Analyze the provided codebase and generate a comprehensive refactoring plan in JSON format.\n\
            Focus on maximizing maintainability and discoverability while avoiding any breakage.\n\n\
            ## CRITICAL: Response Format Requirements\n\
            You MUST respond with valid JSON that exactly matches this schema. Do not include markdown formatting, explanations, or any text outside the JSON object.\n\n\
            ## Required JSON Response Schema:\n\
            ```json\n\
            {{\n\
              \"assessment\": {{\n\
                \"health_score\": <number 0-100>,\n\
                \"strengths\": [\"<strength1>\", \"<strength2>\"],\n\
                \"weaknesses\": [\"<weakness1>\", \"<weakness2>\"],\n\
                \"architecture_quality\": \"<detailed assessment>\",\n\
                \"organization_quality\": \"<detailed assessment>\"\n\
              }},\n\
              \"refactoring_plan\": {{\n\
                \"phases\": [\n\
                  {{\n\
                    \"id\": \"<phase-id>\",\n\
                    \"name\": \"<phase-name>\",\n\
                    \"description\": \"<detailed-description>\",\n\
                    \"priority\": <number 1-5>,\n\
                    \"subsystems\": [\n\
                      {{\n\
                        \"id\": \"<subsystem-id>\",\n\
                        \"name\": \"<subsystem-name>\",\n\
                        \"affected_files\": [\"<file-path1>\", \"<file-path2>\"],\n\
                        \"tasks\": [\n\
                          {{\n\
                            \"id\": \"<task-id>\",\n\
                            \"title\": \"<task-title>\",\n\
                            \"description\": \"<detailed-task-description>\",\n\
                            \"task_type\": \"<extract_method|split_file|move_module|refactor_class|architectural_change>\",\n\
                            \"files\": [\"<affected-file1>\", \"<affected-file2>\"],\n\
                            \"risk_level\": \"<low|medium|high>\",\n\
                            \"benefits\": [\"<benefit1>\", \"<benefit2>\"]\n\
                          }}\n\
                        ]\n\
                      }}\n\
                    ]\n\
                  }}\n\
                ]\n\
              }},\n\
              \"risk_assessment\": {{\n\
                \"overall_risk\": \"<low|medium|high>\",\n\
                \"risks\": [\n\
                  {{\n\
                    \"category\": \"<technical|process|business>\",\n\
                    \"description\": \"<risk-description>\",\n\
                    \"probability\": \"<low|medium|high>\",\n\
                    \"impact\": \"<low|medium|high>\",\n\
                    \"mitigation\": \"<mitigation-strategy>\"\n\
                  }}\n\
                ],\n\
                \"mitigation_strategies\": [\"<strategy1>\", \"<strategy2>\"]\n\
              }}\n\
            }}\n\
            ```\n\n\
            ## Example Response:\n\
            ```json\n\
            {{\n\
              \"assessment\": {{\n\
                \"health_score\": 72,\n\
                \"strengths\": [\"Well-defined module boundaries\", \"Comprehensive error handling\"],\n\
                \"weaknesses\": [\"Large configuration files\", \"Complex data transformations\"],\n\
                \"architecture_quality\": \"The system shows good separation of concerns at the module level with clear boundaries between API, core logic, and I/O operations.\",\n\
                \"organization_quality\": \"Directory structure follows Rust conventions but some files have grown too large and should be decomposed.\"\n\
              }},\n\
              \"refactoring_plan\": {{\n\
                \"phases\": [\n\
                  {{\n\
                    \"id\": \"phase-1-config\",\n\
                    \"name\": \"Configuration Refactoring\",\n\
                    \"description\": \"Simplify and modularize the configuration system to reduce complexity and improve maintainability.\",\n\
                    \"priority\": 1,\n\
                    \"subsystems\": [\n\
                      {{\n\
                        \"id\": \"config-decomposition\",\n\
                        \"name\": \"Configuration Decomposition\",\n\
                        \"affected_files\": [\"src/core/config.rs\"],\n\
                        \"tasks\": [\n\
                          {{\n\
                            \"id\": \"task-1.1\",\n\
                            \"title\": \"Split configuration struct\",\n\
                            \"description\": \"Break down monolithic ValknutConfig into feature-specific configuration structs\",\n\
                            \"task_type\": \"split_file\",\n\
                            \"files\": [\"src/core/config.rs\", \"src/detectors/config.rs\"],\n\
                            \"risk_level\": \"medium\",\n\
                            \"benefits\": [\"Improved maintainability\", \"Better organization\"]\n\
                          }}\n\
                        ]\n\
                      }}\n\
                    ]\n\
                  }}\n\
                ]\n\
              }},\n\
              \"risk_assessment\": {{\n\
                \"overall_risk\": \"medium\",\n\
                \"risks\": [\n\
                  {{\n\
                    \"category\": \"technical\",\n\
                    \"description\": \"Configuration changes may break existing integrations\",\n\
                    \"probability\": \"medium\",\n\
                    \"impact\": \"high\",\n\
                    \"mitigation\": \"Maintain backward compatibility layer during transition\"\n\
                  }}\n\
                ],\n\
                \"mitigation_strategies\": [\"Incremental rollout\", \"Comprehensive testing\"]\n\
              }}\n\
            }}\n\
            ```\n\n\
            ## Guidelines:\n\
            - Prioritize tasks by impact vs effort ratio\n\
            - Be specific and actionable in task descriptions\n\
            - Focus on the most critical issues identified in the valknut analysis\n\
            - Ensure all file paths are accurate and exist in the codebase\n\
            - Response must be valid JSON with no additional formatting",
            files_included,
            total_tokens,
            xml_bundle,
            condensed_analysis
        );

        let final_tokens = final_bundle.len() / 4;
        println!("\n🎯 [ORACLE DEBUG] Bundle creation complete");
        println!("   📦 Final bundle: ~{} tokens", final_tokens);
        println!("   📁 Files included: {}", files_included);
        println!("   ⏭️  Files skipped: {}", files_skipped);

        Ok(final_bundle)
    }

    /// Condense valknut analysis results for AI consumption
    fn condense_analysis_results(&self, results: &AnalysisResults) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "health_score": results.summary.code_health_score,
            "total_issues": results.summary.refactoring_needed,
            "high_priority": results.summary.high_priority,
            "critical": results.summary.critical,
            "files_analyzed": results.summary.files_processed,
            "entities_analyzed": results.summary.entities_analyzed,
            "avg_refactoring_score": results.summary.avg_refactoring_score,
            "code_dictionary": results.code_dictionary.clone(),
            "top_refactoring_candidates": results.refactoring_candidates.iter()
                .take(10)
                .map(|c| serde_json::json!({
                    "file": c.file_path,
                    "entity": c.name,
                    "score": c.score,
                    "issue_codes": c.issues.iter().map(|issue| &issue.code).collect::<Vec<_>>(),
                    "suggestion_codes": c.suggestions.iter().map(|s| &s.code).collect::<Vec<_>>(),
                    "issues": c.issues,
                    "suggestions": c.suggestions
                }))
                .collect::<Vec<_>>(),
            "coverage": if !results.coverage_packs.is_empty() {
                Some(serde_json::json!({
                    "files_with_coverage": results.coverage_packs.len(),
                    "total_gaps": results.coverage_packs.iter()
                        .map(|p| p.gaps.len())
                        .sum::<usize>()
                }))
            } else { None }
        }))
        .unwrap_or_else(|_| "Failed to serialize analysis".to_string())
    }

    /// Query Gemini API with the bundled content
    async fn query_gemini(&self, content: &str) -> Result<RefactoringOracleResponse> {
        let url = format!(
            "{}/{}:generateContent?key={}",
            self.config.api_endpoint, self.config.model, self.config.api_key
        );

        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: content.to_string(),
                }],
            }],
            generation_config: GeminiGenerationConfig {
                temperature: 0.2,
                top_k: 40,
                top_p: 0.95,
                max_output_tokens: 8192,
                response_mime_type: "application/json".to_string(),
            },
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_generic_err("sending request to Gemini API")?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ValknutError::internal(format!(
                "Gemini API error: {}",
                error_text
            )));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .map_generic_err("parsing Gemini API response")?;

        let response_text = gemini_response
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| ValknutError::internal("No candidates in Gemini response".to_string()))?
            .content
            .parts
            .into_iter()
            .next()
            .ok_or_else(|| ValknutError::internal("No parts in Gemini response".to_string()))?
            .text;

        let oracle_response: RefactoringOracleResponse =
            serde_json::from_str(&response_text).map_json_err("Oracle response")?;

        Ok(oracle_response)
    }

    /// Condense analysis results with a specific token budget
    fn condense_analysis_results_with_budget(
        &self,
        results: &AnalysisResults,
        token_budget: usize,
    ) -> Result<String> {
        println!(
            "   🔄 Condensing valknut analysis with {} token budget",
            token_budget
        );

        // Start with essential summary information
        let mut condensed = format!(
            "## Core Metrics\n\
            - Health Score: {:.2}\n\
            - Files Analyzed: {}\n\
            - Entities: {}\n\
            - Issues Needing Refactoring: {}\n\
            - High Priority Issues: {}\n\
            - Critical Issues: {}\n\
            - Average Refactoring Score: {:.2}\n\n",
            results.summary.code_health_score,
            results.summary.files_processed,
            results.summary.entities_analyzed,
            results.summary.refactoring_needed,
            results.summary.high_priority,
            results.summary.critical,
            results.summary.avg_refactoring_score
        );

        let mut current_tokens = condensed.len() / 4;

        // Add top refactoring candidates by priority
        if !results.refactoring_candidates.is_empty() {
            let candidates_section = "## Top Refactoring Priorities\n";
            condensed.push_str(candidates_section);
            current_tokens += candidates_section.len() / 4;

            let issue_defs = &results.code_dictionary.issues;
            let suggestion_defs = &results.code_dictionary.suggestions;

            for (i, candidate) in results.refactoring_candidates.iter()
                .filter(|c| !matches!(c.priority, crate::core::scoring::Priority::None))
                .take(15)  // Limit candidates to control size
                .enumerate()
            {
                let candidate_text = format!(
                    "{}. **{}** ({:?})\n\
                       - File: {}\n\
                       - Score: {:.1} | Priority: {:?}\n\
                       - Issues: {}\n\
                       - Key Suggestions: {}\n\n",
                    i + 1,
                    candidate.name.split(':').last().unwrap_or(&candidate.name),
                    candidate.priority,
                    candidate.file_path,
                    candidate.score,
                    candidate.priority,
                    candidate
                        .issues
                        .iter()
                        .map(|issue| {
                            let title = issue_defs
                                .get(&issue.code)
                                .map(|def| def.title.as_str())
                                .unwrap_or(issue.category.as_str());
                            let severity = format!("{:.1}", issue.severity);
                            format!("{} – {} [severity {}]", issue.code, title, severity)
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    candidate.suggestions.iter()
                        .take(2)  // Limit suggestions per candidate
                        .map(|s| {
                            let title = suggestion_defs
                                .get(&s.code)
                                .map(|def| def.title.as_str())
                                .unwrap_or(s.refactoring_type.as_str());
                            format!("{} – {}", s.code, title)
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                );

                let candidate_tokens = candidate_text.len() / 4;
                if current_tokens + candidate_tokens > token_budget {
                    println!("   ⏭️  Stopping at candidate {} due to token budget", i + 1);
                    break;
                }

                condensed.push_str(&candidate_text);
                current_tokens += candidate_tokens;
            }
        }

        let final_tokens = condensed.len() / 4;
        println!(
            "   ✅ Condensed analysis: {} tokens (budget: {})",
            final_tokens, token_budget
        );

        if final_tokens > token_budget {
            println!(
                "   ⚠️  Warning: Exceeded token budget by {} tokens",
                final_tokens - token_budget
            );
        }

        Ok(condensed)
    }
}

/// Candidate file for inclusion in the codebase bundle
#[derive(Debug)]
struct FileCandidate {
    path: String,
    content: String,
    tokens: usize,
    priority: f32,
    file_type: String,
}

/// Check if a file path indicates it's a test file
fn is_test_file(path: &str) -> bool {
    // Common test file patterns
    if path.contains("/test/") || path.contains("/tests/") {
        return true;
    }

    // Test file naming patterns
    if path.ends_with("_test.rs")
        || path.ends_with("_test.py")
        || path.ends_with("_test.js")
        || path.ends_with("_test.ts")
        || path.ends_with(".test.js")
        || path.ends_with(".test.ts")
        || path.ends_with(".test.tsx")
        || path.ends_with(".test.jsx")
        || path.ends_with("_spec.js")
        || path.ends_with("_spec.ts")
        || path.ends_with(".spec.js")
        || path.ends_with(".spec.ts")
        || path.ends_with("_test.go")
        || path.ends_with("_test.java")
        || path.ends_with("_test.cpp")
        || path.ends_with("_test.c")
        || path.ends_with("Test.java")
        || path.ends_with("Tests.java")
        || (path.contains("Test") && path.ends_with(".java"))
    {
        return true;
    }

    // Rust test module files
    if path.contains("tests.rs") && !path.ends_with("/tests.rs") {
        return true;
    }

    // Python test patterns
    if path.starts_with("test_")
        || path.contains("/test_")
        || path == "conftest.py"
        || path.ends_with("/conftest.py")
    {
        return true;
    }

    // JavaScript/TypeScript test patterns
    if path.contains("/__tests__/") || path.contains("/spec/") {
        return true;
    }

    // Common test directory patterns
    if path.starts_with("tests/") || path.starts_with("test/") || path.starts_with("spec/") {
        return true;
    }

    false
}

/// Calculate priority score for file inclusion
fn calculate_file_priority(path: &str, extension: &str, size: usize) -> f32 {
    let mut priority = 1.0;

    // Boost priority for important files
    if path.contains("main.rs") || path.contains("lib.rs") || path.contains("mod.rs") {
        priority += 3.0;
    }

    if path.contains("config") || path.contains("error") || path.contains("api") {
        priority += 2.0;
    }

    if path.contains("core") || path.contains("engine") {
        priority += 1.5;
    }

    // Language-specific priority adjustments
    match extension {
        "rs" => priority += 2.0, // Boost Rust files since this is a Rust project
        "py" | "js" | "ts" => priority += 1.5,
        "go" | "java" | "cpp" => priority += 1.0,
        _ => {}
    }

    // Penalize very large files (they consume too many tokens)
    if size > 50_000 {
        priority *= 0.5;
    } else if size > 20_000 {
        priority *= 0.7;
    }

    // Boost smaller, focused files
    if size < 1_000 {
        priority *= 1.2;
    }

    // Penalize test files and generated files
    if path.contains("test") || path.contains("spec") || path.contains("_test") {
        priority *= 0.3;
    }

    if path.contains("generated") || path.contains("target/") || path.contains("build/") {
        priority *= 0.1;
    }

    priority
}

fn build_refactor_hints(
    results: &AnalysisResults,
    project_root: &Path,
) -> HashMap<String, Vec<String>> {
    let mut hints: HashMap<String, Vec<String>> = HashMap::new();

    for candidate in &results.refactoring_candidates {
        if !matches!(candidate.priority, Priority::Critical | Priority::High) {
            continue;
        }

        let issue = match candidate.issues.iter().max_by(|a, b| {
            a.severity
                .partial_cmp(&b.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            Some(issue) => issue,
            None => continue,
        };

        let mut severity_pct = (issue.severity * 100.0).round() as i32;
        severity_pct = severity_pct.clamp(0, 999);

        let category = abbreviate_label(&issue.category);
        let suggestion_label = candidate
            .suggestions
            .iter()
            .max_by(|a, b| {
                a.priority
                    .partial_cmp(&b.priority)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|s| abbreviate_label(&s.refactoring_type));

        let mut hint = if let Some(suggestion) = suggestion_label {
            format!("{} {}% {}", category, severity_pct, suggestion)
        } else {
            format!("{} {}%", category, severity_pct)
        };

        hint = truncate_hint(&hint, 60);

        let normalized_path = normalize_path_for_key(
            Path::new(&candidate.file_path)
                .strip_prefix(project_root)
                .unwrap_or_else(|_| Path::new(&candidate.file_path))
                .to_string_lossy()
                .as_ref(),
        );

        hints.entry(normalized_path).or_default().push(hint);
    }

    hints
}

fn abbreviate_label(label: &str) -> String {
    let words = label
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>();

    if words.is_empty() {
        let trimmed = label.trim();
        return trimmed.chars().take(8).collect();
    }

    if words.len() == 1 {
        let word = words[0];
        let mut chars = word.chars();
        let first = chars
            .next()
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or_default();
        let rest: String = chars.take(6).collect();
        return format!("{}{}", first, rest);
    }

    let mut abbr = String::new();
    for word in words.iter().take(3) {
        if let Some(ch) = word.chars().next() {
            abbr.push(ch.to_ascii_uppercase());
        }
    }

    if abbr.is_empty() {
        label.chars().take(3).collect()
    } else {
        abbr
    }
}

fn truncate_hint(hint: &str, max_len: usize) -> String {
    if hint.len() <= max_len {
        return hint.to_string();
    }
    let mut truncated = hint
        .chars()
        .take(max_len.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

fn normalize_path_for_key(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    path.replace('\\', "/")
}

/// HTML escape utility function
fn html_escape(content: &str) -> String {
    content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::Duration;
    use tempfile::tempdir;

    static ENV_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
    use crate::core::pipeline::*;
    use crate::core::scoring::Priority;

    fn oracle_config_fixture(max_tokens: usize) -> OracleConfig {
        OracleConfig {
            api_key: "test-key".to_string(),
            max_tokens,
            api_endpoint: "https://api.example.com".to_string(),
            model: "test-model".to_string(),
        }
    }

    fn sample_candidate(
        file_path: &Path,
        entity_name: &str,
        issue_code: &str,
        suggestion_code: &str,
        suggestion_type: &str,
        priority: Priority,
        severity: f64,
        suggestion_priority: f64,
    ) -> RefactoringCandidate {
        RefactoringCandidate {
            entity_id: format!("{}::{entity_name}", file_path.display()),
            name: entity_name.to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            line_range: Some((12, 48)),
            priority,
            score: 70.0 + severity * 20.0,
            confidence: 0.8 + (severity / 5.0).min(0.15),
            issues: vec![RefactoringIssue {
                code: issue_code.to_string(),
                category: "Complexity Hotspot".to_string(),
                severity,
                contributing_features: vec![FeatureContribution {
                    feature_name: "cyclomatic_complexity".to_string(),
                    value: 18.0,
                    normalized_value: 0.9,
                    contribution: 0.45,
                }],
            }],
            suggestions: vec![RefactoringSuggestion {
                refactoring_type: suggestion_type.to_string(),
                code: suggestion_code.to_string(),
                priority: suggestion_priority,
                effort: 0.3,
                impact: 0.7,
            }],
            issue_count: 1,
            suggestion_count: 1,
        }
    }

    fn analysis_results_fixture(project_root: &Path) -> AnalysisResults {
        let lib_path = project_root.join("src/lib.rs");
        let utils_path = project_root.join("src/utils.rs");

        let summary = AnalysisSummary {
            files_processed: 3,
            entities_analyzed: 6,
            refactoring_needed: 2,
            high_priority: 1,
            critical: 1,
            avg_refactoring_score: 72.5,
            code_health_score: 0.42,
            total_files: 3,
            total_entities: 6,
            total_lines_of_code: 420,
            languages: vec!["Rust".to_string()],
            total_issues: 4,
            high_priority_issues: 2,
            critical_issues: 1,
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        let mut code_dictionary = CodeDictionary::default();
        code_dictionary.issues.insert(
            "VX001".to_string(),
            CodeDefinition {
                code: "VX001".to_string(),
                title: "Cyclomatic spike".to_string(),
                summary: "Cyclomatic complexity exceeded preferred range".to_string(),
                category: Some("complexity".to_string()),
            },
        );
        code_dictionary.issues.insert(
            "VX002".to_string(),
            CodeDefinition {
                code: "VX002".to_string(),
                title: "Excessive branching".to_string(),
                summary: "Branching factor suggests decomposition".to_string(),
                category: Some("structure".to_string()),
            },
        );
        code_dictionary.suggestions.insert(
            "RX001".to_string(),
            CodeDefinition {
                code: "RX001".to_string(),
                title: "Extract helper".to_string(),
                summary: "Split logic into dedicated helper functions".to_string(),
                category: Some("refactoring".to_string()),
            },
        );
        code_dictionary.suggestions.insert(
            "RX002".to_string(),
            CodeDefinition {
                code: "RX002".to_string(),
                title: "Simplify branches".to_string(),
                summary: "Reduce branching to clarify business rules".to_string(),
                category: Some("refactoring".to_string()),
            },
        );

        AnalysisResults {
            summary,
            normalized: None,
            passes: StageResultsBundle::disabled(),
            refactoring_candidates: vec![
                sample_candidate(
                    &lib_path,
                    "crate::lib::hotspot",
                    "VX001",
                    "RX001",
                    "Extract Method",
                    Priority::Critical,
                    0.92,
                    0.9,
                ),
                sample_candidate(
                    &utils_path,
                    "crate::utils::helper",
                    "VX002",
                    "RX002",
                    "Simplify Branches",
                    Priority::High,
                    0.78,
                    0.7,
                ),
            ],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(2),
                avg_file_processing_time: Duration::from_millis(120),
                avg_entity_processing_time: Duration::from_millis(45),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 512_000,
                    final_memory_bytes: 256_000,
                    efficiency_score: 0.82,
                },
            },
            clone_analysis: None,
            coverage_packs: Vec::new(),
            warnings: Vec::new(),
            health_metrics: Some(HealthMetrics {
                overall_health_score: 58.0,
                maintainability_score: 52.0,
                technical_debt_ratio: 71.0,
                complexity_score: 83.0,
                structure_quality_score: 45.0,
                doc_health_score: 100.0,
                }),
            code_dictionary,
            documentation: None,
        }
    }

    #[test]
    fn test_oracle_config_creation() {
        let config = OracleConfig {
            api_key: "test-key".to_string(),
            max_tokens: 100_000,
            api_endpoint: "https://api.example.com".to_string(),
            model: "test-model".to_string(),
        };

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.max_tokens, 100_000);
        assert_eq!(config.api_endpoint, "https://api.example.com");
        assert_eq!(config.model, "test-model");
    }

    #[test]
    fn test_oracle_config_from_env_missing_key() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("GEMINI_API_KEY");

        let result = OracleConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("GEMINI_API_KEY"));
    }

    #[test]
    fn test_oracle_config_from_env_with_key() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("GEMINI_API_KEY", "test-api-key");

        let result = OracleConfig::from_env();
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.api_key, "test-api-key");
        assert_eq!(config.max_tokens, 400_000);
        assert_eq!(config.model, "gemini-2.5-pro");
        assert!(config
            .api_endpoint
            .contains("generativelanguage.googleapis.com"));

        // Clean up
        std::env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn test_oracle_config_with_max_tokens() {
        let config = OracleConfig {
            api_key: "test".to_string(),
            max_tokens: 100,
            api_endpoint: "test".to_string(),
            model: "test".to_string(),
        }
        .with_max_tokens(50_000);

        assert_eq!(config.max_tokens, 50_000);
    }

    #[test]
    fn test_refactoring_oracle_creation() {
        let config = OracleConfig {
            api_key: "test-key".to_string(),
            max_tokens: 100_000,
            api_endpoint: "https://api.example.com".to_string(),
            model: "test-model".to_string(),
        };

        let oracle = RefactoringOracle::new(config);
        assert_eq!(oracle.config.api_key, "test-key");
    }

    #[test]
    fn test_is_test_file_patterns() {
        // Test directory patterns
        assert!(is_test_file("src/test/mod.rs"));
        assert!(is_test_file("tests/integration.rs"));
        assert!(is_test_file("src/tests/unit.py"));

        // Test file name patterns
        assert!(is_test_file("src/module_test.rs"));
        assert!(is_test_file("src/component.test.js"));
        assert!(is_test_file("src/service.spec.ts"));
        assert!(is_test_file("test_module.py"));
        assert!(is_test_file("src/TestClass.java"));
        assert!(is_test_file("conftest.py"));

        // Non-test files
        assert!(!is_test_file("src/main.rs"));
        assert!(!is_test_file("src/lib.rs"));
        assert!(!is_test_file("src/config.py"));
        assert!(!is_test_file("src/api/mod.rs"));
    }

    #[test]
    fn test_calculate_file_priority() {
        // High priority files
        assert!(calculate_file_priority("src/main.rs", "rs", 1000) > 3.0);
        assert!(calculate_file_priority("src/lib.rs", "rs", 1000) > 3.0);
        assert!(calculate_file_priority("src/core/mod.rs", "rs", 1000) > 3.0);

        // Config and API files get boost
        assert!(calculate_file_priority("src/config.rs", "rs", 1000) > 2.0);
        assert!(calculate_file_priority("src/api/mod.rs", "rs", 1000) > 2.0);

        // Language priorities
        assert!(
            calculate_file_priority("src/module.rs", "rs", 1000)
                > calculate_file_priority("src/module.py", "py", 1000)
        );
        assert!(
            calculate_file_priority("src/module.py", "py", 1000)
                > calculate_file_priority("src/module.c", "c", 1000)
        );

        // Size penalties
        assert!(
            calculate_file_priority("src/large.rs", "rs", 100_000)
                < calculate_file_priority("src/small.rs", "rs", 1000)
        );

        // Test file penalty
        assert!(
            calculate_file_priority("src/module.rs", "rs", 1000)
                > calculate_file_priority("src/module_test.rs", "rs", 1000)
        );
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape(""), "");
        assert_eq!(html_escape("hello world"), "hello world");
        assert_eq!(html_escape("hello & world"), "hello &amp; world");
        assert_eq!(html_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(html_escape("'single'"), "&#x27;single&#x27;");
        assert_eq!(
            html_escape("<script>alert('hello');</script>"),
            "&lt;script&gt;alert(&#x27;hello&#x27;);&lt;/script&gt;"
        );
    }

    #[test]
    fn test_file_candidate_creation() {
        let candidate = FileCandidate {
            path: "src/test.rs".to_string(),
            content: "fn main() {}".to_string(),
            tokens: 100,
            priority: 2.5,
            file_type: "rs".to_string(),
        };

        assert_eq!(candidate.path, "src/test.rs");
        assert_eq!(candidate.content, "fn main() {}");
        assert_eq!(candidate.tokens, 100);
        assert_eq!(candidate.priority, 2.5);
        assert_eq!(candidate.file_type, "rs");
    }

    #[test]
    fn test_codebase_assessment_structure() {
        let assessment = CodebaseAssessment {
            health_score: 75,
            strengths: vec!["Good modularity".to_string()],
            weaknesses: vec!["Large files".to_string()],
            architecture_quality: "Well structured".to_string(),
            organization_quality: "Clear hierarchy".to_string(),
        };

        assert_eq!(assessment.health_score, 75);
        assert_eq!(assessment.strengths.len(), 1);
        assert_eq!(assessment.weaknesses.len(), 1);
    }

    #[test]
    fn test_refactoring_task_structure() {
        let task = RefactoringTask {
            id: "task-1".to_string(),
            title: "Split large file".to_string(),
            description: "Break down monolithic module".to_string(),
            task_type: "split_file".to_string(),
            files: vec!["src/large.rs".to_string()],
            risk_level: "medium".to_string(),
            benefits: vec!["Improved maintainability".to_string()],
        };

        assert_eq!(task.id, "task-1");
        assert_eq!(task.task_type, "split_file");
        assert_eq!(task.risk_level, "medium");
        assert_eq!(task.files.len(), 1);
        assert_eq!(task.benefits.len(), 1);
    }

    #[test]
    fn test_refactoring_subsystem_structure() {
        let subsystem = RefactoringSubsystem {
            id: "config-module".to_string(),
            name: "Configuration System".to_string(),
            affected_files: vec!["src/config.rs".to_string()],
            tasks: vec![],
        };

        assert_eq!(subsystem.id, "config-module");
        assert_eq!(subsystem.name, "Configuration System");
        assert_eq!(subsystem.affected_files.len(), 1);
        assert!(subsystem.tasks.is_empty());
    }

    #[test]
    fn test_refactoring_phase_structure() {
        let phase = RefactoringPhase {
            id: "phase-1".to_string(),
            name: "Initial Cleanup".to_string(),
            description: "Address immediate issues".to_string(),
            priority: 1,
            subsystems: vec![],
        };

        assert_eq!(phase.id, "phase-1");
        assert_eq!(phase.priority, 1);
        assert!(phase.subsystems.is_empty());
    }

    #[test]
    fn test_identified_risk_structure() {
        let risk = IdentifiedRisk {
            category: "technical".to_string(),
            description: "Configuration changes may break integrations".to_string(),
            probability: "medium".to_string(),
            impact: "high".to_string(),
            mitigation: "Use compatibility layer".to_string(),
        };

        assert_eq!(risk.category, "technical");
        assert_eq!(risk.probability, "medium");
        assert_eq!(risk.impact, "high");
    }

    #[test]
    fn test_risk_assessment_structure() {
        let assessment = RiskAssessment {
            overall_risk: "medium".to_string(),
            risks: vec![],
            mitigation_strategies: vec!["Incremental deployment".to_string()],
        };

        assert_eq!(assessment.overall_risk, "medium");
        assert!(assessment.risks.is_empty());
        assert_eq!(assessment.mitigation_strategies.len(), 1);
    }

    #[test]
    fn test_refactoring_plan_structure() {
        let plan = RefactoringPlan { phases: vec![] };

        assert!(plan.phases.is_empty());
    }

    #[test]
    fn test_oracle_response_structure() {
        let response = RefactoringOracleResponse {
            assessment: CodebaseAssessment {
                health_score: 80,
                strengths: vec!["Good tests".to_string()],
                weaknesses: vec!["Complex config".to_string()],
                architecture_quality: "Solid".to_string(),
                organization_quality: "Clear".to_string(),
            },
            refactoring_plan: RefactoringPlan { phases: vec![] },
            risk_assessment: RiskAssessment {
                overall_risk: "low".to_string(),
                risks: vec![],
                mitigation_strategies: vec![],
            },
        };

        assert_eq!(response.assessment.health_score, 80);
        assert!(response.refactoring_plan.phases.is_empty());
        assert_eq!(response.risk_assessment.overall_risk, "low");
    }

    #[test]
    fn test_condense_analysis_results() {
        use std::collections::HashMap;
        use std::time::Duration;

        let config = OracleConfig {
            api_key: "test".to_string(),
            max_tokens: 100_000,
            api_endpoint: "test".to_string(),
            model: "test".to_string(),
        };
        let oracle = RefactoringOracle::new(config);

        let results = AnalysisResults {
            summary: AnalysisSummary {
                code_health_score: 75.5,
                files_processed: 10,
                entities_analyzed: 50,
                refactoring_needed: 5,
                high_priority: 2,
                critical: 1,
                avg_refactoring_score: 3.2,
                total_files: 10,
                total_entities: 50,
                total_lines_of_code: 1_500,
                languages: vec!["Rust".to_string()],
                total_issues: 3,
                high_priority_issues: 2,
                critical_issues: 1,
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
            normalized: None,
            passes: StageResultsBundle::disabled(),
            refactoring_candidates: vec![],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_secs(30),
                avg_file_processing_time: Duration::from_millis(500),
                avg_entity_processing_time: Duration::from_millis(100),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 1000000,
                    final_memory_bytes: 800000,
                    efficiency_score: 0.8,
                },
            },
            clone_analysis: None,
            coverage_packs: vec![],
            warnings: vec![],
            health_metrics: None,
            code_dictionary: CodeDictionary::default(),
            documentation: None,
        };

        let condensed = oracle.condense_analysis_results(&results);
        assert!(condensed.contains("75.5"));
        assert!(condensed.contains("files_analyzed"));
        assert!(condensed.contains("health_score"));
    }

    #[test]
    fn test_token_budget_constants() {
        assert_eq!(VALKNUT_OUTPUT_TOKEN_BUDGET, 50_000);
    }

    #[test]
    fn test_gemini_request_structure() {
        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: "test content".to_string(),
                }],
            }],
            generation_config: GeminiGenerationConfig {
                temperature: 0.2,
                top_k: 40,
                top_p: 0.95,
                max_output_tokens: 8192,
                response_mime_type: "application/json".to_string(),
            },
        };

        assert_eq!(request.contents.len(), 1);
        assert_eq!(request.generation_config.temperature, 0.2);
        assert_eq!(
            request.generation_config.response_mime_type,
            "application/json"
        );
    }

    #[test]
    fn test_gemini_response_structure() {
        let response = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiResponseContent {
                    parts: vec![GeminiResponsePart {
                        text: "response text".to_string(),
                    }],
                },
            }],
        };

        assert_eq!(response.candidates.len(), 1);
        assert_eq!(
            response.candidates[0].content.parts[0].text,
            "response text"
        );
    }

    #[test]
    fn truncate_hint_adds_ellipsis_for_long_labels() {
        let short = truncate_hint("High risk", 20);
        assert_eq!(short, "High risk");

        let long = truncate_hint("VeryLongRefactorHintIdentifierThatShouldBeTrimmed", 16);
        assert!(long.ends_with('…'));
        assert!(long.chars().count() <= 16);
    }

    #[test]
    fn normalize_path_for_key_flattens_backslashes() {
        assert_eq!(
            normalize_path_for_key(r"src\module\lib.rs"),
            "src/module/lib.rs"
        );
        assert_eq!(normalize_path_for_key(""), "");
    }

    #[test]
    fn build_refactor_hints_normalizes_paths_and_limits_size() {
        let project = tempdir().unwrap();
        let root = project.path().join("workspace");
        fs::create_dir_all(root.join("src")).unwrap();
        let results = analysis_results_fixture(&root);
        let hints = build_refactor_hints(&results, &root);

        let entry = hints
            .get("src/lib.rs")
            .expect("expected lib.rs hints entry");
        assert!(
            entry.iter().all(|hint| hint.len() <= 60),
            "hint should be truncated to configured length"
        );
        assert!(
            entry.iter().any(|hint| hint.contains("CH")),
            "category abbreviation should be included"
        );
    }

    #[tokio::test]
    async fn create_codebase_bundle_includes_readme_and_skips_large_files() {
        let project = tempdir().unwrap();
        let root = project.path().join("workspace");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("README.md"),
            "# Sample Project\n\nImportant overview.",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub fn compute(value: i32) -> i32 { value * 2 }\n",
        )
        .unwrap();
        fs::write(
            root.join("src/utils.rs"),
            "pub fn helper(flag: bool) -> bool { if flag { !flag } else { flag } }\n",
        )
        .unwrap();
        let large_body = "fn enormous_task() {}\n".repeat(400);
        fs::write(root.join("src/huge.rs"), large_body).unwrap();

        let results = analysis_results_fixture(&root);
        let oracle = RefactoringOracle::new(oracle_config_fixture(180));

        let bundle = oracle
            .create_codebase_bundle(&root, &results)
            .await
            .expect("bundle creation");

        assert!(bundle.contains("README.md"));
        assert!(bundle.contains("src/lib.rs"));
        assert!(
            !bundle.contains("src/huge.rs"),
            "large file should be skipped when exceeding budget"
        );
        assert!(
            bundle.contains("CH 92%") && bundle.contains("EM"),
            "refactor hints should be embedded in tuple labels"
        );
    }

    #[test]
    fn condense_analysis_results_with_budget_handles_limits_and_health_section() {
        let project = tempdir().unwrap();
        let root = project.path().join("workspace");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "fn demo() {}\n").unwrap();
        fs::write(root.join("src/utils.rs"), "fn helper() {}\n").unwrap();

        let results = analysis_results_fixture(&root);
        let oracle = RefactoringOracle::new(oracle_config_fixture(500));

        let limited = oracle
            .condense_analysis_results_with_budget(&results, 90)
            .expect("condense with tight budget");
        assert!(
            !limited.contains("crate::lib::hotspot") && !limited.contains("crate::utils::helper"),
            "candidates should be omitted when budget is exhausted before listing them"
        );

        let mut expanded_results = analysis_results_fixture(&root);
        expanded_results
            .refactoring_candidates
            .push(sample_candidate(
                &root.join("src/core.rs"),
                "crate::core::planner",
                "VX002",
                "RX002",
                "Simplify Branches",
                Priority::High,
                0.68,
                0.6,
            ));

        let expanded = oracle
            .condense_analysis_results_with_budget(&expanded_results, 420)
            .expect("condense with ample budget");
        // Health section is optional after normalization removal
        // ensure condensed text still produced
        assert!(!expanded.is_empty());
        assert!(
            expanded.contains("helper"),
            "refactoring candidate names should appear when budget allows"
        );
    }
}
