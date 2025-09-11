//! MCP JSON-RPC 2.0 server implementation for stdio communication.

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncBufReader};
use tracing::{info, error, debug};
use serde_json;

use crate::mcp::protocol::{
    JsonRpcRequest, JsonRpcResponse, McpInitResult, McpCapabilities, McpTool, McpServerInfo,
    ToolCallParams, error_codes, create_analyze_code_schema, create_refactoring_suggestions_schema,
    create_validate_quality_gates_schema, create_analyze_file_quality_schema,
};
use crate::mcp::tools::{
    execute_analyze_code, execute_refactoring_suggestions, execute_validate_quality_gates, execute_analyze_file_quality,
    AnalyzeCodeParams, RefactoringSuggestionsParams, ValidateQualityGatesParams, AnalyzeFileQualityParams
};

/// MCP server that handles JSON-RPC 2.0 communication over stdin/stdout
pub struct McpServer {
    /// Server name and version information
    server_info: McpServerInfo,
}

impl McpServer {
    /// Create a new MCP server instance
    pub fn new(version: &str) -> Self {
        Self {
            server_info: McpServerInfo {
                name: "valknut".to_string(),
                version: version.to_string(),
            },
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
                        format!("Failed to read request: {}", e)
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
                "Empty request".to_string()
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
                    format!("Invalid JSON: {}", e)
                );
            }
        };

        debug!("Handling method: {}", request.method);

        // Validate JSON-RPC version
        if request.jsonrpc != "2.0" {
            return JsonRpcResponse::error(
                request.id,
                error_codes::INVALID_REQUEST,
                "Only JSON-RPC 2.0 is supported".to_string()
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
                format!("Method not found: {}", request.method)
            ),
        }
    }

    /// Handle MCP initialization
    fn handle_initialize(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        let result = McpInitResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: McpCapabilities {
                tools: vec![
                    McpTool {
                        name: "analyze_code".to_string(),
                        description: "Analyze code for refactoring opportunities and quality metrics".to_string(),
                        input_schema: create_analyze_code_schema(),
                    },
                    McpTool {
                        name: "get_refactoring_suggestions".to_string(),
                        description: "Get specific refactoring suggestions for a code entity".to_string(),
                        input_schema: create_refactoring_suggestions_schema(),
                    },
                    McpTool {
                        name: "validate_quality_gates".to_string(),
                        description: "Validate code against quality gate thresholds for CI/CD integration".to_string(),
                        input_schema: create_validate_quality_gates_schema(),
                    },
                    McpTool {
                        name: "analyze_file_quality".to_string(),
                        description: "Analyze quality metrics and issues for a specific file".to_string(),
                        input_schema: create_analyze_file_quality_schema(),
                    },
                ],
            },
            server_info: self.server_info.clone(),
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    /// Handle tools list request
    fn handle_tools_list(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        let tools = vec![
            McpTool {
                name: "analyze_code".to_string(),
                description: "Analyze code for refactoring opportunities and quality metrics".to_string(),
                input_schema: create_analyze_code_schema(),
            },
            McpTool {
                name: "get_refactoring_suggestions".to_string(),
                description: "Get specific refactoring suggestions for a code entity".to_string(),
                input_schema: create_refactoring_suggestions_schema(),
            },
            McpTool {
                name: "validate_quality_gates".to_string(),
                description: "Validate code against quality gate thresholds for CI/CD integration".to_string(),
                input_schema: create_validate_quality_gates_schema(),
            },
            McpTool {
                name: "analyze_file_quality".to_string(),
                description: "Analyze quality metrics and issues for a specific file".to_string(),
                input_schema: create_analyze_file_quality_schema(),
            },
        ];

        let result = serde_json::json!({
            "tools": tools
        });

        JsonRpcResponse::success(id, result)
    }

    /// Handle tool call request
    async fn handle_tool_call(
        &self,
        id: Option<serde_json::Value>,
        params: Option<serde_json::Value>
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters".to_string()
                );
            }
        };

        let tool_params: ToolCallParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid tool call parameters: {}", e)
                );
            }
        };

        // Execute the requested tool
        let result = match tool_params.name.as_str() {
            "analyze_code" => {
                let params: AnalyzeCodeParams = match serde_json::from_value(tool_params.arguments) {
                    Ok(p) => p,
                    Err(e) => {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            format!("Invalid analyze_code parameters: {}", e)
                        );
                    }
                };
                
                match execute_analyze_code(params).await {
                    Ok(result) => result,
                    Err((code, message)) => {
                        return JsonRpcResponse::error(id, code, message);
                    }
                }
            }
            "get_refactoring_suggestions" => {
                let params: RefactoringSuggestionsParams = match serde_json::from_value(tool_params.arguments) {
                    Ok(p) => p,
                    Err(e) => {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            format!("Invalid get_refactoring_suggestions parameters: {}", e)
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
                let params: ValidateQualityGatesParams = match serde_json::from_value(tool_params.arguments) {
                    Ok(p) => p,
                    Err(e) => {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            format!("Invalid validate_quality_gates parameters: {}", e)
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
                let params: AnalyzeFileQualityParams = match serde_json::from_value(tool_params.arguments) {
                    Ok(p) => p,
                    Err(e) => {
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            format!("Invalid analyze_file_quality parameters: {}", e)
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
                    format!("Unknown tool: {}", tool_params.name)
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