# Valknut CLI Usage Guide

Complete reference for the Valknut command-line interface.

## Table of Contents
- [Overview](#overview)
- [Global Options](#global-options)
- [Commands](#commands)
- [Analysis Commands](#analysis-commands)
- [Configuration Commands](#configuration-commands)
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
| `--survey` | Enable/disable usage analytics collection (default: enabled) |
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
| `-o, --out <DIR>` | PATH | `out` | Output directory for reports |
| `-f, --format <FORMAT>` | ENUM | `jsonl` | Output format |
| `-q, --quiet` | FLAG | - | Suppress non-essential output |

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

### Legacy Commands (Backward Compatibility)

#### `structure` - Structure Analysis Only

Analyze code structure and generate refactoring recommendations.

```bash
valknut structure [OPTIONS] <PATH>
```

| Option | Type | Description |
|--------|------|-------------|
| `-e, --extensions <EXTS>` | LIST | File extensions (comma-separated) |
| `--branch-only` | FLAG | Enable only branch reorganization |
| `--file-split-only` | FLAG | Enable only file splitting analysis |
| `-n, --top <COUNT>` | INT | Maximum recommendations to show |
| `-f, --format <FORMAT>` | ENUM | Output format |

#### `impact` - Impact Analysis Only

Analyze dependency cycles and clone detection for impact assessment.

```bash
valknut impact [OPTIONS] <PATH>
```

| Option | Type | Description |
|--------|------|-------------|
| `-e, --extensions <EXTS>` | LIST | File extensions (comma-separated) |
| `--cycles` | FLAG | Enable cycle detection |
| `--clones` | FLAG | Enable clone detection |
| `--chokepoints` | FLAG | Enable chokepoint detection |
| `--min-similarity <RATIO>` | FLOAT | Minimum similarity threshold (0.0-1.0) |
| `--min-total-loc <COUNT>` | INT | Minimum total LOC for clone groups |
| `-n, --top <COUNT>` | INT | Maximum recommendations to show |
| `-f, --format <FORMAT>` | ENUM | Output format |

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