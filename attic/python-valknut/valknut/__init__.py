"""
Valknut: Static code analysis library for ranking refactorability.

A deterministic, static-only pipeline that scores and ranks code by refactor priority,
generates LLM-ready Refactor Briefs, and exposes the system via FastAPI MCP server.
"""

__version__ = "0.1.0"
__author__ = "Nathan Rice"
__email__ = "nathan.alexander.rice@gmail.com"

from valknut.core.config import RefactorRankConfig
from valknut.core.pipeline import analyze, Pipeline
from valknut.core.briefs import RefactorBrief

__all__ = [
    "RefactorRankConfig",
    "analyze", 
    "Pipeline",
    "RefactorBrief",
    "__version__",
]