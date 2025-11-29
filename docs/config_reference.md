# Configuration Reference

The Valknut CLI reads YAML config from `.valknut.yml` / `.valknut.yaml` (auto-discovered) or any file passed via `--config`. The easiest starting point is `valknut.yml.example` at repo root.

## Top-level sections

- `analysis` — enables modules and language rules.
- `denoise` — clone/LSH noise filtering and stop-motif mining.
- `scoring` — weights and normalization used for health scores.
- `graph` — dependency graph settings.
- `lsh` / `dedupe` — duplicate detection parameters.
- `languages` — per-language thresholds and extensions.
- `io` — cache and report paths.
- `performance` — thread counts and timeouts.
- `structure` — structural pack generation thresholds.
- `coverage` — discovery patterns and staleness limits.

## Common tweaks

### Enable/disable analyses
```yaml
analysis:
  modules:
    complexity: true
    dependencies: true
    duplicates: true
    refactoring: true
    structure: true
    coverage: true
```

### Language scope and thresholds
```yaml
analysis:
  languages:
    enabled: [python, javascript, typescript, rust, go]
    max_file_size_mb: 10
    complexity_thresholds:
      rust: 15
      python: 10
```

### File filters
```yaml
analysis:
  files:
    include_patterns: ["**/*"]
    exclude_patterns:
      - "*/node_modules/*"
      - "*/target/*"
    max_files: 2000
```

### Coverage discovery
```yaml
analysis:
  coverage:
    enabled: true
    auto_discover: true
    search_paths:
      - "./coverage/"
      - "./reports/"
    file_patterns: ["coverage.lcov", "coverage.xml", "lcov.info"]
```

### Output location and format
```yaml
io:
  report_dir: "./reports"
  report_format: "json"   # json|yaml|html|csv|sonar|markdown|jsonl
```

### Performance guardrails
```yaml
performance:
  max_threads: null          # auto
  memory_limit_mb: 4096
  file_timeout_seconds: 45
  total_timeout_seconds: 900
```

### Quality gates (for CI)
```yaml
analysis:
  quality:
    confidence_threshold: 0.7
    max_analysis_time_per_file: 30
    strict_mode: false

scoring:
  weights:
    complexity: 1.0
    coverage: 0.7
    structure: 0.9
    graph: 0.8
```

## Tips

- Keep `valknut.yml.example` nearby and prune sections you don’t need; absent keys fall back to sensible defaults.
- For large monorepos, start with `profile: fast` and tighter `max_files` to shorten CI time, then relax gradually.
- Use `valknut validate-config --config path.yml --verbose` to confirm the file parses and to see resolved defaults.
