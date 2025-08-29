"""
FastAPI server with MCP integration for valknut.
"""

import asyncio
import logging
import time
from pathlib import Path
from typing import Any, Dict, List, Optional
from uuid import UUID

from fastapi import FastAPI, HTTPException, Request, Security
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse, StreamingResponse
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials

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
    convert_brief_to_mcp,
    format_mcp_error,
    get_mcp_manifest,
    validate_mcp_request,
)
from valknut.core.briefs import BriefGenerator
from valknut.core.config import RefactorRankConfig, get_default_config
from valknut.core.pipeline import analyze, get_result
from valknut.core.scoring import WeightedScorer

logger = logging.getLogger(__name__)

# Global server state
server_config: RefactorRankConfig = get_default_config()
security = HTTPBearer(auto_error=False)


def create_app(config: RefactorRankConfig) -> FastAPI:
    """Create FastAPI application with MCP integration."""
    global server_config
    server_config = config
    
    app = FastAPI(
        title="Refactor Rank MCP Server",
        description="Static code analysis for refactorability ranking with MCP integration",
        version="0.1.0",
        docs_url="/docs" if config.server.auth == "none" else None,
        redoc_url="/redoc" if config.server.auth == "none" else None,
    )
    
    # Add CORS middleware
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )
    
    @app.middleware("http")
    async def add_process_time_header(request: Request, call_next):
        """Add processing time header."""
        start_time = time.time()
        response = await call_next(request)
        process_time = time.time() - start_time
        response.headers["X-Process-Time"] = str(process_time)
        return response
    
    return app


app = create_app(get_default_config())


# Authentication dependency
async def get_current_user(credentials: Optional[HTTPAuthorizationCredentials] = Security(security)):
    """Validate authentication if required."""
    if server_config.server.auth == "none":
        return None
    
    if server_config.server.auth == "bearer":
        if not credentials:
            raise HTTPException(status_code=401, detail="Authorization header required")
        
        if credentials.credentials != server_config.server.bearer_token:
            raise HTTPException(status_code=401, detail="Invalid token")
    
    return credentials


# Health check endpoint
@app.get("/healthz")
async def health_check():
    """Health check endpoint."""
    return {"status": "ok", "timestamp": time.time()}


# MCP Manifest endpoint
@app.get("/mcp/manifest")
async def mcp_manifest():
    """Return MCP manifest."""
    return get_mcp_manifest().model_dump()


# Core analysis endpoints
@app.post("/analyze", response_model=AnalyzeRepoResponse)
async def analyze_repositories(
    request: AnalyzeRepoRequest,
    user=Security(get_current_user)
) -> AnalyzeRepoResponse:
    """
    Analyze repositories and return result ID.
    
    This endpoint starts the analysis pipeline and returns immediately with a result ID.
    Use other endpoints to retrieve the actual results.
    """
    try:
        # Create config with overrides
        config = server_config.model_copy(deep=True)
        
        # Apply config overrides
        if request.config:
            for key, value in request.config.items():
                if hasattr(config, key):
                    setattr(config, key, value)
        
        # Apply top_k override
        if request.top_k:
            config.ranking.top_k = request.top_k
        
        # Update roots with provided paths
        config.roots = []
        for path_str in request.paths:
            path = Path(path_str)
            if path.exists():
                from valknut.core.config import RootConfig
                config.roots.append(RootConfig(path=str(path)))
            else:
                logger.warning(f"Path does not exist: {path_str}")
        
        if not config.roots:
            raise HTTPException(status_code=400, detail="No valid paths provided")
        
        # Run analysis
        logger.info(f"Starting analysis for paths: {request.paths}")
        result = await analyze(config)
        
        return AnalyzeRepoResponse(
            result_id=str(result.result_id),
            total_files=result.total_files,
            total_entities=result.total_entities,
            processing_time=result.processing_time,
        )
        
    except Exception as e:
        logger.error(f"Analysis failed: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/results/{result_id}/summary")
