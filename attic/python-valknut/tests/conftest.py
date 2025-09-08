"""
Pytest configuration for MCP integration tests.
"""

import asyncio
import os
import tempfile
from pathlib import Path
from typing import Generator, Iterator

import pytest

# Configure asyncio event loop for tests
@pytest.fixture(scope="session")
def event_loop():
    """Create an instance of the default event loop for the test session."""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()


@pytest.fixture(scope="session")
def test_fixtures_dir() -> Path:
    """Provide path to test fixtures directory."""
    return Path(__file__).parent / "fixtures"


@pytest.fixture(scope="session")  
def temp_dir() -> Generator[Path, None, None]:
    """Provide a temporary directory for test outputs."""
    with tempfile.TemporaryDirectory(prefix="valknut_test_") as temp_dir:
        yield Path(temp_dir)


@pytest.fixture(autouse=True)
def setup_test_environment():
    """Set up test environment variables."""
    # Ensure tests run in a clean environment
    os.environ["REFACTOR_RANK_LOG_LEVEL"] = "WARNING"
    os.environ["REFACTOR_RANK_TEST_MODE"] = "1"
    
    yield
    
    # Clean up after test
    if "REFACTOR_RANK_TEST_MODE" in os.environ:
        del os.environ["REFACTOR_RANK_TEST_MODE"]


@pytest.fixture
def mcp_test_config():
    """Provide test configuration for MCP server."""
    return {
        "server": {
            "host": "localhost",
            "port": 8000,
            "auth": "none"
        },
        "ranking": {
            "top_k": 10
        },
        "weights": {
            "complexity": 0.3,
            "clone_mass": 0.2,
            "centrality": 0.2,
            "cycles": 0.1,
            "type_friction": 0.1,
            "smell_prior": 0.1
        }
    }


# Pytest markers for test categorization
def pytest_configure(config):
    """Configure pytest markers."""
    config.addinivalue_line(
        "markers", "integration: marks tests as integration tests"
    )
    config.addinivalue_line(
        "markers", "mcp: marks tests as MCP-specific tests"
    )
    config.addinivalue_line(
        "markers", "async_server: marks tests that require async server"
    )
    config.addinivalue_line(
        "markers", "performance: marks tests as performance tests"
    )
    config.addinivalue_line(
        "markers", "stress: marks tests as stress tests"
    )
    config.addinivalue_line(
        "markers", "slow: marks tests as slow running tests"
    )


def pytest_collection_modifyitems(config, items):
    """Modify test collection to add markers based on test names."""
    for item in items:
        # Add markers based on test file/function names
        if "test_mcp" in item.nodeid:
            item.add_marker(pytest.mark.mcp)
        
        if "integration" in item.nodeid:
            item.add_marker(pytest.mark.integration)
        
        if "async_server" in item.nodeid or "AsyncMCP" in item.nodeid:
            item.add_marker(pytest.mark.async_server)
        
        if "performance" in item.name.lower() or "benchmark" in item.name.lower():
            item.add_marker(pytest.mark.performance)
        
        if "stress" in item.name.lower() or "load" in item.name.lower():
            item.add_marker(pytest.mark.stress)
            item.add_marker(pytest.mark.slow)
        
        if "comprehensive" in item.name.lower():
            item.add_marker(pytest.mark.slow)


# Custom pytest options
def pytest_addoption(parser):
    """Add custom pytest command line options."""
    parser.addoption(
        "--run-stress-tests",
        action="store_true", 
        default=False,
        help="Run stress tests (can be slow)"
    )
    parser.addoption(
        "--mcp-port-range",
        action="store",
        default="8000-8100",
        help="Port range for MCP server tests (e.g., 8000-8100)"
    )


def pytest_runtest_setup(item):
    """Setup for individual tests."""
    # Skip stress tests unless explicitly requested
    if "stress" in [marker.name for marker in item.iter_markers()]:
        if not item.config.getoption("--run-stress-tests"):
            pytest.skip("Stress tests skipped (use --run-stress-tests to run)")


@pytest.fixture(scope="session")
def port_manager():
    """Manage port allocation for parallel test execution."""
    class PortManager:
        def __init__(self, start_port=8000, end_port=8100):
            self.start_port = start_port
            self.end_port = end_port
            self.current_port = start_port
            self.used_ports = set()
        
        def get_port(self):
            while self.current_port <= self.end_port:
                port = self.current_port
                self.current_port += 1
                if port not in self.used_ports:
                    self.used_ports.add(port)
                    return port
            raise RuntimeError("No available ports in range")
        
        def release_port(self, port):
            self.used_ports.discard(port)
    
    return PortManager()


# Async fixtures for MCP server testing
@pytest.fixture
async def async_mcp_server(port_manager):
    """Provide async MCP server for testing."""
    from tests.integration.test_mcp_async_server import AsyncMCPServerManager
    
    port = port_manager.get_port()
    server = AsyncMCPServerManager(port=port)
    
    try:
        await server.start()
        yield server
    finally:
        await server.stop()
        port_manager.release_port(port)


# Utility functions for tests
class MCPTestUtils:
    """Utility functions for MCP testing."""
    
    @staticmethod
    def create_test_payload(tool_name: str, **kwargs):
        """Create a test payload for MCP tool."""
        payloads = {
            "analyze_repo": {"paths": kwargs.get("paths", ["/test/path"])},
            "get_topk": {"result_id": kwargs.get("result_id", "test-uuid")},
            "get_item": {
                "result_id": kwargs.get("result_id", "test-uuid"),
                "entity_id": kwargs.get("entity_id", "test-entity")
            },
            "set_weights": {"weights": kwargs.get("weights", {"complexity": 0.5})},
            "get_impact_packs": {"result_id": kwargs.get("result_id", "test-uuid")},
            "ping": {}
        }
        
        payload = payloads.get(tool_name, {})
        payload.update(kwargs)
        return payload
    
    @staticmethod
    def validate_response_schema(response_data: dict, tool_name: str):
        """Validate response data against expected schema."""
        required_fields = {
            "analyze_repo": {"result_id", "status", "total_files", "total_entities", "processing_time"},
            "get_topk": {"items"},
            "get_item": {"brief"},
            "set_weights": {"ok", "message"},
            "get_impact_packs": {"impact_packs"},
            "ping": {"time", "status"}
        }
        
        if tool_name in required_fields:
            for field in required_fields[tool_name]:
                assert field in response_data, f"Missing required field '{field}' in {tool_name} response"


@pytest.fixture
def mcp_test_utils():
    """Provide MCP test utilities."""
    return MCPTestUtils()