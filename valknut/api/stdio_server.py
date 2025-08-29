"""
Stdio-based MCP server implementation for valknut.

This module implements the Model Context Protocol (MCP) over stdin/stdout
using JSON-RPC 2.0 for seamless integration with Claude Code.
"""

import asyncio
import json
import logging
import sys
import time
from typing import Any, Dict, List, Optional, Set, Union
from uuid import UUID, uuid4

from valknut.api.mcp import (
    AnalyzeRepoRequest,
    AnalyzeRepoResponse,
    GetImpactPacksRequest,
    GetImpactPacksResponse,
    GetItemRequest,
    GetItemResponse,
    GetTopKRequest,
    GetTopKResponse,
    PingRequest,
    PingResponse,
    SetWeightsRequest,
    SetWeightsResponse,
    get_mcp_manifest,
)
from valknut.core.config import RefactorRankConfig, RootConfig, get_default_config
from valknut.core.pipeline import analyze, get_result
from valknut.core.scoring import WeightedScorer

# Global state for the stdio server
_analysis_results: Dict[str, Any] = {}
_server_config: RefactorRankConfig = get_default_config()

# Set up stderr-only logging to avoid interfering with stdout
stderr_handler = logging.StreamHandler(sys.stderr)
stderr_handler.setFormatter(
    logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s')
)
logger = logging.getLogger(__name__)
logger.addHandler(stderr_handler)
logger.setLevel(logging.INFO)


class JSONRPCError(Exception):
    """JSON-RPC error response."""
    
    def __init__(self, code: int, message: str, data: Optional[Any] = None):
        super().__init__(message)
        self.code = code
        self.message = message
        self.data = data


