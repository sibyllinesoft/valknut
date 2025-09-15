//! AI Refactoring Oracle - Gemini 2.5 Pro integration for intelligent refactoring suggestions
//!
//! This module provides intelligent refactoring suggestions by using scribe-analyzer to bundle 
//! codebase contents and sending them to Gemini 2.5 Pro along with valknut analysis results.

use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::core::errors::{Result, ValknutError, ValknutResultExt};
use crate::api::results::AnalysisResults;
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
        let api_key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| ValknutError::config("GEMINI_API_KEY environment variable not set".to_string()))?;
        
        Ok(Self {
            api_key,
            max_tokens: 400_000,  // Default 400k tokens for codebase bundle
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
        let bundle = self.create_codebase_bundle(project_path, analysis_results).await?;
        
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
        
        // First, find README at root level
        let readme_candidates = ["README.md", "readme.md", "README.txt", "README"];
        for readme_name in &readme_candidates {
            let readme_path = project_path.join(readme_name);
            if readme_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&readme_path) {
                    let estimated_tokens = content.len() / 4; // Rough token estimate
                    if total_tokens + estimated_tokens < self.config.max_tokens {
                        xml_files.push(format!(
                            "    <file path=\"{}\" type=\"documentation\" tokens=\"{}\">\n{}\n    </file>",
                            readme_name,
                            estimated_tokens,
                            html_escape(&content)
                        ));
                        total_tokens += estimated_tokens;
                        files_included += 1;
                        println!("   ✅ Included README: {} ({} tokens)", readme_name, estimated_tokens);
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
                let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
                
                // Skip common directories and files we don't want
                !name.starts_with('.') && 
                name != "target" &&
                name != "node_modules" &&
                name != "__pycache__" &&
                name != "dist" &&
                name != "build" &&
                name != "coverage" &&
                name != "tmp" &&
                name != "temp"
            });
        
        let mut candidate_files = Vec::new();
        
        // Collect all candidate source files with metadata
        for entry in walker {
            let entry = entry.map_generic_err("walking project directory")?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    // Include main source files
                    if matches!(ext, "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "java" | "cpp" | "c" | "h" | "hpp" | "cs" | "php") {
                        let relative_path = path.strip_prefix(project_path)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string();
                        
                        // Skip test files
                        if is_test_file(&relative_path) {
                            continue;
                        }
                        
                        if let Ok(content) = std::fs::read_to_string(path) {
                            let estimated_tokens = content.len() / 4;
                            let priority = calculate_file_priority(&relative_path, ext, content.len());
                            
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
        
        println!("   📋 Found {} candidate source files", candidate_files.len());
        
        // Sort by priority (higher priority first)
        candidate_files.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
        
        // Add files until we hit token budget
        for candidate in candidate_files {
            if total_tokens + candidate.tokens > self.config.max_tokens {
                files_skipped += 1;
                if files_skipped <= 5 { // Only log first few skipped files
                    println!("   ⏭️  Skipped: {} ({} tokens) - would exceed budget", candidate.path, candidate.tokens);
                }
                continue;
            }
            
            xml_files.push(format!(
                "    <file path=\"{}\" type=\"{}\" tokens=\"{}\" priority=\"{:.2}\">\n{}\n    </file>",
                candidate.path,
                candidate.file_type,
                candidate.tokens,
                candidate.priority,
                html_escape(&candidate.content)
            ));
            
            total_tokens += candidate.tokens;
            files_included += 1;
            
            println!("   ✅ Included: {} ({} tokens, priority: {:.2})", candidate.path, candidate.tokens, candidate.priority);
        }
        
        if files_skipped > 5 {
            println!("   ⏭️  ... and {} more files skipped due to token budget", files_skipped - 5);
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
        println!("   📊 Analysis token budget: {} tokens", VALKNUT_OUTPUT_TOKEN_BUDGET);
        let condensed_analysis = self.condense_analysis_results_with_budget(analysis_results, VALKNUT_OUTPUT_TOKEN_BUDGET)?;
        
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
            "top_refactoring_candidates": results.refactoring_candidates.iter()
                .take(10)
                .map(|c| serde_json::json!({
                    "file": c.file_path,
                    "entity": c.name,
                    "score": c.score,
                    "issues": c.issues,
                    "suggestions": c.suggestions
                }))
                .collect::<Vec<_>>(),
            "directory_health": results.directory_health_tree.as_ref().map(|tree| {
                serde_json::json!({
                    "overall_health": tree.tree_statistics.avg_health_score,
                    "issues_count": tree.tree_statistics.total_directories,
                    "hotspots": tree.tree_statistics.hotspot_directories.iter().take(5).collect::<Vec<_>>()
                })
            }),
            "coverage": if !results.coverage_packs.is_empty() {
                Some(serde_json::json!({
                    "files_with_coverage": results.coverage_packs.len(),
                    "total_gaps": results.coverage_packs.iter()
                        .map(|p| p.gaps.len())
                        .sum::<usize>()
                }))
            } else { None }
        })).unwrap_or_else(|_| "Failed to serialize analysis".to_string())
    }
    
    /// Query Gemini API with the bundled content
    async fn query_gemini(&self, content: &str) -> Result<RefactoringOracleResponse> {
        let url = format!(
            "{}/{}:generateContent?key={}",
            self.config.api_endpoint,
            self.config.model,
            self.config.api_key
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
        
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_generic_err("sending request to Gemini API")?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ValknutError::internal(format!("Gemini API error: {}", error_text)));
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
        
        let oracle_response: RefactoringOracleResponse = serde_json::from_str(&response_text)
            .map_json_err("Oracle response")?;
        
        Ok(oracle_response)
    }

    /// Condense analysis results with a specific token budget
    fn condense_analysis_results_with_budget(
        &self, 
        results: &AnalysisResults, 
        token_budget: usize
    ) -> Result<String> {
        println!("   🔄 Condensing valknut analysis with {} token budget", token_budget);
        
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
                    candidate.issues.iter()
                        .map(|issue| format!("{} (severity: {:.1})", issue.category, issue.severity))
                        .collect::<Vec<_>>()
                        .join(", "),
                    candidate.suggestions.iter()
                        .take(2)  // Limit suggestions per candidate
                        .map(|s| s.refactoring_type.clone())
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

        // Add directory health information if available and within budget
        if let Some(tree) = &results.directory_health_tree {
            if current_tokens < token_budget * 3 / 4 {  // Only if we have 25% budget left
                let health_section = format!(
                    "## Directory Health Overview\n\
                    - Average Health Score: {:.2}\n\
                    - Total Directories: {}\n\
                    - Problematic Areas: {}\n\n",
                    tree.tree_statistics.avg_health_score,
                    tree.tree_statistics.total_directories,
                    tree.tree_statistics.hotspot_directories.iter()
                        .take(3)
                        .map(|h| format!("{} (health: {:.2})", h.path.display(), h.health_score))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                
                let health_tokens = health_section.len() / 4;
                if current_tokens + health_tokens <= token_budget {
                    condensed.push_str(&health_section);
                    current_tokens += health_tokens;
                }
            }
        }

        let final_tokens = condensed.len() / 4;
        println!("   ✅ Condensed analysis: {} tokens (budget: {})", final_tokens, token_budget);
        
        if final_tokens > token_budget {
            println!("   ⚠️  Warning: Exceeded token budget by {} tokens", final_tokens - token_budget);
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
    if path.ends_with("_test.rs") || 
       path.ends_with("_test.py") ||
       path.ends_with("_test.js") ||
       path.ends_with("_test.ts") ||
       path.ends_with(".test.js") ||
       path.ends_with(".test.ts") ||
       path.ends_with(".test.tsx") ||
       path.ends_with(".test.jsx") ||
       path.ends_with("_spec.js") ||
       path.ends_with("_spec.ts") ||
       path.ends_with(".spec.js") ||
       path.ends_with(".spec.ts") ||
       path.ends_with("_test.go") ||
       path.ends_with("_test.java") ||
       path.ends_with("_test.cpp") ||
       path.ends_with("_test.c") ||
       path.ends_with("Test.java") ||
       path.ends_with("Tests.java") {
        return true;
    }
    
    // Rust test module files
    if path.contains("tests.rs") && !path.ends_with("/tests.rs") {
        return true;
    }
    
    // Python test patterns
    if path.starts_with("test_") || 
       path.contains("/test_") ||
       path == "conftest.py" ||
       path.ends_with("/conftest.py") {
        return true;
    }
    
    // JavaScript/TypeScript test patterns
    if path.contains("/__tests__/") ||
       path.contains("/spec/") {
        return true;
    }
    
    // Common test directory patterns
    if path.starts_with("tests/") ||
       path.starts_with("test/") ||
       path.starts_with("spec/") {
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
        "rs" => priority += 2.0,  // Boost Rust files since this is a Rust project
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

/// HTML escape utility function
fn html_escape(content: &str) -> String {
    content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}