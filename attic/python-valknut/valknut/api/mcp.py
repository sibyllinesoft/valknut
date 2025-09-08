"""
Model Context Protocol (MCP) tool definitions and schemas.
"""

import time
from typing import Any, Dict, List, Optional
from uuid import UUID

from pydantic import BaseModel, Field

from valknut.core.briefs import RefactorBrief


class MCPToolRequest(BaseModel):
    """Base model for MCP tool requests."""
    pass


class MCPToolResponse(BaseModel):
    """Base model for MCP tool responses."""
    pass


# Tool request/response models

class AnalyzeRepoRequest(MCPToolRequest):
    """Request to analyze a repository."""
    
    paths: List[str] = Field(..., description="List of repository paths to analyze")
    config: Optional[Dict[str, Any]] = Field(default=None, description="Optional configuration overrides")
    top_k: Optional[int] = Field(default=None, description="Optional top-k limit")


class AnalyzeRepoResponse(MCPToolResponse):
    """Response from analyze_repo tool."""
    
    result_id: str = Field(..., description="Unique result ID for this analysis")
    status: str = Field(default="completed", description="Analysis status")
    total_files: int = Field(..., description="Number of files analyzed")
    total_entities: int = Field(..., description="Number of entities analyzed")
    processing_time: float = Field(..., description="Processing time in seconds")


class GetTopKRequest(MCPToolRequest):
    """Request to get top-k entities."""
    
    result_id: str = Field(..., description="Result ID from analyze_repo")


class GetTopKResponse(MCPToolResponse):
    """Response from get_topk tool."""
    
    items: List[Dict[str, Any]] = Field(..., description="List of top-k brief items")


class GetItemRequest(MCPToolRequest):
    """Request to get a specific item."""
    
    result_id: str = Field(..., description="Result ID from analyze_repo")
    entity_id: str = Field(..., description="Entity ID to retrieve")


class GetItemResponse(MCPToolResponse):
    """Response from get_item tool."""
    
    brief: Optional[Dict[str, Any]] = Field(..., description="Refactor brief or None if not found")


class GetImpactPacksRequest(MCPToolRequest):
    """Request to get impact packs."""
    
    result_id: str = Field(..., description="Result ID from analyze_repo")


class GetImpactPacksResponse(MCPToolResponse):
    """Response from get_impact_packs tool."""
    
    impact_packs: List[Dict[str, Any]] = Field(..., description="List of impact packs")


class SetWeightsRequest(MCPToolRequest):
    """Request to set feature weights."""
    
    weights: Dict[str, float] = Field(..., description="Feature weights")


class SetWeightsResponse(MCPToolResponse):
    """Response from set_weights tool."""
    
    ok: bool = Field(default=True, description="Success indicator")
    message: str = Field(default="Weights updated successfully", description="Status message")


class PingRequest(MCPToolRequest):
    """Request to ping the server."""
    pass


class PingResponse(MCPToolResponse):
    """Response from ping tool."""
    
    time: str = Field(..., description="Current server time")
    status: str = Field(default="ok", description="Server status")


# MCP Manifest

class MCPToolSchema(BaseModel):
    """MCP tool schema definition."""
    
    name: str = Field(..., description="Tool name")
    description: str = Field(..., description="Tool description")
    input_schema: Dict[str, Any] = Field(..., description="JSON schema for input")
    output_schema: Dict[str, Any] = Field(..., description="JSON schema for output")


class MCPManifest(BaseModel):
    """MCP server manifest."""
    
    version: str = Field(default="0.1", description="MCP version")
    name: str = Field(default="valknut", description="Server name")
    description: str = Field(default="Static code analysis for refactorability ranking", description="Server description")
    tools: List[MCPToolSchema] = Field(..., description="Available tools")


