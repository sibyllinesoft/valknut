# Valknut CLI Usage Guide

Complete reference for the Valknut command-line interface.

## Table of Contents
- [Overview](#overview)
- [Global Options](#global-options)
- [Commands](#commands)
- [Analysis Commands](#analysis-commands)
- [Configuration Commands](#configuration-commands)
- [Information Commands](#information-commands)
- [`doc-audit` - Documentation Coverage](#doc-audit---documentation-coverage)
- [Integration Commands](#integration-commands)
- [Output Formats](#output-formats)
- [Quality Gates](#quality-gates)
- [Examples](#examples)

## Overview

Valknut provides a rich CLI interface for comprehensive code analysis. The main entry point is the `analyze` command, with additional commands for configuration management and integration.

```bash
valknut --help
```

## Global Options

Available across all commands:

| Option | Description |
|--------|-------------|
| `-v, --verbose` | Enable verbose logging for debugging |
| `--survey` | Opt in to usage analytics collection (disabled by default) |
| `--survey-verbosity LEVEL` | Set survey invitation verbosity level [low, medium, high, maximum] |

## Commands

### `analyze` - Comprehensive Code Analysis

The primary command for analyzing codebases with comprehensive analysis including structure, complexity, and refactoring opportunities.

```bash
valknut analyze [OPTIONS] <PATHS>...
```

#### Required Arguments
- `<PATHS>...` - One or more directories or files to analyze

#### Analysis Options
| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `-c, --config <FILE>` | PATH | - | Configuration file path |
| `-o, --out <DIR>` | PATH | `.valknut` | Output directory for reports |
| `-f, --format <FORMAT>` | ENUM | `jsonl` | Output format |
| `-q, --quiet` | FLAG | - | Suppress non-essential output |
| `--profile <fast\|balanced\|thorough\|extreme>` | ENUM | `fast` | Pre-tuned performance/accuracy presets (tunes file limits & LSH precision) |

#### Module Toggles & Coverage
| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--no-complexity` | FLAG | false | Disable complexity scoring |
| `--no-structure` | FLAG | false | Disable structure analysis |
| `--no-refactoring` | FLAG | false | Disable refactoring analysis |
| `--no-impact` | FLAG | false | Disable dependency/impact analysis (cycles, chokepoints) |
| `--no-lsh` | FLAG | false | Disable LSH clone detection |
| `--no-coverage` | FLAG | false | Disable coverage analysis |
| `--coverage-file <PATH>` | PATH | auto-discover | Force a specific coverage file (lcov/xml/json) |
| `--no-coverage-auto-discover` | FLAG | false | Skip searching for coverage files |

#### Oracle & Documentation
| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--oracle` | FLAG | off | Run Gemini-powered refactoring oracle (needs `GEMINI_API_KEY`) |
| `--oracle-max-tokens <N>` | INT | 500000 | Cap tokens sent to the oracle |
| `doc-audit --root <PATH>` | PATH | `.` | Standalone documentation audit; same engine powers doc health in `analyze` |

#### Clone Detection Options
| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--semantic-clones` | FLAG | - | Enable semantic clone detection using LSH |
| `--strict-dedupe` | FLAG | - | Favor high-precision clone filtering |
| `--denoise` | FLAG | - | Enable weighted denoising heuristics for higher accuracy |
| `--min-function-tokens <COUNT>` | INT | 40 | Minimum tokens per entity considered for clone search |
| `--min-match-tokens <COUNT>` | INT | 24 | Minimum overlap tokens required to treat entities as clones |
| `--require-blocks <COUNT>` | INT | 2 | Minimum distinct structural blocks required for matches |
| `--similarity <SCORE>` | FLOAT | 0.82 | Similarity threshold (0.0-1.0) for accepted clone pairs |
| `--denoise-dry-run` | FLAG | - | Collect denoising stats without affecting clone ranking |

#### Structural Verification (APTED)
> **Default:** Structural verification runs automatically using APTED. Pass `--no-apted-verify` to skip this step when benchmarking raw LSH output.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--apted-verify` | FLAG | - | Verify clone candidates with tree edit distance (APTED) |
| `--apted-max-nodes <COUNT>` | INT | 4000 | Maximum AST nodes to include per entity when building APTED trees |
| `--apted-max-pairs <COUNT>` | INT | 25 | Maximum clone candidates per entity to verify (0 = reuse LSH limit) |
| `--no-apted-verify` | FLAG | - | Disable structural verification (APTED is enabled by default) |

#### Quality Gate Options
| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--quality-gate` | FLAG | - | Enable quality gate mode |
| `--fail-on-issues` | FLAG | - | Fail build if any issues found |
| `--max-complexity <SCORE>` | FLOAT | 75 | Maximum complexity score (0-100) |
| `--min-health <SCORE>` | FLOAT | 60 | Minimum health score (0-100) |
| `--max-debt <RATIO>` | FLOAT | 30 | Maximum technical debt ratio (0-100) |
| `--min-maintainability <SCORE>` | FLOAT | 20 | Minimum maintainability index (0-100) |
| `--max-issues <COUNT>` | INT | 50 | Maximum total issues count |
| `--max-critical <COUNT>` | INT | 0 | Maximum critical issues count |
| `--max-high-priority <COUNT>` | INT | 5 | Maximum high-priority issues count |

#### Examples
```bash
# Basic analysis
valknut analyze ./src

# Generate team HTML report
valknut analyze --format html --out reports/ ./src

# Quality gate for CI/CD
valknut analyze --quality-gate --max-complexity 75 --min-health 60 ./src

# Fail on any issues
valknut analyze --fail-on-issues ./src

# Custom configuration
valknut analyze --config custom.yml --format markdown ./src

# Benchmark clone verification (default path = .)
make bench-clone-verification BENCH_CLONE_PATH=../path/to/project

# Force dependency graph & docs surfaced in HTML/JSON
valknut analyze src crates --format html --semantic-clones --denoise --coverage-file coverage/lcov.info
```

## Output Fields (JSON / HTML)
- `documentation.file_doc_health`: per-file doc health (0-100); Treemap “Docs” color uses severity = 100 - score.
- `documentation.file_doc_issues`, `directory_doc_health`, `directory_doc_issues`: granular doc gap counts and directory health.
- `clone_analysis.clone_pairs` & `coverage_packs`: remain unchanged; shown in Clones and Coverage tabs.
```

### Configuration Commands

#### `init-config` - Initialize Configuration File

Create a new configuration file with default settings.

```bash
valknut init-config [OPTIONS]
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `-o, --output <FILE>` | PATH | `.valknut.yml` | Output configuration file name |
| `-f, --force` | FLAG | - | Overwrite existing configuration file |

#### `validate-config` - Validate Configuration

Validate a Valknut configuration file for syntax and content errors.

```bash
valknut validate-config [OPTIONS]
```

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `-c, --config <FILE>` | PATH | Yes | Path to configuration file |
| `-v, --verbose` | FLAG | - | Show detailed configuration breakdown |

#### `print-default-config` - Print Default Configuration

Output the default configuration in YAML format.

```bash
valknut print-default-config
```

### Information Commands

#### `list-languages` - List Supported Languages

Display all supported programming languages and their analysis status.

```bash
valknut list-languages
```

Shows:
- Language name
- File extensions
- Support status (Full/Experimental)
- Available features

#### `doc-audit` - Documentation Coverage

Audit source files for missing docstrings or doc comments, verify README coverage
in complex directories, and flag READMEs that may be stale.

```bash
valknut doc-audit [OPTIONS]
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--root <DIR>` | PATH | `.` | Project root to scan |
| `--complexity-threshold <COUNT>` | INT | `8` | Require READMEs for directories with more descendants than this threshold |
| `--max-readme-commits <COUNT>` | INT | `10` | Mark README as stale when more commits than this touch the directory |
| `--strict` | FLAG | - | Exit with non-zero status if any issues are detected |
| `--format <FORMAT>` | ENUM | `text` | Output format (`text`, `json`) |
| `--ignore-dir <NAME>` | STRING | - | Additional directory name to ignore (repeatable) |
| `--ignore-suffix <SUFFIX>` | STRING | - | Additional file suffix to ignore (repeatable) |
| `--ignore <GLOB>` | STRING | test globs preloaded | Glob patterns to ignore (repeatable) |
| `--config <FILE>` | PATH | auto-discover `.valknut.docaudit.yml` | Load doc-audit settings from a YAML file |

Example strict audit with JSON output:

```bash
valknut doc-audit --strict --format json --ignore-dir node_modules
```

Doc audits ship with sensible defaults that skip common test paths (e.g., `**/tests/**`, `**/*_test.*`); add more with `--ignore` or a config file when needed.

### Integration Commands

#### `mcp-stdio` - MCP Server for IDE Integration

Run MCP server over stdio for Claude Code integration.

```bash
valknut mcp-stdio [OPTIONS]
```

| Option | Type | Description |
|--------|------|-------------|
| `-c, --config <FILE>` | PATH | Configuration file |

#### `mcp-manifest` - Generate MCP Manifest

Generate MCP manifest JSON for IDE integration setup.

```bash
valknut mcp-manifest [OPTIONS]
```

| Option | Type | Description |
|--------|------|-------------|
| `-o, --output <FILE>` | PATH | Output file (default: stdout) |

## Output Formats

Valknut supports multiple output formats for different use cases:

| Format | Extension | Description | Use Case |
|--------|-----------|-------------|-----------|
| `jsonl` | `.jsonl` | Line-delimited JSON | Streaming, ETL pipelines |
| `json` | `.json` | Single JSON file | API integration, tools |
| `yaml` | `.yml/.yaml` | YAML format | Human-readable config |
| `markdown` | `.md` | Markdown team report | Documentation, reviews |
| `html` | `.html` | Interactive HTML report | Team dashboards |
| `sonar` | `.json` | SonarQube integration | SonarQube import |
| `csv` | `.csv` | Spreadsheet data | Excel analysis |
| `ci-summary` | `.json` | CI/CD optimized | Automated systems |
| `pretty` | - | Human-readable console | Terminal viewing |

> **Note:** Advanced clone detection and boilerplate learning live under the
> `valknut_rs::experimental` module behind the optional `experimental` Cargo feature.

### Format-Specific Options

#### HTML Reports
```bash
valknut analyze --format html --out reports/ ./src
```

Generates:
- `index.html` - Main dashboard
- `complexity/` - Complexity analysis pages
- `structure/` - Structure analysis results
- `assets/` - CSS, JS, and images

#### Markdown Reports
```bash
valknut analyze --format markdown --out docs/ ./src
```

Creates team-friendly reports perfect for:
- Architecture Decision Records (ADRs)
- Code review documentation
- Technical debt planning
- Sprint planning

#### CI/CD Integration Formats
```bash
# For automated systems
valknut analyze --format ci-summary ./src

# For SonarQube
valknut analyze --format sonar --out sonar-reports/ ./src
```

## Quality Gates

Quality gates enable automated quality control in CI/CD pipelines. When enabled, Valknut exits with code 1 if any thresholds are exceeded.

### Basic Usage
```bash
# Enable quality gates
valknut analyze --quality-gate ./src

# Fail on any issues
valknut analyze --fail-on-issues ./src
```

### Threshold Configuration

| Metric | CLI Option | Range | Default | Description |
|--------|------------|-------|---------|-------------|
| Complexity Score | `--max-complexity` | 0-100 | 75 | Maximum allowed complexity (lower is better) |
| Health Score | `--min-health` | 0-100 | 60 | Minimum required health (higher is better) |
| Technical Debt | `--max-debt` | 0-100% | 30 | Maximum debt ratio (lower is better) |
| Maintainability | `--min-maintainability` | 0-100 | 20 | Minimum maintainability index |
| Total Issues | `--max-issues` | 0+ | 50 | Maximum total issues allowed |
| Critical Issues | `--max-critical` | 0+ | 0 | Maximum critical issues |
| High Priority | `--max-high-priority` | 0+ | 5 | Maximum high-priority issues |

### Quality Gate Examples

#### Strict Quality Gate
```bash
valknut analyze \
  --quality-gate \
  --max-complexity 60 \
  --min-health 70 \
  --max-debt 20 \
  --max-issues 25 \
  --max-critical 0 \
  ./src
```

#### Permissive Quality Gate
```bash
valknut analyze \
  --quality-gate \
  --max-complexity 85 \
  --min-health 40 \
  --max-debt 50 \
  --max-issues 100 \
  ./src
```

## Examples

### Basic Workflow
```bash
# 1. Analyze codebase
valknut analyze ./src

# 2. Generate team report
valknut analyze --format html --out reports/ ./src

# 3. Create configuration for future use
valknut init-config --output project-config.yml

# 4. Use custom configuration
valknut analyze --config project-config.yml ./src
```

### CI/CD Integration
```bash
# GitHub Actions / Jenkins
valknut analyze \
  --quality-gate \
  --max-complexity 75 \
  --min-health 60 \
  --format ci-summary \
  --out quality-reports/ \
  ./src

# SonarQube Integration
valknut analyze \
  --format sonar \
  --out sonar-reports/ \
  ./src
```

### Development Workflow
```bash
# Quick check during development
valknut analyze --quiet ./src/my-module

# Detailed analysis with progress
valknut analyze --verbose --format pretty ./src

# Focus on critical issues
valknut analyze --fail-on-issues --max-critical 0 ./src
```

### Language-Specific Analysis
```bash
# Python projects
valknut analyze --config python-config.yml ./src

# TypeScript/JavaScript projects  
valknut analyze --config web-config.yml ./frontend/src

# Multi-language monorepo
valknut analyze --config monorepo-config.yml .
```

### Report Generation
```bash
# Executive summary (markdown)
valknut analyze --format markdown --out docs/quality/ ./src

# Technical deep-dive (HTML)
valknut analyze --format html --out reports/technical/ ./src

# Data export (CSV)
valknut analyze --format csv --out data/ ./src

# Multiple formats
valknut analyze --format json --out reports/json/ ./src
valknut analyze --format html --out reports/html/ ./src
valknut analyze --format markdown --out reports/md/ ./src
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Analysis completed successfully, all quality gates passed |
| 1 | Quality gate failure or analysis issues found |
| 2 | Configuration error or invalid arguments |
| 3 | File system error (permissions, missing files) |
| 4 | Analysis engine error |

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `VALKNUT_CONFIG` | Default configuration file path | - |
| `VALKNUT_CACHE_DIR` | Cache directory location | `~/.valknut/cache` |
| `VALKNUT_LOG_LEVEL` | Log level (error, warn, info, debug) | `info` |
| `RUST_LOG` | Rust logging configuration | - |

## Configuration File Integration

CLI options can be combined with configuration files. The precedence order is:

1. CLI arguments (highest priority)
2. Configuration file settings
3. Built-in defaults (lowest priority)

### Example Configuration Override
```bash
# Configuration file sets max_complexity: 80
# CLI override to 60
valknut analyze --config config.yml --max-complexity 60 ./src
# Result: max_complexity = 60 (CLI wins)
```

## Troubleshooting

### Common Issues

#### "No valid paths provided"
- Ensure paths exist and are accessible
- Check file permissions
- Use absolute paths if relative paths fail

#### "Configuration validation failed"
- Run `valknut validate-config --config your-config.yml`
- Check YAML syntax and structure
- Verify all required fields are present

#### "Quality gate failed"
- Review threshold settings
- Use `--verbose` to see detailed violation information
- Generate HTML report to visualize issues

#### Performance Issues
- Use `--max-files` to limit analysis scope
- Enable caching in configuration
- Consider excluding large generated directories

### Debug Mode
```bash
# Enable verbose logging
valknut --verbose analyze ./src

# Rust-level debugging
RUST_LOG=debug valknut analyze ./src

# Performance profiling
RUST_LOG=valknut=trace valknut analyze ./src
```
