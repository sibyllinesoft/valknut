# Valknut CLI Usage Guide

The Valknut CLI provides command-line access to the high-performance code structure analyzer implemented in Rust.

## Installation

Build the CLI from source:

```bash
cargo build --bin valknut --release
```

The binary will be available at `target/release/valknut`.

## Quick Start

Analyze code structure in a directory:

```bash
# Basic analysis with default settings
valknut structure /path/to/your/code

# Pretty output format
valknut structure /path/to/your/code --format pretty

# Use custom configuration
valknut structure /path/to/your/code --config valknut-config.yml

# Limit to top 5 recommendations
valknut structure /path/to/your/code --top 5 --verbose
```

## Commands

### `structure` - Analyze Code Structure

Analyzes directory organization and file splitting opportunities.

```bash
valknut structure [OPTIONS] <PATH>
```

**Arguments:**
- `<PATH>` - Directory or file path to analyze

**Options:**
- `--branch-only` - Only analyze directory organization (no file splitting)  
- `--file-split-only` - Only analyze file splitting opportunities (no directory analysis)
- `-n, --top <N>` - Maximum number of recommendations to show
- `-e, --extensions <EXTS>` - File extensions to analyze (comma-separated)

**Global Options:**
- `-c, --config <FILE>` - Configuration file path (YAML or JSON)
- `-f, --format <FORMAT>` - Output format: `json`, `yaml`, or `pretty`
- `-v, --verbose` - Enable verbose logging
- `-h, --help` - Show help
- `-V, --version` - Show version

## Configuration

The structure analyzer is highly configurable. Create a `valknut-config.yml` file:

```yaml
# Analysis toggles
structure:
  enable_branch_packs: true      # Directory reorganization analysis
  enable_file_split_packs: true  # File splitting analysis
  top_packs: 20                  # Max recommendations to return

# Directory analysis settings
fsdir:
  max_files_per_dir: 25          # Files before directory pressure
  max_subdirs_per_dir: 10        # Subdirs before pressure
  max_dir_loc: 2000              # Lines of code before pressure  
  min_branch_recommendation_gain: 0.15  # Min improvement needed
  min_files_for_split: 5         # Min files for split consideration
  target_loc_per_subdir: 1000    # Target LOC per subdirectory

# File analysis settings
fsfile:
  huge_loc: 800                  # LOC threshold for "huge" files
  huge_bytes: 128000             # Byte threshold for "huge" files
  min_split_loc: 200             # Min LOC for split consideration
  min_entities_per_split: 3      # Min entities per split

# Partitioning algorithm settings  
partitioning:
  balance_tolerance: 0.25        # ±25% balance tolerance
  max_clusters: 4                # Max clusters per partition
  min_clusters: 2                # Min clusters per partition
  naming_fallbacks:              # Fallback names for clusters
    - "core"
    - "io" 
    - "api"
    - "util"
```

## Output Formats

### JSON Format (Default)

Machine-readable structured output:

```json
{
  "packs": [
    {
      "kind": "file_split",
      "file": "src/large_file.rs", 
      "reasons": ["loc 1200 > 800", "3 cohesion communities"],
      "suggested_splits": [
        {
          "name": "large_file_core.rs",
          "entities": ["function_a", "function_b"],
          "loc": 400
        }
      ],
      "value": {"score": 0.8},
      "effort": {"estimated_hours": 2.5}
    }
  ],
  "summary": {
    "structural_issues_found": 1,
    "analysis_timestamp": "2025-09-07T22:38:26Z"
  }
}
```

### YAML Format

Human-readable structured output:

```yaml
packs:
  - kind: file_split
    file: src/large_file.rs
    reasons:
      - loc 1200 > 800
      - 3 cohesion communities
    suggested_splits:
      - name: large_file_core.rs
        entities: [function_a, function_b]
        loc: 400
    value:
      score: 0.8
    effort:
      estimated_hours: 2.5
summary:
  structural_issues_found: 1
  analysis_timestamp: 2025-09-07T22:38:26Z
```

### Pretty Format

Human-readable formatted output:

```
🏗️  Valknut Structure Analysis Results
=====================================

📊 Found 1 potential improvements:

1. 📄 File Split Analysis
   📁 File: src/large_file.rs
   📋 Reasons:
      • loc 1200 > 800
      • 3 cohesion communities
   ✂️  Suggested splits:
      • large_file_core.rs (400 LOC)
   💎 Value: score=0.80
   ⏱️  Estimated effort: 2.5 hours

📈 Summary Statistics
--------------------
Structural issues found: 1
```

## Examples

### Analyze Python Project

```bash
valknut structure ./my-python-project --format pretty --verbose
```

### Focus on Large Files Only

```bash
valknut structure ./src --file-split-only --top 10
```

### Custom Configuration for Team Standards

```bash
valknut structure ./codebase \
  --config team-standards.yml \
  --format json \
  --top 15
```

### Quick Directory Organization Check

```bash
valknut structure ./src --branch-only --format pretty
```

## Integration

The CLI is designed for integration with development workflows:

- **CI/CD**: Use JSON output for automated quality gates
- **IDE Integration**: Parse JSON output for editor plugins  
- **Code Review**: Generate reports with pretty format
- **Refactoring Planning**: Use effort estimates for sprint planning

## Performance

The Rust implementation provides excellent performance characteristics:

- **Speed**: Analyzes large codebases (100k+ LOC) in seconds
- **Memory**: Efficient memory usage with SIMD optimizations
- **Concurrency**: Parallel analysis of multiple files
- **Accuracy**: Graph-based algorithms for precise recommendations

## Troubleshooting

**"Path does not exist"**: Verify the path argument is correct  
**"No issues found"**: Your code already has good structure! Try lowering thresholds in config  
**High memory usage**: For very large codebases, consider analyzing subdirectories separately  
**Slow performance**: Enable `--verbose` to see which operations take time