def get_mcp_manifest() -> MCPManifest:
    """Generate MCP manifest with tool schemas."""
    
    tools = [
        MCPToolSchema(
            name="analyze_repo",
            description="Analyze repositories for refactorability and return a result ID",
            input_schema={
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of repository paths to analyze"
                    },
                    "config": {
                        "type": "object",
                        "description": "Optional configuration overrides"
                    },
                    "top_k": {
                        "type": "integer",
                        "description": "Optional top-k limit",
                        "minimum": 1
                    }
                },
                "required": ["paths"]
            },
            output_schema={
                "type": "object",
                "properties": {
                    "result_id": {"type": "string", "description": "Unique result ID"},
                    "status": {"type": "string", "description": "Analysis status"},
                    "total_files": {"type": "integer", "description": "Files analyzed"},
                    "total_entities": {"type": "integer", "description": "Entities analyzed"},
                    "processing_time": {"type": "number", "description": "Processing time in seconds"}
                },
                "required": ["result_id", "status", "total_files", "total_entities", "processing_time"]
            }
        ),
        
        MCPToolSchema(
            name="get_topk",
            description="Get top-k refactor brief items for a result",
            input_schema={
                "type": "object",
                "properties": {
                    "result_id": {"type": "string", "description": "Result ID from analyze_repo"}
                },
                "required": ["result_id"]
            },
            output_schema={
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "items": {"type": "object"},
                        "description": "List of refactor brief items"
                    }
                },
                "required": ["items"]
            }
        ),
        
        MCPToolSchema(
            name="get_item",
            description="Get a specific refactor brief item",
            input_schema={
                "type": "object",
                "properties": {
                    "result_id": {"type": "string", "description": "Result ID"},
                    "entity_id": {"type": "string", "description": "Entity ID to retrieve"}
                },
                "required": ["result_id", "entity_id"]
            },
            output_schema={
                "type": "object", 
                "properties": {
                    "brief": {
                        "type": ["object", "null"],
                        "description": "Refactor brief or null if not found"
                    }
                },
                "required": ["brief"]
            }
        ),
        
        MCPToolSchema(
            name="set_weights",
            description="Update feature weights for scoring",
            input_schema={
                "type": "object",
                "properties": {
                    "weights": {
                        "type": "object",
                        "properties": {
                            "complexity": {"type": "number", "minimum": 0, "maximum": 1},
                            "clone_mass": {"type": "number", "minimum": 0, "maximum": 1},
                            "centrality": {"type": "number", "minimum": 0, "maximum": 1},
                            "cycles": {"type": "number", "minimum": 0, "maximum": 1},
                            "type_friction": {"type": "number", "minimum": 0, "maximum": 1},
                            "smell_prior": {"type": "number", "minimum": 0, "maximum": 1}
                        },
                        "description": "Feature weights (0.0 to 1.0)"
                    }
                },
                "required": ["weights"]
            },
            output_schema={
                "type": "object",
                "properties": {
                    "ok": {"type": "boolean", "description": "Success indicator"},
                    "message": {"type": "string", "description": "Status message"}
                },
                "required": ["ok", "message"]
            }
        ),
        
        MCPToolSchema(
            name="get_impact_packs",
            description="Get impact packs for a result",
            input_schema={
                "type": "object",
                "properties": {
                    "result_id": {"type": "string", "description": "Result ID from analyze_repo"}
                },
                "required": ["result_id"]
            },
            output_schema={
                "type": "object",
                "properties": {
                    "impact_packs": {
                        "type": "array",
                        "items": {"type": "object"},
                        "description": "List of impact packs"
                    }
                },
                "required": ["impact_packs"]
            }
        ),
        
        MCPToolSchema(
            name="ping",
            description="Ping the server to check if it's alive",
            input_schema={
                "type": "object",
                "properties": {}
            },
            output_schema={
                "type": "object",
                "properties": {
                    "time": {"type": "string", "description": "Current server time"},
                    "status": {"type": "string", "description": "Server status"}
                },
                "required": ["time", "status"]
            }
        )
    ]
    
    return MCPManifest(tools=tools)


# Utility functions for MCP integration

def convert_brief_to_mcp(brief: RefactorBrief) -> Dict[str, Any]:
    """Convert RefactorBrief to MCP-compatible format."""
    return brief.to_dict()


def validate_mcp_request(request_data: Dict[str, Any], tool_name: str) -> bool:
    """Validate MCP request against tool schema."""
    manifest = get_mcp_manifest()
    
    # Find tool schema
    tool_schema = None
    for tool in manifest.tools:
        if tool.name == tool_name:
            tool_schema = tool
            break
    
    if not tool_schema:
        return False
    
    # Basic validation (could be more comprehensive)
    input_schema = tool_schema.input_schema
    required_fields = input_schema.get("required", [])
    
    for field in required_fields:
        if field not in request_data:
            return False
    
    return True


def format_mcp_error(error_message: str, error_code: str = "INTERNAL_ERROR") -> Dict[str, Any]:
    """Format error response for MCP."""
    return {
        "error": {
            "code": error_code,
            "message": error_message,
            "timestamp": time.time()
        }
    }