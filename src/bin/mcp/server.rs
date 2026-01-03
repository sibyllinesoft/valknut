//! MCP JSON-RPC 2.0 server implementation for stdio communication.

use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncBufReader};
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::mcp::protocol::{
    create_analyze_code_schema, create_analyze_file_quality_schema,
    create_refactoring_suggestions_schema, create_validate_quality_gates_schema, error_codes,
    ContentItem, JsonRpcRequest, JsonRpcResponse, McpCapabilities, McpInitResult, McpServerInfo,
    McpTool, ToolCallParams, ToolResult,
};
use crate::mcp::tools::{
    execute_analyze_code, execute_analyze_file_quality, execute_refactoring_suggestions,
    execute_validate_quality_gates, AnalyzeCodeParams, AnalyzeFileQualityParams,
    RefactoringSuggestionsParams, ValidateQualityGatesParams,
};
use valknut_rs::api::results::AnalysisResults;

/// Session-level analysis cache for avoiding redundant work
#[derive(Debug, Clone)]
struct AnalysisCache {
    path: PathBuf,
    results: Arc<AnalysisResults>,
    timestamp: std::time::Instant,
}

/// MCP server that handles JSON-RPC 2.0 communication over stdin/stdout
pub struct McpServer {
    /// Server name and version information
    server_info: McpServerInfo,
    /// Session-level cache to avoid re-running analysis for recently analyzed paths
    analysis_cache: Arc<Mutex<HashMap<PathBuf, AnalysisCache>>>,
}

/// Factory, caching, and request handling methods for [`McpServer`].
impl McpServer {
    /// Create a new MCP server instance
    pub fn new(version: &str) -> Self {
        Self {
            server_info: McpServerInfo {
                name: "valknut".to_string(),
                version: version.to_string(),
            },
            analysis_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get cached analysis results if available and still valid (within 5 minutes)
    async fn get_cached_analysis(&self, path: &PathBuf) -> Option<Arc<AnalysisResults>> {
        let cache = self.analysis_cache.lock().await;
        if let Some(cached) = cache.get(path) {
            // Check if cache is still valid (5 minutes)
            if cached.timestamp.elapsed().as_secs() < 300 {
                info!("Using cached analysis results for: {}", path.display());
                return Some(cached.results.clone());
            } else {
                info!("Cache expired for: {}", path.display());
            }
        }
        None
    }

    /// Cache analysis results for a path
    async fn cache_analysis(&self, path: PathBuf, results: AnalysisResults) {
        let mut cache = self.analysis_cache.lock().await;

        // Limit cache size to prevent memory growth
        if cache.len() >= 10 {
            // Remove oldest entry
            if let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, entry)| entry.timestamp)
                .map(|(key, _)| key.clone())
            {
                cache.remove(&oldest_key);
                info!("Evicted oldest cache entry: {}", oldest_key.display());
            }
        }

        cache.insert(
            path.clone(),
            AnalysisCache {
                path: path.clone(),
                results: Arc::new(results),
                timestamp: std::time::Instant::now(),
            },
        );
        info!("Cached analysis results for: {}", path.display());
    }

