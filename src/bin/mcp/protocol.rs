//! MCP protocol types and message handling for JSON-RPC 2.0 communication.

use serde::{Deserialize, Serialize};

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

/// Factory methods for [`JsonRpcResponse`].
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
    #[allow(dead_code)]
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

/// Create tool schema for validate_quality_gates
pub fn create_validate_quality_gates_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Path to the code directory or file to validate"
            },
            "max_complexity": {
                "type": "number",
                "minimum": 1.0,
                "maximum": 100.0,
                "description": "Maximum allowed complexity score (optional)"
            },
            "min_health": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 100.0,
                "description": "Minimum required health score (optional)"
            },
            "max_debt": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 100.0,
                "description": "Maximum allowed technical debt ratio (optional)"
            },
            "max_issues": {
                "type": "integer",
                "minimum": 0,
                "description": "Maximum allowed number of issues (optional)"
            }
        },
        "required": ["path"]
    })
}

/// Create tool schema for analyze_file_quality
pub fn create_analyze_file_quality_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "Path to the specific file to analyze"
            },
            "include_suggestions": {
                "type": "boolean",
                "default": true,
                "description": "Whether to include refactoring suggestions in the report"
            }
        },
        "required": ["file_path"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_rpc_success_has_result_and_no_error() {
        let payload = json!({"status": "ok"});
        let response = JsonRpcResponse::success(Some(json!(1)), payload.clone());

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, Some(json!(1)));
        assert!(response.error.is_none());
        assert_eq!(response.result, Some(payload));
    }

    #[test]
    fn json_rpc_error_sets_error_payload() {
        let response =
            JsonRpcResponse::error(None, error_codes::METHOD_NOT_FOUND, "missing method".into());

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.id.is_none());

        let error = response.error.expect("error payload");
        assert_eq!(error.code, error_codes::METHOD_NOT_FOUND);
        assert_eq!(error.message, "missing method");
        assert!(error.data.is_none());
    }

    #[test]
    fn analyze_code_schema_declares_required_fields_and_enum() {
        let schema = create_analyze_code_schema();
        assert_eq!(schema["type"], "object");

        let properties = schema["properties"].as_object().expect("properties object");
        let path = properties
            .get("path")
            .expect("path property present")
            .as_object()
            .expect("path property object");
        assert_eq!(path.get("type"), Some(&json!("string")));

        let format = properties
            .get("format")
            .expect("format property present")
            .as_object()
            .expect("format property object");
        let allowed_values = format
            .get("enum")
            .and_then(|value| value.as_array())
            .expect("enum array");
        assert_eq!(
            allowed_values,
            &vec![json!("json"), json!("markdown"), json!("html")]
        );
        assert_eq!(format.get("default"), Some(&json!("json")));

        let required = schema["required"].as_array().expect("required array");
        assert!(required.iter().any(|value| value == "path"));
    }

    #[test]
    fn refactoring_suggestions_schema_limits_max_suggestions() {
        let schema = create_refactoring_suggestions_schema();

        let required = schema["required"].as_array().expect("required entries");
        assert_eq!(required, &vec![json!("entity_id")]);

        let properties = schema["properties"].as_object().expect("properties object");
        let max_suggestions = properties
            .get("max_suggestions")
            .expect("max_suggestions property")
            .as_object()
            .expect("max_suggestions object");

        assert_eq!(max_suggestions.get("type"), Some(&json!("number")));
        assert_eq!(max_suggestions.get("minimum"), Some(&json!(1)));
        assert_eq!(max_suggestions.get("maximum"), Some(&json!(50)));
        assert_eq!(max_suggestions.get("default"), Some(&json!(10)));
    }

    #[test]
    fn validate_quality_gates_schema_has_optional_thresholds() {
        let schema = create_validate_quality_gates_schema();
        assert_eq!(schema["type"], "object");

        let required = schema["required"].as_array().expect("required array");
        assert_eq!(required, &vec![json!("path")]);

        let properties = schema["properties"].as_object().expect("properties object");
        let path = properties
            .get("path")
            .expect("path property")
            .as_object()
            .expect("path object");
        assert_eq!(path.get("type"), Some(&json!("string")));

        for key in ["max_complexity", "min_health", "max_debt"] {
            let entry = properties
                .get(key)
                .unwrap_or_else(|| panic!("{key} property missing"));
            assert!(
                entry.is_object(),
                "{key} property should be an object describing constraints"
            );
        }

        let max_issues = properties
            .get("max_issues")
            .expect("max_issues property")
            .as_object()
            .expect("max_issues object");
        assert_eq!(max_issues.get("type"), Some(&json!("integer")));
        assert_eq!(max_issues.get("minimum"), Some(&json!(0)));
    }

    #[test]
    fn analyze_file_quality_schema_requires_file_path() {
        let schema = create_analyze_file_quality_schema();

        let required = schema["required"].as_array().expect("required entries");
        assert_eq!(required, &vec![json!("file_path")]);

        let properties = schema["properties"].as_object().expect("properties object");
        let file_path = properties
            .get("file_path")
            .expect("file_path property")
            .as_object()
            .expect("file_path object");
        assert_eq!(file_path.get("type"), Some(&json!("string")));

        let include_suggestions = properties
            .get("include_suggestions")
            .expect("include_suggestions property")
            .as_object()
            .expect("include_suggestions object");
        assert_eq!(include_suggestions.get("type"), Some(&json!("boolean")));
        assert_eq!(include_suggestions.get("default"), Some(&json!(true)));
    }
}