class MCPStdioServer:
    """MCP server using stdio transport with JSON-RPC 2.0."""
    
    def __init__(self, config: Optional[RefactorRankConfig] = None):
        self.config = config or get_default_config()
        self.capabilities: Set[str] = set()
        self.client_info: Optional[Dict[str, Any]] = None
        self.initialized = False
        
        # Analysis result cache
        self.analysis_cache: Dict[str, Any] = {}
        
        # MCP tools
        self.tools = {
            "analyze_repo": self._analyze_repo,
            "get_topk": self._get_topk,
            "get_item": self._get_item,
            "get_impact_packs": self._get_impact_packs,
            "set_weights": self._set_weights,
            "ping": self._ping,
        }
        
        # Built-in MCP methods
        self.methods = {
            "initialize": self._initialize,
            "tools/list": self._list_tools,
            "tools/call": self._call_tool,
            "ping": self._ping,
        }
    
    async def run(self):
        """Main server loop reading from stdin and writing to stdout."""
        logger.info("MCP stdio server starting")
        
        try:
            while True:
                try:
                    # Read line from stdin
                    line = sys.stdin.readline()
                    if not line:
                        # EOF reached
                        break
                    
                    line = line.strip()
                    if not line:
                        continue
                    
                    # Parse JSON-RPC message
                    try:
                        message = json.loads(line)
                    except json.JSONDecodeError as e:
                        logger.error(f"Invalid JSON received: {e}")
                        await self._send_error_response(
                            None, -32700, "Parse error", {"details": str(e)}
                        )
                        continue
                    
                    # Handle message
                    await self._handle_message(message)
                    
                except EOFError:
                    break
                except Exception as e:
                    logger.error(f"Error in server loop: {e}")
                    await self._send_error_response(
                        None, -32603, "Internal error", {"details": str(e)}
                    )
                    
        except KeyboardInterrupt:
            logger.info("Server interrupted by user")
        finally:
            logger.info("MCP stdio server shutting down")
    
    async def _handle_message(self, message: Dict[str, Any]):
        """Handle incoming JSON-RPC message."""
        # Validate JSON-RPC format
        if not isinstance(message, dict) or "jsonrpc" not in message:
            await self._send_error_response(
                message.get("id"), -32600, "Invalid Request"
            )
            return
        
        if message["jsonrpc"] != "2.0":
            await self._send_error_response(
                message.get("id"), -32600, "Invalid Request", 
                {"details": "Only JSON-RPC 2.0 is supported"}
            )
            return
        
        method = message.get("method")
        if not method:
            await self._send_error_response(
                message.get("id"), -32600, "Invalid Request", 
                {"details": "Missing method"}
            )
            return
        
        message_id = message.get("id")
        params = message.get("params", {})
        
        # Check if initialized (except for initialize method)
        if method != "initialize" and not self.initialized:
            await self._send_error_response(
                message_id, -32002, "Not initialized"
            )
            return
        
        # Handle the method
        try:
            if method in self.methods:
                result = await self.methods[method](params)
                if message_id is not None:  # Only respond if not notification
                    await self._send_success_response(message_id, result)
            else:
                await self._send_error_response(
                    message_id, -32601, "Method not found", {"method": method}
                )
        except JSONRPCError as e:
            await self._send_error_response(message_id, e.code, e.message, e.data)
        except Exception as e:
            logger.error(f"Error handling method {method}: {e}")
            await self._send_error_response(
                message_id, -32603, "Internal error", {"details": str(e)}
            )
    
    async def _send_success_response(self, message_id: Any, result: Any):
        """Send successful JSON-RPC response."""
        response = {
            "jsonrpc": "2.0",
            "id": message_id,
            "result": result
        }
        await self._send_message(response)
    
    async def _send_error_response(self, message_id: Any, code: int, message: str, data: Optional[Any] = None):
        """Send error JSON-RPC response."""
        error = {
            "code": code,
            "message": message
        }
        if data is not None:
            error["data"] = data
        
        response = {
            "jsonrpc": "2.0",
            "id": message_id,
            "error": error
        }
        await self._send_message(response)
    
    async def _send_message(self, message: Dict[str, Any]):
        """Send JSON-RPC message to stdout."""
        try:
            json_str = json.dumps(message, separators=(',', ':'))
            sys.stdout.write(json_str + '\n')
            sys.stdout.flush()
        except Exception as e:
            logger.error(f"Failed to send message: {e}")
    
    async def _initialize(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Handle MCP initialize request."""
        # Validate client info
        client_info = params.get("clientInfo")
        if not client_info:
            raise JSONRPCError(-32600, "Missing clientInfo")
        
        protocol_version = params.get("protocolVersion")
        if not protocol_version or protocol_version != "2024-11-05":
            raise JSONRPCError(-32600, "Unsupported protocol version")
        
        self.client_info = client_info
        self.capabilities = set(params.get("capabilities", {}).keys())
        self.initialized = True
        
        logger.info(f"Initialized with client: {client_info.get('name', 'Unknown')}")
        
        return {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "valknut",
                "version": "0.1.0"
            }
        }
    
    async def _list_tools(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """List available MCP tools."""
        manifest = get_mcp_manifest()
        tools = []
        
        for tool in manifest.tools:
            tools.append({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.input_schema
            })
        
        return {"tools": tools}
    
    async def _call_tool(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Call an MCP tool."""
        name = params.get("name")
        if not name:
            raise JSONRPCError(-32600, "Missing tool name")
        
        if name not in self.tools:
            raise JSONRPCError(-32601, f"Tool not found: {name}")
        
        arguments = params.get("arguments", {})
        
        try:
            result = await self.tools[name](arguments)
            return {
                "content": [{
                    "type": "text",
                    "text": json.dumps(result, indent=2)
                }]
            }
        except Exception as e:
            logger.error(f"Tool {name} failed: {e}")
            raise JSONRPCError(-32603, f"Tool execution failed: {str(e)}")
    
    async def _analyze_repo(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Analyze repositories and cache results."""
        try:
            # Validate request
            if "paths" not in params:
                raise JSONRPCError(-32600, "Missing required parameter: paths")
            
            paths = params["paths"]
            if not isinstance(paths, list):
                raise JSONRPCError(-32600, "Parameter 'paths' must be a list")
            
            # Create config with overrides
            config = self.config.model_copy(deep=True)
            
            # Apply config overrides
            if "config" in params and params["config"]:
                for key, value in params["config"].items():
                    if hasattr(config, key):
                        setattr(config, key, value)
            
            # Apply top_k override
            if "top_k" in params and params["top_k"]:
                config.ranking.top_k = params["top_k"]
            
            # Update roots with provided paths
            config.roots = []
            for path_str in paths:
                from pathlib import Path
                path = Path(path_str)
                if path.exists():
                    config.roots.append(RootConfig(path=str(path)))
                else:
                    logger.warning(f"Path does not exist: {path_str}")
            
            if not config.roots:
                raise JSONRPCError(-32600, "No valid paths provided")
            
            # Run analysis
            logger.info(f"Starting analysis for paths: {paths}")
            result = await analyze(config)
            
            # Cache result
            result_id = str(result.result_id)
            self.analysis_cache[result_id] = result
            
            return {
                "result_id": result_id,
                "status": "completed",
                "total_files": result.total_files,
                "total_entities": result.total_entities,
                "processing_time": result.processing_time,
            }
            
        except JSONRPCError:
            raise
        except Exception as e:
            logger.error(f"Analysis failed: {e}")
            raise JSONRPCError(-32603, f"Analysis failed: {str(e)}")
    
    async def _get_topk(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Get top-k entities for a result."""
        if "result_id" not in params:
            raise JSONRPCError(-32600, "Missing required parameter: result_id")
        
        result_id = params["result_id"]
        result = self.analysis_cache.get(result_id)
        
        if not result:
            raise JSONRPCError(-32602, "Result not found")
        
        # Convert top-k entities to brief format
        items = []
        scorer = WeightedScorer(result.config.weights)
        
        for feature_vector, score in result.top_k_entities:
            brief_dict = {
                "entity_id": feature_vector.entity_id,
                "score": score,
                "features": feature_vector.normalized_features,
                "explanations": scorer.explain_score(feature_vector),
            }
            items.append(brief_dict)
        
        return {"items": items}
    
    async def _get_item(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Get a specific entity item."""
        if "result_id" not in params:
            raise JSONRPCError(-32600, "Missing required parameter: result_id")
        if "entity_id" not in params:
            raise JSONRPCError(-32600, "Missing required parameter: entity_id")
        
        result_id = params["result_id"]
        entity_id = params["entity_id"]
        
        result = self.analysis_cache.get(result_id)
        if not result:
            raise JSONRPCError(-32602, "Result not found")
        
        # Find entity in results
        for feature_vector, score in result.ranked_entities:
            if feature_vector.entity_id == entity_id:
                brief_dict = {
                    "entity_id": feature_vector.entity_id,
                    "score": score,
                    "features": feature_vector.normalized_features,
                    "explanations": WeightedScorer(result.config.weights).explain_score(feature_vector),
                }
                return {"brief": brief_dict}
        
        return {"brief": None}
    
    async def _get_impact_packs(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Get impact packs for a result."""
        if "result_id" not in params:
            raise JSONRPCError(-32600, "Missing required parameter: result_id")
        
        result_id = params["result_id"]
        result = self.analysis_cache.get(result_id)
        
        if not result:
            raise JSONRPCError(-32602, "Result not found")
        
        # Convert impact packs to dictionaries
        impact_packs_data = []
        for pack in result.impact_packs:
            pack_dict = {
                "pack_id": pack.pack_id,
                "pack_type": pack.pack_type,
                "title": pack.title,
                "description": pack.description,
                "entities": pack.entities,
                "value_estimate": pack.value_estimate,
                "effort_estimate": pack.effort_estimate,
                "priority_score": pack.priority_score,
                "metadata": pack.metadata,
            }
            impact_packs_data.append(pack_dict)
        
        return {"impact_packs": impact_packs_data}
    
    async def _set_weights(self, params: Dict[str, Any]) -> Dict[str, Any]:
        """Update feature weights."""
        if "weights" not in params:
            raise JSONRPCError(-32600, "Missing required parameter: weights")
        
        weights = params["weights"]
        if not isinstance(weights, dict):
            raise JSONRPCError(-32600, "Parameter 'weights' must be an object")
        
        # Update server config weights
        for key, value in weights.items():
            if hasattr(self.config.weights, key):
                setattr(self.config.weights, key, value)
        
        return {
            "ok": True,
            "message": "Weights updated successfully"
        }
    
    async def _ping(self, params: Dict[str, Any] = None) -> Dict[str, Any]:
        """Ping the server."""
        return {
            "time": str(time.time()),
            "status": "ok"
        }


async def run_stdio_server(config: Optional[RefactorRankConfig] = None):
    """Run the stdio MCP server."""
    server = MCPStdioServer(config)
    await server.run()


def main():
    """Main entry point for stdio server."""
    config = get_default_config()
    
    # Run the server
    try:
        asyncio.run(run_stdio_server(config))
    except KeyboardInterrupt:
        logger.info("Server stopped by user")
    except Exception as e:
        logger.error(f"Server error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()