    /// Execute analyze_code with session-level caching
    async fn execute_analyze_code_cached(
        &self,
        params: AnalyzeCodeParams,
    ) -> Result<ToolResult, (i32, String)> {
        info!(
            "Executing analyze_code tool with caching for path: {}",
            params.path
        );

        // Validate path exists
        let path = std::path::Path::new(&params.path);
        if !path.exists() {
            return Err((
                error_codes::INVALID_PARAMS,
                format!("Path does not exist: {}", params.path),
            ));
        }

        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Check cache first
        if let Some(cached_results) = self.get_cached_analysis(&canonical_path).await {
            // Format cached results according to requested format
            let formatted_output =
                match self.format_analysis_results(&cached_results, &params.format) {
                    Ok(output) => output,
                    Err(e) => {
                        error!("Failed to format cached results: {}", e);
                        return Err((
                            error_codes::INTERNAL_ERROR,
                            format!("Failed to format cached results: {}", e),
                        ));
                    }
                };

            return Ok(ToolResult {
                content: vec![ContentItem {
                    content_type: "text".to_string(),
                    text: formatted_output,
                }],
            });
        }

        // Cache miss - run fresh analysis
        let analysis_config = valknut_rs::api::config_types::AnalysisConfig::default()
            .with_confidence_threshold(0.75)
            .with_max_files(5000)
            .with_languages(vec![
                "python".to_string(),
                "typescript".to_string(),
                "javascript".to_string(),
                "rust".to_string(),
            ]);

        let mut engine = match valknut_rs::api::engine::ValknutEngine::new(analysis_config).await {
            Ok(engine) => engine,
            Err(e) => {
                error!("Failed to create analysis engine: {}", e);
                return Err((
                    error_codes::ANALYSIS_ERROR,
                    format!("Failed to create analysis engine: {}", e),
                ));
            }
        };

        let results = match engine.analyze_directory(path).await {
            Ok(results) => results,
            Err(e) => {
                error!("Analysis failed: {}", e);
                return Err((
                    error_codes::ANALYSIS_ERROR,
                    format!("Analysis failed: {}", e),
                ));
            }
        };

        // Cache the results and format the output
        let formatted_output = match self.format_analysis_results(&results, &params.format) {
            Ok(output) => output,
            Err(e) => {
                error!("Failed to format results: {}", e);
                return Err((
                    error_codes::INTERNAL_ERROR,
                    format!("Failed to format results: {}", e),
                ));
            }
        };

        // Cache the results after successful formatting
        self.cache_analysis(canonical_path, results).await;

        Ok(ToolResult {
            content: vec![ContentItem {
                content_type: "text".to_string(),
                text: formatted_output,
            }],
        })
    }

    /// Format analysis results according to requested format
    fn format_analysis_results(
        &self,
        results: &AnalysisResults,
        format: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        match format {
            "json" => {
                // Direct JSON serialization for JSON format
                serde_json::to_string_pretty(results).map_err(|e| e.into())
            }
            "html" => {
                // Use the report generator for HTML output
                let generator = valknut_rs::io::reports::ReportGenerator::new();
                let report_format = valknut_rs::core::config::ReportFormat::Html;
                // Create a temporary directory path for the report generation
                let temp_path = std::env::temp_dir().join("valknut_mcp_report");
                match generator.generate_report(results, &temp_path, report_format) {
                    Ok(_) => {
                        // Read the generated file and return its contents
                        let report_file = temp_path.with_extension("html");
                        std::fs::read_to_string(report_file).map_err(|e| e.into())
                    }
                    Err(e) => Err(e.into()),
                }
            }
            _ => {
                // Default to JSON for unsupported formats
                serde_json::to_string_pretty(results).map_err(|e| e.into())
            }
        }
    }

    /// Run the MCP server, processing JSON-RPC messages over stdin/stdout
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting MCP JSON-RPC 2.0 server");

        let stdin = tokio::io::stdin();
        let mut reader = AsyncBufReader::new(stdin);
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();

        loop {
            line.clear();

            let response = match reader.read_line(&mut line).await {
                Ok(0) => {
                    debug!("EOF reached, shutting down MCP server");
                    break;
                }
                Ok(_) => self.handle_request(&line).await,
                Err(e) => {
                    error!("Error reading from stdin: {}", e);
                    JsonRpcResponse::error(
                        None,
                        error_codes::INTERNAL_ERROR,
                        format!("Failed to read request: {}", e),
                    )
                }
            };

            Self::write_response(&mut stdout, &response).await?;
        }

