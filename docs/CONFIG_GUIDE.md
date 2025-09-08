# Valknut Configuration Guide

## Overview

Valknut uses a single, comprehensive YAML configuration file (`.valknut.yml`) to control all analysis settings. This replaces the previous multiple configuration file approach and provides a centralized, version-controlled configuration system.

## Configuration File

### Default Location
- **Primary**: `.valknut.yml` in your project root
- **CLI Override**: Use `--config <file>` to specify a different configuration file

### Generating Configuration

```bash
# Generate default configuration
valknut init-config

# Generate with custom name
valknut init-config --output my-config.json

# Overwrite existing configuration
valknut init-config --force
```

## Configuration Structure

The configuration is organized into logical sections:

### Analysis Settings
Controls which analysis modules are enabled:

```json
{
  "analysis": {
    "enable_scoring": true,
    "enable_graph_analysis": true,
    "enable_lsh_analysis": true,
    "enable_refactoring_analysis": true,
    "enable_structure_analysis": true,
    "enable_names_analysis": false,
    "enable_coverage_analysis": false,
    "max_files": 0,
    "exclude_patterns": [
      "*/node_modules/*",
      "*/venv/*", 
      "*/target/*"
    ]
  }
}
```

### Quality Gates
Configure CI/CD integration and quality thresholds:

```json
{
  "quality_gates": {
    "enabled": false,
    "max_complexity": 75.0,
    "min_health": 60.0,
    "max_debt": 30.0,
    "min_maintainability": 20.0,
    "max_issues": 50,
    "max_critical": 0,
    "max_high_priority": 5
  }
}
```

### Structure Analysis
Control directory organization and file analysis:

```json
{
  "structure": {
    "structure": {
      "enable_branch_packs": true,
      "enable_file_split_packs": true,
      "top_packs": 10
    },
    "fsdir": {
      "max_files_per_dir": 25,
      "max_subdirs_per_dir": 10,
      "max_dir_loc": 2000
    },
    "fsfile": {
      "huge_loc": 800,
      "huge_bytes": 128000,
      "min_split_loc": 200
    }
  }
}
```

### Semantic Naming (Optional)
AI-powered function name analysis:

```json
{
  "names": {
    "enabled": false,
    "embedding_model": "Qwen/Qwen3-Embedding-0.6B-GGUF",
    "min_mismatch": 0.65,
    "min_impact": 3,
    "protect_public_api": true
  }
}
```

### Language Support
Per-language settings and thresholds:

```json
{
  "languages": {
    "python": {
      "enabled": true,
      "file_extensions": [".py", ".pyi"],
      "max_file_size_mb": 10,
      "complexity_threshold": 10.0
    },
    "typescript": {
      "enabled": true,
      "file_extensions": [".ts", ".tsx", ".d.ts"],
      "max_file_size_mb": 5,
      "complexity_threshold": 10.0
    }
  }
}
```

## Configuration Presets

The configuration includes built-in presets for common use cases:

### Strict Quality Gates
```json
{
  "_presets": {
    "strict_quality_gates": {
      "quality_gates": {
        "enabled": true,
        "max_complexity": 50.0,
        "min_health": 70.0,
        "max_debt": 25.0,
        "max_critical": 0,
        "max_high_priority": 3
      }
    }
  }
}
```

### Semantic Analysis
```json
{
  "_presets": {
    "semantic_analysis": {
      "names": {
        "enabled": true
      },
      "analysis": {
        "enable_names_analysis": true
      }
    }
  }
}
```

## Common Configuration Patterns

### CI/CD Optimized
Minimal configuration for fast CI/CD analysis:

```json
{
  "analysis": {
    "enable_graph_analysis": false,
    "enable_lsh_analysis": false,
    "enable_names_analysis": false,
    "max_files": 1000
  },
  "quality_gates": {
    "enabled": true,
    "max_complexity": 70.0,
    "min_health": 65.0
  },
  "performance": {
    "file_timeout_seconds": 20,
    "batch_size": 50
  }
}
```

### Development Mode
Full analysis for local development:

```json
{
  "analysis": {
    "enable_scoring": true,
    "enable_graph_analysis": true,
    "enable_refactoring_analysis": true,
    "enable_structure_analysis": true
  },
  "quality_gates": {
    "enabled": false
  },
  "io": {
    "enable_caching": true,
    "default_format": "html"
  }
}
```

### Security/Compliance Mode
Focused on code quality and maintainability:

```json
{
  "quality_gates": {
    "enabled": true,
    "max_complexity": 50.0,
    "min_health": 80.0,
    "max_debt": 15.0,
    "max_critical": 0,
    "max_high_priority": 2
  },
  "scoring": {
    "weights": {
      "complexity": 1.5,
      "structure": 1.2,
      "refactoring": 1.0
    }
  }
}
```

## Migration from Legacy Configs

If you have existing configuration files:

1. **From `valknut-config.yml`**: Use `valknut init-config` to generate the new format, then manually copy your custom settings
2. **From multiple configs**: All settings are now unified in `.valknut.yml`

### Migration Examples

**Old YAML format:**
```yaml
structure:
  max_files_per_dir: 30
  huge_loc: 1000
quality_gates:
  enabled: true
  max_complexity: 80
```

**New YAML format:
```json
{
  "structure": {
    "fsdir": {
      "max_files_per_dir": 30
    },
    "fsfile": {
      "huge_loc": 1000
    }
  },
  "quality_gates": {
    "enabled": true,
    "max_complexity": 80.0
  }
}
```

## Validation

Validate your configuration:

```bash
# Validate current configuration
valknut validate-config

# Validate specific configuration file
valknut validate-config --config my-config.json

# Print default configuration for reference
valknut print-default-config
```

## Environment-Specific Configurations

Use different configurations for different environments:

```bash
# Development
valknut analyze --config .valknut-dev.json ./src

# CI/CD
valknut analyze --config ci-examples/.valknut-ci.json ./src

# Production analysis
valknut analyze --config .valknut-prod.json ./src
```

## Tips and Best Practices

1. **Start with defaults**: Use `valknut init-config` to generate a baseline
2. **Gradual customization**: Adjust thresholds based on your codebase characteristics
3. **Environment-specific**: Use different configs for dev, CI, and production
4. **Version control**: Commit your `.valknut.json` to share team standards
5. **Documentation**: Add comments to your configuration (use `"_comment"` fields)
6. **Validation**: Always validate configuration changes before committing

## Troubleshooting

### Common Issues

1. **Invalid JSON**: Use a JSON validator or `jq` to check syntax
2. **Missing fields**: Required fields will show clear error messages
3. **Type mismatches**: Ensure numbers are not quoted strings
4. **Path issues**: Use absolute paths or ensure relative paths are correct

### Debug Configuration

```bash
# Verbose output to see configuration loading
valknut analyze --verbose ./src

# Print effective configuration
valknut print-default-config > current-config.json
```

## Examples

See the `ci-examples/` directory for complete configuration examples:
- `.valknut-ci.json` - Optimized for CI/CD pipelines
- GitHub Actions, GitLab CI, Azure DevOps examples with corresponding configs

For more details on specific analysis modules, see their respective documentation files.