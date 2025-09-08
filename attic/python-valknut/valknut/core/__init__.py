"""Core valknut components."""

from valknut.core.config import RefactorRankConfig, get_default_config, load_config
from valknut.core.pipeline import analyze, Pipeline
from valknut.core.briefs import RefactorBrief

# Initialize language adapters when core is imported
def _initialize_adapters():
    """Initialize default language adapters."""
    # Python adapter
    try:
        from valknut.lang.python_adapter import register_python_adapter
        register_python_adapter()
    except ImportError:
        pass  # Language adapter dependencies not available
    
    # TypeScript adapter
    try:
        from valknut.lang.typescript_adapter import register_typescript_adapter
        register_typescript_adapter()
    except ImportError:
        pass  # tree-sitter-typescript not available
    
    # JavaScript adapter
    try:
        from valknut.lang.javascript_adapter import register_javascript_adapter
        register_javascript_adapter()
    except ImportError:
        pass  # tree-sitter-javascript not available
    
    # Rust adapter
    try:
        from valknut.lang.rust_adapter import register_rust_adapter
        register_rust_adapter()
    except ImportError:
        pass  # tree-sitter-rust not available

_initialize_adapters()

__all__ = [
    "RefactorRankConfig",
    "get_default_config",
    "load_config", 
    "analyze",
    "Pipeline",
    "RefactorBrief",
]