        info!("MCP server shutdown complete");
        Ok(())
    }

    /// Writes a JSON-RPC response to stdout.
    async fn write_response(
        stdout: &mut tokio::io::Stdout,
        response: &JsonRpcResponse,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let response_json = serde_json::to_string(response)?;
        stdout.write_all(response_json.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
        Ok(())
    }

    /// Handle a single JSON-RPC request
    async fn handle_request(&self, request_line: &str) -> JsonRpcResponse {
        let request_line = request_line.trim();
        if request_line.is_empty() {
            return JsonRpcResponse::error(
                None,
                error_codes::INVALID_REQUEST,
                "Empty request".to_string(),
            );
        }

        // Parse JSON-RPC request
        let request: JsonRpcRequest = match serde_json::from_str(request_line) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse JSON-RPC request: {}", e);
                return JsonRpcResponse::error(
                    None,
                    error_codes::PARSE_ERROR,
                    format!("Invalid JSON: {}", e),
                );
            }
        };

        debug!("Handling method: {}", request.method);

        // Validate JSON-RPC version
        if request.jsonrpc != "2.0" {
            return JsonRpcResponse::error(
                request.id,
                error_codes::INVALID_REQUEST,
                "Only JSON-RPC 2.0 is supported".to_string(),
            );
        }

        // Route method to appropriate handler
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tool_call(request.id, request.params).await,
            _ => JsonRpcResponse::error(
                request.id,
                error_codes::METHOD_NOT_FOUND,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    /// Handle MCP initialization
    fn handle_initialize(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        let result = McpInitResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: McpCapabilities {
                tools: self.available_tools(),
            },
            server_info: self.server_info.clone(),
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    /// Handle tools list request
    fn handle_tools_list(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        let result = serde_json::json!({
            "tools": self.available_tools()
        });

        JsonRpcResponse::success(id, result)
    }

    /// Returns the list of available MCP tools.
    fn available_tools(&self) -> Vec<McpTool> {
        vec![
            McpTool {
                name: "analyze_code".to_string(),
                description: "Analyze code for refactoring opportunities and quality metrics"
                    .to_string(),
                input_schema: create_analyze_code_schema(),
            },
            McpTool {
                name: "get_refactoring_suggestions".to_string(),
                description: "Get specific refactoring suggestions for a code entity".to_string(),
                input_schema: create_refactoring_suggestions_schema(),
            },
            McpTool {
                name: "validate_quality_gates".to_string(),
                description: "Validate code against quality gate thresholds for CI/CD integration"
                    .to_string(),
                input_schema: create_validate_quality_gates_schema(),
            },
            McpTool {
                name: "analyze_file_quality".to_string(),
                description: "Analyze quality metrics and issues for a specific file".to_string(),
                input_schema: create_analyze_file_quality_schema(),
            },
        ]
    }

    /// Handle tool call request
    async fn handle_tool_call(
        &self,
        id: Option<serde_json::Value>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let tool_params = match Self::parse_tool_params(params) {
            Ok(p) => p,
            Err(response) => return response.with_id(id),
        };

        let tool_result = self.dispatch_tool(&tool_params.name, tool_params.arguments).await;

        match tool_result {
            Ok(result) => JsonRpcResponse::success(id, serde_json::to_value(result).unwrap()),
            Err((code, message)) => JsonRpcResponse::error(id, code, message),
        }
    }

    /// Parse tool call parameters from JSON.
    fn parse_tool_params(params: Option<serde_json::Value>) -> Result<ToolCallParams, JsonRpcResponse> {
        let params = params.ok_or_else(|| {
            JsonRpcResponse::error(None, error_codes::INVALID_PARAMS, "Missing parameters".to_string())
        })?;

        serde_json::from_value(params).map_err(|e| {
            JsonRpcResponse::error(
                None,
                error_codes::INVALID_PARAMS,
                format!("Invalid tool call parameters: {}", e),
            )
        })
    }

    /// Dispatch to the appropriate tool handler.
    async fn dispatch_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolResult, (i32, String)> {
        match name {
            "analyze_code" => self.dispatch_analyze_code(arguments).await,
            "get_refactoring_suggestions" => Self::dispatch_refactoring_suggestions(arguments).await,
            "validate_quality_gates" => Self::dispatch_validate_quality_gates(arguments).await,
            "analyze_file_quality" => Self::dispatch_analyze_file_quality(arguments).await,
            _ => Err((error_codes::TOOL_NOT_FOUND, format!("Unknown tool: {}", name))),
        }
    }

    /// Dispatch analyze_code tool.
    async fn dispatch_analyze_code(&self, arguments: serde_json::Value) -> Result<ToolResult, (i32, String)> {
        let params = serde_json::from_value::<AnalyzeCodeParams>(arguments)
            .map_err(|e| (error_codes::INVALID_PARAMS, format!("Invalid analyze_code parameters: {}", e)))?;
        self.execute_analyze_code_cached(params).await
    }

    /// Dispatch get_refactoring_suggestions tool.
    async fn dispatch_refactoring_suggestions(arguments: serde_json::Value) -> Result<ToolResult, (i32, String)> {
        let params = serde_json::from_value::<RefactoringSuggestionsParams>(arguments)
            .map_err(|e| (error_codes::INVALID_PARAMS, format!("Invalid get_refactoring_suggestions parameters: {}", e)))?;
        execute_refactoring_suggestions(params).await
    }

    /// Dispatch validate_quality_gates tool.
    async fn dispatch_validate_quality_gates(arguments: serde_json::Value) -> Result<ToolResult, (i32, String)> {
        let params = serde_json::from_value::<ValidateQualityGatesParams>(arguments)
            .map_err(|e| (error_codes::INVALID_PARAMS, format!("Invalid validate_quality_gates parameters: {}", e)))?;
        execute_validate_quality_gates(params).await
    }

    /// Dispatch analyze_file_quality tool.
    async fn dispatch_analyze_file_quality(arguments: serde_json::Value) -> Result<ToolResult, (i32, String)> {
        let params = serde_json::from_value::<AnalyzeFileQualityParams>(arguments)
            .map_err(|e| (error_codes::INVALID_PARAMS, format!("Invalid analyze_file_quality parameters: {}", e)))?;
        execute_analyze_file_quality(params).await
    }
}

/// Extension trait for JsonRpcResponse to set id.
trait JsonRpcResponseExt {
    /// Replaces the response ID and returns the modified response.
    fn with_id(self, id: Option<serde_json::Value>) -> Self;
}

/// [`JsonRpcResponseExt`] implementation for [`JsonRpcResponse`].
impl JsonRpcResponseExt for JsonRpcResponse {
    /// Replaces the response ID and returns the modified response.
    fn with_id(mut self, id: Option<serde_json::Value>) -> Self {
        self.id = id;
        self
    }
}

/// Run the MCP server with the given version
pub async fn run_mcp_server(version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new(version);
    server.run().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::time::Duration;
    use tempfile::tempdir;
    use valknut_rs::core::pipeline::{CodeDefinition, CodeDictionary};

    /// Creates sample analysis results for testing.
    fn sample_results() -> AnalysisResults {
        let summary = valknut_rs::api::results::AnalysisSummary {
            files_processed: 1,
            entities_analyzed: 1,
            refactoring_needed: 1,
            high_priority: 1,
            critical: 0,
            avg_refactoring_score: 0.5,
            code_health_score: 0.7,
            total_files: 1,
            total_entities: 1,
            total_lines_of_code: 120,
            languages: vec!["Rust".to_string()],
            total_issues: 1,
            high_priority_issues: 1,
            critical_issues: 0,
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        let candidate = valknut_rs::api::results::RefactoringCandidate {
            entity_id: "src/lib.rs::sample_fn".to_string(),
            name: "sample_fn".to_string(),
            file_path: "src/lib.rs".to_string(),
            line_range: Some((5, 25)),
            priority: valknut_rs::core::scoring::Priority::High,
            score: 0.5,
            confidence: 0.8,
            issues: vec![valknut_rs::api::results::RefactoringIssue {
                code: "CMPLX".to_string(),
                category: "complexity".to_string(),
                severity: 1.6,
                contributing_features: vec![valknut_rs::api::results::FeatureContribution {
                    feature_name: "cyclomatic_complexity".to_string(),
                    value: 12.0,
                    normalized_value: 0.6,
                    contribution: 0.8,
                }],
            }],
            suggestions: vec![valknut_rs::api::results::RefactoringSuggestion {
                refactoring_type: "extract_method".to_string(),
                code: "XTRMTH".to_string(),
                priority: 0.8,
                effort: 0.3,
                impact: 0.7,
            }],
            issue_count: 1,
            suggestion_count: 1,
            coverage_percentage: None,
        };

        let mut code_dictionary = CodeDictionary::default();
        code_dictionary.issues.insert(
            "CMPLX".to_string(),
            CodeDefinition {
                code: "CMPLX".to_string(),
                title: "High Complexity".to_string(),
                summary: "Function is too complex".to_string(),
                category: Some("complexity".to_string()),
            },
        );

        AnalysisResults {
            summary,
            normalized: None,
            passes: valknut_rs::api::results::StageResultsBundle::disabled(),
            refactoring_candidates: vec![candidate],
            statistics: valknut_rs::api::results::AnalysisStatistics {
                total_duration: Duration::from_secs(1),
                avg_file_processing_time: Duration::from_millis(120),
                avg_entity_processing_time: Duration::from_millis(40),
                features_per_entity: HashMap::new(),
                priority_distribution: HashMap::new(),
                issue_distribution: HashMap::new(),
                memory_stats: valknut_rs::api::results::MemoryStats {
                    peak_memory_bytes: 256_000,
                    final_memory_bytes: 128_000,
                    efficiency_score: 0.75,
                },
            },
            health_metrics: None,
            clone_analysis: None,
            coverage_packs: Vec::new(),
            warnings: vec!["Minor warning".to_string()],
            code_dictionary,
            documentation: None,
        }
    }

    #[test]
    fn available_tools_expose_expected_entries() {
        let server = McpServer::new("1.0.0");
        let tools = server.available_tools();
        let names: Vec<_> = tools.iter().map(|tool| tool.name.as_str()).collect();
        assert!(names.contains(&"analyze_code"));
        assert!(names.contains(&"get_refactoring_suggestions"));
        assert!(names.contains(&"validate_quality_gates"));
        assert!(names.contains(&"analyze_file_quality"));
    }

    #[test]
    fn handle_initialize_returns_capabilities() {
        let server = McpServer::new("1.0.0");
        let response = server.handle_initialize(Some(json!(1)));
        assert!(response.error.is_none());
        assert_eq!(response.id, Some(json!(1)));
        let result = response.result.unwrap();
        assert_eq!(result["server_info"]["version"], "1.0.0");
        assert!(result["capabilities"]["tools"].is_array());
    }

    #[test]
    fn handle_tools_list_wraps_available_tools() {
        let server = McpServer::new("1.0.0");
        let response = server.handle_tools_list(None);
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert!(!result["tools"].as_array().unwrap().is_empty());
    }

    #[test]
    fn format_analysis_results_defaults_to_json() {
        let server = McpServer::new("1.0.0");
        let results = sample_results();
        let json_output = server
            .format_analysis_results(&results, "json")
            .expect("json formatting");
        assert!(json_output.contains("\"files_processed\": 1"));

        let fallback_output = server
            .format_analysis_results(&results, "unsupported")
            .expect("fallback formatting");
        assert!(fallback_output.contains("\"code_health_score\": 0.7"));
    }

    #[tokio::test]
    async fn cache_analysis_provides_hits_and_expires() {
        let server = McpServer::new("1.0.0");
        let path = PathBuf::from("src/lib.rs");
        server.cache_analysis(path.clone(), sample_results()).await;
        assert!(server.get_cached_analysis(&path).await.is_some());

        {
            let mut cache = server.analysis_cache.lock().await;
            if let Some(entry) = cache.get_mut(&path) {
                entry.timestamp = std::time::Instant::now() - Duration::from_secs(400);
            }
        }

        assert!(server.get_cached_analysis(&path).await.is_none());
    }

    #[tokio::test]
    async fn cache_analysis_evicts_when_limit_exceeded() {
        let server = McpServer::new("1.0.0");
        for i in 0..11 {
            let path = PathBuf::from(format!("src/file{i}.rs"));
            server.cache_analysis(path, sample_results()).await;
        }

        let cache = server.analysis_cache.lock().await;
        assert_eq!(cache.len(), 10);
    }

    #[tokio::test]
    async fn handle_request_validates_jsonrpc_version() {
        let server = McpServer::new("1.0.0");
        let request = json!({
            "jsonrpc": "1.0",
            "method": "initialize",
            "id": 1
        });
        let response = server.handle_request(&request.to_string()).await;
        assert_eq!(response.error.unwrap().code, error_codes::INVALID_REQUEST);
    }

    #[tokio::test]
    async fn handle_request_unknown_method_returns_error() {
        let server = McpServer::new("1.0.0");
        let request = json!({
            "jsonrpc": "2.0",
            "method": "does_not_exist",
            "id": 1
        });
        let response = server.handle_request(&request.to_string()).await;
        assert_eq!(response.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn handle_request_rejects_empty_payload() {
        let server = McpServer::new("1.0.0");
        let response = server.handle_request("   ").await;
        let error = response.error.expect("expected error");
        assert_eq!(error.code, error_codes::INVALID_REQUEST);
        assert!(
            error.message.contains("Empty request"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[tokio::test]
    async fn handle_request_reports_parse_errors() {
        let server = McpServer::new("1.0.0");
        let response = server.handle_request("{\"jsonrpc\":").await;
        let error = response.error.expect("expected parse error");
        assert_eq!(error.code, error_codes::PARSE_ERROR);
        assert!(
            error.message.contains("Invalid JSON"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[tokio::test]
    async fn handle_tool_call_validates_parameters() {
        let server = McpServer::new("1.0.0");
        let missing = server.handle_tool_call(Some(json!(1)), None).await;
        assert_eq!(missing.error.unwrap().code, error_codes::INVALID_PARAMS);

        let invalid = server
            .handle_tool_call(
                Some(json!(2)),
                Some(json!({
                    "name": "analyze_code",
                    "arguments": "not an object"
                })),
            )
            .await;
        assert_eq!(invalid.error.unwrap().code, error_codes::INVALID_PARAMS);

        let unknown = server
            .handle_tool_call(
                Some(json!(3)),
                Some(json!({
                    "name": "unknown_tool",
                    "arguments": {}
                })),
            )
            .await;
        assert_eq!(unknown.error.unwrap().code, error_codes::TOOL_NOT_FOUND);
    }

    #[tokio::test]
    async fn execute_analyze_code_cached_returns_cached_output() {
        let server = McpServer::new("1.0.0");
        let temp = tempdir().expect("temp dir");
        let project_dir = temp.path().join("project");
        std::fs::create_dir_all(&project_dir).expect("create project dir");
        let canonical = project_dir.canonicalize().expect("canonical path");

        server
            .cache_analysis(canonical.clone(), sample_results())
            .await;

        let params = AnalyzeCodeParams {
            path: project_dir.to_string_lossy().to_string(),
            format: "json".to_string(),
        };

        let result = server
            .execute_analyze_code_cached(params)
            .await
            .expect("cached analyze");

        assert_eq!(result.content.len(), 1);
        assert!(result.content[0].text.contains("\"files_processed\": 1"));
    }

    #[tokio::test]
    async fn handle_tool_call_analyze_file_quality_missing_file_errors() {
        let server = McpServer::new("1.0.0");
        let response = server
            .handle_tool_call(
                Some(json!(9)),
                Some(json!({
                    "name": "analyze_file_quality",
                    "arguments": {
                        "file_path": "/path/that/does/not/exist.rs"
                    }
                })),
            )
            .await;

        let error = response.error.expect("expected error response");
        assert_eq!(error.code, error_codes::INVALID_PARAMS);
        assert!(
            error.message.contains("File does not exist"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[tokio::test]
    async fn execute_analyze_code_cached_rejects_missing_path() {
        let server = McpServer::new("1.0.0");
        let params = AnalyzeCodeParams {
            path: "/definitely/does/not/exist".to_string(),
            format: "json".to_string(),
        };

        let err = server
            .execute_analyze_code_cached(params)
            .await
            .unwrap_err();
        assert_eq!(err.0, error_codes::INVALID_PARAMS);
        assert!(
            err.1.contains("Path does not exist"),
            "unexpected error text: {}",
            err.1
        );
    }

    #[test]
    fn format_analysis_results_attempts_html_generation() {
        let server = McpServer::new("1.0.0");
        let results = sample_results();
        let base_path = std::env::temp_dir().join("valknut_mcp_report");
        let _ = std::fs::remove_file(&base_path);
        let _ = std::fs::remove_file(base_path.with_extension("html"));

        let html_result = server.format_analysis_results(&results, "html");
        assert!(
            html_result.is_err(),
            "expected HTML formatting to currently fail due to missing generated file"
        );

        let _ = std::fs::remove_file(&base_path);
        let _ = std::fs::remove_file(base_path.with_extension("html"));
    }
}
