//! MCP protocol types and message handling for JSON-RPC 2.0 communication.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JSON-RPC 2.0 request structure
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
    pub id: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 error structure
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// MCP tool definition for tool discovery
#[derive(Debug, Serialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// MCP capabilities reported during initialization
#[derive(Debug, Serialize)]
pub struct McpCapabilities {
    pub tools: Vec<McpTool>,
}

/// MCP initialization result
#[derive(Debug, Serialize)]
pub struct McpInitResult {
    pub protocol_version: String,
    pub capabilities: McpCapabilities,
    pub server_info: McpServerInfo,
}

/// MCP server information
#[derive(Debug, Clone, Serialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
}

/// Tool execution request parameters
#[derive(Debug, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Tool execution result
#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub content: Vec<ContentItem>,
}

/// Content item in tool result
#[derive(Debug, Serialize)]
pub struct ContentItem {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

impl JsonRpcResponse {
    /// Create a successful response
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

/// MCP error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    
    // MCP-specific error codes
    pub const TOOL_NOT_FOUND: i32 = -32001;
    pub const TOOL_EXECUTION_ERROR: i32 = -32002;
    pub const ANALYSIS_ERROR: i32 = -32003;
}

/// Create tool schema for analyze_code
pub fn create_analyze_code_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Path to the code file or directory to analyze"
            },
            "format": {
                "type": "string",
                "enum": ["json", "markdown", "html"],
                "default": "json",
                "description": "Output format for analysis results"
            }
        },
        "required": ["path"]
    })
}

/// Create tool schema for get_refactoring_suggestions
pub fn create_refactoring_suggestions_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "entity_id": {
                "type": "string",
                "description": "Identifier of the code entity to get refactoring suggestions for"
            },
            "max_suggestions": {
                "type": "number",
                "minimum": 1,
                "maximum": 50,
                "default": 10,
                "description": "Maximum number of suggestions to return"
            }
        },
        "required": ["entity_id"]
    })
}