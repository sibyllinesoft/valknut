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
    JsonRpcRequest, JsonRpcResponse, McpCapabilities, McpInitResult, McpServerInfo, McpTool,
    ToolCallParams, ToolResult, ContentItem,
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
            if let Some(oldest_key) = cache.iter()
                .min_by_key(|(_, entry)| entry.timestamp)
                .map(|(key, _)| key.clone()) {
                cache.remove(&oldest_key);
                info!("Evicted oldest cache entry: {}", oldest_key.display());
            }
        }
        
        cache.insert(path.clone(), AnalysisCache {
            path: path.clone(),
            results: Arc::new(results),
            timestamp: std::time::Instant::now(),
        });
        info!("Cached analysis results for: {}", path.display());
    }

    /// Execute analyze_code with session-level caching
    async fn execute_analyze_code_cached(&self, params: AnalyzeCodeParams) -> Result<ToolResult, (i32, String)> {
        info!("Executing analyze_code tool with caching for path: {}", params.path);

        // Validate path exists
        let path = std::path::Path::new(&params.path);
        if !path.exists() {
            return Err((
                error_codes::INVALID_PARAMS,
                format!("Path does not exist: {}", params.path),
            ));
        }

        let canonical_path = path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

        // Check cache first
        if let Some(cached_results) = self.get_cached_analysis(&canonical_path).await {
            // Format cached results according to requested format
            let formatted_output = match self.format_analysis_results(&cached_results, &params.format) {
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
    fn format_analysis_results(&self, results: &AnalysisResults, format: &str) -> Result<String, Box<dyn std::error::Error>> {
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

            // Read a line from stdin
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF reached, exit gracefully
                    debug!("EOF reached, shutting down MCP server");
                    break;
                }
                Ok(_) => {
                    // Process the JSON-RPC request
                    let response = self.handle_request(&line).await;

                    // Write response to stdout
                    let response_json = serde_json::to_string(&response)?;
                    stdout.write_all(response_json.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
                Err(e) => {
                    error!("Error reading from stdin: {}", e);
                    // Send error response and continue
                    let error_response = JsonRpcResponse::error(
                        None,
                        error_codes::INTERNAL_ERROR,
                        format!("Failed to read request: {}", e),
                    );
                    let response_json = serde_json::to_string(&error_response)?;
                    stdout.write_all(response_json.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
            }
        }

        info!("MCP server shutdown complete");
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
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters".to_string(),
                );
            }
        };

        let tool_params: ToolCallParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid tool call parameters: {}", e),
                );
            }
        };

        // Execute the requested tool
        let result = match tool_params.name.as_str() {
            "analyze_code" => {
                let params: AnalyzeCodeParams = match serde_json::from_value(tool_params.arguments)
                {
                    Ok(p) => p,
                    Err(e) => {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            format!("Invalid analyze_code parameters: {}", e),
                        );
                    }
                };

                match self.execute_analyze_code_cached(params).await {
                    Ok(result) => result,
                    Err((code, message)) => {
                        return JsonRpcResponse::error(id, code, message);
                    }
                }
            }
            "get_refactoring_suggestions" => {
                let params: RefactoringSuggestionsParams =
                    match serde_json::from_value(tool_params.arguments) {
                        Ok(p) => p,
                        Err(e) => {
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!("Invalid get_refactoring_suggestions parameters: {}", e),
                            );
                        }
                    };

                match execute_refactoring_suggestions(params).await {
                    Ok(result) => result,
                    Err((code, message)) => {
                        return JsonRpcResponse::error(id, code, message);
                    }
                }
            }
            "validate_quality_gates" => {
                let params: ValidateQualityGatesParams =
                    match serde_json::from_value(tool_params.arguments) {
                        Ok(p) => p,
                        Err(e) => {
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!("Invalid validate_quality_gates parameters: {}", e),
                            );
                        }
                    };

                match execute_validate_quality_gates(params).await {
                    Ok(result) => result,
                    Err((code, message)) => {
                        return JsonRpcResponse::error(id, code, message);
                    }
                }
            }
            "analyze_file_quality" => {
                let params: AnalyzeFileQualityParams =
                    match serde_json::from_value(tool_params.arguments) {
                        Ok(p) => p,
                        Err(e) => {
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!("Invalid analyze_file_quality parameters: {}", e),
                            );
                        }
                    };

                match execute_analyze_file_quality(params).await {
                    Ok(result) => result,
                    Err((code, message)) => {
                        return JsonRpcResponse::error(id, code, message);
                    }
                }
            }
            _ => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::TOOL_NOT_FOUND,
                    format!("Unknown tool: {}", tool_params.name),
                );
            }
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }
}

/// Run the MCP server with the given version
pub async fn run_mcp_server(version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new(version);
    server.run().await
}
