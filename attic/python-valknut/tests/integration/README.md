# MCP Integration Tests

This directory contains comprehensive end-to-end testing for the Model Context Protocol (MCP) integration in Valknut.

## Overview

The test suite provides complete coverage of MCP functionality including:

- **Complete MCP workflow testing** from server startup through all tool calls
- **Realistic test scenarios** with actual code analysis on multiple languages
- **All MCP endpoints testing**: analyze_repo, get_topk, get_item, get_impact_packs, set_weights, ping
- **MCP manifest generation and tool schema validation**
- **Error handling and edge cases** in MCP responses
- **Authentication testing** with bearer tokens
- **HTTP request testing** with actual FastAPI server integration
- **JSON response schema validation** against MCP schemas
- **Simple and complex codebase testing**
- **Performance testing** for reasonable response times

## Test Files

### Core Test Modules

- **`test_mcp_e2e.py`** - Main end-to-end tests covering complete MCP workflows
- **`test_mcp_schema_validation.py`** - Detailed schema validation and compliance testing  
- **`test_mcp_async_server.py`** - Async server testing with real HTTP server lifecycle
- **`test_mcp_comprehensive.py`** - Comprehensive test suite runner and stress testing

### Test Fixtures

- **`../fixtures/simple_python/`** - Simple Python code with refactoring opportunities
- **`../fixtures/complex_typescript/`** - Complex TypeScript service with various code smells
- **`../fixtures/javascript_legacy/`** - Legacy JavaScript with multiple anti-patterns

## Running Tests

### Basic Test Execution

```bash
# Run all MCP integration tests
pytest tests/integration/test_mcp_*.py -v

# Run specific test file
pytest tests/integration/test_mcp_e2e.py -v

# Run with detailed output
pytest tests/integration/test_mcp_e2e.py -v -s
```

### Test Categories

```bash
# Run only MCP-specific tests
pytest -m mcp -v

# Run integration tests
pytest -m integration -v

# Run performance tests
pytest -m performance -v

# Run async server tests
pytest -m async_server -v

# Skip slow tests
pytest -m "not slow" -v
```

### Advanced Testing

```bash
# Run comprehensive test suite
pytest tests/integration/test_mcp_comprehensive.py::TestComprehensiveMCPIntegration::test_comprehensive_mcp_suite -v -s

# Run stress tests (requires explicit flag)
pytest --run-stress-tests -m stress -v -s

# Run with custom port range
pytest --mcp-port-range=9000-9100 tests/integration/test_mcp_async_server.py -v
```

## Test Structure

### MCPServerTestHarness

A test harness class that manages MCP server lifecycle during tests:

```python
with MCPServerTestHarness() as mcp_server:
    response = mcp_server.make_request("POST", "/mcp/ping", json={})
    assert response.status_code == 200
```

### AsyncMCPServerManager

For testing against real running server instances:

```python
async with AsyncMCPServerManager(port=8001) as server:
    response = await server.make_request("GET", "/healthz")
    assert response.status_code == 200
```

## Test Scenarios

### Complete Workflow Tests

1. **Basic Workflow** (`test_complete_workflow_simple_python`)
   - Analyze repository → Get top-k → Get specific items → Get impact packs
   - Uses simple Python fixture with known refactoring opportunities

2. **Complex Workflow** (`test_complete_workflow_complex_typescript`)
   - Tests with complex TypeScript codebase
   - Validates scoring and ranking functionality

3. **Multi-Repository** (`test_multiple_repositories_workflow`)
   - Analyzes multiple codebases simultaneously
   - Validates cross-repository analysis

### Schema Validation Tests

1. **JSON Schema Compliance** (`TestMCPSchemaCompliance`)
   - Validates all tool schemas are valid JSON Schema
   - Tests input/output schema structures
   - Validates constraint handling

2. **Pydantic Model Validation** (`TestPydanticModelValidation`)
   - Tests request/response model validation
   - Validates type checking and error handling

### Authentication Tests

1. **No Authentication** (`test_no_auth_endpoints_accessible`)
   - Validates endpoints work without authentication when disabled

2. **Bearer Token Authentication** (`test_valid_auth_token`)
   - Tests proper authentication with bearer tokens
   - Validates rejection of invalid tokens

### Performance Tests

1. **Response Time Benchmarks** (`test_response_time_benchmarks`)
   - Measures and validates response times
   - Sets performance thresholds for different operations

2. **Concurrent Request Handling** (`test_concurrent_requests`)
   - Tests server behavior under concurrent load
   - Validates thread safety and resource management

3. **Memory Usage Stability** (`test_memory_usage_stability`)
   - Monitors memory usage during repeated operations
   - Detects potential memory leaks

### Error Handling Tests

1. **Invalid Input Handling** (`TestMCPErrorHandling`)
   - Tests malformed JSON, missing fields, invalid types
   - Validates proper HTTP status codes

2. **Edge Cases** (`test_edge_cases`)
   - Tests boundary conditions and extreme values
   - Validates graceful degradation

## Configuration

### Environment Variables

- `REFACTOR_RANK_LOG_LEVEL=WARNING` - Reduces log noise during tests
- `REFACTOR_RANK_TEST_MODE=1` - Enables test-specific behaviors

### Test Configuration

The `conftest.py` file provides:

- Async event loop management
- Test fixture directories
- Port management for parallel execution
- Custom pytest markers and options
- Utility functions for MCP testing

## Performance Benchmarks

The test suite includes performance benchmarks with the following targets:

- **Ping requests**: < 100ms average response time
- **Analysis requests**: < 30 seconds for simple fixtures
- **Top-k retrieval**: < 1 second after analysis
- **Concurrent requests**: > 80% success rate under load
- **Memory usage**: < 100MB growth during extended operation

## Expected Test Results

When all tests pass, you should see:

```
======================== test session starts ========================
tests/integration/test_mcp_e2e.py::TestMCPManifestAndSchemas::test_manifest_generation PASSED
tests/integration/test_mcp_e2e.py::TestMCPCoreWorkflow::test_complete_workflow_simple_python PASSED
tests/integration/test_mcp_e2e.py::TestMCPWeightsManagement::test_set_weights_valid PASSED
tests/integration/test_mcp_e2e.py::TestMCPPerformance::test_analysis_performance PASSED
[... many more tests ...]

Performance benchmarks:
  Average ping time: 45.23ms
  Analysis time: 12.34s
  Get top-k time: 234.56ms

======================== X passed in Y.Zs ========================
```

## Troubleshooting

### Common Issues

1. **Port conflicts**: Use `--mcp-port-range` to specify different ports
2. **Server startup timeout**: Increase timeout in `AsyncMCPServerManager`
3. **Memory issues in stress tests**: Install `psutil` for memory monitoring
4. **Fixture paths**: Ensure test fixture directories exist

### Debug Mode

Run tests with debug output:

```bash
pytest tests/integration/test_mcp_e2e.py -v -s --tb=long --log-level=DEBUG
```

### Server Logs

Check server logs for detailed error information:

```bash
REFACTOR_RANK_LOG_LEVEL=DEBUG pytest tests/integration/test_mcp_async_server.py -v -s
```

## Contributing

When adding new MCP functionality:

1. Add corresponding test cases in appropriate test files
2. Update schema validation tests for new endpoints
3. Add performance benchmarks for new operations
4. Include error handling tests for new failure modes
5. Update this README with new test descriptions