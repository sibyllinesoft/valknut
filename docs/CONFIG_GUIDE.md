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
Controls which analysis modules are enabled and the associated discovery rules:

```json
{
  "analysis": {
    "modules": {
      "complexity": true,
      "dependencies": true,
      "duplicates": true,
      "refactoring": true,
      "structure": true,
      "coverage": false
    },
    "files": {
      "include_patterns": ["**/*"],
      "exclude_patterns": [
        "*/node_modules/*",
        "*/venv/*",
        "*/target/*"
      ],
      "max_files": null,
      "follow_symlinks": false
    },
    "quality": {
      "confidence_threshold": 0.7,
      "max_analysis_time_per_file": 30,
      "strict_mode": false
    },
    "coverage": {
      "enabled": false,
      "auto_discover": true,
      "file_path": null,
      "max_age_days": 7,
      "search_paths": ["./coverage/", "./reports/"]
    }
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

The CLI now exposes four built-in presets designed to cover the most common workflows. They can be combined with explicit flags or configuration overrides:

| Preset     | Focus                                                             |
| ---------- | ----------------------------------------------------------------- |
| `fast`     | Lightweight structure + complexity pass (coverage and clones off) |
| `default`  | Balanced daily-driver configuration                               |
| `deep`     | Full analysis with LSH, denoising, and stricter heuristics         |
| `ci`       | Deterministic output tuned for CI/CD pipelines                    |

Example usage:

```bash
valknut analyze --preset deep ./src
```

When a preset is selected the CLI applies the corresponding module toggles before merging config files. You can still adjust individual sections in YAML; explicit settings always win over the preset defaults.

## Common Configuration Patterns

### CI/CD Optimized
Minimal configuration for fast CI/CD analysis:

```json
{
  "analysis": {
    "modules": {
      "dependencies": false,
      "duplicates": false,
      "coverage": true
    },
    "files": {
      "max_files": 1000
    }
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
    "modules": {
      "complexity": true,
      "dependencies": true,
      "duplicates": true,
      "refactoring": true,
      "structure": true,
      "coverage": true
    }
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

1. **From legacy `valknut-config-*.yml`**: Use `valknut init-config` to generate the new format, then manually copy your custom settings
2. **From multiple configs**: All settings are now unified in `valknut.yml`

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
valknut analyze --config docs/ci/.valknut-ci.json ./src

# Production analysis
valknut analyze --config .valknut-prod.json ./src
```

## Documentation Health (docs)

The `docs` section controls when documentation gaps are penalized and how scores are surfaced.

```json
{
  "docs": {
    "min_fn_nodes": 5,
    "min_file_nodes": 50,
    "min_files_per_dir": 5
  }
}
```

- Files/functions smaller than the thresholds are ignored to avoid noisy penalties.
- Outputs:
  - `documentation.doc_health_score` — project score (0–100).
  - `documentation.file_doc_health` — per-file score (0–100); treemap “Docs” severity = `100 - score`.
  - `documentation.file_doc_issues`, `documentation.directory_doc_health`, `documentation.directory_doc_issues`.

## Tips and Best Practices

1. **Start with defaults**: Use `valknut init-config` to generate a baseline
2. **Gradual customization**: Adjust thresholds based on your codebase characteristics
3. **Environment-specific**: Use different configs for dev, CI, and production
4. **Version control**: Commit your `valknut.yml` to share team standards
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

See `config/` and `docs/ci/` directories for complete configuration examples:
- `config/valknut.yml.example` - Annotated reference configuration in YAML
- `docs/ci/.valknut-ci.json` - Optimized for CI/CD pipelines
- GitHub Actions, GitLab CI, Azure DevOps templates with corresponding configs

For more details on specific analysis modules, see their respective documentation files.
