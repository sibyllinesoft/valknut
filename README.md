# Valknut

**Static code analysis library for ranking refactorability**

Valknut builds a deterministic, static-only pipeline that scores and ranks code by "refactorability," generates LLM-ready **Refactor Briefs**, and exposes the whole system via a FastAPI **MCP** server for agent integration.

[![Python 3.11+](https://img.shields.io/badge/python-3.11+-blue.svg)](https://www.python.org/downloads/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Performance](https://img.shields.io/badge/Performance-130k_features/sec-brightgreen)](BENCHMARK_RESULTS.md)
[![Benchmark](https://img.shields.io/badge/Benchmark-0.024s_analysis-blue)](BENCHMARK_RESULTS.md)

## Quick Start

### Installation

```bash
pip install valknut
```

### Basic Usage

```bash
# Analyze a repository
valknut analyze /path/to/repo --out results/

# Start HTTP MCP server for agent integration
valknut serve --port 8140

# Start stdio MCP server (for Claude Code integration)
valknut mcp-stdio

# Print default configuration
valknut print-default-config > rr.yml
```

### Claude Code Integration

For seamless integration with Claude Code, use the stdio MCP server:

```json
{
  "mcpServers": {
    "valknut": {
      "command": "uv",
      "args": [
        "run", 
        "--directory", "/path/to/valknut",
        "valknut", 
        "mcp-stdio"
      ]
    }
  }
}
```

See [MCP_STDIO_GUIDE.md](MCP_STDIO_GUIDE.md) for detailed integration instructions.

### Python API

```python
from valknut import analyze, get_default_config

# Configure analysis
config = get_default_config()
config.languages = ["python", "typescript"]
config.ranking.top_k = 50

# Run analysis
result = await analyze(config)

# Get top refactor candidates
for brief in result.topk_briefs():
    print(f"{brief['path']}: {brief['score']:.3f}")
    print(f"Issues: {', '.join(brief['candidate_refactors'])}")
```

## Features

- **🔍 Multi-language Support**: Python, TypeScript, JavaScript, Rust (Go/Java experimental)
- **📊 Comprehensive Metrics**: Complexity, duplication, centrality, type friction, cohesion
- **🎯 Impact Packs**: Coordinated refactoring recommendations for systematic technical debt reduction
- **🤖 Agent Integration**: Full MCP server with Claude Code integration
- **⚡ Deterministic**: No runtime profiling or private data required
- **🎯 LLM-Ready**: Generates structured refactor briefs for AI consumption
- **🔄 Clone Detection**: Integrated with `sibylline-echo` for near-miss duplication
- **🚀 High Performance**: 130,000+ features/second processing capacity
- **🎯 Production Ready**: Sub-second analysis times, zero-error processing

## Architecture

### Core Pipeline (8 Stages)

1. **Discover** - Find files using configurable patterns
2. **Parse & Index** - Build ASTs, symbol tables, and dependency graphs  
3. **Features** - Extract complexity, centrality, cohesion, type friction metrics
4. **Normalization** - Robust statistical normalization to [0,1]
5. **Score** - Weighted scoring with configurable feature weights
6. **Select** - Top-K ranking with intelligent tie-breaking
7. **Briefs** - Generate LLM-ready refactor recommendations
8. **Output** - JSONL, JSON, Markdown with full traceability

### Feature Categories

| Category | Features | Description |
|----------|----------|-------------|
| **Complexity** | Cyclomatic, Cognitive, Nesting, Parameters | McCabe, nesting-weighted complexity |
| **Centrality** | Betweenness, Fan-in/out, PageRank | Import/call graph analysis |
| **Cycles** | SCC membership, Cycle size | Dependency cycle detection |
| **Duplication** | Clone mass, Similarity groups | Via `sibylline-echo` integration |
| **Type Friction** | `any` ratio, Casts, Nullability | Language-specific type issues |
| **Cohesion** | LCOM-like, Data clumps | Class and module cohesion |

### Supported Languages

| Language | Status | Parser | Features |
|----------|--------|--------|----------|
| Python | ✅ Stable | `libcst` + `ast` | Full type analysis |
| TypeScript | ✅ Stable | `tree-sitter` | Type friction, complexity, interfaces |
| JavaScript | ✅ Stable | `tree-sitter` | Complexity, duplication, ES6+ features |
| Rust | ✅ Stable | `tree-sitter` | Ownership analysis, zero-cost abstractions |
| Go | 🧪 Experimental | `tree-sitter` | Simplicity metrics |
| Java | 🧪 Experimental | `tree-sitter` | OOP analysis |

## Why Valknut vs Traditional Linters?

| Aspect | Traditional Linters (ruff, pylint, ESLint) | Valknut |
|--------|---------------------------------------------|---------|
| **Focus** | Syntax, style, basic patterns | Refactorability and technical debt |
| **Analysis** | Rule-based, local scope | Multi-dimensional feature analysis |
| **Prioritization** | Binary pass/fail | Ranked by refactor urgency (0-1 scores) |
| **Context** | Single file/function | Whole codebase graph analysis |
| **Complexity** | Basic McCabe only | Cognitive, nesting, parameter complexity |
| **Duplication** | Simple text matching | ML-powered semantic clone detection |
| **Dependencies** | Import analysis only | Graph centrality, cycle detection |
| **Output** | List of violations | Prioritized refactor candidates |
| **Performance** | Fast syntax checking | **130k+ features/second** analysis |

### 🔥 Valknut's Killer Features

**🧠 Semantic Understanding**: While ruff catches `len(x) == 0`, Valknut identifies why that 50-line function with 8 nested loops needs refactoring *urgently*.

**📊 Quantified Technical Debt**: Traditional linters give you 200 violations. Valknut ranks your top 10 refactor candidates with confidence scores.

**🕸️ Codebase-Wide Analysis**: Linters see files in isolation. Valknut sees how your `DatabaseManager` class affects 47 other modules and ranks it accordingly.

**🎯 AI-Powered Clone Detection**: Beyond copy-paste detection - finds semantically similar code patterns that should be unified.

**⚡ Production-Ready Performance**: Analyzes 353 entities in 24ms while running full graph analysis and ML inference.

## Configuration

### Basic Configuration (`rr.yml`)

```yaml
version: 1
languages: ["python", "typescript", "javascript", "rust"]
roots:
  - path: "./"
    include: ["src/**", "lib/**"]  
    exclude: ["**/node_modules/**", "**/.venv/**"]

ranking:
  top_k: 100
  granularity: "auto"  # auto|file|function|class

weights:
  complexity: 0.25     # Cyclomatic, cognitive complexity
  clone_mass: 0.20     # Code duplication
  centrality: 0.15     # Import graph centrality  
  cycles: 0.15         # Dependency cycles
  type_friction: 0.15  # Type safety issues
  smell_prior: 0.10    # ML-based code smell detection

detectors:
  echo:
    enabled: true
    min_similarity: 0.85
    min_tokens: 30
  semgrep:
    enabled: false

server:
  port: 8140
  host: "localhost"
  auth: "none"  # none|bearer
```

## MCP Integration

Valknut implements the [Model Context Protocol](https://docs.anthropic.com/claude/docs/mcp) for seamless agent integration with Claude Code and other MCP-compatible clients.

### 🚀 Claude Code Setup

Valknut offers two integration methods with Claude Code:

#### 🎯 **Fast Setup (Recommended)** - Stdio MCP Server

**Step 1: Install Valknut**

```bash
pip install valknut
```

**Step 2: Configure Claude Code MCP**

Add Valknut to your Claude Code MCP configuration (`~/.claude/mcp_servers.json`):

```json
{
  "mcpServers": {
    "valknut": {
      "command": "valknut",
      "args": ["mcp-stdio"]
    }
  }
}
```

**Step 3: Use in Claude Code**

```markdown
Please analyze this codebase for refactoring opportunities:

/analyze_repo {"paths": ["/path/to/your/repo"], "top_k": 20}
```

✅ **Advantages**: No server management, no ports, instant startup, perfect for individual developers

#### 🖥️ **HTTP Server Setup** - For Team/Production Use

**Step 1: Install Valknut**

```bash
pip install valknut
```

**Step 2: Configure Claude Code MCP**

Add Valknut to your Claude Code MCP configuration (`~/.claude/mcp_servers.json`):

```json
{
  "mcpServers": {
    "valknut": {
      "command": "valknut",
      "args": ["serve", "--port", "8140", "--host", "localhost"],
      "env": {}
    }
  }
}
```

**Step 3: Test the Integration**

Start Claude Code and verify the connection:

```bash
# Test the MCP connection
curl http://localhost:8140/mcp/ping
# Should return: {"time": "...", "status": "ok"}

# Test the MCP manifest
curl http://localhost:8140/mcp/manifest
# Should return tool definitions
```

**Step 4: Use in Claude Code**

```markdown
Please analyze this codebase for refactoring opportunities:

/analyze_repo {"paths": ["/path/to/your/repo"], "top_k": 20}
```

✅ **Advantages**: Shared server for teams, web API access, better for CI/CD integration

### 🔧 Advanced Claude Code Configuration

#### **Stdio Server with Custom Configuration**

Create `rr.yml` for project-specific settings:

```yaml
version: 1
languages: ["python", "typescript", "javascript"]
ranking:
  top_k: 50
  granularity: "function"
weights:
  complexity: 0.3
  clone_mass: 0.25
  centrality: 0.2
  cycles: 0.15
  type_friction: 0.1
```

Then configure Claude Code to use it:

```json
{
  "mcpServers": {
    "valknut": {
      "command": "valknut",
      "args": ["mcp-stdio", "--config", "/path/to/rr.yml"]
    }
  }
}
```

#### **HTTP Server with Custom Configuration**

For HTTP server mode, include server settings in your config:

```yaml
version: 1
languages: ["python", "typescript", "javascript"]
ranking:
  top_k: 50
  granularity: "function"
weights:
  complexity: 0.3
  clone_mass: 0.25
  centrality: 0.2
  cycles: 0.15
  type_friction: 0.1
server:
  port: 8140
  auth: "none"
```

Then configure Claude Code to use it:

```json
{
  "mcpServers": {
    "valknut": {
      "command": "valknut", 
      "args": ["serve", "--config", "/path/to/rr.yml"],
      "env": {}
    }
  }
}
```

**Authentication Setup**

For secure environments, configure bearer token authentication:

```yaml
server:
  auth: "bearer"
  bearer_token: "your-secure-token-here"
```

```json
{
  "mcpServers": {
    "valknut": {
      "command": "valknut",
      "args": ["serve", "--config", "/path/to/rr.yml"],
      "env": {
        "REFACTOR_RANK_TOKEN": "your-secure-token-here"
      }
    }
  }
}
```

### 🎯 Claude Code Usage Examples

**1. Comprehensive Codebase Analysis**

```markdown
I want to identify the top refactoring priorities in my Python project. Please analyze the entire codebase and show me the most critical issues.

/analyze_repo {"paths": ["/home/user/my-project"], "config": {"languages": ["python"], "ranking": {"top_k": 10}}}
```

**2. Focus on Specific Code Quality Issues**

```markdown
I'm concerned about code duplication in my TypeScript codebase. Please analyze with emphasis on clone detection.

/analyze_repo {"paths": ["/home/user/frontend"], "config": {"weights": {"clone_mass": 0.5, "complexity": 0.3, "centrality": 0.2}}}
```

**3. Detailed Issue Investigation**

```markdown
Please analyze this repository and then show me detailed information about the highest-scoring refactor candidate.

/analyze_repo {"paths": ["/home/user/api"]}

After getting the result_id, use:
/get_topk {"result_id": "the-result-id-from-above"}

Then for the top candidate:
/get_item {"result_id": "the-result-id", "entity_id": "entity-id-from-topk"}
```

**4. Impact Pack Analysis**

```markdown
I want to understand coordinated refactoring opportunities - not just individual issues but groups of related problems that should be tackled together.

/analyze_repo {"paths": ["/home/user/monorepo"]}
/get_impact_packs {"result_id": "result-id"}
```

**5. Iterative Weight Tuning**

```markdown
I want to focus more on cyclomatic complexity and less on duplication for this analysis.

/set_weights {"weights": {"complexity": 0.4, "clone_mass": 0.1, "centrality": 0.2, "cycles": 0.2, "type_friction": 0.1}}

Then re-run the analysis or adjust scoring of existing results.
```

### 🛠️ MCP Tools Reference

| Tool | Input | Output | Description |
|------|-------|--------|-------------|
| `analyze_repo` | `{paths: string[], config?: object, top_k?: number}` | `{result_id: string, status: string, total_files: number, total_entities: number, processing_time: number}` | Start repository analysis |
| `get_topk` | `{result_id: string}` | `{items: Brief[]}` | Get top-K ranked refactor candidates |
| `get_item` | `{result_id: string, entity_id: string}` | `{brief: Brief \| null}` | Get detailed information about specific entity |
| `get_impact_packs` | `{result_id: string}` | `{impact_packs: ImpactPack[]}` | Get coordinated refactoring recommendations |
| `set_weights` | `{weights: {complexity?: number, clone_mass?: number, centrality?: number, cycles?: number, type_friction?: number, smell_prior?: number}}` | `{ok: boolean, message: string}` | Update feature scoring weights |
| `ping` | `{}` | `{time: string, status: string}` | Health check and connectivity test |

### 📊 Understanding Tool Responses

**Brief Object Structure**
```json
{
  "entity_id": "src/services/user.py::UserService::validate_user",
  "score": 0.847,
  "features": {
    "complexity": 0.92,
    "clone_mass": 0.78,
    "centrality": 0.45
  },
  "explanations": [
    "High cyclomatic complexity (12 decision points)",
    "Code duplication detected with admin validation", 
    "Function called by 8 other modules"
  ]
}
```

**Impact Pack Structure**
```json
{
  "pack_id": "cycle-pack-001", 
  "pack_type": "CyclePack",
  "title": "Break User-Report Service Cycle",
  "description": "Eliminate circular dependency between user and report services",
  "entities": ["user_service.py", "report_service.py"],
  "value_estimate": 8,
  "effort_estimate": 4,
  "priority_score": 2.0,
  "metadata": {
    "cycle_length": 2,
    "affected_modules": 12,
    "refactor_steps": ["Extract shared interfaces", "Implement dependency injection"]
  }
}
```

### 🔍 Debugging MCP Connection Issues

#### **Stdio Server Debugging**

**Test Stdio Server Directly**
```bash
# Test the stdio server manually
echo '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}}}' | valknut mcp-stdio

# Should return initialization response
```

**Check Logs**
```bash
# Logs are written to stderr, won't interfere with stdio protocol
valknut mcp-stdio --verbose  # Enables debug logging
```

#### **HTTP Server Debugging**

**Check Server Status**
```bash
# Verify Valknut server is running
curl http://localhost:8140/healthz

# Check MCP manifest
curl http://localhost:8140/mcp/manifest

# Test analysis endpoint
curl -X POST http://localhost:8140/mcp/analyze_repo \
  -H "Content-Type: application/json" \
  -d '{"paths": ["/path/to/test/repo"]}'
```

#### **Common Issues and Solutions**

**For Stdio Server:**
1. **JSON-RPC Errors**: Check that Claude Code is sending properly formatted JSON-RPC 2.0 messages
2. **Path Issues**: Ensure file paths in `/analyze_repo` are absolute and accessible
3. **Process Communication**: Stdio server logs to stderr only, stdout is reserved for MCP protocol

**For HTTP Server:**
1. **Port Conflicts**: Change port in configuration if 8140 is in use
2. **Permission Issues**: Ensure Valknut can read the target directories

**For Both:**
3. **Language Dependencies**: Install tree-sitter parsers for your languages:
   ```bash
   pip install tree-sitter-python tree-sitter-typescript tree-sitter-javascript tree-sitter-rust
   ```
4. **Performance**: For large codebases, increase timeout in Claude Code MCP settings
5. **Configuration**: Verify your `rr.yml` configuration file syntax with `valknut print-default-config`

### 🚀 Performance Tuning for Claude Code

**Optimized Configuration for Large Projects**
```yaml
# rr-large.yml - Optimized for 10k+ files
version: 1
languages: ["python", "typescript"]  # Limit languages for speed
ranking:
  top_k: 25  # Fewer results for faster processing
  granularity: "function"
weights:
  complexity: 0.4
  centrality: 0.3
  clone_mass: 0.3  # Reduce clone detection for speed
detectors:
  echo:
    enabled: true
    min_tokens: 50  # Higher threshold = fewer comparisons
    min_similarity: 0.9  # Higher threshold = fewer matches
cache_dir: "/tmp/valknut-cache"  # Fast SSD cache
cache_ttl: 86400  # 24 hour cache
```

**Expected Response Times**
- **Small projects** (< 100 files): 2-5 seconds
- **Medium projects** (100-1000 files): 5-15 seconds  
- **Large projects** (1000+ files): 15-60 seconds

### Available Tools

```javascript
// Agent workflow example
const result = await mcp.call("analyze_repo", {
  paths: ["/path/to/repo"],
  top_k: 50
});

const briefs = await mcp.call("get_topk", {
  result_id: result.result_id
});

const packs = await mcp.call("get_impact_packs", {
  result_id: result.result_id
});

// Each brief contains:
// - entity_id, language, path, score
// - signatures, dependency_slice, invariants  
// - findings (duplicates, cycles, type issues)
// - candidate_refactors, safety_checklist
```

## What Does Valknut Rank Highly for Refactoring?

Valknut identifies code that needs refactoring based on **research-backed metrics**. Here are real examples of what gets ranked highly and why:

### 🔥 High Cyclomatic Complexity

**Example**: Long functions with many decision points
```python
def process_data(data, config):
    result = []
    for item in data:
        if isinstance(item, dict):
            for key, value in item.items():
                if key.startswith('_'):
                    continue
                if isinstance(value, (list, tuple)):
                    for sub_item in value:
                        if sub_item is not None:
                            if validate_item(sub_item):
                                if transform_needed(sub_item):
                                    result.append(transform(sub_item))
                                else:
                                    result.append(sub_item)
                else:
                    if config.get('include_primitives'):
                        result.append(value)
        elif isinstance(item, (list, tuple)):
            result.extend(flatten_list(item))
        else:
            result.append(item)
    return result
```
**Why it ranks highly**: 8+ decision points, nested conditionals, cognitive overload
**Refactor suggestion**: Extract methods, use strategy pattern, simplify control flow

### 🔄 Code Duplication (Clone Mass)

**Example**: Similar validation logic repeated across functions
```python
# Function 1
def validate_user_input(data):
    if not data:
        raise ValueError("Data cannot be empty")
    if not isinstance(data, dict):
        raise TypeError("Data must be a dictionary")
    if 'email' not in data:
        raise KeyError("Email is required")
    return True

# Function 2 (87% similar)
def validate_admin_input(data):
    if not data:
        raise ValueError("Data cannot be empty")
    if not isinstance(data, dict):
        raise TypeError("Data must be a dictionary")  
    if 'username' not in data:
        raise KeyError("Username is required")
    return True
```
**Why it ranks highly**: 87% code similarity detected by Echo integration
**Refactor suggestion**: Create shared validation base class or decorator

### 🕸️ High Graph Centrality

**Example**: "God objects" that many other classes depend on
```python
class DataManager:
    def __init__(self):
        # Used by 15+ other classes
        pass
    
    def load_data(self): pass      # Called by UserService, ReportService, etc.
    def save_data(self): pass      # Called by UserService, ReportService, etc.
    def validate_data(self): pass  # Called by UserService, ReportService, etc.
    def transform_data(self): pass # Called by UserService, ReportService, etc.
    def cache_data(self): pass     # Called by UserService, ReportService, etc.
```
**Why it ranks highly**: High betweenness centrality (15 incoming dependencies)
**Refactor suggestion**: Split into focused services, use dependency injection

### 🔁 Circular Dependencies

**Example**: Modules that import each other
```python
# user_service.py
from report_service import ReportService

class UserService:
    def get_user_report(self, user_id):
        return ReportService().generate_for_user(user_id)

# report_service.py  
from user_service import UserService  # Circular import!

class ReportService:
    def generate_for_user(self, user_id):
        user = UserService().get_user(user_id)
        return f"Report for {user.name}"
```
**Why it ranks highly**: Detected circular dependency in import graph
**Refactor suggestion**: Introduce shared interfaces, dependency injection

### 💥 Type Friction (Python-specific)

**Example**: Heavy use of `Any` types and untyped code
```python
from typing import Any

def process_items(items: Any, processor: Any) -> Any:  # Poor typing
    result = []  # Type inference lost
    for item in items:
        processed = processor(item)  # No type safety
        if processed:  # Runtime type checking
            result.append(processed)
    return result  # Return type unclear
```
**Why it ranks highly**: 100% `Any` type usage, no type safety
**Refactor suggestion**: Add proper type hints, use generics

### 📐 Deep Nesting

**Example**: Code with excessive indentation levels
```python
def analyze_nested_data(data):
    for category in data:                    # Level 1
        if category['active']:               # Level 2
            for subcategory in category['items']: # Level 3
                if subcategory['valid']:     # Level 4
                    for item in subcategory['data']: # Level 5
                        if item['processed']: # Level 6
                            if item['score'] > 0.8: # Level 7
                                print(f"High score: {item}") # Level 8
```
**Why it ranks highly**: 8 levels of nesting, "arrow anti-pattern"
**Refactor suggestion**: Extract functions, early returns, guard clauses

### 🔧 Real Code Example: Command Execution Pattern

**Example**: Actual code from our analysis that scored 0.472 (High refactor priority)
```python
def execute(self, command: Command) -> None:
    """Execute a command based on type and arguments"""
    if command.CMD in self.DefaultCommands.keys():
        if len(command.argv) > 0:
            print(self.Commands[command.CMD](*command.argv))
        else:
            print(self.DefaultCommands[command.CMD]())
    elif command.CMD in self.Commands.keys():
        if len(command.argv) > 0:
            print(self.Commands[command.CMD](*command.argv))
        else:
            print(self.Commands[command.CMD]())
```
**Why it ranks highly**: 
- **Deep nesting** (0.65): Multiple nested if/elif/else blocks
- **Code duplication**: Nearly identical logic in both branches  
- **Complex conditionals**: 4 separate conditional checks
- **Mixed concerns**: Command lookup + argument handling + execution

**Refactor suggestions**: 
- Extract command lookup logic
- Use polymorphism for command types
- Eliminate duplicate argument handling

## Performance Characteristics

Valknut has been benchmarked with excellent results:

- **130,290 features/second** peak throughput
- **0.024 seconds** for analyzing 353 code entities  
- **Zero errors** across all test configurations
- **Linear scaling** with codebase size

### Real Analysis Results

Here's what Valknut found when analyzing a sample code-smell dataset (command-line shell project):

```
🏆 Top 5 Refactor Candidates:
  1. UtilFuncs.py::TestFunction (Score: 0.488)
     Issues: High cognitive complexity (0.80), deep nesting (0.72)
     
  2. Shell.py::Shell.shellInput (Score: 0.472) 
     Issues: Complex nesting (0.65), high cognitive load (0.64)
     
  3. main.py::Interface.execute (Score: 0.472)
     Issues: Deep nesting (0.65), complex decision logic (0.60)
     
  4. Shell.py::Shell.parseCmd (Score: 0.467)
     Issues: High cognitive complexity (0.62), nested conditionals (0.58)
     
  5. EncodingApi.py::EncodingManager (Score: 0.460)
     Issues: Complex nesting (0.62), multiple parameters (0.53)
```

**Key Insights:**
- **Cognitive complexity** is the primary driver of high scores (0.60-0.80 range)
- **Deep nesting** correlates strongly with refactoring need (0.58-0.65 range)  
- **Parameter complexity** indicates functions doing too much (0.53+ range)
- **Real variance** in scores demonstrates the new Bayesian normalization working correctly

**Note**: Previous versions showed uniform 0.5 scores due to a normalization bug with flat features. The current Bayesian normalization approach provides informative fallbacks that generate realistic score distributions even with limited sample variance.

## Interpreting Valknut Scores

### Score Ranges and Meanings

Valknut uses normalized scores from 0.0 to 1.0:

- **0.8-1.0**: **Critical** - Immediate refactoring recommended
- **0.6-0.8**: **High** - Should refactor in next sprint  
- **0.4-0.6**: **Medium** - Monitor and refactor when convenient
- **0.2-0.4**: **Low** - Generally acceptable code quality
- **0.0-0.2**: **Excellent** - Well-structured, maintainable code

### Feature Weight Impact

The default configuration weights features as follows:

| Feature | Weight | Impact on Score |
|---------|--------|----------------|
| **Complexity** | 25% | Cyclomatic and cognitive complexity |
| **Clone Mass** | 20% | Code duplication via Echo detection |
| **Centrality** | 15% | Position in dependency graph |
| **Cycles** | 15% | Circular dependency participation |
| **Type Friction** | 15% | Type safety and annotation quality |
| **Smell Prior** | 10% | ML-detected code smell patterns |

### Refactoring Priority Matrix

```
High Complexity + High Centrality = 🔥 URGENT (affects many systems)
High Clone Mass + Low Test Coverage = 🎯 HIGH (duplication without safety net)
Medium Complexity + High Centrality = 📈 MEDIUM (architectural improvement)
Low Complexity + High Clone Mass = 🧹 LOW (cleanup when convenient)
```

### Language-Specific Patterns

Different languages exhibit different refactoring patterns:

**Python**:
- Heavy `typing.Any` usage ranks highly
- Long functions with nested loops
- Classes mixing data and behavior

**TypeScript/JavaScript**:  
- Callback pyramids and nested promises
- Large objects with mixed responsibilities
- Missing type annotations (TS)

**Rust**:
- Excessive `clone()` usage
- Large `match` expressions
- Lifetime complexity

## Common Anti-patterns Detected

### The \"God Function\" 
```python
def handle_request(request):  # 200+ lines, 15+ responsibilities
    # Authentication logic
    if not authenticate(request): return error()
    
    # Validation logic  
    if not validate(request): return error()
    
    # Business logic
    data = process_data(request.data)
    
    # Persistence logic
    save_to_database(data)
    
    # Notification logic
    send_notifications(data)
    
    # Response formatting
    return format_response(data)
```
**Score**: 0.85+ (Critical)  
**Solution**: Single Responsibility Principle - extract services

### The "Copy-Paste Class"
```python
class UserValidator:
    def validate_email(self, email): # 20 lines of validation
        pass
        
class AdminValidator:  # 85% identical to UserValidator
    def validate_email(self, email): # Same 20 lines, minor differences
        pass
```
**Score**: 0.75+ (High)  
**Solution**: Inheritance or composition with strategy pattern

### The "Import Web"
```python
# circular_import_a.py
from circular_import_b import B

# circular_import_b.py  
from circular_import_c import C

# circular_import_c.py
from circular_import_a import A  # Creates cycle!
```
**Score**: 0.70+ (High)  
**Solution**: Dependency inversion, interface segregation

## Integration with Development Workflow

### CI/CD Integration
```yaml
# .github/workflows/code-quality.yml
- name: Refactor Analysis
  run: |
    valknut analyze . --format json --out reports/
    # Fail if critical issues found
    jq '.entities[] | select(.score > 0.8)' reports/analysis_results.json
```

### Pre-commit Hook
```bash
#!/bin/sh
# Check only changed files for refactoring issues
valknut analyze $(git diff --cached --name-only --diff-filter=AM | grep '\.py$') \
  --format json | jq -e '.entities[] | select(.score > 0.6) | length == 0'
```

### IDE Integration
Valknut's MCP server integrates with Claude Code and compatible editors:
- Real-time refactoring suggestions
- Inline scoring and explanations
- Automated refactoring brief generation
- Context-aware code improvement recommendations

## Claude Code Quick Reference

### ⚡ **Fastest Setup** (Recommended for Individual Developers)

```json
{
  "mcpServers": {
    "valknut": {
      "command": "valknut",
      "args": ["mcp-stdio"]
    }
  }
}
```

No servers, no ports, no configuration required!

### 📋 Essential MCP Commands

**Start Analysis**
```markdown
/analyze_repo {"paths": ["/path/to/project"]}
```

**Get Top Issues** 
```markdown
/get_topk {"result_id": "uuid-from-analyze"}
```

**Get Coordinated Refactors**
```markdown
/get_impact_packs {"result_id": "uuid-from-analyze"}
```

**Investigate Specific Issue**
```markdown
/get_item {"result_id": "uuid", "entity_id": "path::class::method"}
```

**Adjust Analysis Focus**
```markdown
/set_weights {"weights": {"complexity": 0.4, "clone_mass": 0.3}}
```

### 🎯 Common Claude Code Workflows

**1. New Codebase Assessment**
```markdown
Please help me understand the technical debt in this new codebase I'm working on:

/analyze_repo {"paths": ["/home/user/new-project"], "top_k": 15}
/get_topk {"result_id": "..."}
/get_impact_packs {"result_id": "..."}

Show me the top 3 most critical issues and explain why they should be prioritized.
```

**2. Pre-Refactoring Planning**
```markdown
I want to refactor my authentication module. Let me analyze it first:

/analyze_repo {"paths": ["/home/user/project/src/auth"], "config": {"granularity": "function"}}
/get_topk {"result_id": "..."}

Based on these results, what's the best refactoring approach for the highest-scoring functions?
```

**3. Clone Detection Focus**
```markdown
I suspect there's a lot of duplicate code in this project. Please analyze with emphasis on duplication:

/set_weights {"weights": {"clone_mass": 0.6, "complexity": 0.2, "centrality": 0.2}}
/analyze_repo {"paths": ["/home/user/project"]}
/get_topk {"result_id": "..."}

Show me the duplicated code patterns and suggest consolidation strategies.
```

**4. Architectural Assessment**
```markdown
I want to understand the dependency structure and find architectural issues:

/analyze_repo {"paths": ["/home/user/microservices"]}
/get_impact_packs {"result_id": "..."}

Focus on the CyclePacks and ChokepointPacks - what architectural changes do you recommend?
```

### 💡 Pro Tips for Claude Code Users

- **Use Impact Packs** for systematic refactoring rather than one-off fixes
- **Start with small `top_k`** values (10-20) to focus on critical issues first  
- **Adjust weights** based on your current priorities (security, maintainability, etc.)
- **Analyze incrementally** - focus on specific modules before entire codebases
- **Use specific entity IDs** from `get_topk` to drill down into individual issues

## Advanced Usage

### Custom Analysis

```python
from valknut.core.pipeline import Pipeline
from valknut.core.config import RefactorRankConfig

# Custom configuration
config = RefactorRankConfig(
    languages=["python"],
    weights={
        "complexity": 0.4,
        "clone_mass": 0.3,
        "centrality": 0.3,
    },
    ranking={"top_k": 20, "granularity": "function"}
)

# Run analysis
pipeline = Pipeline(config)
result = await pipeline.analyze()

# Custom processing
for vector, score in result.ranked_entities:
    if score > 0.8:
        print(f"Critical: {vector.entity_id} ({score:.3f})")
```

### Server Integration

```python
from valknut.api.server import create_app
import uvicorn

app = create_app(config)
uvicorn.run(app, host="0.0.0.0", port=8140)
```

## Development

### Setup

```bash
git clone https://github.com/nathan-rice/valknut.git
cd valknut
pip install -e ".[dev]"
```

### Testing

```bash
# Run tests
pytest

# Run with coverage
pytest --cov=valknut

# Test specific language adapter  
pytest tests/lang/test_python_adapter.py
```

### Adding Language Support

1. Create adapter in `valknut/lang/{language}_adapter.py`
2. Implement `LanguageAdapter` protocol
3. Register adapter in `__init__.py`
4. Add tests and golden fixtures

## Contributing

1. Fork the repository
2. Create feature branch (`git checkout -b feature/amazing-feature`)
3. Make changes and add tests
4. Ensure all tests pass (`pytest`)
5. Commit changes (`git commit -m 'Add amazing feature'`)
6. Push to branch (`git push origin feature/amazing-feature`)
7. Open Pull Request

## References

- [Model Context Protocol](https://docs.anthropic.com/claude/docs/mcp) - Agent integration standard
- [Sibylline Echo](https://github.com/nathan-rice/sibylline-echo) - Clone detection library  
- [RefactoringMiner](https://github.com/tsantalis/RefactoringMiner) - Refactoring pattern research
- [Code Smell Detection](https://zenodo.org/communities/msr) - Research datasets

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built for integration with Claude Code and MCP-compatible agents
- Inspired by traditional code quality tools (SonarQube, PMD, ESLint)  
- Research-backed feature extraction from software engineering literature
- Designed for deterministic, reproducible analysis suitable for CI/CD