async def get_result_summary(result_id: str, user=Security(get_current_user)):
    """Get analysis result summary."""
    try:
        result = get_result(UUID(result_id))
        if not result:
            raise HTTPException(status_code=404, detail="Result not found")
        
        return {
            "result_id": str(result.result_id),
            "total_files": result.total_files,
            "total_entities": result.total_entities,
            "processing_time": result.processing_time,
            "top_k_count": len(result.top_k_entities),
            "config": result.config.model_dump(exclude={"server"}),
        }
        
    except ValueError:
        raise HTTPException(status_code=400, detail="Invalid result ID")
    except Exception as e:
        logger.error(f"Failed to get summary: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/results/{result_id}/briefs")
async def get_result_briefs(result_id: str, user=Security(get_current_user)):
    """Stream refactor briefs as JSONL."""
    try:
        result = get_result(UUID(result_id))
        if not result:
            raise HTTPException(status_code=404, detail="Result not found")
        
        async def generate_briefs():
            """Generate JSONL stream of briefs."""
            # Initialize brief generator
            scorer = WeightedScorer(result.config.weights)
            brief_generator = BriefGenerator(result.config.briefs, scorer)
            
            for feature_vector, score in result.top_k_entities:
                # Find corresponding entity
                entity = None
                for lang_index in result.config.languages:
                    # This is simplified - in reality we'd need to maintain entity references
                    pass
                
                if entity:
                    brief = brief_generator.generate_brief(entity, feature_vector, score, None)
                    brief_dict = convert_brief_to_mcp(brief)
                    yield f"{brief_dict}\n"
                else:
                    # Fallback brief from feature vector
                    brief_dict = {
                        "entity_id": feature_vector.entity_id,
                        "score": score,
                        "features": feature_vector.normalized_features,
                    }
                    yield f"{brief_dict}\n"
        
        return StreamingResponse(
            generate_briefs(),
            media_type="application/x-jsonlines",
            headers={"Content-Disposition": f"attachment; filename=briefs-{result_id}.jsonl"}
        )
        
    except ValueError:
        raise HTTPException(status_code=400, detail="Invalid result ID")
    except Exception as e:
        logger.error(f"Failed to get briefs: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get("/results/{result_id}/impact_packs")
async def get_result_impact_packs(result_id: str, user=Security(get_current_user)):
    """Get impact packs for a result."""
    try:
        result = get_result(UUID(result_id))
        if not result:
            raise HTTPException(status_code=404, detail="Result not found")
        
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
        
    except ValueError:
        raise HTTPException(status_code=400, detail="Invalid result ID")
    except Exception as e:
        logger.error(f"Failed to get impact packs: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/warmup")
async def warmup_analysis(
    request: AnalyzeRepoRequest,
    user=Security(get_current_user)
):
    """Preindex a codebase for faster subsequent analysis."""
    try:
        # This would implement preindexing/warming
        # For now, just run a normal analysis
        response = await analyze_repositories(request, user)
        return {"message": "Warmup completed", "result_id": response.result_id}
        
    except Exception as e:
        logger.error(f"Warmup failed: {e}")
        raise HTTPException(status_code=500, detail=str(e))


# MCP Tool endpoints
@app.post("/mcp/analyze_repo", response_model=AnalyzeRepoResponse)
async def mcp_analyze_repo(request: Dict[str, Any], user=Security(get_current_user)):
    """MCP tool: analyze_repo"""
    try:
        if not validate_mcp_request(request, "analyze_repo"):
            raise HTTPException(status_code=400, detail="Invalid request format")
        
        # Convert to internal request format
        internal_request = AnalyzeRepoRequest(**request)
        return await analyze_repositories(internal_request, user)
        
    except Exception as e:
        logger.error(f"MCP analyze_repo failed: {e}")
        return JSONResponse(
            status_code=500,
            content=format_mcp_error(str(e))
        )


@app.post("/mcp/get_topk", response_model=GetTopKResponse)
async def mcp_get_topk(request: Dict[str, Any], user=Security(get_current_user)):
    """MCP tool: get_topk"""
    try:
        if not validate_mcp_request(request, "get_topk"):
            raise HTTPException(status_code=400, detail="Invalid request format")
        
        result_id = request["result_id"]
        result = get_result(UUID(result_id))
        
        if not result:
            raise HTTPException(status_code=404, detail="Result not found")
        
        # Convert top-k entities to brief format
        items = []
        scorer = WeightedScorer(result.config.weights)
        brief_generator = BriefGenerator(result.config.briefs, scorer)
        
        for feature_vector, score in result.top_k_entities:
            # Create simplified brief
            brief_dict = {
                "entity_id": feature_vector.entity_id,
                "score": score,
                "features": feature_vector.normalized_features,
                "explanations": scorer.explain_score(feature_vector),
            }
            items.append(brief_dict)
        
        return GetTopKResponse(items=items)
        
    except Exception as e:
        logger.error(f"MCP get_topk failed: {e}")
        return JSONResponse(
            status_code=500,
            content=format_mcp_error(str(e))
        )


@app.post("/mcp/get_item", response_model=GetItemResponse)
async def mcp_get_item(request: Dict[str, Any], user=Security(get_current_user)):
    """MCP tool: get_item"""
    try:
        if not validate_mcp_request(request, "get_item"):
            raise HTTPException(status_code=400, detail="Invalid request format")
        
        result_id = request["result_id"]
        entity_id = request["entity_id"]
        
        result = get_result(UUID(result_id))
        if not result:
            raise HTTPException(status_code=404, detail="Result not found")
        
        # Find entity in results
        for feature_vector, score in result.ranked_entities:
            if feature_vector.entity_id == entity_id:
                brief_dict = {
                    "entity_id": feature_vector.entity_id,
                    "score": score,
                    "features": feature_vector.normalized_features,
                    "explanations": WeightedScorer(result.config.weights).explain_score(feature_vector),
                }
                return GetItemResponse(brief=brief_dict)
        
        return GetItemResponse(brief=None)
        
    except Exception as e:
        logger.error(f"MCP get_item failed: {e}")
        return JSONResponse(
            status_code=500,
            content=format_mcp_error(str(e))
        )


@app.post("/mcp/set_weights", response_model=SetWeightsResponse)
async def mcp_set_weights(request: Dict[str, Any], user=Security(get_current_user)):
    """MCP tool: set_weights"""
    try:
        if not validate_mcp_request(request, "set_weights"):
            raise HTTPException(status_code=400, detail="Invalid request format")
        
        # Update global server config weights
        weights = request["weights"]
        for key, value in weights.items():
            if hasattr(server_config.weights, key):
                setattr(server_config.weights, key, value)
        
        return SetWeightsResponse(
            ok=True,
            message="Weights updated successfully"
        )
        
    except Exception as e:
        logger.error(f"MCP set_weights failed: {e}")
        return JSONResponse(
            status_code=500,
            content=format_mcp_error(str(e))
        )


@app.post("/mcp/get_impact_packs", response_model=GetImpactPacksResponse)
async def mcp_get_impact_packs(request: Dict[str, Any], user=Security(get_current_user)):
    """MCP tool: get_impact_packs"""
    try:
        if not validate_mcp_request(request, "get_impact_packs"):
            raise HTTPException(status_code=400, detail="Invalid request format")
        
        result_id = request["result_id"]
        result = get_result(UUID(result_id))
        
        if not result:
            raise HTTPException(status_code=404, detail="Result not found")
        
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
        
        return GetImpactPacksResponse(impact_packs=impact_packs_data)
        
    except Exception as e:
        logger.error(f"MCP get_impact_packs failed: {e}")
        return JSONResponse(
            status_code=500,
            content=format_mcp_error(str(e))
        )


@app.post("/mcp/ping", response_model=PingResponse)
async def mcp_ping(request: Dict[str, Any] = None, user=Security(get_current_user)):
    """MCP tool: ping"""
    try:
        return PingResponse(
            time=str(time.time()),
            status="ok"
        )
    except Exception as e:
        logger.error(f"MCP ping failed: {e}")
        return JSONResponse(
            status_code=500,
            content=format_mcp_error(str(e))
        )


if __name__ == "__main__":
    import uvicorn
    
    config = get_default_config()
    uvicorn.run(
        "valknut.api.server:app",
        host=config.server.host,
        port=config.server.port,
        reload=True,
    )